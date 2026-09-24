// Copyright (c) 2026 Srihan Yeleswarapu.
// Source available under the Lagom License (LICENSE.md).

//! `lagom fmt` — canonically format; print, or rewrite with `--write`.

use super::args::{has_flag, read_source, source_path, CliError, CliResult};
use super::fmt;

pub fn cmd_fmt(rest: &[String]) -> CliResult {
    let path = source_path(rest)?;
    let src = read_source(&path)?;
    let (program, diags) = lagom_parser::parse(&src);
    if !diags.is_empty() {
        return Err(CliError::Compile(lagom_driver::render_diagnostics(
            &path.display().to_string(),
            &src,
            &diags,
        )));
    }
    let formatted = fmt::format(&program);
    if has_flag(rest, "--write") {
        std::fs::write(&path, &formatted)
            .map_err(|e| CliError::Message(format!("writing {}: {e}", path.display())))?;
        println!("formatted {}", path.display());
    } else {
        print!("{formatted}");
    }
    Ok(())
}
