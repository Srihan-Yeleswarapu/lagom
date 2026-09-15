//! Shared CLI infrastructure: the error type every command returns, the
//! source-file argument convention, flag reading, stdin capture, and the
//! one place a front-end failure becomes user output.

use std::path::{Path, PathBuf};

use lagom_driver::{render_diagnostics, FrontendError};

/// A command's failure, and how `main` turns it into an exit code.
pub enum CliError {
    /// A plain CLI-level problem (usage, missing files).
    Message(String),
    /// A compilation that stopped before code could run; the payload is
    /// the rendered diagnostics.
    Compile(String),
    /// A toolchain/environment failure (missing runtime, rustc, output
    /// path) — the user's setup, not a compiler bug.
    Tool(String),
    /// Propagate the child program's exit code.
    Exit(u8),
}

pub type CliResult = Result<(), CliError>;

/// The file a command operates on: the argument, or `main.lagom` in the
/// project directory the command is run from (33.1's `lagom new quiz &&
/// lagom run` flow).
pub fn source_path(rest: &[String]) -> Result<PathBuf, CliError> {
    // Flags (`--trace`, `--release`, `-o out`, …) never name the source.
    let mut args = rest.iter();
    let mut path = None;
    while let Some(a) = args.next() {
        if a == "-o" {
            let _ = args.next();
        } else if !a.starts_with('-') {
            path = Some(PathBuf::from(a));
            break;
        }
    }
    let path = path.unwrap_or_else(|| PathBuf::from("main.lagom"));
    if !path.exists() {
        return Err(CliError::Message(format!(
            "no such file: {} — pass a .lagom file, or run from a project directory (try `lagom new <name>`).",
            path.display()
        )));
    }
    Ok(path)
}

pub fn read_source(path: &Path) -> Result<String, CliError> {
    std::fs::read_to_string(path)
        .map_err(|e| CliError::Message(format!("reading {}: {e}", path.display())))
}

/// Render a front-end failure the student way (26.2), with the file named.
pub fn render_failure(path: &Path, src: &str, e: &FrontendError) -> CliError {
    match e {
        FrontendError::Diagnostics(diags) => {
            CliError::Compile(render_diagnostics(&path.display().to_string(), src, diags))
        }
        FrontendError::Internal { stage, message } => CliError::Message(format!(
            "internal compiler error in {stage}: {message}\nThis is a compiler bug, not your fault — please report it."
        )),
        FrontendError::Tool(message) => CliError::Tool(message.clone()),
    }
}

/// `--release` and friends: simple boolean flags.
pub fn has_flag(rest: &[String], name: &str) -> bool {
    rest.iter().any(|a| a == name)
}

/// Lines from stdin, for the interpreter backend (`--trace`, tests).
pub fn read_stdin_lines() -> Vec<String> {
    use std::io::BufRead;
    let mut out = Vec::new();
    for line in std::io::stdin().lock().lines() {
        match line {
            Ok(l) => out.push(l),
            Err(_) => break,
        }
    }
    out
}

/// Print an interpreter RunOutcome the student way: the message plus, on
/// unhandled failures, the LOM failure report (26.5.2 — the payoff).
pub fn report_outcome(outcome: lagom_interp::RunOutcome) -> CliResult {
    match outcome {
        lagom_interp::RunOutcome::Completed => Ok(()),
        lagom_interp::RunOutcome::Failed { message, report } => {
            eprintln!("{message}");
            if let Some(report) = report {
                eprintln!("\n{report}");
            }
            Err(CliError::Exit(1))
        }
        lagom_interp::RunOutcome::Panicked { message, report } => {
            eprintln!("{message}");
            if let Some(report) = report {
                eprintln!("\n{report}");
            }
            Err(CliError::Exit(101))
        }
    }
}
