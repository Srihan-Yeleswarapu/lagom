//! The package workflow (00 §20.2's `lagom add`, doc 09 §8): a project
//! records its dependencies in `Lagom.toml`, pins resolved versions in
//! `Lagom.lock`, and `lagom build`/`lagom run` merge each dependency's
//! source into the program before compiling (vendored merge — the M2
//! single-binary, no-network-at-build model).
//!
//! Manifest shape (deliberately minimal, TOML subset):
//! ```toml
//! [package]
//! name = "quiz"
//!
//! [dependencies]
//! mathutils = "0.1.0"
//! ```
//!
//! Three source shapes, in preference order:
//! - **path** — `helpers = "../helpers"`: vendored from that directory,
//!   re-read on every build (dev-owned).
//! - **git** — `cards = "git+<url>"`: `lagom add` clones HEAD into
//!   `.lagom/<name>/`, records the manifest entry unpinned, and pins the
//!   *resolved* commit hash in `Lagom.lock` (`cards = "git+<url>@<sha>"`).
//!   `run`/`build` then always reproduce that commit: they re-fetch a
//!   missing vendored copy (so a fresh checkout builds) and check out the
//!   pinned hash when the repository has moved on — a pin is never
//!   silently upgraded.
//! - **registry version** — `mathutils = "0.1.0"`: recorded now, fetched
//!   when the registry ships (M3 tooling); the build reports vendoring
//!   gaps honestly instead of compiling without the dependency.
//!
//! `.lagom/` is machine-local (like `target/`), always regenerated from
//! the lock — it must not be committed.

use std::path::{Path, PathBuf};

use super::args::{CliError, CliResult};

pub const MANIFEST: &str = "Lagom.toml";
pub const LOCK: &str = "Lagom.lock";
/// Machine-local vendored sources (git clones, registry packs).
pub const VENDOR_DIR: &str = ".lagom";

/// One recorded dependency: name → source location.
#[derive(Debug, Clone, PartialEq)]
pub struct Dependency {
    pub name: String,
    /// A relative path, a version string, or a `git+<url>` location.
    pub source: String,
}

/// The `[dependencies]` table of `Lagom.toml`, in file order.
pub fn read_dependencies(dir: &Path) -> Vec<Dependency> {
    let Ok(text) = std::fs::read_to_string(dir.join(MANIFEST)) else {
        return Vec::new();
    };
    let mut deps = Vec::new();
    let mut in_deps = false;
    for line in text.lines() {
        let line = line.trim();
        if line == "[dependencies]" {
            in_deps = true;
            continue;
        }
        if line.starts_with('[') {
            in_deps = false;
            continue;
        }
        if in_deps {
            if let Some((name, source)) = line.split_once('=') {
                let source = source.trim().trim_matches('"').to_string();
                deps.push(Dependency { name: name.trim().to_string(), source });
            }
        }
    }
    deps
}

/// Whether `source` names a git dependency — the explicit `git+` prefix or
/// any transport git itself accepts (https, ssh, `git@`, `file://`, `*.git`).
pub fn is_git_source(source: &str) -> bool {
    source.starts_with("git+") || is_git_url(source)
}

fn is_git_url(source: &str) -> bool {
    ["http://", "https://", "ssh://", "git@", "file://"]
        .iter()
        .any(|p| source.starts_with(p))
        || (source.ends_with(".git") && !looks_like_version(source))
}

/// `owner/repo` (exactly one slash, nothing on disk named that) names the
/// GitHub project `owner/repo`.
pub fn github_shorthand(source: &str) -> Option<String> {
    if source.contains("://") || source.starts_with("git@") {
        return None;
    }
    let (owner, repo) = source.split_once('/')?;
    if owner.is_empty() || repo.is_empty() || repo.contains('/') {
        return None;
    }
    if Path::new(source).exists() {
        return None; // it is a path dependency, not a name
    }
    if owner.chars().chain(repo.chars()).any(|c| c.is_whitespace()) {
        return None;
    }
    Some(format!("https://github.com/{source}"))
}

/// `0.1.1`, `2.0` — digits and dots only: a registry version, not a path.
pub fn looks_like_version(source: &str) -> bool {
    !source.is_empty()
        && source.contains('.')
        && source
            .split('.')
            .all(|p| !p.is_empty() && p.chars().all(|c| c.is_ascii_digit()))
}

