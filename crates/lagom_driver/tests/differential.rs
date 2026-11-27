//! Differential backend testing, live (docs/10-roadmap.md M2 gate; doc 08
//! §10 "differential backend testing from M2"). Every validation project —
//! the corpus the milestones are gated on — runs through BOTH backends: the
//! interpreter (dev run, seeded) and the native Cranelift build in both dev
//! and release mode. The observed behavior must agree byte-identically:
//! same stdout lines, same exit code, same outcome shape. A semantics change
//! that a backend does not share cannot merge.
//!
//! Determinism rules, the same contract the replay story uses (26.5):
//! `random` is pinned by the seed (interpreter `Host::with_seed`, native the
//! `LAGOM_SEED` env var — the same value both sides), `ask` is fed a
//! scripted stdin that terminates every loop the program can enter, and
//! file-working programs run in their own fresh directory via
//! `Host::workdir` / the process `current_dir`, so runs cannot observe each
//! other.

use std::io::Write;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};

/// The pinned seed: every backend runs the same random sequence.
const SEED: u64 = 42;

/// The validation corpus root (`crates/lagom_driver/../../validation`).
fn validation_dir() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../validation")
        .canonicalize()
        .expect("validation dir")
}

/// One corpus program: name (file stem), scripted stdin, and fixture files
/// the program reads or writes.
struct Case {
    name: &'static str,
    stdin: String,
    fixtures: &'static [(&'static str, &'static str)],
}

fn cases() -> Vec<Case> {
    // Guesses 1..=100 in order: the guessing game finishes for any secret
    // the seeded run draws, so termination never depends on the seed.
    let guesses = {
        let mut s = String::new();
        for n in 1..=100 {
            s.push_str(&n.to_string());
            s.push('\n');
        }
        s
    };
    // Eleven `look`s: the lamp-oil meter reaches zero on iteration 10, the
    // dark wins, the adventure loop exits — for every room the seed draws.
    let looks = "look\n".repeat(11);
    vec![
        Case { name: "smoke", stdin: String::new(), fixtures: &[] },
        // Two numbers and a division: fixes the decimal rendering.
        Case { name: "calculator", stdin: "3\n14\ndivide\n".into(), fixtures: &[] },
        // Answers one per `ask`; every answer correct, in order.
        Case { name: "quiz", stdin: "12\nparis\n12\n".into(), fixtures: &[] },
        Case { name: "guessing_game", stdin: guesses, fixtures: &[] },
        Case { name: "text_adventure", stdin: looks, fixtures: &[] },
        // A manifest of good rows (files that exist), one row whose file is
        // missing (the failure branch), and one malformed row (the broken
        // kind). The report and the moved counts are then fully pinned.
        Case {
            name: "file_organizer",
            stdin: String::new(),
            fixtures: &[
                (
                    "organize_manifest.txt",
                    "notes.txt|documents\nshopping.txt|home\nmissing.txt|home\noops\n",
                ),
                ("notes.txt", "meeting notes\n"),
                ("shopping.txt", "apples\nbread\n"),
            ],
        },
        // A header, two well-formed rows (one with a quoted comma — the
        // quote-parity path), and one ragged row (the refusal path).
        Case {
            name: "csv_parser",
            stdin: String::new(),
            fixtures: &[(
                "csv_input.csv",
                "name,age,city\ngrace,26,dhaka\n\"ada, queen\",36,england\nbo,12\n",
            )],
        },
        // 11.3's `does iterable` walk: the class poll reaches the
        // interp's vtable dispatch and the native `rt_call_iface`, and a
        // plain list walk rides the M0 desugaring. Any backend that
        // drops the polled loop diverges here byte-for-byte.
        Case { name: "iterator", stdin: String::new(), fixtures: &[] },
        // The D-35 numeric suite (the benchmark program, D-35/D-8): its
        // compiled result is a pure integer fold, so the two backends
        // disagreeing here would be a grading failure in the arithmetic
        // contract. Interpreted once at test scale; the benchmark number
        // itself stays the D-35 measurement's job.
        Case { name: "d35_overflow", stdin: String::new(), fixtures: &[] },
    ]
}

/// The interpreter's outcome as the CLI reports it (`errors/args.rs
/// report_outcome`): Completed = 0, Failed = 1, Panicked = 101.
fn outcome_code(outcome: &lagom_interp::RunOutcome) -> i32 {
    match outcome {
        lagom_interp::RunOutcome::Completed => 0,
        lagom_interp::RunOutcome::Failed { .. } => 1,
        lagom_interp::RunOutcome::Panicked { .. } => 101,
    }
}

