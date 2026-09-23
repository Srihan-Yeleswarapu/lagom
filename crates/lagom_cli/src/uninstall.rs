//! `lagom uninstall` — remove an installed copy of the compiler (doc 09,
//! the tooling story: an install story owes its users the reverse steps).
//!
//! The release page's install is exactly two things: unpack a folder
//! (`lagom` + its bundled `lib/lagom` runtime, anywhere the user chose)
//! and optionally append a PATH line to a shell rc file. Uninstall
//! therefore deletes that folder and — with `--rc` — removes the PATH
//! line from the rc files it is found in. Nothing else is ever touched:
//! no `~/.cargo` (a `cargo install` copy is cargo's to remove via
//! `cargo uninstall lagom`), and a folder holding `Cargo.toml` is
//! reported and skipped — a source checkout was not installed by the
//! release page and must not be deleted from inside it.
//!
//! The copy the user is running is usually the copy being removed (it is
//! the one on their PATH). The running executable file is attempted last
//! and reported if the OS refuses (Windows locks a running exe) — the
//! honest "one file left" ending, never a silent failure.
//!
//! `lagom uninstall` alone only *plans* (prints, deletes nothing) — the
//! same shape as the compiler's own teaching diagnostics: show what, say
//! why, act only when asked.

use std::path::{Path, PathBuf};

use super::args::{has_flag, CliError, CliResult};

/// Everything one plan step covers: where it is, whether it still exists,
/// and why it will (or will not) be touched.
struct PlanStep {
    label: &'static str,
    path: Option<PathBuf>,
    skip_reason: Option<String>,
}

impl PlanStep {
    fn for_folder(label: &'static str, path: PathBuf) -> PlanStep {
        let skip_reason = if !path.is_dir() {
            Some("not found — nothing to do (already uninstalled?)".to_string())
        } else if let Some(reason) = refuse_reason(&path) {
            Some(reason)
        } else {
            None
        };
        PlanStep {
            label,
            path: Some(path),
            skip_reason,
        }
    }

    /// The shell startup files that currently contain a PATH line putting a
    /// `lagom` folder on PATH. Missing files are simply absent.
    fn rc_files() -> Vec<PathBuf> {
        let Some(home) = home_dir() else {
            return Vec::new();
        };
        [".zshrc", ".bashrc", ".profile", ".config/fish/config.fish"]
            .iter()
            .map(|rc| home.join(rc))
            .filter(|rc| rc.exists())
            .filter(|rc| {
                std::fs::read_to_string(rc)
                    .map(|text| rc_lines_installing_lagom(&text).next().is_some())
                    .unwrap_or(false)
            })
            .collect()
    }
}

/// The home directory (rc files live there; no external crates).
fn home_dir() -> Option<PathBuf> {
    std::env::var_os("HOME")
        .or_else(|| std::env::var_os("USERPROFILE"))
        .filter(|h| !h.is_empty())
        .map(PathBuf::from)
}

/// Why a folder must not be deleted by this tool. Returns `None` for the
/// normal package layout — a `lagom` binary next to a bundled `lib/lagom`.
fn refuse_reason(path: &Path) -> Option<String> {
    if path.parent().is_none() || path == Path::new("/") {
        return Some(
            "refusing to remove a filesystem root — point me at the folder lagom was unpacked into"
                .to_string(),
        );
    }
    if path.join("Cargo.toml").exists() {
        return Some(
            "refusing: this folder holds Cargo.toml — a source checkout, not a release install \
             (delete it yourself when you are sure)"
                .to_string(),
        );
    }
    if path.join("CACHEDIR.TAG").exists()
        || path.parent().is_some_and(|p| p.join("CACHEDIR.TAG").exists())
        || path.join(".fingerprint").is_dir()
        || path.join("incremental").is_dir()
        || path.join(".cargo-lock").exists()
    {
        return Some(
            "refusing: this is a cargo build cache (a target directory), not an install — \
             nothing was unpacked here; `cargo clean` removes it"
                .to_string(),
        );
    }
    if path.join("cargo.exe").exists() || path.join("cargo").exists() {
        return Some(
            "refusing: this folder looks like the cargo/rust toolchain itself — \
             a `cargo install` copy of lagom is removed with `cargo uninstall lagom`, never by hand"
                .to_string(),
        );
    }
    None
}

/// The lines of `text` that put a `lagom` folder on PATH (the exact line the
/// install page appends, or a hand-written variant pointing at `lagom`).
fn rc_lines_installing_lagom(text: &str) -> impl Iterator<Item = usize> + '_ {
    text.lines()
        .enumerate()
        .filter(|(_, line)| {
            let lower = line.to_lowercase();
            lower.contains("lagom") && lower.contains("path")
        })
        .map(|(i, _)| i)
}

/// Delete `target` recursively. A file in `protect` (the running executable)
/// is attempted and, if the OS refuses (a running exe on Windows), recorded
/// as a leftover instead of failing the whole removal.
/// Returns (files removed, files left behind).
fn remove_tree(target: &Path, protect: &[PathBuf]) -> Result<(usize, Vec<PathBuf>), String> {
    let mut removed = 0usize;
    let mut leftovers: Vec<PathBuf> = Vec::new();
    let entries = match std::fs::read_dir(target) {
        Ok(e) => e,
        Err(e) => return Err(format!("{}: {e}", target.display())),
    };
    for entry in entries.flatten() {
        let p = entry.path();
        if p.is_dir() {
            let (n, rest) = remove_tree(&p, protect)?;
            removed += n;
            let fully_removed = rest.is_empty();
            leftovers.extend(rest);
            if fully_removed {
                let _ = std::fs::remove_dir(&p);
            }
        } else if protect.iter().any(|q| p == *q) {
            if std::fs::remove_file(&p).is_ok() {
                removed += 1;
            } else {
                leftovers.push(p);
            }
        } else if let Err(e) = std::fs::remove_file(&p) {
            return Err(format!("{}: {e}", p.display()));
        } else {
            removed += 1;
        }
    }
    // The folder itself goes too when everything inside it did (ignored
    // when something remained — e.g. a protected running exe on Windows).
    let _ = std::fs::remove_dir(target);
    Ok((removed, leftovers))
}