/// Run `git` and treat a nonzero exit as failure text; the caller owns the
/// teaching message built on top.
pub fn git_capture(args: &[&str]) -> Result<String, String> {
    let out = std::process::Command::new("git").args(args).output();
    match out {
        Err(e) => Err(format!("{e}")),
        Ok(out) if out.status.success() => {
            Ok(String::from_utf8_lossy(&out.stdout).trim().to_string())
        }
        Ok(out) => Err(String::from_utf8_lossy(&out.stderr).trim().to_string()),
    }
}

/// Shallow-clone `url` into `dest` (removing any previous vendored copy —
/// re-adding is the update story; a fresh clone is what a pin resolves).
/// The failure names the package, not just the transport.
fn clone_into(package: &str, url: &str, dest: &Path) -> Result<(), CliError> {
    let _ = std::fs::remove_dir_all(dest);
    let dest_str = dest.to_str().unwrap_or_default();
    git_capture(&["clone", "--depth", "1", "--quiet", url, dest_str]).map_err(|git_err| {
        CliError::Message(format!(
            "could not fetch {package} from {url}: {git_err}\n\nCheck the URL and your network. The dependency was not recorded."
        ))
    })?;
    Ok(())
}

/// The full commit hash vendored at `dest`.
fn vendored_sha(dest: &Path) -> Result<String, String> {
    let dest_str = dest.to_str().unwrap_or_default();
    git_capture(&["-C", dest_str, "rev-parse", "HEAD"])
}

fn dep(name: &str, source: &str) -> Dependency {
    Dependency { name: name.to_string(), source: source.to_string() }
}

/// Read (or synthesize) the manifest and add one dependency. A `path` that
/// exists is recorded verbatim; a git source is cloned and pinned;
/// anything else is treated as a registry version.
pub fn cmd_add(rest: &[String]) -> CliResult {
    let Some(name) = rest.first() else {
        return Err(CliError::Message(
            "usage: lagom add <package> <path | git+<url> | owner/repo | version>".into(),
        ));
    };
    let requested = rest
        .get(1)
        .cloned()
        .unwrap_or_else(|| "0.1.0".to_string());
    upsert_manifest()?;

    if is_git_source(&requested) {
        let pinned = add_git(name, &requested)?;
        refresh_lock(name, Some(pinned))?;
    } else if !Path::new(&requested).exists() {
        if let Some(url) = github_shorthand(&requested) {
            println!("(no `{requested}` directory here — treating it as the GitHub package {url})");
            let pinned = add_git(name, &format!("git+{url}"))?;
            refresh_lock(name, Some(pinned))?;
        } else {
            // Registry version: recorded, locked to the given version, to be
            // fetched by the registry (M3).
            record(name, &requested)?;
            println!("added {name} ({requested}) to {MANIFEST}");
            refresh_lock(name, Some(dep(name, &requested)))?;
        }
    } else {
        // Path dependency, recorded and vendored in place.
        record(name, &requested)?;
        println!("added {name} ({requested}) to {MANIFEST}");
        refresh_lock(name, Some(dep(name, &requested)))?;
    }
    Ok(())
}

/// Add (or refresh) a git dependency: clone, verify it carries `.lagom`
/// sources, record the manifest entry unpinned, update the lock to the
/// resolved commit, and return full resolution.
fn add_git(name: &str, requested: &str) -> Result<Dependency, CliError> {
    let url = requested.strip_prefix("git+").unwrap_or(requested);
    let dest = Path::new(VENDOR_DIR).join(name);
    clone_into(name, url, &dest)?;
    let sources = list_lagom(&dest);
    if sources.is_empty() {
        let _ = std::fs::remove_dir_all(&dest);
        return Err(CliError::Message(format!(
            "cloned {url} but it contains no .lagom sources — that is not a Lagom package.\nThe empty clone was removed; the dependency was not recorded."
        )));
    }
    let sha = vendored_sha(&dest).map_err(|e| {
        CliError::Message(format!("could not resolve the fetched commit for {name}: {e}"))
    })?;
    let short: String = sha.chars().take(12).collect();

    // Manifest keeps the unpinned URL; the lock keeps the resolved commit.
    let manifest_entry = dep(name, &format!("git+{url}"));
    _ = record(name, &manifest_entry.source)?;
    println!(
        "added {name} ({url}) — {} .lagom source(s) vendored in {VENDOR_DIR}/{name}, pinned to {short} in {LOCK}",
        sources.len()
    );
    Ok(dep(name, &format!("{}@{sha}", manifest_entry.source)))
}

