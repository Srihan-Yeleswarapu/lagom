//! End-to-end integration tests (§33.1's "corpus green"): the full pipeline
//! from Lagom source through the driver to a native executable, then the
//! observed output. Only frozen-spec-guaranteed behavior is asserted.
//!
//! These tests compile and run real executables; each uses a distinct output
//! path so parallel tests never collide.

use std::io::Write;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

/// Compile `src` to a native executable in the temp dir (dev mode).
///
/// The integration tests default to `no_pdb = true` so CI artifacts do not
/// accumulate `.pdb` files unless a test explicitly opts into debug info.
fn build_exe(name: &str, src: &str, dev: bool, no_pdb: bool) -> PathBuf {
    let suffix = if cfg!(windows) { ".exe" } else { "" };
    let out = std::env::temp_dir().join(format!(
        "lagom_it_{}_{}{}",
        name,
        std::process::id(),
        suffix
    ));
    let _ = std::fs::remove_file(&out);
    match lagom_driver::build_native(src, &out, dev, no_pdb) {
        Ok(p) => p,
        Err(e) => panic!("building `{name}` failed: {e:?}"),
    }
}

/// Run an executable with scripted stdin; returns (exit code, stdout, stderr).
fn run(exe: &Path, stdin: &str, envs: &[(&str, &str)]) -> (i32, String, String) {
    let mut cmd = Command::new(exe);
    cmd.stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());
    for (k, v) in envs {
        cmd.env(k, v);
    }
    let mut child = cmd.spawn().expect("spawn the built executable");
    child
        .stdin
        .take()
        .expect("stdin pipe")
        .write_all(stdin.as_bytes())
        .expect("write stdin");
    let out = child.wait_with_output().expect("wait for the executable");
    (
        out.status.code().unwrap_or(-1),
        String::from_utf8_lossy(&out.stdout).into_owned(),
        String::from_utf8_lossy(&out.stderr).into_owned(),
    )
}

fn assert_says(name: &str, src: &str, want: &[&str]) {
    let exe = build_exe(name, src, true, true);
    let (code, stdout, stderr) = run(&exe, "", &[]);
    assert_eq!(code, 0, "exit code; stderr: {stderr}");
    let lines: Vec<&str> = stdout.lines().collect();
    assert_eq!(lines, want, "stdout of `{name}`");
}

// ---------------------------------------------------------------------------
// The full pipeline: source → native code → behavior
// ---------------------------------------------------------------------------

#[test]
fn hello_world_to_native_code() {
    assert_says("hello", "say \"Hello, world!\"", &["Hello, world!"]);
}

/// 8.4 type aliases run end-to-end, identically on both backends: the alias
/// is checked in sema and gone by HIR, so interpreter and native agree by
/// construction — asserted here by construction (same program, same output).
#[test]
fn type_aliases_run_on_both_backends() {
    let src = "\
a type called score is a number
a type called scores is a list of score

function total
    takes scores called xs
    returns a score
    gives back combine xs with start 0 using start plus it

make changing best equal to 0 of type score
make results equal to a list of 4, 9, 2 of type scores
repeat for each x in results
    if x is greater than best
        set best to x
say total results
say best
";
    // Interpreter parity: seeded dev run.
    let (host, outcome) = lagom_driver::run_interpreted(src, Vec::new(), Some(1));
    assert!(matches!(outcome, lagom_interp::RunOutcome::Completed), "{outcome:?}");
    assert_eq!(host.stdout, vec!["15", "9"], "interpreter output");
    // Native parity: same program, same output.
    assert_says("alias_parity", src, &["15", "9"]);
}

