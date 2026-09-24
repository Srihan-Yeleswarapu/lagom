// Copyright (c) 2026 Srihan Yeleswarapu.
// Source available under the Lagom License (LICENSE.md).

//! `lagom test` — run the file's `test` blocks on the interpreter.

use super::args::{read_source, render_failure, source_path, CliError, CliResult};

pub fn cmd_test(rest: &[String]) -> CliResult {
    let path = source_path(rest)?;
    let src = read_source(&path)?;
    // Fail fast on compile errors before any test runs.
    let fe = lagom_driver::frontend(&src).map_err(|e| render_failure(&path, &src, &e))?;
    let _ = fe;
    let results = lagom_driver::run_tests_interpreted(&src, None);
    if results.is_empty() {
        println!("no tests found — write one with:\n\ntest \"the name of what you are checking\"\n    check that 1 plus 1 is equal to 2");
        return Ok(());
    }
    let mut failed = 0;
    for t in &results {
        if t.passed {
            println!("PASS  {}", t.name);
        } else {
            failed += 1;
            println!("FAIL  {}", t.name);
            if let Some(msg) = &t.message {
                println!("      {msg}");
            }
            // The LOM failure report (26.5): a failing test explains itself —
            // what the program was doing, where each value came from.
            if let Some(report) = &t.report {
                for line in report.lines() {
                    println!("      {line}");
                }
            }
        }
    }
    let passed = results.len() - failed;
    println!("\n{passed} passed, {failed} failed");
    if failed > 0 {
        Err(CliError::Message(String::new()))
    } else {
        Ok(())
    }
}