/// Insert or update exactly one dependency row in the manifest's
/// `[dependencies]` table, preserving every other line.
fn record(name: &str, source: &str) -> Result<Vec<Dependency>, CliError> {
    let mut deps = read_dependencies(Path::new("."));
    deps.retain(|d| d.name != name);
    deps.push(dep(name, source));
    write_dependencies(&deps)?;
    Ok(read_dependencies(Path::new(".")))
}

/// Rewrite the lock with `name`'s entry replaced by `entry`. Pins recorded
/// for *other* dependencies survive; manifest rows missing from the lock
/// (hand-edited manifests, projects without an add round-trip) are
/// appended unpinned. Returns the final lock contents in file order.
fn refresh_lock(name: &str, entry: Option<Dependency>) -> Result<Vec<Dependency>, CliError> {
    let mut lock = read_lock(Path::new("."));
    lock.retain(|l| l.name != name);
    if let Some(dep) = entry {
        lock.retain(|l| l.name != dep.name);
        lock.push(dep);
    }
    for d in read_dependencies(Path::new(".")) {
        if !lock.iter().any(|l| l.name == d.name) {
            lock.push(d);
        }
    }
    lock.retain(|l| !l.name.is_empty());
    write_lock(&lock)?;
    Ok(lock)
}

/// Remove one dependency from the manifest (and the lock, and its vendored
/// clone under `.lagom/` when it was a git dependency). Other pins survive.
pub fn cmd_remove(rest: &[String]) -> CliResult {
    let Some(name) = rest.first() else {
        return Err(CliError::Message("usage: lagom remove <package>".into()));
    };
    let deps = read_dependencies(Path::new("."));
    let removed = deps.iter().find(|d| d.name == *name).cloned();
    let mut remaining = deps;
    remaining.retain(|d| d.name != *name);
    let removed = match removed {
        Some(gone) => gone,
        None => return Err(CliError::Message(format!("`{name}` is not a dependency"))),
    };
    write_dependencies(&remaining)?;
    refresh_lock(name, None)?;
    if is_git_source(&removed.source) {
        let _ = std::fs::remove_dir_all(Path::new(VENDOR_DIR).join(name));
    }
    println!("removed {name} from {MANIFEST}");
    Ok(())
}

/// Ensure a manifest exists (creating a minimal one when absent).
fn upsert_manifest() -> CliResult {
    let path = Path::new(MANIFEST);
    if path.exists() {
        return Ok(());
    }
    let name = std::env::current_dir()
        .ok()
        .and_then(|d| d.file_name().map(|n| n.to_string_lossy().to_string()))
        .unwrap_or_else(|| "project".to_string());
    std::fs::write(path, format!("[package]\nname = \"{name}\"\n\n[dependencies]\n"))
        .map_err(|e| CliError::Message(format!("writing {MANIFEST}: {e}")))
}

/// Rewrite only the `[dependencies]` table, preserving every other line.
fn write_dependencies(deps: &[Dependency]) -> CliResult {
    let path = Path::new(MANIFEST);
    let text = std::fs::read_to_string(path).unwrap_or_else(|_| "[package]\nname = \"project\"\n".into());
    let mut out: Vec<String> = Vec::new();
    let mut written = false;
    let mut in_deps = false;
    for line in text.lines() {
        let trimmed = line.trim();
        if trimmed == "[dependencies]" {
            in_deps = true;
            out.push(line.to_string());
            for d in deps {
                out.push(format!("{} = \"{}\"", d.name, d.source));
            }
            written = true;
            continue;
        }
        if in_deps && (trimmed.is_empty() || trimmed.starts_with('[') || trimmed.contains('=')) {
            in_deps = false;
            if trimmed.starts_with('[') {
                out.push(line.to_string());
            }
            continue;
        }
        out.push(line.to_string());
    }
    if !written {
        out.push("[dependencies]".to_string());
        for d in deps {
            out.push(format!("{} = \"{}\"", d.name, d.source));
        }
    }
    std::fs::write(path, out.join("\n") + "\n")
        .map_err(|e| CliError::Message(format!("writing {MANIFEST}: {e}")))
}

