//! The diagnostic surface must be coherent: a live error's page exists
//! (E0390 once had its prose under the wrong key, so `lagom explain E0390`
//! said "no write-up yet"), a program that cannot run is rejected rather
//! than panicking at runtime (`send "x", m` once passed check and crashed
//! both backends), and `lagom words` lists the real vocabulary.

use std::fs;
use std::path::PathBuf;
use std::process::Command;
use std::sync::atomic::{AtomicUsize, Ordering};

static SEQ: AtomicUsize = AtomicUsize::new(0);

fn lagom(args: &[&str]) -> (i32, String) {
    let exe = env!("CARGO_BIN_EXE_lagom");
    let out = Command::new(exe).args(args).output().expect("run lagom");
    (
        out.status.code().unwrap_or(-1),
        format!(
            "{}{}",
            String::from_utf8_lossy(&out.stdout),
            String::from_utf8_lossy(&out.stderr)
        )
        .replace("\r\n", "\n"),
    )
}

fn check(src: &str) -> (i32, String) {
    let n = SEQ.fetch_add(1, Ordering::SeqCst);
    let path: PathBuf = std::env::temp_dir().join(format!("lagom_surf_{}_{}.lagom", std::process::id(), n));
    fs::write(&path, src).expect("write program");
    let (code, text) = lagom(&["check", path.to_str().unwrap()]);
    let _ = fs::remove_file(&path);
    (code, text)
}

/// The wait-without-spawn page lives at E0390 and shares its headline with
/// the live diagnostic — page and error can never drift apart silently.
#[test]
fn wait_without_spawn_page_matches_the_live_diagnostic() {
    let (code, text) = check("wait for all tasks\n");
    assert_eq!(code, 1, "the program must not check:\n{text}");
    let headline = "`wait for all tasks` found no `start a task` to wait for.";
    assert!(text.contains(headline), "{text}");
    let (page_code, page) = lagom(&["explain", "E0390"]);
    assert_eq!(page_code, 0, "{page}");
    assert!(page.contains(headline), "{page}");
}

/// A comma-form send is channel vocabulary without the prep that carries
/// the channel: rejected at check time (E0391) with the page's own fix —
/// never a false pass that panics in the backend.
#[test]
fn comma_form_send_is_rejected_with_the_page_fix() {
    let (code, text) = check("make m equal to a channel of text\nsend \"x\", m\n");
    assert_eq!(code, 1, "comma-form send must not pass check:\n{text}");
    assert!(text.contains("missing the `to`"), "{text}");
    assert!(text.contains("send <value> to <channel>"), "{text}");
    let (_, page) = lagom(&["explain", "E0391"]);
    assert!(page.contains("send <value> to <channel>"), "{page}");
    // Same rule for `receive`.
    let (code, text) = check("make m equal to a channel of text\nsay receive(m)\n");
    assert_eq!(code, 1, "comma-form receive must not pass check:\n{text}");
    assert!(text.contains("missing the `from`"), "{text}");
}

/// `lagom words` reflects the real language surface, tasks included.
#[test]
fn words_lists_the_task_and_channel_vocabulary() {
    let (_, text) = lagom(&["words"]);
    for phrase in [
        "start a task",
        "keep going",
        "wait for all tasks",
        "a channel of",
        "send",
        "receive",
    ] {
        assert!(text.contains(phrase), "words must list `{phrase}`:\n{text}");
    }
}

/// The explainer is context-aware: with a file argument, the static page
/// continues with the code's REAL instances from that program — span, note,
/// and the contextual fix the checker computed (did-you-mean drawn from the
/// scope's actual bindings). A clean file says the code does not fire
/// rather than inventing an example.
#[test]
fn explain_page_with_a_file_shows_the_codes_own_context() {
    let n = SEQ.fetch_add(1, Ordering::SeqCst);
    let path = std::env::temp_dir().join(format!("lagom_ctx_{}_{}.lagom", std::process::id(), n));
    // Two names starting with `sco` — the context fix names `scoer`, the
    // binding the program actually made, not a canned example.
    fs::write(&path, "make scoer equal to 3\nsay scouer\n").expect("write program");
    let (code, page) = lagom(&["explain", "E0344", path.to_str().unwrap()]);
    assert_eq!(code, 0, "{page}");
    // The page itself opens first...
    assert!(page.contains("E0344 — "), "the teaching page opens:\n{page}");
    // ...then the context section renders the real occurrence...
    assert!(
        page.contains("`E0344` in") && page.contains("1 occurrence"),
        "the context section titles the instances:\n{page}"
    );
    // ...with the compiler's context-real suggestion drawn from THIS scope.
    assert!(
        page.contains("did you mean `scoer`?"),
        "the context fix names the program's own binding (not `score`)::\n{page}"
    );
    // A clean file reports honestly: nothing to point at, no fake example.
    fs::write(&path, "say \"fine\"\n").expect("rewrite program");
    let (code, page) = lagom(&["explain", "E0344", path.to_str().unwrap()]);
    assert_eq!(code, 0, "{page}");
    assert!(
        page.contains("does not fire — the file checks out clean"),
        "no false instances on a clean file:\n{page}"
    );
    let _ = fs::remove_file(&path);
}