/// Seeded replay parity, at the trace level: two runs with the same seed
/// produce byte-identical timelines (26.5's replay story), so a trace taken
/// from a failing run describes any replay of that run.
#[test]
fn trace_replays_identically_with_a_seed() {
    let src = "\
function roll twice
    returns a number
    gives back random from 1 to 6 plus random from 1 to 6

make a equal to roll twice
make b equal to roll twice
say \"{a} then {b}\"
";
    let t1 = lagom_driver::run_traced(src, Vec::new(), Some(7)).expect("trace");
    let t2 = lagom_driver::run_traced(src, Vec::new(), Some(7)).expect("trace");
    assert_eq!(t1, t2, "same seed, same trace");
    let different = lagom_driver::run_traced(src, Vec::new(), Some(8)).expect("trace");
    assert_ne!(t1, different, "different seeds, different trace");
    // The timeline names the bindings with their source lines (provenance).
    let rendered = format!("{t1}");
    assert!(rendered.contains("bound `a` ="), "{rendered}");
    assert!(rendered.contains("(line 5)"), "{rendered}");
    assert!(rendered.ends_with("end: the run completed.\n"), "{rendered}");
}

/// The trace states truncation honestly: a run producing more events than
/// the ring's budget shows the most recent steps and says how many were
/// dropped — it never poses as complete.
#[test]
fn trace_reports_dropped_steps_when_history_exceeds_the_ring() {
    let src = "\
make changing x equal to 0
repeat 40 times
    set x to x plus 1
";
    let report = lagom_driver::run_traced(src, Vec::new(), None).expect("trace");
    let rendered = format!("{report}");
    if report.dropped > 0 {
        assert!(rendered.contains("oldest steps were dropped"), "{rendered}");
    } else {
        // 40 events fit the budget; either way the trace is honest.
        assert!(rendered.contains("end: the run completed."), "{rendered}");
    }
}

/// `--trace` executes the program exactly once: the driver's traced run
/// returns output, outcome, and timeline from a single execution. The
/// observable is an `append file` side effect — one run appends one line;
/// a second execution would append two. (Regresses the double run where the
/// CLI executed the program once for output and again for the trace.)
#[test]
fn trace_runs_the_program_exactly_once() {
    let path = std::env::temp_dir().join(format!("lagom_trace_once_{}.txt", std::process::id()));
    let _ = std::fs::remove_file(&path);
    let ps = path.to_string_lossy().replace('\\', "/");
    let src = format!(
        "\nattempt append file \"one\" at \"{ps}\" if it fails then\n    say problem\notherwise\n    say result\n"
    );
    // The CLI's exact path: front end (entry check) → one traced execution.
    let fe = lagom_driver::frontend(&src).expect("frontend");
    let (report, host, outcome) = lagom_driver::run_traced_fe(fe, &src, Vec::new(), None);
    assert!(matches!(outcome, lagom_interp::RunOutcome::Completed), "{outcome:?}");
    // On success `append file` binds `result` to the written text (S-10).
    assert_eq!(host.stdout, vec!["one"], "the otherwise branch ran");
    // One execution appended one line; the timeline came from that same run.
    let contents = std::fs::read_to_string(&path).unwrap_or_default();
    let _ = std::fs::remove_file(&path);
    assert_eq!(contents, "one", "the program ran exactly once (no second append)");
    assert!(format!("{report}").contains("end:"), "timeline rendered");
}

/// Every validation program still checks clean (M2 regression: M1's `where`
/// keyword silently broke text_adventure.lagom, which used `where` as a
/// variable name — and no test noticed). A future reserved word must never
/// retro-break a validation file again.
#[test]
fn all_validation_programs_check_clean() {
    let dir = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../validation")
        .canonicalize()
        .expect("validation dir");
    let mut checked = 0;
    let entries = std::fs::read_dir(&dir).expect("validation dir");
    for entry in entries {
        let path = entry.expect("entry").path();
        if path.extension().and_then(|e| e.to_str()) != Some("lagom") {
            continue;
        }
        checked += 1;
        let src = std::fs::read_to_string(&path).expect("read");
        let fe = lagom_driver::frontend(&src)
            .unwrap_or_else(|e| panic!("{} must check clean: {e:?}", path.display()));
        let _ = fe;
    }
    assert!(checked >= 8, "expected the validation corpus, found {checked}");
}

