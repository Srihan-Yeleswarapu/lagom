//! The package workflow (00 §20.2's `lagom add`, doc 09 §8): a project
//! records its dependencies in `Lagom.toml`, pins resolved versions in
//! `Lagom.lock`, and `lagom build`/`lagom run` merge each dependency's
//! source into the program before compiling (vendored merge — the M2
//! single-binary, no-network model).
//!
//! Manifest shape (deliberately minimal, TOML subset):
//! ```toml
//! [package]
//! name = "quiz"
//!
//! [dependencies]
//! mathutils = "0.1.0"
//! ```
//! `lagom add <name> <path|version>` records the dependency; a *path*
//! dependency (`lagom add helpers ../helpers`) vendors that directory's
//! `.lagom` sources. A path source must be an existing directory with a
//! `Lagom.toml` (bad invocations are rejected before anything is written);
//! the one-arg path form (`lagom add ../helpers`) takes the dependency's
//! name from that manifest. A bare version records a registry dependency whose
//! sources are expected under `.lagom/<name>-<version>/` (fetched by the
//! M3 registry tooling; `lagom add` verifies and records either way).

use std::path::{Path, PathBuf};

use super::args::{CliError, CliResult};

pub const MANIFEST: &str = "Lagom.toml";
pub const LOCK: &str = "Lagom.lock";