/// `lagom uninstall [--rc] [--yes]`
///
/// No flags: print the plan, touch nothing. `--yes`: execute it —
/// remove the install folder(s) and, with `--rc`, strip the PATH lines
/// from the shell rc files that mention them.
pub fn cmd_uninstall(rest: &[String]) -> CliResult {
    let with_rc = has_flag(rest, "--rc");
    let apply = has_flag(rest, "--yes");

    // Candidates: the folder the running binary comes from (the one a PATH
    // install actually uses), then the two conventional unpack spots from
    // the release page's own examples.
    let mut steps: Vec<PlanStep> = Vec::new();
    if let Some(dir) = std::env::current_exe()
        .ok()
        .and_then(|p| p.parent().map(Path::to_path_buf))
    {
        steps.push(PlanStep::for_folder("the copy lagom is running from", dir));
    }
    for (label, dir) in [
        (
            "~/lagom (the release page's example spot)",
            home_dir().map(|h| h.join("lagom")),
        ),
        (
            "~/Downloads/lagom",
            home_dir().map(|h| h.join("Downloads").join("lagom")),
        ),
    ] {
        let Some(dir) = dir else { continue };
        // Skip candidates already covered by the running copy's folder.
        if steps.iter().any(|s| s.path.as_ref() == Some(&dir)) {
            continue;
        }
        if dir.is_dir() {
            steps.push(PlanStep::for_folder(label, dir));
        }
    }

    let rc_files = if with_rc { PlanStep::rc_files() } else { Vec::new() };
    let nothing_found = steps.iter().all(|s| s.skip_reason.is_some()) && rc_files.is_empty();

    println!("uninstall plan{}", if with_rc { " (--rc)" } else { "" });
    for step in &steps {
        match (&step.path, &step.skip_reason) {
            (_, Some(reason)) => println!("  {}: skipped — {reason}", step.label),
            (Some(path), None) => println!(
                "  {}: delete {} (the binary and its bundled lib/lagom runtime)",
                step.label,
                path.display()
            ),
            (None, None) => {}
        }
    }
    if with_rc {
        if rc_files.is_empty() {
            println!("  shell PATH lines: none found — nothing to clean up");
        } else {
            for rc in &rc_files {
                println!(
                    "  shell PATH lines: remove the `…/lagom` PATH entry from {}",
                    rc.display()
                );
            }
            println!("  (open a new terminal afterwards for PATH to catch up)");
        }
    } else if !PlanStep::rc_files().is_empty() {
        // Only mention --rc when a PATH line actually exists somewhere.
        println!("  PATH lines in shell rc files can be cleaned too — rerun with --rc to include them");
    }

    if nothing_found {
        println!("\nnothing to uninstall — lagom is not installed here.");
        return Ok(());
    }

    if !apply {
        println!(
            "\nnothing was changed — this was the plan only.\nRun `lagom uninstall {}--yes` to do it.",
            if with_rc { "--rc " } else { "" }
        );
        return Ok(());
    }

    let running_exe = std::env::current_exe().ok();
    let mut removed_any = false;
    let mut leftovers: Vec<PathBuf> = Vec::new();
    for step in &steps {
        if step.skip_reason.is_some() {
            continue;
        }
        let Some(path) = &step.path else { continue };
        // Protect the running executable itself (see the module doc).
        let protect: Vec<PathBuf> = running_exe
            .iter()
            .filter(|exe| exe.starts_with(path))
            .cloned()
            .collect();
        let (files, left) = remove_tree(path, &protect).map_err(CliError::Message)?;
        println!("removed {files} file(s) under {}", path.display());
        leftovers.extend(left);
        removed_any = true;
    }
    for rc in &rc_files {
        let text = std::fs::read_to_string(rc)
            .map_err(|e| CliError::Message(format!("could not read {}: {e}", rc.display())))?;
        let drop: Vec<usize> = rc_lines_installing_lagom(&text).collect();
        if drop.is_empty() {
            continue;
        }
        let kept: Vec<&str> = text
            .lines()
            .enumerate()
            .filter(|(i, _)| !drop.contains(i))
            .map(|(_, l)| l)
            .collect();
        let mut out = kept.join("\n");
        if text.ends_with('\n') && !out.is_empty() {
            out.push('\n');
        }
        std::fs::write(rc, out)
            .map_err(|e| CliError::Message(format!("could not rewrite {}: {e}", rc.display())))?;
        println!("removed {} PATH line(s) from {}", drop.len(), rc.display());
        removed_any = true;
    }

    if !removed_any {
        return Err(CliError::Message(
            "nothing was removed — every plan step was skipped".to_string(),
        ));
    }

    if leftovers.is_empty() {
        println!("\nlagom is uninstalled. Reopen your terminal; `lagom` is gone.");
    } else {
        println!("\nalmost done — the OS would not delete these while this command runs:");
        for l in &leftovers {
            println!("  {}", l.display());
        }
        println!("Delete them (and the now-empty folder) once this command has exited.");
    }
    Ok(())
}