/// The profiler's data source: a dev run records a FunctionEntry per call,
/// so per-function call counts are exact (not sampled). Ring is dev-only —
/// a release-mode run must not record anything.
#[test]
fn profile_counts_function_calls_from_the_lom_ring() {
    let src = "\
function double
    takes number called n
    returns a number
    gives back n times 2

function apply twice
    takes number called n
    returns a number
    gives back double double n

say \"{apply twice 3}\"
";
    let (_, _, interp) = lagom_driver::run_full(src, Vec::new(), None).expect("run");
    let calls = lagom_driver::call_counts(&interp).expect("ring present in dev");
    let get = |name: &str| calls.iter().find(|(n, _)| n == name).map(|(_, c)| *c).unwrap_or(0);
    assert_eq!(get("double"), 2, "apply twice calls double twice; ring: {calls:?}");
    assert_eq!(get("apply twice"), 1, "ring: {calls:?}");
}

#[test]
fn language_surface_runs_natively() {
    // One program touching every M0 area the gate names: variables (both
    // kinds), arithmetic, comparisons, and/or/not, if, all three loops,
    // stop, lists, maps, structs, interpolation, functions, say/ask,
    // `number from`, `can fail` + `attempt`.
    let src = r#"
structure item
    has name of type text
    has price of type number

make cart equal to a list of an item with name "pen" and price 3, an item with name "ink" and price 4

function total
    takes a list of item called items
    returns a number
    make changing sum equal to 0
    repeat for each entry in items
        set sum to sum plus price of entry
    gives back sum

make changing n equal to total cart
say "total {n}"

repeat 3 times using i
    if i is equal to 2
        say "middle"
    otherwise
        say "round {i}"

make changing countdown equal to 3
repeat while countdown is greater than 0
    decrease countdown by 1
    if countdown is equal to 0
        say "liftoff"
        stop

make stock equal to a map from "pen" to 9, "ink" to 2
make ink left equal to stock at "ink"
say "ink left: {ink left}"

attempt number from ask "" if it fails then
    say "not a number"
otherwise
    say "got {result}"
"#;
    let exe = build_exe("surface", src, true, true);
    let (code, stdout, stderr) = run(&exe, "oops\n", &[]);
    assert_eq!(code, 0, "stderr: {stderr}");
    let lines: Vec<&str> = stdout.lines().collect();
    assert_eq!(
        lines,
        &[
            "total 7",
            "round 0",
            "round 1",
            "middle",
            "liftoff",
            "ink left: 2",
            "not a number",
        ]
    );
}

/// 7.0.3 + 7.9 regression: a nested call with a name argument (`say greet
/// who`) and the same call inside interpolation must print the value, not
/// silently print `nothing` (the pre-fix behavior on both backends).
#[test]
fn nested_call_with_name_argument_prints_value() {
    let src = r#"
function greet
    takes text called name
    returns a text
    gives back "Hello, {name}!"

make who equal to "bo"
say greet who
say "{greet who}"
"#;
    assert_says("nested_call", src, &["Hello, bo!", "Hello, bo!"]);
}

/// 5.1/S-1: the call form `remainder of a and b` works natively and matches
/// the infix form `a remainder of b`.
#[test]
fn remainder_of_call_form_runs_natively() {
    let src = r#"
make total equal to 7
make rest equal to remainder of total and 2
make also equal to 7 remainder of 2
say "{rest} {also}"
"#;
    assert_says("remainder_of", src, &["1 1"]);
}

