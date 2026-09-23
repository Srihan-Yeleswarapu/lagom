//! `lagom uninstall` behavior, tested against the real binary (doc 09's
//! install story: the uninstall is part of the product, so it is gated like
//! one). Plan-only default, `--yes` execution, the refusals (source
//! checkouts, cargo caches), the rc cleanup, and the honest nothing-found
//! report. HOME is redirected at a sandbox for every run, so the tests
//! never touch the real machine's rc files or `~/lagom`.

use std::fs;
use std::path::PathBuf;
use std::process::Command;
use std::sync::atomic::{AtomicUsize, Ordering};

/// Unique sandbox root per invocation — the tests run in parallel.
static SEQ: AtomicUsize = AtomicUsize::new(0);

/// One sandbox: a fake home (for the rc files) plus an install folder.
struct Sandbox {
    root: PathBuf,
    home: PathBuf,
}

impl Sandbox {
    fn fresh(tag: &str) -> Sandbox {
        let n = SEQ.fetch_add(1, Ordering::SeqCst);
        let root = std::env::temp_dir().join(format!(
            "lagom_uninst_{tag}_{}_{}",
            std::process::id(),
            n
        ));
        let _ = fs::remove_dir_all(&root);
        let home = root.join("home");
        fs::create_dir_all(&home).expect("sandbox home");
        Sandbox { root, home }
    }

    /// A package-style install folder: a binary (this test executable stands
    /// in for `lagom`) plus the bundled `lib/lagom` directory. It lives at
    /// `<home>/lagom` — the release page's own example spot, as the child
    /// (whose HOME is the sandbox) sees it.
    fn install_dir(&self, name: &str) -> PathBuf {
        let dir = self.home.join(name);
        fs::create_dir_all(dir.join("lib/lagom")).expect("install dir");
        fs::write(dir.join("lagom"), b"binary").expect("binary");
        fs::write(dir.join("lib/lagom/rustc.txt"), "stub").expect("stamp");
        dir
    }

    /// Run the built `lagom uninstall` with HOME pointed at the sandbox and
    /// the running binary's own location outside every candidate folder.
    /// Returns (exit code, combined stdout+stderr).
    fn run(&self, args: &[&str]) -> (i32, String) {
        let exe = env!("CARGO_BIN_EXE_lagom");
        let mut cmd = Command::new(exe);
        cmd.args(["uninstall"]).args(args);
        cmd.env("HOME", &self.home);
        cmd.env("USERPROFILE", &self.home); // Windows spelling of HOME
        let out = cmd.output().expect("run lagom uninstall");
        let text = format!(
            "{}{}",
            String::from_utf8_lossy(&out.stdout),
            String::from_utf8_lossy(&out.stderr)
        )
        .replace("\r\n", "\n");
        (out.status.code().unwrap_or(-1), text)
    }
}

impl Drop for Sandbox {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.root);
    }
}

/// The plan is the default: nothing is deleted, the folder still exists.
#[test]
fn plan_only_by_default() {
    let sb = Sandbox::fresh("plan");
    let dir = sb.install_dir("lagom");
    let (code, out) = sb.run(&[]);
    assert_eq!(code, 0, "{out}");
    assert!(out.contains("uninstall plan"), "{out}");
    assert!(out.contains("nothing was changed"), "{out}");
    assert!(dir.join("lagom").exists(), "plan-only must not delete");
    assert!(dir.join("lib/lagom").exists(), "plan-only must not delete");
}

/// `--yes` removes the install folder when it is not the running copy
/// (the fake home is `~/lagom`; the test binary runs from target/, so the
/// running-copy candidate is skipped as an unrelated non-package folder).
#[test]
fn yes_removes_the_install_folder() {
    let sb = Sandbox::fresh("yes");
    let dir = sb.install_dir("lagom");
    let (code, out) = sb.run(&["--yes"]);
    assert_eq!(code, 0, "{out}");
    assert!(
        !dir.exists(),
        "the install folder should be gone: {}",
        dir.display()
    );
    assert!(out.contains("removed"), "{out}");
    assert!(out.contains("lagom is uninstalled"), "{out}");
}

