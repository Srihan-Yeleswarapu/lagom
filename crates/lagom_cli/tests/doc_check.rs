// Copyright (c) 2026 Srihan Yeleswarapu.
// Source available under the Lagom License (LICENSE.md).

//! `lagom doc --check` regression tests (doc 09, decision D-40): every
//! example form the docs use must check, and every unsupported or wrong pin
//! must fail loudly — never a false pass. The contract lives in doc 09
//! ("Decision D-40: the executable-example contract"); these tests pin it.

use std::fs;
use std::path::PathBuf;
use std::process::Command;
use std::sync::atomic::{AtomicUsize, Ordering};

/// Unique temp dir per invocation — the tests run in parallel.
static SEQ: AtomicUsize = AtomicUsize::new(0);

/// Write `src` into a fresh temp dir as `doc.lagom`, run the built
/// `lagom doc --check` on it, and return (exit code, combined output).
fn check(src: &str) -> (i32, String) {
    let n = SEQ.fetch_add(1, Ordering::SeqCst);
    let dir = std::env::temp_dir().join(format!("lagom_doccheck_{}_{}", std::process::id(), n));
    let _ = fs::remove_dir_all(&dir);
    fs::create_dir_all(&dir).expect("temp dir");
    let path: PathBuf = dir.join("doc.lagom");
    fs::write(&path, src).expect("write doc file");
    let exe = env!("CARGO_BIN_EXE_lagom");
    let out = Command::new(exe)
        .args(["doc", "--check"])
        .arg(&path)
        .current_dir(&dir)
        .output()
        .expect("run lagom doc --check");
    let text = format!(
        "{}{}",
        String::from_utf8_lossy(&out.stdout),
        String::from_utf8_lossy(&out.stderr)
    )
    .replace("\r\n", "\n");
    let _ = fs::remove_dir_all(&dir);
    (out.status.code().unwrap_or(-1), text)
}

const DOUBLE_FN: &str = "\
function double
    takes number called n
    returns a number
    gives back n times 2
";

/// The audit's exact case: a `make` example pins its bound value — the
/// teaching echo is the meaning of `=` (D-40, rule 2).
#[test]
fn make_example_pins_the_bound_value() {
    let (code, out) = check(&format!(
        "## Doubles n.\n## >>> make x equal to double 2\n## = 4\n{DOUBLE_FN}"
    ));
    assert_eq!(code, 0, "{out}");
    assert!(out.contains("1 example(s) ok"), "{out}");
}

/// The say form: the pin is the `say` output (D-40, rule 1 — output wins).
#[test]
fn say_example_pins_the_printed_output() {
    let (code, out) = check(&format!(
        "## Greets loudly.\n## >>> say double 21\n## = 42\n{DOUBLE_FN}"
    ));
    assert_eq!(code, 0, "{out}");
}

/// Consecutive `## >>>` lines are ONE example: later statements see earlier
/// bindings (D-40, rule 1).
#[test]
fn consecutive_gtgtgt_lines_are_one_example() {
    let (code, out) = check(&format!(
        "## Chain.\n## >>> make base equal to 10\n## >>> make grown equal to base times 3\n## = 30\n{DOUBLE_FN}"
    ));
    assert_eq!(code, 0, "{out}");
}

/// An unpinned example still must compile and run (the Rust doctest rule).
#[test]
fn unpinned_example_still_must_run() {
    let (code, out) = check(&format!(
        "## Runs.\n## >>> make x equal to double 2\n{DOUBLE_FN}"
    ));
    assert_eq!(code, 0, "{out}");
}

/// A wrong pinned value fails with the expected/actual diff — the pin is a
/// hard contract, never ignored.
#[test]
fn wrong_pinned_value_fails() {
    let (code, out) = check(&format!(
        "## Doubles n.\n## >>> make x equal to double 2\n## = 5\n{DOUBLE_FN}"
    ));
    assert_ne!(code, 0, "{out}");
    assert!(out.contains("value mismatch"), "{out}");
    assert!(out.contains("\"5\""), "{out}");
    assert!(out.contains("\"4\""), "{out}");
}

/// Wrong `say` output fails the same way.
#[test]
fn wrong_pinned_output_fails() {
    let (code, out) = check(&format!(
        "## Greets loudly.\n## >>> say double 21\n## = 43\n{DOUBLE_FN}"
    ));
    assert_ne!(code, 0, "{out}");
    assert!(out.contains("output mismatch"), "{out}");
}

/// A bare call prints nothing and binds nothing: a `=` on it fails with the
/// remedy, never a false pass (D-40, rule 4).
#[test]
fn pin_on_silent_example_fails_loudly() {
    let (code, out) = check(&format!(
        "## Silent.\n## >>> double 2\n## = 4\n{DOUBLE_FN}"
    ));
    assert_ne!(code, 0, "{out}");
    assert!(
        out.contains("prints nothing and binds nothing"),
        "{out}"
    );
}

/// A `=` with no example above it cannot be checked at all: loud failure.
#[test]
fn stray_equals_line_fails_loudly() {
    let (code, out) = check(&format!("## Stray.\n## = 4\n{DOUBLE_FN}"));
    assert_ne!(code, 0, "{out}");
    assert!(out.contains("no `## >>>` example above it"), "{out}");
}

/// An example that does not compile fails with student diagnostics — and
/// the check runs even when no doc comment is attached to a declaration
/// (the early `no doc comments` return must never skip checking).
#[test]
fn uncompilable_example_fails_with_student_diagnostics() {
    let (code, out) = check("## Bad name.\n## >>> say scoer\n");
    assert_ne!(code, 0, "{out}");
    assert!(out.contains("does not compile"), "{out}");
    assert!(out.contains("scoer"), "{out}");
    assert!(out.contains("I do not know what"), "{out}");
}

/// The file scope is visible to the example (the Rust doctest rule), and a
/// file that does not compile reports that — not a confusing example error.
#[test]
fn file_itself_must_compile_first() {
    let (code, out) = check(&format!(
        "## Doubles n.\n## >>> make x equal to double 2\n## = 4\nfunction double\n    takes number called n\n    gives back n times 2\n    say scoer\n"
    ));
    assert_ne!(code, 0, "{out}");
    assert!(out.contains("does not compile"), "{out}");
}