#[test]
fn comma_call_form_is_the_seven_nine_shape() {
    // 7.9: flowing calls bind at the additive level; the frozen forms for a
    // two-argument call are the flowing form (`most of a and b`), the
    // mixed comma form (`most of a, b`), and the paren form (`most (a), b`).
    // (G-27 made `bigger`/`smaller` math-module builtins, so the student
    // function here uses a free name.)
    let src = r#"
function biggest
    takes number called a
    takes number called b
    returns a number
    if a is greater than b
        gives back a
    gives back b

say "flowing {biggest of 3 and 9}"
say "mixed {biggest of 3, 9}"
say "paren {biggest (3), 9}"
"#;
    assert_says(
        "comma",
        src,
        &["flowing 9", "mixed 9", "paren 9"],
    );
}

// ---------------------------------------------------------------------------
// G-26/G-27 and text-`at`: the M0 surface the demo and corpus pin
// ---------------------------------------------------------------------------

#[test]
fn pass_on_binds_result() {
    // G-26: the readable `?` forwards the success value — statements after
    // `attempt … and pass the problem on` read it as `result`; the failure
    // short-circuits to the caller.
    let fail_src = r#"
function parse age
    takes text called answer
    returns a number
    can fail
    attempt number from answer and pass the problem on
    gives back result

attempt parse age "abc" if it fails then
    say problem
otherwise
    say result
"#;
    let ok_src = fail_src.replace("\"abc\"", "\"21\"");
    assert_says("passon_fail", fail_src, &["\"abc\" is not a number — a number is digits, maybe starting with a minus."]);
    assert_says("passon_ok", &ok_src, &["21"]);
}

#[test]
fn bigger_and_smaller_of_two_numbers() {
    // G-27: §7.8's `bigger of a and b` — the maximum — and its `smaller`
    // sibling, in the math module, riding the closed call grammar.
    assert_says(
        "bigger_smaller",
        "use math\nsay bigger of 3 and 4\nsay smaller of 3 and 4\nsay bigger of 9 and 2",
        &["4", "3", "9"],
    );
}

#[test]
fn text_at_reads_a_code_point() {
    // §7.7: `greeting at 2` — the Unicode code point by position, 0-based.
    assert_says(
        "text_at",
        "make greeting equal to \"hello\"\nsay greeting at 1\nsay greeting at 0",
        &["e", "h"],
    );
}

// ---------------------------------------------------------------------------
// Convert-failure vs index-error routing (the campaign's parity contract)
// ---------------------------------------------------------------------------

#[test]
fn convert_failure_flows_to_attempt_index_error_crashes() {
    // Convert (text→number) propagates out of can-fail functions and is
    // handled by `attempt`; an index past the end is a panic with a line.
    let src = r#"
function parse raw
    takes text called raw
    returns a number
    can fail
    gives back attempt number from raw

attempt parse raw "not a number" if it fails then
    say "caught the problem"
otherwise
    say "impossible"

make xs equal to a list of 1, 2
say xs at 9
"#;
    let exe = build_exe("routing", src, true, true);
    let (code, stdout, stderr) = run(&exe, "", &[]);
    assert_eq!(stdout.lines().next(), Some("caught the problem"), "stdout: {stdout}");
    assert_ne!(code, 0, "the index error must crash");
    assert!(stderr.contains("line 14"), "stderr: {stderr}");
    assert!(stderr.contains("past the end"), "stderr: {stderr}");
}

// ---------------------------------------------------------------------------
// LAGOM_SEED determinism (26.5 replay story)
// ---------------------------------------------------------------------------

#[test]
fn lagom_seed_pins_the_random_sequence() {
    let src = r#"
make secret equal to random from 1 to 100
say "secret {secret}"
"#;
    let exe = build_exe("seeded", src, true, true);
    let (c1, o1, e1) = run(&exe, "", &[("LAGOM_SEED", "7")]);
    let (c2, o2, e2) = run(&exe, "", &[("LAGOM_SEED", "7")]);
    assert_eq!(c1, 0, "{e1}");
    assert_eq!(c2, 0, "{e2}");
    assert_eq!(o1, o2, "same seed must replay identically");
    let secret: i64 = o1
        .trim()
        .strip_prefix("secret ")
        .expect("output shape")
        .parse()
        .expect("secret is a number");
    assert!((1..=100).contains(&secret));
}

