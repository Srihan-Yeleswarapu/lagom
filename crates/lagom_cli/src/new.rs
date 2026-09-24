// Copyright (c) 2026 Srihan Yeleswarapu.
// Source available under the Lagom License (LICENSE.md).

//! `lagom new` — start a new Lagom project directory.

use std::path::Path;

use super::args::{CliError, CliResult};
use super::template::NEW_TEMPLATE;

pub fn cmd_new(rest: &[String]) -> CliResult {
    let Some(name) = rest.first() else {
        return Err(CliError::Message("usage: lagom new <name>".to_string()));
    };
    if name.contains('/') || name.contains('\\') || name.contains("..") {
        return Err(CliError::Message("project names are single directory names".to_string()));
    }
    let dir = Path::new(name);
    if dir.exists() {
        return Err(CliError::Message(format!("{name} already exists")));
    }
    std::fs::create_dir_all(dir).map_err(|e| CliError::Message(format!("{e}")))?;
    let main = dir.join("main.lagom");
    std::fs::write(&main, NEW_TEMPLATE).map_err(|e| CliError::Message(format!("{e}")))?;
    println!("created {name}/main.lagom — try:\n\n  cd {name}\n  lagom run");
    Ok(())
}