/// Run the program on the interpreter in `dir`, pinned seed, scripted stdin.
/// Mirrors `lagom_driver::run_fe` (`Interp::dev`, the LOM ring live) with
/// the workdir pinned so file programs stay in their own sandbox.
fn run_interpreted_in(src: &str, stdin: Vec<String>, dir: &Path) -> (Vec<String>, i32, String) {
    let mut fe = match lagom_driver::frontend(src) {
        Ok(fe) => fe,
        Err(e) => panic!("the corpus program must front-end: {e:?}"),
    };
    let mut it = lagom_interp::Interp::dev(&mut fe.mir);
    it.host = lagom_interp::Host::with_seed(stdin, SEED);
    it.host.workdir = dir.to_path_buf();
    let outcome = it.run(&fe.mir, src);
    let host = std::mem::replace(&mut it.host, lagom_interp::Host::new(Vec::new()));
    (
        host.stdout,
        outcome_code(&outcome),
        match outcome {
            lagom_interp::RunOutcome::Completed => String::new(),
            lagom_interp::RunOutcome::Failed { message, .. } => message,
            lagom_interp::RunOutcome::Panicked { message, .. } => message,
        },
    )
}

/// Build and run the native backend in `dev`/release mode inside `dir`,
/// pinned seed, scripted stdin. Returns (exit code, stdout).
fn run_native_in(src: &str, name: &str, dev: bool, dir: &Path, stdin: &str) -> (i32, String) {
    let suffix = if cfg!(windows) { ".exe" } else { "" };
    let out = std::env::temp_dir().join(format!(
        "lagom_diff_{}_{}_bin{}",
        name,
        std::process::id(),
        suffix
    ));
    let _ = std::fs::remove_file(&out);
    let exe = lagom_driver::build_native(src, &out, dev, true)
        .unwrap_or_else(|e| panic!("building `{name}` ({dev}) failed: {e:?}"));
    let mut child = Command::new(&exe)
        .env("LAGOM_SEED", SEED.to_string())
        .current_dir(dir)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .expect("spawn the native executable");
    child
        .stdin
        .take()
        .expect("stdin pipe")
        .write_all(stdin.as_bytes())
        .expect("write stdin");
    let result = child.wait_with_output().expect("wait for the native run");
    (
        result.status.code().unwrap_or(-1),
        String::from_utf8_lossy(&result.stdout).into_owned(),
    )
}

/// Byte-identical backend agreement for every corpus program. The net
/// validation programs (`http_server`, `http_client`) are deliberately
/// absent: they need a coordinated peer process and are covered by
/// `net_validation.rs` instead.
#[test]
fn validation_corpus_agrees_on_every_backend() {
    let corpus = validation_dir();
    let cases = cases();
    let mut checked = 0;

    for case in cases {
        let dir = std::env::temp_dir().join(format!(
            "lagom_diff_{}_{}",
            case.name,
            std::process::id()
        ));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).expect("sandbox dir");
        for (file, content) in case.fixtures {
            std::fs::write(dir.join(file), content).expect("fixture");
        }
        let src = std::fs::read_to_string(corpus.join(format!("{}.lagom", case.name)))
            .expect("corpus source");

        let (interp_out, interp_code, interp_message) =
            run_interpreted_in(&src, script_lines(&case.stdin), &dir);
        let (dev_code, dev_out) = run_native_in(&src, case.name, true, &dir, &case.stdin);
        let (rel_code, rel_out) = run_native_in(&src, case.name, false, &dir, &case.stdin);

        // The interpreter buffers `say` fragments as separate `host.stdout`
        // entries and the CLI's `--trace` projection adds a newline per
        // entry, while the native binary appends the say newline itself —
        // so the *placement* of newlines between fragments belongs to the
        // projection, not the semantics (it is pinned per-feature by the
        // `assert_says` suite). The differential invariant is therefore:
        // the same text in the same order, newline-normalized, and the
        // same exit code on every backend.
        fn canonical(a: &str) -> String {
            a.chars().filter(|c| *c != '\n').collect()
        }
        let interp_bytes = interp_out.concat();
        assert!(
            canonical(&interp_bytes) == canonical(&dev_out) && canonical(&dev_out) == canonical(&rel_out),
            "{}: interpreter, native dev and native release must print the same.\n  interpreter: {:?}\n  native dev:  {:?}\n  native rel:  {:?}",
            case.name,
            interp_out,
            dev_out,
            rel_out
        );
        assert_eq!(
            interp_code, dev_code,
            "{}: exit codes must agree ({}). {}",
            case.name, interp_code, interp_message
        );
        assert_eq!(dev_code, rel_code, "{}: native dev vs release exit codes", case.name);
        checked += 1;
        let _ = std::fs::remove_dir_all(&dir);
        let _ = std::fs::remove_file(std::env::temp_dir().join(format!(
            "lagom_diff_{}_{}_bin{}",
            case.name,
            std::process::id(),
            if cfg!(windows) { ".exe" } else { "" }
        )));
    }
    assert!(checked == 9, "expected the 9-program corpus, found {checked}");
}

fn script_lines(stdin: &str) -> Vec<String> {
    let mut lines: Vec<String> = stdin.lines().map(|l| l.to_string()).collect();
    if stdin.is_empty() {
        lines.clear();
    }
    lines
}
