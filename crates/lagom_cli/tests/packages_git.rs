//! The package manager's end-to-end gate (docs/10-roadmap.md, M2): "a real
//! package published end-to-end". A library is "published" as a git
//! repository; a consumer project `lagom add`s it, the manifest records the
//! URL, `Lagom.lock` pins the resolved commit hash, the sources vendor into
//! `.lagom/`, and `lagom run` builds and executes the merged program —
//! including refetching the vendored copy on a fresh checkout, so pinning
//! is reproducibility, not decoration.
//!
//! Everything is local git (no network): git itself is the transport under
//! test, the same transport a GitHub URL uses.

use std::fs;
use std::path::Path;
use std::process::Command;
use std::sync::atomic::{AtomicUsize, Ordering};

static SEQ: AtomicUsize = AtomicUsize::new(0);

/// A unique temp dir base — the tests run in parallel.
fn tmpbase(tag: &str) -> std::path::PathBuf {
    let n = SEQ.fetch_add(1, Ordering::SeqCst);
    let dir =
        std::env::temp_dir().join(format!("lagom_pkggit_{tag}_{}_{}", std::process::id(), n));
    let _ = fs::remove_dir_all(&dir);
    fs::create_dir_all(&dir).expect("temp dir");
    dir
}

fn run_in(dir: &Path, args: &[&str]) -> (i32, String) {
    let out = Command::new(env!("CARGO_BIN_EXE_lagom"))
        .args(args)
        .current_dir(dir)
        .output()
        .expect("run lagom");
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

/// The published "package": a git repository holding a small teaching
/// library (manifest + doc comments + one `.lagom` source).
fn publish_library(dir: &Path) -> String {
    fs::write(
        dir.join("cards.lagom"),
        "## The suit of a rank 0..3.\n## >>> say suit name 1\n## = diamonds\nfunction suit name\n    takes number called index\n    returns text\n    make suits equal to a list of \"hearts\", \"diamonds\", \"clubs\", \"spades\"\n    gives back suits at index\n",
    )
    .expect("library source");
    fs::write(
        dir.join("Lagom.toml"),
        "[package]\nname = \"cards\"\n\n[dependencies]\n",
    )
    .expect("library manifest");
    git(
        dir,
        &["init", "-q", "-b", "main"],
    );
    // Reproduce an old pinned commit when the repository moved on (the
    // default host policy decides whether a *publishing* host allows this).
    git(dir, &["config", "uploadpack.allowReachableSHA1InWant", "true"]);
    git(dir, &["add", "."]);
    git(
        dir,
        &["-c", "user.email=ci@lagom.test", "-c", "user.name=Card Library", "commit", "-q", "-m", "the cards teaching library"],
    );
    let head = git_capture_out(dir, &["rev-parse", "HEAD"]);
    assert_eq!(head.len(), 40, "full sha expected, got {head:?}");
    dir.to_str().expect("utf8 lib dir").to_string()
}

fn git(dir: &Path, args: &[&str]) {
    let out = Command::new("git")
        .args(args)
        .current_dir(dir)
        .output()
        .expect("run git (the package transport)");
    assert!(
        out.status.success(),
        "git {args:?} failed: {}",
        String::from_utf8_lossy(&out.stderr)
    );
}

fn git_capture_out(dir: &Path, args: &[&str]) -> String {
    let out = Command::new("git")
        .args(args)
        .current_dir(dir)
        .output()
        .expect("run git");
    String::from_utf8_lossy(&out.stdout).trim().to_string()
}

/// The full gate: publish → add (clone + pin) → run (vendored merge) →
/// fresh checkout refetches → run replays the pin.
#[test]
fn package_published_end_to_end() {
    let root = tmpbase("e2e");
    let lib = root.join("cards");
    fs::create_dir_all(&lib).expect("lib dir");
    let url = publish_library(&lib);

    let consumer = root.join("player");
    fs::create_dir_all(&consumer).expect("consumer dir");
    fs::write(consumer.join("main.lagom"), "say suit name 1\n").expect("entry");

    // 1. `lagom add` records the dependency and vendors the clone.
    let (code, out) = run_in(&consumer, &["add", "cards", &format!("git+{url}")]);
    assert_eq!(code, 0, "{out}");
    assert!(out.contains("vendored in .lagom/cards"), "{out}");
    assert!(out.contains("pinned to"), "{out}");
    assert!(Path::new(&consumer).join(".lagom").join("cards").join("cards.lagom").exists());

    // 2. The manifest keeps the unpinned URL; the lock pins the commit.
    let manifest = fs::read_to_string(consumer.join("Lagom.toml")).unwrap();
    assert!(!manifest.contains("@"), "manifest must stay unpinned: {manifest}");
    let lock = fs::read_to_string(consumer.join("Lagom.lock")).unwrap();
    assert!(lock.contains("git+"), "{lock}");
    let pin = git_capture_out(&lib, &["rev-parse", "HEAD"]);
    assert!(lock.contains(&format!("cards = \"git+{url}@{pin}\"")), "{lock}");

    // 3. `lagom run` merges the vendored library and executes the program.
    let (code, out) = run_in(&consumer, &["run"]);
    assert_eq!(code, 0, "{out}");
    // The whole story on failure: the run output AND the vendored source
    // (the clone is the only content the merge could have seen).
    let vendored = fs::read_to_string(consumer.join(".lagom").join("cards").join("cards.lagom")).unwrap_or_default();
    assert!(out.contains("diamonds"), "{out}\n--- vendored cards.lagom ---\n{vendored}");

    // 4. A fresh checkout (machine-local vendor dir deleted) refetches the
    //    locked pin and builds again.
    fs::remove_dir_all(consumer.join(".lagom")).expect("drop vendor dir");
    let (code, out) = run_in(&consumer, &["run"]);
    assert_eq!(code, 0, "{out}");
    // Same story dump on the refetch path.
    let vendored = fs::read_to_string(consumer.join(".lagom").join("cards").join("cards.lagom")).unwrap_or_default();
    let lock = fs::read_to_string(consumer.join("Lagom.lock")).unwrap_or_default();
    assert!(out.contains("diamonds"), "{out}\n--- lock ---\n{lock}\n--- vendored cards.lagom ---\n{vendored}");
    assert!(Path::new(&consumer).join(".lagom").join("cards").join("cards.lagom").exists());
    let vendored_pin = git_capture_out(&consumer.join(".lagom").join("cards"), &["rev-parse", "HEAD"]);
    assert_eq!(vendored_pin, pin, "the refetch must reproduce the locked commit");

    let _ = fs::remove_dir_all(&root);
}

/// Re-adding a published package updates the pin to the new HEAD (re-add
/// is the update story). And a checkout whose lock pins an older commit
/// builds that older commit even after the repository moved on — the pin
/// is reproducibility, never silently upgraded.
#[test]
fn repins_on_readd_and_reproduces_an_older_pin() {
    let root = tmpbase("repin");
    let lib = root.join("cards");
    fs::create_dir_all(&lib).expect("lib dir");
    let url = publish_library(&lib);
    let first_pin = git_capture_out(&lib, &["rev-parse", "HEAD"]);

    let consumer = root.join("player");
    fs::create_dir_all(&consumer).expect("consumer dir");
    fs::write(consumer.join("main.lagom"), "say suit name 2\n").expect("entry");
    let (code, out) = run_in(&consumer, &["add", "cards", &format!("git+{url}")]);
    assert_eq!(code, 0, "{out}");
    // v1 list order: index 2 is `clubs`.
    let (code, out) = run_in(&consumer, &["run"]);
    assert_eq!(code, 0, "{out}");
    // Dump the whole story on the first v1 run: the lock pin and the
    // vendored source tell wrong-clone apart from wrong-merge.
    let vendored = fs::read_to_string(consumer.join(".lagom").join("cards").join("cards.lagom")).unwrap_or_default();
    let lock = fs::read_to_string(consumer.join("Lagom.lock")).unwrap_or_default();
    assert!(out.contains("clubs"), "v1 output expected: {out}\n--- lock ---\n{lock}\n--- vendored cards.lagom ---\n{vendored}");

    // The publisher ships v2: same library, different list order —
    // index 2 is now `diamonds`.
    fs::write(
        lib.join("cards.lagom"),
        "function suit name\n    takes number called index\n    returns text\n    make suits equal to a list of \"spades\", \"clubs\", \"diamonds\", \"hearts\"\n    gives back suits at index\n",
    )
    .expect("v2 library source");
    git(&lib, &["add", "."]);
    git(
        &lib,
        &["-c", "user.email=ci@lagom.test", "-c", "user.name=Card Library", "commit", "-q", "-m", "v2 list order"],
    );
    let second_pin = git_capture_out(&lib, &["rev-parse", "HEAD"]);
    assert_ne!(first_pin, second_pin);

    // Re-add: the vendored copy refreshes and the lock re-pins to the new
    // HEAD; the consumer now runs the second version.
    let (code, out) = run_in(&consumer, &["add", "cards", &format!("git+{url}")]);
    assert_eq!(code, 0, "{out}");
    let lock = fs::read_to_string(consumer.join("Lagom.lock")).unwrap();
    assert!(lock.contains(&format!("@{second_pin}")), "lock must now pin {second_pin}: {lock}");
    let (code, out) = run_in(&consumer, &["run"]);
    assert_eq!(code, 0, "{out}");
    assert!(out.contains("diamonds"), "v2 output expected: {out}");

    // The old pin is still reproducible after the repository moved on:
    // a fresh vendored fetch of a project locked to `first_pin` checks out
    // that exact commit (fetched by hash), and the v1 output returns.
    let older = root.join("older");
    fs::create_dir_all(&older).expect("older consumer dir");
    fs::write(older.join("main.lagom"), "say suit name 2\n").expect("entry");
    fs::write(
        older.join("Lagom.toml"),
        format!("[package]\nname = \"older\"\n\n[dependencies]\ncards = \"git+{url}\"\n"),
    )
    .expect("manifest");
    fs::write(
        older.join("Lagom.lock"),
        format!("# Lagom.lock — resolved dependencies.\ncards = \"git+{url}@{first_pin}\"\n"),
    )
    .expect("lock");
    let (code, out) = run_in(&older, &["run"]);
    assert_eq!(code, 0, "{out}");
    let vendored_pin =
        git_capture_out(&older.join(".lagom").join("cards"), &["rev-parse", "HEAD"]);
    assert_eq!(vendored_pin, first_pin, "the fetch must reproduce the locked commit");
    assert!(out.contains("clubs"), "v1 output expected from the old pin: {out}");

    let _ = fs::remove_dir_all(&root);
}

/// An unreachable or wrong source URL fails honestly: no clone, no
/// recorded dependency, a teaching message with the name.
#[test]
fn a_bad_git_url_fails_honestly() {
    let root = tmpbase("badurl");
    let consumer = root.join("app");
    fs::create_dir_all(&consumer).expect("consumer dir");
    let missing = root.join("nonexistent-repo").to_str().unwrap().to_string();

    let (code, out) = run_in(&consumer, &["add", "cards", &format!("git+{missing}")]);
    assert_eq!(code, 1, "{out}");
    assert!(out.contains("could not fetch"), "{out}");
    assert!(out.contains("cards"), "{out}");
    let manifest = fs::read_to_string(consumer.join("Lagom.toml")).unwrap_or_default();
    assert!(!manifest.contains("cards"), "failed fetch must not record the dep: {manifest}");
    let _ = fs::remove_dir_all(&root);
}