#[test]
fn interpreter_seed_replays_identically() {
    let src = "make secret equal to random from 1 to 100\nsay \"secret {secret}\"\n";
    let (_, a) = lagom_driver::run_interpreted(src, Vec::new(), Some(7));
    let (_, b) = lagom_driver::run_interpreted(src, Vec::new(), Some(7));
    assert_eq!(a, b, "same seed must replay identically on the interpreter");
}

// ---------------------------------------------------------------------------
// `lagom test` semantics (S-12) at the driver level
// ---------------------------------------------------------------------------

#[test]
fn test_blocks_report_pass_fail_and_explanation() {
    let src = r#"
function double
    takes number called n
    returns a number
    gives back n times 2

test "doubling two gives four"
    check that double 2 is equal to 4

test "a deliberately wrong check"
    check that double 3 is equal to 7

test "a test that traps"
    make broken equal to a list of 1, 2
    check that broken at 9 is equal to 1
"#;
    let results = lagom_driver::run_tests_interpreted(src, None);
    assert_eq!(results.len(), 3);
    assert!(results[0].passed, "first test must pass");
    assert!(!results[1].passed);
    let msg = results[1].message.clone().expect("failure message");
    assert!(msg.contains("check failed"), "{msg}");
    assert!(msg.contains("left = 6"), "{msg}");
    assert!(msg.contains("right = 7"), "{msg}");
    assert!(!results[2].passed);
    // Failing tests carry the LOM report (26.5): provenance, not just a panic.
    let report = results[2].report.clone().expect("LOM report for the trap");
    assert!(report.contains("`broken` became [1, 2] (line"), "{report}");
}

// ---------------------------------------------------------------------------
// A program with no script body (test-only or library) has no entry:
// `lagom run` has nothing to execute. The native binary must still build
// and exit 0 — never a phony "internal: unhandled failure" from reading an
// uninitialized fail slot.
// ---------------------------------------------------------------------------

#[test]
fn test_only_program_builds_and_runs_clean() {
    let src = "test \"adds\"\n    check that 1 plus 1 is equal to 2\n";
    let fe = lagom_driver::frontend(src).expect("test-only source checks out");
    assert!(!fe.has_entry, "test-only program has no script body");
    assert_eq!(fe.test_count, 1);

    let exe = build_exe("testonly", src, true, true);
    let (code, stdout, stderr) = run(&exe, "", &[]);
    assert_eq!(code, 0, "stderr: {stderr}");
    assert_eq!(stdout, "", "a no-entry binary prints nothing");
    assert_eq!(stderr, "", "a no-entry binary prints nothing to stderr");
}

// ---------------------------------------------------------------------------
// The LOM failure report (26.5) on the interpreter backend (D-32)
// ---------------------------------------------------------------------------

#[test]
fn unhandled_failure_produces_the_lom_report() {
    let src = r#"
make bottom equal to 0
make xs equal to a list of 1
say xs at 5
"#;
    let (_, outcome) = lagom_driver::run_interpreted(src, Vec::new(), None);
    let report = match outcome {
        lagom_interp::RunOutcome::Panicked { message, report } => {
            assert!(message.contains("line 4"), "{message}");
            report.expect("dev builds report")
        }
        other => panic!("expected a panic, got {other:?}"),
    };
    assert!(report.contains("what happened"), "{report}");
    assert!(report.contains("what the program was doing"), "{report}");
    assert!(report.contains("`bottom` became 0 (line 2)"), "{report}");
    assert!(report.contains("`xs` became [1] (line 3)"), "{report}");
}

// ---------------------------------------------------------------------------
// Invalid programs: student diagnostics, never a compiler crash
// ---------------------------------------------------------------------------

