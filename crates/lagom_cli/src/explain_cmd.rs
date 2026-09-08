//! `lagom explain` — the teaching write-up for an error code.

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
    match explain::lookup(code) {
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
