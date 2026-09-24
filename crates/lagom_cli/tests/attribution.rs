// Copyright (c) 2026 Srihan Yeleswarapu.
// Source available under the Lagom License (LICENSE.md).

//! Attribution, gated by the test suite like every other behavior (doc 09:
//! the CI release smoke test asserts the same lines, and license-guard
//! asserts the sources still carry them — this file pins the *runtime*
//! output): `lagom version` must print the copyright and the build's
//! provenance, `lagom license` must print the license exactly as it ships
//! in the repo, and `lagom help` must end with the notice. A copy of the
//! compiler whose attribution was stripped fails here too — the tests run
//! whatever binary is built, official or not.

use std::process::Command;

/// Run the built `lagom` with the given arguments.
/// Returns (exit code, combined stdout+stderr).
fn run(args: &[&str]) -> (i32, String) {
    let exe = env!("CARGO_BIN_EXE_lagom");
    let out = Command::new(exe)
        .args(args)
        .output()
        .expect("run the lagom binary");
    let mut text = String::from_utf8_lossy(&out.stdout).into_owned();
    text.push_str(&String::from_utf8_lossy(&out.stderr));
    (out.status.code().unwrap_or(-1), text)
}

const NOTICE: &str = "Copyright (c) 2026 Srihan Yeleswarapu";

#[test]
fn version_prints_the_copyright_line() {
    let (code, out) = run(&["version"]);
    assert_eq!(code, 0);
    // The identity line stays first and machine-shaped (26.1): installers
    // and scripts read it, so the attribution comes after it.
    let first = out.lines().next().expect("version prints at least one line");
    assert!(
        first.starts_with("lagom ") && first.contains(env!("CARGO_PKG_VERSION")),
        "identity line changed shape: {first:?}"
    );
    assert!(out.contains(NOTICE), "version lost the copyright line:\n{out}");
    assert!(
        out.contains("the Lagom License"),
        "version no longer names the license:\n{out}"
    );
    assert!(
        out.contains("lagom license"),
        "version no longer points at `lagom license`:\n{out}"
    );
}

#[test]
fn version_prints_the_build_provenance() {
    // Either CI stamped the commit or the build says honestly that it is a
    // local checkout — but one of the two must always be printed.
    let (_, out) = run(&["version"]);
    assert!(
        out.contains("built from a local checkout") || out.contains("built from commit "),
        "version lost the provenance line:\n{out}"
    );
}

#[test]
fn license_prints_the_full_terms() {
    let (code, out) = run(&["license"]);
    assert_eq!(code, 0);
    assert!(out.starts_with("# The Lagom License"), "wrong text first:\n{out}");
    assert!(out.contains(NOTICE), "the printed license lost the copyright");
    assert!(
        out.contains("What you may NOT do without the Author's prior written permission"),
        "the printed license lost §2 (the no-modify/no-redistribute terms)"
    );
    // The file ends with a newline; the printed copy must too.
    assert!(out.ends_with('\n'), "the printed license lost its trailing newline");
}

#[test]
fn license_output_matches_the_repo_license_exactly() {
    // The embedded copy must be the repo's LICENSE.md verbatim — not a
    // truncated or paraphrased stand-in. The two drift apart only if the
    // `include_str!` is unwound (which license-guard also checks), but the
    // suite asserts the property itself, independent of any workflow.
    let repo_license = std::fs::read_to_string(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../LICENSE.md"
    ))
    .expect("read the repo's LICENSE.md");
    let (code, out) = run(&["license"]);
    assert_eq!(code, 0);
    assert_eq!(out, repo_license, "`lagom license` is not printing LICENSE.md verbatim");
}

#[test]
fn help_ends_with_the_copyright_notice() {
    let (code, out) = run(&["help"]);
    assert_eq!(code, 0);
    // The footer line continues past the name (it names the license and
    // the command), so the *last* line must start with the notice.
    let last = out.lines().rev().find(|l| !l.trim().is_empty()).expect("a footer line");
    assert!(
        last.starts_with(NOTICE) && last.contains("the Lagom License"),
        "help lost the copyright footer:\n{out}"
    );
}
