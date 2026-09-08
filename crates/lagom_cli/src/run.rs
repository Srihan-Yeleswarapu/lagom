//! `lagom run` and `lagom build`: the two native-backend commands. They
//! share one helper for the source → (stem, dev) → executable plumbing.

use std::path::PathBuf;

use super::args::{
    has_flag, read_source, read_stdin_lines, render_failure, report_outcome, source_path, CliError,
    CliResult,
};

/// Everything both commands need before touching the backend: the source,
/// its stem, the dev/release choice, and whether debug info should be
/// emitted (`--pdb`).
struct BuildRequest {
    path: PathBuf,
    src: String,
    stem: String,
    dev: bool,
    /// `true` when the user passed `--pdb`.
    want_pdb: bool,
}

impl BuildRequest {
    fn parse(rest: &[String]) -> Result<BuildRequest, CliError> {
        let path = source_path(rest)?;
        let src = read_source(&path)?;
        let stem = path
            .file_stem()
            .and_then(|s| s.to_str())
            .unwrap_or("program")
            .to_string();
        let dev = !has_flag(rest, "--release");
        let want_pdb = has_flag(rest, "--pdb");
        Ok(BuildRequest {
            path,
            src,
            stem,
            dev,
            want_pdb,
        })
    }
}

/// `lagom run [file] [--release]` — compile and execute.
pub fn cmd_run(rest: &[String]) -> CliResult {
    // `--trace` runs on the interpreter backend (26.5.3, the teaching mode):
    // LOM probes are live and an unhandled failure prints the failure report.
    // Compile errors still render as student diagnostics, never a panic.
    if has_flag(rest, "--trace") {
        return cmd_run_trace(rest);
    }
    let req = BuildRequest::parse(rest)?;

    let build = lagom_driver::NativeBuild::new(&req.stem).map_err(CliError::Tool)?;
    let compiled = lagom_driver::compile_object(&req.src, req.dev, !req.want_pdb)
        .map_err(|e| render_failure(&req.path, &req.src, &e))?;
    // A file with no script body has nothing for `run` to execute; say so
    // instead of launching a binary that would only initialize the runtime.
    if !compiled.has_entry {
        nothing_to_run(&req.path, compiled.test_count);
        return Ok(());
    }
    let object = compiled.object;
    let exe_name = if cfg!(windows) { format!("{}.exe", req.stem) } else { req.stem };
    let exe = build.dir.join(exe_name);
    lagom_driver::link(&build.dir, &object, &exe)
        .map_err(|e| CliError::Message(format!("linking failed: {e}")))?;

    let status = std::process::Command::new(&exe)
        .status()
        .map_err(|e| CliError::Message(format!("running {}: {e}", exe.display())))?;
    match status.code() {
        Some(c) if c != 0 => Err(CliError::Exit(c as u8)),
        _ => Ok(()),
    }
}

/// The `--trace` path: the interpreter, with stdin only when the program
/// asks for it (the driver owns that policy).
fn cmd_run_trace(rest: &[String]) -> CliResult {
    let path = source_path(rest)?;
    let src = read_source(&path)?;
    let fe = lagom_driver::frontend(&src).map_err(|e| render_failure(&path, &src, &e))?;
    // Same honest report as the native path (the interpreter would run
    // nothing and exit silently).
    if !fe.has_entry {
        nothing_to_run(&path, fe.test_count);
        return Ok(());
    }
    let stdin = if lagom_driver::needs_input(&src) {
        read_stdin_lines()
    } else {
        Vec::new()
    };
    let (host, outcome) = lagom_driver::run_interpreted(&src, stdin, None);
    // The interpreter buffers output in the host; flush it in order.
    for line in &host.stdout {
        println!("{line}");
    }
    report_outcome(outcome)
}

/// `lagom build [file] [-o out] [--release]` — compile to an executable.
pub fn cmd_build(rest: &[String]) -> CliResult {
    let req = BuildRequest::parse(rest)?;

    // `-o out` wins; otherwise the binary lands next to the invocation dir.
    let out: PathBuf = match rest.iter().position(|a| a == "-o") {
        Some(i) => rest
            .get(i + 1)
            .map(PathBuf::from)
            .ok_or_else(|| CliError::Message("`-o` needs a path".to_string()))?,
        None => PathBuf::from(if cfg!(windows) {
            format!("{}.exe", req.stem)
        } else {
            req.stem
        }),
    };

    let exe =
        lagom_driver::build_native(&req.src, &out, req.dev, !req.want_pdb)
            .map_err(|e| render_failure(&req.path, &req.src, &e))?;
    println!("built {}", exe.display());
    Ok(())
}

/// The honest report for a program with no script body: `run` executes the
/// script; tests belong to `lagom test`. Informational, exit 0 — nothing
/// failed.
fn nothing_to_run(path: &Path, test_count: usize) {
    if test_count > 0 {
        println!(
            "nothing to run in {} — it only declares tests ({} of them); run them with `lagom test`",
            path.display(),
            test_count
        );
    } else {
        println!(
            "nothing to run in {} — it only declares functions and structures",
            path.display()
        );
    }
}