fn expect_diagnostic(src: &str, code: &str, snippet: &str) {
    match lagom_driver::frontend(src) {
        Ok(_) => panic!("`{src}` unexpectedly compiled"),
        Err(lagom_driver::FrontendError::Diagnostics(diags)) => {
            let found = diags
                .iter()
                .find(|d| d.code == code)
                .unwrap_or_else(|| panic!("expected {code} in {:?}", diags.iter().map(|d| d.code).collect::<Vec<_>>()));
            assert!(found.span.start <= found.span.end, "span order for {code}");
            let rendered =
                lagom_driver::render_diagnostics("prog.lagom", src, std::slice::from_ref(found));
            assert!(rendered.contains(snippet), "rendered:\n{rendered}");
            assert!(rendered.contains("line "), "rendered must name the line:\n{rendered}");
        }
        Err(other) => panic!("internal error, not a student diagnostic: {other:?}"),
    }
}

#[test]
fn invalid_programs_get_source_aware_diagnostics() {
    // Parse error (E0203: expected end of line), with the offending words.
    expect_diagnostic(
        "say 1 then stop now",
        "E0203",
        "say 1 then stop now",
    );
    // Unknown name (the checker's "make it first" lesson).
    expect_diagnostic("say mystery", "E0344", "mystery");
    // A can-fail call that is not handled (E0302; E0333 is the
    // missing-`gives back` rule).
    expect_diagnostic(
        "function f\n    takes number called n\n    returns a number\n    can fail\n    fail with \"no\"\n\nmake x equal to f 1",
        "E0302",
        "f 1",
    );
    // `fail with` in a function that does not say `can fail` (E0337).
    expect_diagnostic(
        "function f\n    fail with \"no\"",
        "E0337",
        "fail with",
    );
    // Arity: too few values for the parameters (E0354).
    expect_diagnostic(
        "function add\n    takes number called a\n    takes number called b\n    returns a number\n    gives back a plus b\n\nsay add 1",
        "E0354",
        "add",
    );
}

// ---------------------------------------------------------------------------
// Release builds run the same program correctly (the silent mode)
// ---------------------------------------------------------------------------

#[test]
fn release_build_produces_a_working_binary() {
    assert_says_release("release", "say 6 times 7", &["42"]);
}

fn assert_says_release(name: &str, src: &str, want: &[&str]) {
    let exe = build_exe(name, src, false, true);
    let (code, stdout, stderr) = run(&exe, "", &[]);
    assert_eq!(code, 0, "stderr: {stderr}");
    let lines: Vec<&str> = stdout.lines().collect();
    assert_eq!(lines, want);
}

// The friend flow (33.1's day-one gate): the runtime self-build must produce
// a rlib in the *matching* profile. A release-compiled driver with a cold
// `target/lagom-rt/release` cache used to build a debug rlib and then fail
// to find it, breaking every `cargo install`-style use of the compiler.
#[test]
fn release_driver_self_builds_the_runtime_from_a_cold_cache() {
    // Prove the release rlib really is absent, so the test exercises the
    // self-build rather than a warm cache.
    let manifest = env!("CARGO_MANIFEST_DIR");
    let rt_rlib = Path::new(manifest)
        .join("../../target/lagom-rt/release/liblagom_rt.rlib");
    let saved = std::fs::rename(&rt_rlib, rt_rlib.with_extension("rlib.saved"));
    let result = std::panic::catch_unwind(|| {
        assert_says_release("cold_release", "say 5 plus 5", &["10"]);
    });
    // Restore whatever we moved, even on failure.
    if saved.is_ok() {
        let _ = std::fs::rename(rt_rlib.with_extension("rlib.saved"), &rt_rlib);
    }
    if let Err(e) = result {
        std::panic::resume_unwind(e);
    }
}

