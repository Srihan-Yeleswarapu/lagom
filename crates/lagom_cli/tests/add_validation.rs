//! `lagom add` argument validation (doc 09 §8): every bad invocation fails
//! loudly *before* anything is written — an `add` that errors must never
//! leave a `Lagom.toml`/`Lagom.lock` behind (a stray `add --help` once
//! wrote `--help = "0.1.0"` into a root manifest), and a path source must
//! be a real package directory or the vendor-merge would silently merge
//! nothing. Every currently-valid form keeps working unchanged.

use std::fs;
use std::path::PathBuf;
use std::process::Command;
use std::sync::atomic::{AtomicUsize, Ordering};

/// Unique temp dir per test — the tests run in parallel and share nothing.
static SEQ: AtomicUsize = AtomicUsize::new(0);

/// A fresh scratch directory with the built `lagom` aimed at it.
struct Project {
    dir: PathBuf,
}

impl Project {
    fn new(tag: &str) -> Self {
        let n = SEQ.fetch_add(1, Ordering::SeqCst);
        let dir =
            std::env::temp_dir().join(format!("lagom_add_{}_{}_{}", tag, std::process::id(), n));
        let _ = fs::remove_dir_all(&dir);
        fs::create_dir_all(&dir).expect("temp project dir");
        Self { dir }
    }

    fn lagom(&self, args: &[&str]) -> (i32, String) {
        let exe = env!("CARGO_BIN_EXE_lagom");
        let out = Command::new(exe)
            .args(args)
            .current_dir(&self.dir)
            .output()
            .expect("run lagom add");
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

    fn manifest(&self) -> Option<String> {
        fs::read_to_string(self.dir.join("Lagom.toml")).ok()
    }

    fn lock(&self) -> Option<String> {
        fs::read_to_string(self.dir.join("Lagom.lock")).ok()
    }

    /// A dependency package dir with a declared name (and optional source).
    fn dep(&self, dirname: &str, name: &str, src: Option<&str>) {
        let d = self.dir.join(dirname);
        fs::create_dir_all(&d).expect("dep dir");
        fs::write(
            d.join("Lagom.toml"),
            format!("[package]\nname = \"{name}\"\n\n[dependencies]\n"),
        )
        .expect("dep manifest");
        if let Some(text) = src {
            fs::write(d.join("util.lagom"), text).expect("dep source");
        }
    }
}

impl Drop for Project {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.dir);
    }
}

// ------------------------------------------------- bad invocations: no writes

/// The audit's exact case: `add --help` once wrote `--help = "0.1.0"` into a
/// manifest. Flags are rejected before anything is created.
#[test]
fn flags_are_rejected_without_creating_a_manifest() {
    let p = Project::new("flags");
    let (code, out) = p.lagom(&["add", "--help"]);
    assert_eq!(code, 1, "{out}");
    assert!(out.contains("takes no flags"), "{out}");
    assert!(out.contains("dependency named `--help`"), "{out}");
    assert!(p.manifest().is_none(), "a rejected add must not create a manifest");
    assert!(p.lock().is_none(), "a rejected add must not create a lock");
}

#[test]
fn nonexistent_path_source_is_rejected_before_any_write() {
    let p = Project::new("ghost");
    let (code, out) = p.lagom(&["add", "ghost", "../nowhere"]);
    assert_eq!(code, 1, "{out}");
    assert!(out.contains("no such directory"), "{out}");
    assert!(out.contains("lagom add ghost 0.1.0"), "{out}");
    assert!(p.manifest().is_none());
    assert!(p.lock().is_none());
}

#[test]
fn a_file_as_path_source_is_rejected() {
    let p = Project::new("file");
    fs::write(p.dir.join("plain.txt"), "x").expect("file");
    let (code, out) = p.lagom(&["add", "wrong", "plain.txt"]);
    assert_eq!(code, 1, "{out}");
    assert!(out.contains("is a file, not a package directory"), "{out}");
    assert!(out.contains("lagom add wrong <dir>"), "{out}");
    assert!(p.manifest().is_none());
}

#[test]
fn a_directory_without_a_manifest_is_rejected() {
    let p = Project::new("bare");
    fs::create_dir_all(p.dir.join("bare")).expect("dir");
    let (code, out) = p.lagom(&["add", "bare", "bare"]);
    assert_eq!(code, 1, "{out}");
    assert!(out.contains("has no Lagom.toml"), "{out}");
    // The fix shows the manifest to create, with the declared name.
    assert!(out.contains("[package]"), "{out}");
    assert!(out.contains("name = \"bare\""), "{out}");
    assert!(p.manifest().is_none(), "a rejected add must not create a manifest");
}