/// A second `--yes` run finds nothing and says so honestly (exit 0).
#[test]
fn nothing_found_reports_cleanly() {
    let sb = Sandbox::fresh("empty");
    let (code, out) = sb.run(&["--yes"]);
    assert_eq!(code, 0, "{out}");
    assert!(out.contains("nothing to uninstall"), "{out}");
}

/// A folder holding Cargo.toml is a source checkout: reported, refused,
/// never deleted.
#[test]
fn refuses_source_checkouts() {
    let sb = Sandbox::fresh("checkout");
    let dir = sb.install_dir("lagom");
    fs::write(dir.join("Cargo.toml"), b"[package]\nname = \"lagom\"\n").expect("manifest");
    let (code, out) = sb.run(&["--yes"]);
    assert_eq!(code, 0, "a refusal is a successful plan, not a crash: {out}");
    assert!(out.contains("refusing"), "{out}");
    assert!(out.contains("Cargo.toml"), "{out}");
    assert!(dir.exists(), "a checkout must not be deleted");
}

/// A cargo target directory (no CACHEDIR.TAG shipped, so the incremental/
/// .cargo-lock markers are the check) is refused too.
#[test]
fn refuses_cargo_target_dirs() {
    let sb = Sandbox::fresh("target");
    let dir = sb.install_dir("lagom");
    fs::create_dir_all(dir.join("incremental")).expect("marker");
    let (code, out) = sb.run(&["--yes"]);
    assert_eq!(code, 0, "{out}");
    assert!(out.contains("refusing"), "{out}");
    assert!(out.contains("target directory") || out.contains("cargo"), "{out}");
    assert!(dir.exists(), "a cargo target dir must not be deleted");
}

/// `--rc` strips exactly the PATH lines mentioning lagom from the rc files
/// that have one, leaves the other lines (and files without one) alone, and
/// is named in the plan.
#[test]
fn rc_cleanup_removes_only_lagom_path_lines() {
    let sb = Sandbox::fresh("rc");
    let _dir = sb.install_dir("lagom");
    fs::write(
        sb.home.join(".zshrc"),
        "export EDITOR=helix\nexport PATH=\"$PATH:$HOME/lagom\"\nalias gs='git status'\n",
    )
    .expect("zshrc");
    fs::write(sb.home.join(".bashrc"), "export JAVA_HOME=/opt/java\n").expect("bashrc");

    // Plan mentions the rc cleanup; nothing is rewritten yet.
    let (code, out) = sb.run(&["--rc"]);
    assert_eq!(code, 0, "{out}");
    assert!(out.contains(".zshrc"), "{out}");
    let zshrc = fs::read_to_string(sb.home.join(".zshrc")).expect("read plan-time zshrc");
    assert!(zshrc.contains("lagom"), "plan-only must not rewrite");

    // Execute: the lagom line goes, the rest stays, .bashrc is untouched.
    let (code, out) = sb.run(&["--rc", "--yes"]);
    assert_eq!(code, 0, "{out}");
    assert!(out.contains("PATH line(s) from"), "{out}");
    let zshrc = fs::read_to_string(sb.home.join(".zshrc")).expect("read zshrc");
    assert!(!zshrc.contains("lagom"), "the lagom PATH line must go: {zshrc}");
    assert!(zshrc.contains("EDITOR=helix"), "other lines must stay: {zshrc}");
    assert!(zshrc.contains("alias gs"), "other lines must stay: {zshrc}");
    let bashrc = fs::read_to_string(sb.home.join(".bashrc")).expect("read bashrc");
    assert_eq!(
        bashrc, "export JAVA_HOME=/opt/java\n",
        "an rc without a lagom line must not be touched"
    );
}

/// The help text and dispatch know the command.
#[test]
fn usage_lists_uninstall() {
    let exe = env!("CARGO_BIN_EXE_lagom");
    let out = Command::new(exe).arg("--help").output().expect("help");
    let text = String::from_utf8_lossy(&out.stdout).replace("\r\n", "\n");
    assert!(text.contains("uninstall"), "{text}");
}