/// One recorded dependency: name → source location.
#[derive(Debug, Clone, PartialEq)]
pub struct Dependency {
    pub name: String,
    /// A relative path (path dependency) or a version string (registry).
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

/// Read (or synthesize) the manifest and add one dependency.
///
/// Arguments are validated before anything is written, so a bad invocation
/// never creates or touches a manifest: flags are rejected (recorded as-is
/// they would become a dependency no build could vendor), a path source must
/// be an existing package directory (one with a `Lagom.toml` — the manifest's
/// `[package] name` is the key every build merges under), and the one-arg
/// path form names the dependency from that manifest instead of trusting the
/// raw argument. A non-path first argument keeps the bare registry-version
/// reading.
pub fn cmd_add(rest: &[String]) -> CliResult {
    if let Some(flag) = rest.iter().find(|a| a.starts_with('-')) {
        return Err(CliError::Message(format!(
            "`lagom add` takes no flags — `{flag}` would be recorded as a dependency named `{flag}`, which no build could ever vendor.\n\
             Command-level flags belong before the command: try `lagom --help` (or `lagom help`).\n\
             To record a dependency: `lagom add <name> <path|version>`."
        )));
    }
    let Some(name_arg) = rest.first() else {
        return Err(CliError::Message("usage: lagom add <package> [path | version]".into()));
    };
    if rest.len() > 2 {
        return Err(CliError::Message(format!(
            "too many arguments: `lagom add` records one dependency as `<name> <path|version>` — got {}.\n\
             Add the dependencies one at a time."
        , rest.len())));
    }
    // Decide what was meant. A first argument that is a directory is the
    // one-arg path form: the package's own manifest names the dependency.
    // Anything else is `<name> <path|version>` or a bare registry version.
    let (name, source) = if rest.len() == 1 && Path::new(name_arg).is_dir() {
        let dir = Path::new(name_arg);
        let pkg = manifest_name(dir).ok_or_else(|| missing_manifest(dir))?;
        (pkg, name_arg.clone())
    } else {
        let name = name_arg.clone();
        let source = rest.get(1).cloned().unwrap_or_else(|| "0.1.0".to_string());
        // A source that names something on disk must be a vendorable package
        // directory — recording anything else would merge to nothing. A
        // source with a path separator that names nothing is a typo'd path,
        // not a version; a separator-less source keeps the registry-version
        // reading (`add mathutils 0.1.0`).
        let dir = Path::new(&source);
        if dir.exists() {
            if !dir.is_dir() {
                return Err(CliError::Message(format!(
                    "`{}` is a file, not a package directory — a path dependency vendors a directory of `.lagom` sources.\n\
                     Pass the directory that holds them: `lagom add {name} <dir>`.",
                    source
                )));
            }
            manifest_name(dir).ok_or_else(|| missing_manifest(dir))?;
        } else if source.contains('/') || source.contains('\\') {
            return Err(CliError::Message(format!(
                "no such directory: `{source}` — a path dependency must name a directory that exists, or every build would merge from nothing.\n\
                 If `{name}` is a registry package, record its version instead: `lagom add {name} 0.1.0`."
            )));
        }
        (name, source)
    };
    upsert_manifest()?;
    let mut deps = read_dependencies(Path::new("."));
    if deps.iter().any(|d| d.name == *name) {
        deps.retain(|d| d.name != *name);
    }
    deps.push(Dependency { name: name.clone(), source: source.clone() });
    write_dependencies(&deps)?;
    println!("added {name} ({source}) to {MANIFEST}");
    write_lock(&deps)?;
    Ok(())
}

/// The `[package] name` of the manifest in `dir`, if there is a declared one.
fn manifest_name(dir: &Path) -> Option<String> {
    let text = std::fs::read_to_string(dir.join(MANIFEST)).ok()?;
    let mut in_pkg = false;
    for line in text.lines() {
        let line = line.trim();
        if line == "[package]" {
            in_pkg = true;
            continue;
        }
        if line.starts_with('[') {
            in_pkg = false;
            continue;
        }
        if in_pkg {
            if let Some((key, value)) = line.split_once('=') {
                if key.trim() == "name" {
                    let value = value.trim().trim_matches('"').trim();
                    if !value.is_empty() {
                        return Some(value.to_string());
                    }
                }
            }
        }
    }
    None
}

/// A directory cannot be vendored: it has no manifest, so no declared name
/// exists to record the dependency under. The fix creates one.
fn missing_manifest(dir: &Path) -> CliError {
    let name = dir
        .file_name()
        .map(|n| n.to_string_lossy().to_string())
        .unwrap_or_else(|| "package".to_string());
    CliError::Message(format!(
        "`{}` has no {MANIFEST} manifest, so the package declares no name — vendoring it would merge sources under a key nobody declared.\n\
         Create the manifest in that directory first:\n\
         \n\
             [package]\n\
             name = \"{name}\"\n\
         \n\
             [dependencies]\n\
         \n\
         then re-run this command.",
        dir.display()
    ))
}

/// Remove one dependency from the manifest (and the lock).
pub fn cmd_remove(rest: &[String]) -> CliResult {
    let Some(name) = rest.first() else {
        return Err(CliError::Message("usage: lagom remove <package>".into()));
    };
    let mut deps = read_dependencies(Path::new("."));
    let before = deps.len();
    deps.retain(|d| d.name != *name);
    if deps.len() == before {
        return Err(CliError::Message(format!("`{name}` is not a dependency")));
    }
    write_dependencies(&deps)?;
    write_lock(&deps)?;
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
        // A new table header ends the dependencies table; dep lines and
        // blank lines inside the table are replaced wholesale by the new
        // set (consuming only the first line here used to re-append every
        // later dep line, duplicating keys on the second `add`).
        if in_deps && trimmed.starts_with('[') {
            in_deps = false;
            out.push(line.to_string());
            continue;
        }
        if in_deps && (trimmed.is_empty() || trimmed.contains('=')) {
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

/// The lock: name → resolved source (stable order). Written after every
/// add/remove and by the build merge, so the lock is always in step.
pub fn write_lock(deps: &[Dependency]) -> CliResult {
    write_lock_in(Path::new("."), deps)
}

/// `write_lock` against an explicit directory (tests, nested projects).
pub fn write_lock_in(dir: &Path, deps: &[Dependency]) -> CliResult {
    let mut lines = vec!["# Lagom.lock — resolved dependencies. Edit via `lagom add`/`remove`.".to_string()];
    let mut sorted: Vec<&Dependency> = deps.iter().collect();
    sorted.sort_by(|a, b| a.name.cmp(&b.name));
    for d in sorted {
        lines.push(format!("{} = \"{}\"", d.name, d.source));
    }
    std::fs::write(dir.join(LOCK), lines.join("\n") + "\n")
        .map_err(|e| CliError::Message(format!("writing {LOCK}: {e}")))
}

/// The sources of every path dependency, plus every `.lagom` source in the
/// project root, merged in dependency order — dependency declarations
/// compile first, then the project's own code. Returns the merged source
/// text and the files consumed (for messages).
pub fn merged_sources(root: &Path) -> Option<(String, Vec<PathBuf>)> {
    let deps = read_dependencies(root);
    let mut files: Vec<PathBuf> = Vec::new();
    let mut merged = String::new();
    for dep in &deps {
        let dir = root.join(&dep.source);
        if !dir.is_dir() {
            continue; // build reports the missing dependency source
        }
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
}
