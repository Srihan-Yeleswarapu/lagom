//! `lagom profile` must render user-facing labels that map to the user's
//! code: a student's functions print as themselves, a compiler-synthesized
//! lambda names its owner — with the internal name kept after it for
//! cross-reference against `--trace`. (Before this, the whole row was
//! `%lambda mains128 1 (100.0%)`.)

use std::fs;
use std::path::PathBuf;
use std::process::Command;
use std::sync::atomic::{AtomicUsize, Ordering};

/// Unique temp file per invocation — the tests run in parallel.
static SEQ: AtomicUsize = AtomicUsize::new(0);

fn profile(src: &str) -> String {
    let n = SEQ.fetch_add(1, Ordering::SeqCst);
    let path: PathBuf = std::env::temp_dir().join(format!("lagom_prof_{}_{}.lagom", std::process::id(), n));
    fs::write(&path, src).expect("write profile program");
    let exe = env!("CARGO_BIN_EXE_lagom");
    let out = Command::new(exe).arg("profile").arg(&path).output().expect("run lagom profile");
    let _ = fs::remove_dir_all(path.parent().unwrap().join("target"));
    let text = format!(
        "{}{}",
        String::from_utf8_lossy(&out.stdout),
        String::from_utf8_lossy(&out.stderr)
    )
    .replace("\r\n", "\n");
    let _ = fs::remove_file(&path);
    text
}

/// A task body lowers to a synthetic lambda; the profile row must name the
/// construct and its owner, not bare compiler internals.
#[test]
fn synthetic_lambda_row_names_the_owner_and_keeps_the_internal_name() {
    let src = "\
class counter
    has count of type number

make b equal to a new counter with count 1

start a task
    set count of b to 42

wait for all tasks
say count of b
";
    let text = profile(src);
    let row = text
        .lines()
        .find(|l| l.contains("100.0%"))
        .unwrap_or_else(|| panic!("no profile row in:\n{text}"));
    assert!(row.contains("a lambda in `main`"), "row must name the owner: {row}");
    // The internal name stays for cross-reference with `--trace`.
    assert!(row.contains("[%lambda mains"), "row must keep the internal name: {row}");
}

/// A student's own function is never relabeled.
#[test]
fn student_function_prints_as_itself() {
    let src = "\
function double
    takes number called n
    returns number
    gives back n times 2

say double 21
";
    let text = profile(src);
    assert!(text.contains("double"), "student fn row must print its own name:\n{text}");
    assert!(!text.contains("%lambda"), "no synthetic names on the surface:\n{text}");
}
