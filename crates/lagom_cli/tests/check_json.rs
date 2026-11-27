//! The `lagom check --json` editor contract: an editor surfaces the
//! compiler's teaching diagnostics — it must never have to parse the human
//! rendering to learn what failed, and the pass/fail must live in the exit
//! code so `execFile` callbacks route on it. The document shape mirrors
//! `crates/lagom_web::check` (the browser playground's documented contract)
//! so JSON consumers get one story.

use std::fs;
use std::path::PathBuf;
use std::process::Command;
use std::sync::atomic::{AtomicUsize, Ordering};

static SEQ: AtomicUsize = AtomicUsize::new(0);

fn lagom(args: &[&str]) -> (i32, String, String) {
    let exe = env!("CARGO_BIN_EXE_lagom");
    let out = Command::new(exe).args(args).output().expect("run lagom");
    (
        out.status.code().unwrap_or(-1),
        String::from_utf8_lossy(&out.stdout).replace("\r\n", "\n"),
        String::from_utf8_lossy(&out.stderr).replace("\r\n", "\n"),
    )
}

fn write_program(src: &str) -> PathBuf {
    let n = SEQ.fetch_add(1, Ordering::SeqCst);
    let path: PathBuf = std::env::temp_dir()
        .join(format!("lagom_check_json_{}_{n}.lagom", std::process::id()));
    fs::write(&path, src).expect("write program");
    path
}

#[test]
fn a_clean_program_documents_ok() {
    let path = write_program("say \"hi\"\n");
    let (code, stdout, stderr) = lagom(&["check", path.to_str().unwrap(), "--json"]);
    assert_eq!(code, 0);
    // The whole document lives on stdout; stderr stays silent so JSON
    // consumers never have to demultiplex the channels.
    assert!(stdout.contains(r#""ok": true"#), "{stdout}");
    assert!(stdout.contains(r#""diagnostics": []"#), "{stdout}");
    assert!(stdout.contains(&format!(r#""path": "{}""#, path.display())));
    assert!(
        stderr.trim().is_empty(),
        "a clean check must not write to stderr: {stderr}"
    );
    fs::remove_file(&path).ok();
}

#[test]
fn an_erroring_program_documents_the_diagnostics_and_exits_one() {
    let path = write_program("say it\n");
    let (code, stdout, stderr) = lagom(&["check", path.to_str().unwrap(), "--json"]);
    assert_eq!(code, 1);
    assert!(stdout.contains(r#""ok": false"#), "{stdout}");
    assert!(stdout.contains("\"code\""), "{stdout}");
    assert!(stdout.contains("\"line\"") && stdout.contains("\"endLine\""), "{stdout}");
    assert!(
        stdout.contains(r#""severity": "error""#),
        "the editor needs the severity to route Problems entries: {stdout}"
    );
    assert!(
        stdout.contains("\"rendered\""),
        "the student-mode teaching text travels with the diagnostic: {stdout}"
    );
    // The machine document must not leak the human preamble ("checks out." /
    // the rendered page) around the JSON — it IS the whole stdout payload
    // except whitespace.
    assert!(stdout.trim_start().starts_with('{'), "{stdout}");
    assert!(stderr.trim().is_empty(), "{stderr}");
    fs::remove_file(&path).ok();
}

#[test]
fn human_mode_stays_human() {
    // --json must not change the default face of `lagom check`.
    let path = write_program("say \"hi\"\n");
    let (code, stdout, _) = lagom(&["check", path.to_str().unwrap()]);
    assert_eq!(code, 0);
    assert!(stdout.contains("checks out."), "human mode kept: {stdout}");
    let path2 = write_program("say it\n");
    let (code2, _, stderr2) = lagom(&["check", path2.to_str().unwrap()]);
    assert_eq!(code2, 1);
    // In human mode the teaching text is the stderr payload (the CLI's
    // Compile error channel); --json is what moves it onto stdout.
    assert!(
        stderr2.contains("I do not know what `it` is."),
        "the teaching text stays in human mode: {stderr2}"
    );
    fs::remove_file(&path).ok();
    fs::remove_file(&path2).ok();
}