#[test]
fn too_many_arguments_are_rejected() {
    let p = Project::new("toomany");
    let (code, out) = p.lagom(&["add", "a", "b", "c"]);
    assert_eq!(code, 1, "{out}");
    assert!(out.contains("too many arguments"), "{out}");
    assert!(p.manifest().is_none());
}

// --------------------------------------- the one-arg path form reads the manifest

/// `lagom add <path>` (one arg, a directory) names the dependency from the
/// package's own manifest — never from the raw argument string.
#[test]
fn one_arg_path_form_takes_the_name_from_the_dep_manifest() {
    let p = Project::new("onearg");
    p.dep("vendored", "vendored_tools", Some("function thrice\n    takes number called n\n    gives back n times 3\n"));
    let (code, out) = p.lagom(&["add", "vendored"]);
    assert_eq!(code, 0, "{out}");
    assert!(out.contains("added vendored_tools (vendored)"), "{out}");
    let manifest = p.manifest().expect("manifest");
    assert!(manifest.contains("vendored_tools = \"vendored\""), "{manifest}");
    assert!(!manifest.contains("vendored ="), "the raw argument must not become the name: {manifest}");
    let lock = p.lock().expect("lock");
    assert!(lock.contains("vendored_tools = \"vendored\""), "{lock}");
}

/// The one-arg form against a directory with no manifest fails the same way
/// the two-arg form does — no name exists to record.
#[test]
fn one_arg_path_form_without_a_manifest_is_rejected() {
    let p = Project::new("onearg_bare");
    fs::create_dir_all(p.dir.join("mystery")).expect("dir");
    let (code, out) = p.lagom(&["add", "mystery"]);
    assert_eq!(code, 1, "{out}");
    assert!(out.contains("has no Lagom.toml"), "{out}");
    assert!(p.manifest().is_none());
}

// --------------------------------------------------------- valid forms unchanged

/// The proven two-arg path-dep flow: add → merge → run, byte-for-byte the
/// behavior that existed before validation (and the manifest keeps working
/// when `[dependencies]` is not the last table).
#[test]
fn two_arg_path_dependency_flow_unchanged() {
    let p = Project::new("golden");
    p.dep("helper", "helper", Some("function twice\n    takes number called n\n    gives back n times 2\n"));
    let (code, out) = p.lagom(&["add", "helper", "helper"]);
    assert_eq!(code, 0, "{out}");
    assert!(out.contains("added helper (helper)"), "{out}");
    fs::write(p.dir.join("main.lagom"), "say twice 21\n").expect("main");
    let (code, out) = p.lagom(&["run"]);
    assert_eq!(code, 0, "{out}");
    assert!(out.contains("42"), "{out}");
    // The dep lines live under [dependencies]; the [package] name above
    // (derived from the project directory) must have survived the rewrite.
    let manifest = p.manifest().expect("manifest");
    let proj = p.dir.file_name().unwrap().to_string_lossy().to_string();
    assert!(manifest.contains(&format!("name = \"{proj}\"")), "{manifest}");
    assert!(manifest.contains("helper = \"helper\""), "{manifest}");
}

/// Registry (bare version) dependencies are unaffected, and re-adding the
/// same name replaces the line instead of duplicating it.
#[test]
fn registry_version_add_and_readd_unchanged() {
    let p = Project::new("registry");
    let (code, out) = p.lagom(&["add", "mathutils", "0.1.0"]);
    assert_eq!(code, 0, "{out}");
    let (code, out) = p.lagom(&["add", "mathutils", "0.2.0"]);
    assert_eq!(code, 0, "{out}");
    assert!(out.contains("added mathutils (0.2.0)"), "{out}");
    let manifest = p.manifest().expect("manifest");
    assert_eq!(manifest.matches("mathutils").count(), 1, "{manifest}");
    assert!(manifest.contains("mathutils = \"0.2.0\""), "{manifest}");
    let proj = p.dir.file_name().unwrap().to_string_lossy().to_string();
    assert!(manifest.contains(&format!("name = \"{proj}\"")), "{manifest}");
}

/// A dep source recorded under [dependencies] must still be present after a
/// later add rewrites the table (the duplicate-line corruption this pass
/// fixed: the rewrite used to re-append every earlier dep line).
#[test]
fn second_add_replaces_the_table_without_duplicating() {
    let p = Project::new("retable");
    p.dep("helper", "helper", None);
    p.lagom(&["add", "helper", "helper"]);
    let (code, out) = p.lagom(&["add", "mathutils", "0.1.0"]);
    assert_eq!(code, 0, "{out}");
    let manifest = p.manifest().expect("manifest");
    assert!(manifest.contains("helper = \"helper\""), "{manifest}");
    assert!(manifest.contains("mathutils = \"0.1.0\""), "{manifest}");
    assert_eq!(manifest.matches("helper =").count(), 1, "{manifest}");
}