/// The lock, parsed as `name = "source"` lines (git entries carry `@<sha>`).
pub fn read_lock(dir: &Path) -> Vec<Dependency> {
    let Ok(text) = std::fs::read_to_string(dir.join(LOCK)) else {
        return Vec::new();
    };
    text.lines()
        .filter_map(|line| {
            let line = line.trim();
            let (name, source) = line.split_once('=')?;
            let name = name.trim();
            if name.is_empty() || name.starts_with('#') {
                return None;
            }
            Some(dep(name, source.trim().trim_matches('"')))
        })
        .collect()
}

/// The lock: name → resolved source (stable order). Written after every
/// add/remove and by the build merge, so the lock is always in step.
pub fn write_lock(deps: &[Dependency]) -> CliResult {
    write_lock_in(Path::new("."), deps)
}

/// `write_lock` against an explicit directory (tests, nested projects).
pub fn write_lock_in(dir: &Path, deps: &[Dependency]) -> CliResult {
    let mut lines = vec![
        "# Lagom.lock — resolved dependencies. Edit via `lagom add`/`remove`.".to_string(),
    ];
    let mut sorted: Vec<&Dependency> = deps.iter().collect();
    sorted.sort_by(|a, b| a.name.cmp(&b.name));
    for d in sorted {
        lines.push(format!("{} = \"{}\"", d.name, d.source));
    }
    std::fs::write(dir.join(LOCK), lines.join("\n") + "\n")
        .map_err(|e| CliError::Message(format!("writing {LOCK}: {e}")))
}

/// The sources of every dependency plus every `.lagom` source in the
/// project root, merged in dependency order — dependency declarations
/// compile first, then the project's own code. Returns the merged source
/// text and the files consumed (for messages).
pub fn merged_sources(root: &Path) -> Option<(String, Vec<PathBuf>)> {
    let deps = read_dependencies(root);
    let mut files: Vec<PathBuf> = Vec::new();
    let mut merged = String::new();
    for dep in &deps {
        let Some(dir) = dependency_dir(root, dep) else {
            continue; // the build reports the missing dependency source
        };
        let mut sources: Vec<PathBuf> = list_lagom(&dir);
        sources.sort();
        for f in sources {
            if let Ok(text) = std::fs::read_to_string(&f) {
                merged.push_str(&text);
                merged.push('\n');
                files.push(f);
            }
        }
    }
    // The project's own root sources (excluding dependencies themselves).
    for f in list_lagom(root) {
        if let Ok(text) = std::fs::read_to_string(&f) {
            merged.push_str(&text);
            merged.push('\n');
            files.push(f);
        }
    }
    if merged.is_empty() {
        None
    } else {
        Some((merged, files))
    }
}

/// Where one dependency's sources live, resolved like the lock says: git
/// deps from their vendored clone, path deps in place, registry versions
/// from the `.lagom/<name>-<version>/` pack.
fn dependency_dir(root: &Path, dep: &Dependency) -> Option<PathBuf> {
    if is_git_source(&dep.source) {
        let vendored = root.join(VENDOR_DIR).join(&dep.name);
        return vendored.is_dir().then_some(vendored);
    }
    let plain = root.join(&dep.source);
    if plain.is_dir() {
        return Some(plain);
    }
    if looks_like_version(&dep.source) {
        let vendored = root.join(VENDOR_DIR).join(format!("{}-{}", dep.name, dep.source));
        return vendored.is_dir().then_some(vendored);
    }
    None
}