// ---------------------------------------------------------------------------
// §11.1 function-value application: `f of x`, `f x`, `f of f of 3` — a
// binding holding a function applies its arguments through the shared
// CallClosure path. Both backends must agree.
// ---------------------------------------------------------------------------

#[test]
fn function_value_application_of_form() {
    let src = "\
function double
    takes number called n
    gives back n times 2

make f equal to double
say f of 21
";
    let (host, _outcome) = lagom_driver::run_interpreted(src, Vec::new(), Some(1));
    assert_eq!(host.stdout, vec!["42"], "interpreter: f of 21");
    assert_says("fnval_of", src, &["42"]);
}

#[test]
fn function_value_application_positional_form() {
    // §11.1's own lambda example applied in S59's canonical form.
    let src = "\
make f equal to a function taking n
    gives back n times 3

say f 5
";
    let (host, _outcome) = lagom_driver::run_interpreted(src, Vec::new(), Some(1));
    assert_eq!(host.stdout, vec!["15"], "interpreter: f 5");
    assert_says("fnval_pos", src, &["15"]);
}

#[test]
fn function_value_application_nested_of_calls() {
    // §11.1's canonical `f of f of 3`.
    let src = "\
function double
    takes number called n
    gives back n times 2

make f equal to double
say f of f of 3
";
    let (host, _outcome) = lagom_driver::run_interpreted(src, Vec::new(), Some(1));
    assert_eq!(host.stdout, vec!["12"], "interpreter: f of f of 3");
    assert_says("fnval_nested", src, &["12"]);
}

#[test]
fn field_read_still_wins_for_structures() {
    // The read forms are untouched by application: same shape, non-function
    // callee binding.
    let src = "\
structure player
    has name of type text

make p equal to a player with name \"bo\"
say name of p
make xs equal to a list of 7, 8
say xs at 1
";
    let (host, _outcome) = lagom_driver::run_interpreted(src, Vec::new(), Some(1));
    assert_eq!(host.stdout, vec!["bo", "8"]);
}

// ---------------------------------------------------------------------------
// §12.1 generics-beyond-anything: `anything` as the call-site-acceptable top
// type, and the option tail on a type parameter (`returns some type?`)
// ---------------------------------------------------------------------------

#[test]
fn anything_accepts_any_single_value() {
    // §12.1's frozen spellings: `takes anything called x` accepts a value of
    // any type at the call site and flows it out unchanged.
    let src = "\
function echo
    takes anything called value
    gives back value

say echo 5
say echo \"hi\"
say echo true
";
    let (host, _outcome) = lagom_driver::run_interpreted(src, Vec::new(), Some(1));
    assert_eq!(host.stdout, vec!["5", "hi", "true"], "interpreter: anything flows through");
    assert_says("anything_top", src, &["5", "hi", "true"]);
}

#[test]
fn anything_fills_containers_at_the_call_site() {
    // `a list of anything` accepts a list of numbers (or text, …) — the top
    // type inside containers, per §12.1's `takes a list of anything`.
    let src = "\
function biggest
    takes a list of anything called values
    gives back first of values

say biggest with values a list of 3, 7, 2
";
    let (host, _outcome) = lagom_driver::run_interpreted(src, Vec::new(), Some(1));
    assert_eq!(host.stdout, vec!["3"], "interpreter: list-of-anything call site");
    assert_says("anything_container", src, &["3"]);
}

#[test]
fn some_type_option_tail_types_the_result() {
    // `returns some type?` — the option tail composes with the type
    // parameter, so `first of` inside the body types as the call site's
    // element type.
    let src = "\
function optional first
    takes a list of some type called items
    returns some type?
    gives back first of items

make list equal to a list of 1
say optional first list
";
    let (host, _outcome) = lagom_driver::run_interpreted(src, Vec::new(), Some(1));
    assert_eq!(host.stdout, vec!["1"], "interpreter: some type? result");
    assert_says("some_type_opt", src, &["1"]);
}
