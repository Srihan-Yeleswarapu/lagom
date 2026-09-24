// Copyright (c) 2026 Srihan Yeleswarapu.
// Source available under the Lagom License (LICENSE.md).

//! `lagom check` — parse and check; show diagnostics.

use super::args::{read_source, render_failure, source_path, CliResult};

pub fn cmd_check(rest: &[String]) -> CliResult {
    let path = source_path(rest)?;
    let src = read_source(&path)?;
    match lagom_driver::frontend(&src) {
        Ok(_) => {
            println!("{} checks out.", path.display());
            Ok(())
        }
        Err(e) => Err(render_failure(&path, &src, &e)),
    }
}
