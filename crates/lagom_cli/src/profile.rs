//! `lagom profile` (doc 09 §6, M2 slice): run the program on the interpreter
//! with the LOM ring live and report where the calls went — per-function
//! call counts from the ring's function events, plus the compile-time
//! breakdown. A teaching profiler: exact counts, honest boundaries, no
//! sampling magic.

use std::path::Path;
use std::time::Instant;

use super::args::{
    has_flag, read_source, read_stdin_lines, render_failure, report_outcome, source_path,
    CliResult,
};

/// The user-facing form of a profiled function's internal name: a student's
/// functions print as themselves; a compiler-synthesized lambda names the
/// construct and where it sits, with the internal name kept after it for
/// cross-reference against `--trace`.
pub fn friendly_fn(name: &str) -> String {
    if let Some(owner) = name.strip_prefix("%lambda fn:") {
        return format!("a lambda inside `{owner}`");
    }
    // Script form: `%lambda <owner>s<seq>` (MIR's next_lambda_name bakes the
    // owner in and separates it with `s`).
    if let Some(rest) = name.strip_prefix("%lambda ") {
        let digits = rest.len() - rest.trim_end_matches(|c: char| c.is_ascii_digit()).len();
        if digits > 0 {
            if let Some(owner) = rest[..rest.len() - digits].strip_suffix('s') {
                if !owner.is_empty() {
                    return format!("a lambda in `{owner}`");
                }
            }
        }
    }
    name.to_string()
}

pub fn cmd_profile(rest: &[String]) -> CliResult {
    let path = source_path(rest)?;
    let src = read_source(&path)?;
    let stdin = if lagom_driver::needs_input(&src) && !has_flag(rest, "--no-stdin") {
        read_stdin_lines()
    } else {
        Vec::new()
    };
    // Compile timing: the shared front end (parse+check+lower+verify) —
    // the number 24.3's budget talks about.
    let t0 = Instant::now();
    let fe = match lagom_driver::frontend(&src) {
        Ok(fe) => fe,
        Err(e) => return Err(render_failure(&path, &src, &e)),
    };
    let compile_time = t0.elapsed();
    // One execution supplies the rest: the program output and the
    // per-function call counts from the LOM ring of that same run — counts
    // describe the run the student saw.
    let (host, outcome, interp) = lagom_driver::run_fe(fe, &src, stdin, None);
    // Program output first — profiling must not swallow the run itself.
    for line in &host.stdout {
        println!("{line}");
    }
    match &outcome {
        lagom_interp::RunOutcome::Completed => {}
        lagom_interp::RunOutcome::Failed { .. } | lagom_interp::RunOutcome::Panicked { .. } => {
            return report_outcome(outcome);
        }
    }
    let calls = lagom_driver::call_counts(&interp).unwrap_or_default();

    println!("\n---- profile of {} ----", Path::new(&path).display());
    println!("compile (parse+check+lower): {:.2?}", compile_time);
    if calls.is_empty() {
        println!("no function calls recorded (straight-line script).");
    } else {
        let total: u64 = calls.iter().map(|(_, c)| c).sum();
        println!("function calls ({total} total):");
        for (name, count) in calls.iter().take(15) {
            let pct = if total > 0 { (*count as f64 / total as f64) * 100.0 } else { 0.0 };
            println!("  {:>28}  {count:>8}  ({pct:.1}%)  [{name}]", friendly_fn(name));
        }
    }
    Ok(())
}
