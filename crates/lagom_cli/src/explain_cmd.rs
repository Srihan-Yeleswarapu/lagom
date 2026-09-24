// Copyright (c) 2026 Srihan Yeleswarapu.
// Source available under the Lagom License (LICENSE.md).

//! `lagom explain` — the teaching write-up for an error or lesson code.

use super::args::{CliError, CliResult};
use super::explain;

pub fn cmd_explain(rest: &[String]) -> CliResult {
    let Some(code) = rest.first() else {
        println!(
            "usage: lagom explain <code> — every diagnostic names its code, e.g. `lagom explain E0344`.\n\nAll codes:\n{}",
            explain::code_index()
        );
        return Ok(());
    };
    // Runtime lessons (R00n) and compiler codes (E0nnn) share one command:
    // the LOM failure report names both kinds (26.5's answer 8).
    let writeup = explain::lookup(code)
        .or_else(|| explain::runtime_lesson(code));
    match writeup {
        Some(writeup) => print!("{writeup}"),
        None => {
            return Err(CliError::Message(format!(
                "no write-up for `{code}` yet.\n\nKnown codes:\n{}",
                explain::code_index()
            )))
        }
    }
    Ok(())
}