/// Make sure every recorded dependency the build needs is on disk, and
/// pinned git dependencies are *at* their pin:
/// - a git dep with no vendored copy → clone (a fresh checkout builds),
///   checking out the locked hash when the repository has moved on;
/// - a vendored copy whose HEAD no longer matches its pin → refetch by
///   hash (never silently upgrade);
/// - a registry version with no pack → an honest report, not a silent
///   skip that produces a misleading unknown-name error;
/// - path deps are dev-owned and simply must exist as recorded.
///
/// Called by `lagom run`/`lagom build` before merging sources.
pub fn ensure_vendored(root: &Path) -> Result<(), CliError> {
    let deps = read_dependencies(root);
    if deps.is_empty() {
        return Ok(());
    }
    let lock = read_lock(root);
    let lock_pin = |dep_name: &str| {
        lock.iter()
            .find(|l| l.name == dep_name)
            .and_then(|l| l.source.rsplit_once('@'))
            .map(|(_, sha)| sha.to_string())
    };
    for dep in &deps {
        if is_git_source(&dep.source) {
            let dest = root.join(VENDOR_DIR).join(&dep.name);
            let pin = lock_pin(&dep.name);
            let vendored = dest.is_dir() && !list_lagom(&dest).is_empty();
            match (vendored, pin) {
                (true, None) => {}
                (true, Some(want)) => {
                    if vendored_sha(&dest).unwrap_or_default() != want {
                        refetch_at(dest.clone(), &dep.name, &dep.source, Some(&want))?;
                    }
                }
                (false, pin) => {
                    refetch_at(dest.clone(), &dep.name, &dep.source, pin.as_deref())?;
                }
            }
        } else if looks_like_version(&dep.source) && dependency_dir(root, dep).is_none() {
            return Err(CliError::Message(format!(
                "dependency `{} {}` is recorded but not vendored — registry fetching ships with the registry (M3).\nMeanwhile: vendor it at {VENDOR_DIR}/{}-{} or change it to a path or git+ dependency.",
                dep.name, dep.source, dep.name, dep.source
            )));
        }
    }
    Ok(())
}

/// Clone `url` into `dest` and, when the lock pins an exact hash that is
/// not the fresh HEAD, fetch and check out exactly that hash. Failure to
/// reach the pin never silently substitutes another commit — the build
/// either reproduces the pin or stops with the reason.
fn refetch_at(
    dest: PathBuf,
    name: &str,
    manifest_source: &str,
    pin: Option<&str>,
) -> Result<(), CliError> {
    let url = manifest_source.strip_prefix("git+").unwrap_or(manifest_source);
    clone_into(name, url, &dest)?;
    if let Some(want) = pin {
        let head = vendored_sha(&dest).unwrap_or_default();
        if head != want {
            let dest_str = dest.to_str().unwrap_or_default();
            if let Err(e) = git_capture(&["-C", dest_str, "fetch", "--depth", "1", "origin", want]) {
                return Err(CliError::Message(format!(
                    "{} is pinned to {} in {LOCK}, but fetching that exact commit failed:\n{e}\n\nThe repository may have dropped it; re-pin with `lagom remove {name}` then `lagom add {name} {manifest_source}`.",
                    name,
                    wshort(want)
                )));
            }
            git_capture(&["-C", dest_str, "checkout", "--detach", want]).map_err(|e| {
                CliError::Message(format!("could not check out the pinned commit of {name}: {e}"))
            })?;
            match vendored_sha(&dest) {
                Ok(t) if t == want => {}
                _ => {
                    return Err(CliError::Message(format!(
                        "{} is pinned to {} in {LOCK} but the vendored copy does not match — re-pin with `lagom remove {name}` then `lagom add {name} {manifest_source}`.",
                        name,
                        wshort(want)
                    )));
                }
            }
        }
    }
    let sources = list_lagom(&dest);
    if sources.is_empty() {
        let _ = std::fs::remove_dir_all(&dest);
        return Err(CliError::Message(format!(
            "refetched {name} from {url} but it contains no .lagom sources — not a Lagom package."
        )));
    }
    Ok(())
}

fn wshort(sha: &str) -> String {
    sha.chars().take(12).collect()
}

/// The `.lagom` files directly inside `dir` (non-recursive — a package is
/// one directory of files; nested directories are not searched).
fn list_lagom(dir: &Path) -> Vec<PathBuf> {
    let mut out = Vec::new();
    if let Ok(entries) = std::fs::read_dir(dir) {
        for e in entries.flatten() {
            let p = e.path();
            if p.extension().and_then(|s| s.to_str()) == Some("lagom") {
                out.push(p);
            }
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    fn tmpdir(name: &str) -> PathBuf {
        let dir = std::env::temp_dir().join(format!("lagom_pkg_{}_{}", name, std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        dir
    }

    #[test]
    fn add_remove_round_trip() {
        let dir = tmpdir("roundtrip");
        let dep_dir = dir.join("helpers");
        std::fs::create_dir_all(&dep_dir).unwrap();
        std::fs::write(dep_dir.join("util.lagom"), "function twice\n    takes number called n\n    gives back n times 2\n").unwrap();
        let deps = vec![Dependency { name: "helpers".into(), source: "helpers".into() }];
        write_lock_in(&dir, &deps).unwrap();
        let lock = std::fs::read_to_string(dir.join(LOCK)).unwrap();
        assert!(lock.contains("helpers = \"helpers\""), "{lock}");
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn merged_sources_include_dependency_then_project() {
        let dir = tmpdir("merge");
        let dep_dir = dir.join("helpers");
        std::fs::create_dir_all(&dep_dir).unwrap();
        std::fs::write(dep_dir.join("util.lagom"), "function twice\n    takes number called n\n    gives back n times 2\n").unwrap();
        std::fs::write(dir.join(MANIFEST), "[package]\nname = \"app\"\n\n[dependencies]\nhelpers = \"helpers\"\n").unwrap();
        std::fs::write(dir.join("main.lagom"), "say twice 21\n").unwrap();
        let (merged, files) = merged_sources(&dir).expect("merged");
        // Dependency first, then the project's own file.
        let dep_pos = merged.find("gives back n times 2").expect("dep source");
        let main_pos = merged.find("say twice 21").expect("main source");
        assert!(dep_pos < main_pos);
        assert_eq!(files.len(), 2);
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn git_sources_are_recognized_and_path_names_are_not() {
        assert!(is_git_source("git+https://github.com/o/r"));
        assert!(is_git_source("git+../a/local/path"));
        assert!(is_git_source("https://host.com/r.git"));
        assert!(is_git_source("git@host:team/repo.git"));
        assert!(is_git_source("file:///srv/packages/cards"));
        assert!(!is_git_source("0.1.0"));
        assert!(!is_git_source("helpers"));
    }

    #[test]
    fn shorthand_expands_only_when_the_path_is_absent() {
        let dir = tmpdir("shorthand");
        let sampled = dir.join("owner").join("repo");
        std::fs::create_dir_all(&sampled).unwrap();
        // A real local directory named `owner/repo` is a path dependency.
        assert_eq!(github_shorthand(sampled.to_str().unwrap().split_once('/').unwrap().1), None);
        // A real local directory anywhere in the string stays a path check.
        assert_eq!(github_shorthand(sampled.parent().unwrap().join("repo").to_str().unwrap()), None);
        // A name-shaped string names the GitHub package.
        assert_eq!(github_shorthand("cards/deck").as_deref(), Some("https://github.com/cards/deck"));
        assert_eq!(github_shorthand("0.1.0"), None);
        assert_eq!(github_shorthand("owner/a/b"), None);
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn versions_are_versions() {
        assert!(looks_like_version("0.1.0"));
        assert!(looks_like_version("2.0"));
        assert!(!looks_like_version("0.1.0-rc1"));
        assert!(!looks_like_version("helpers"));
    }

    #[test]
    fn lock_round_trips_and_tolerates_pins() {
        let dir = tmpdir("lockpins");
        let deps = vec![
            Dependency { name: "cards".into(), source: "git+https://host/o/r@abc123def456".into() },
            Dependency { name: "helpers".into(), source: "helpers".into() },
        ];
        write_lock_in(&dir, &deps).unwrap();
        let lock = read_lock(&dir);
        assert_eq!(lock.len(), 2);
        assert!(
            lock.iter().any(|d| d.name == "cards" && d.source.ends_with("@abc123def456")),
            "pin survives the lock round trip"
        );
        let _ = std::fs::remove_dir_all(&dir);
    }

    /// A git dependency vendored by a previous session resolves from
    /// `.lagom/<name>` — including when its manifest value is the pinned
    /// form written by another tool.
    #[test]
    fn merged_sources_resolves_git_deps_from_the_vendor_dir() {
        let dir = tmpdir("gitmerge");
        let vendored = dir.join(VENDOR_DIR).join("cards");
        std::fs::create_dir_all(&vendored).unwrap();
        std::fs::write(vendored.join("cards.lagom"), "function suit name\n    returns text\n    gives back \"hearts\"\n").unwrap();
        std::fs::write(
            dir.join(MANIFEST),
            "[package]\nname = \"app\"\n\n[dependencies]\ncards = \"git+https://host/o/r\"\n",
        )
        .unwrap();
        std::fs::write(dir.join("main.lagom"), "say suit name\n").unwrap();
        let (merged, files) = merged_sources(&dir).expect("merged");
        assert!(merged.contains("gives back \"hearts\""), "{merged}");
        assert_eq!(files.len(), 2);
        let _ = std::fs::remove_dir_all(&dir);
    }
}
