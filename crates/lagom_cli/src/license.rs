// Copyright (c) 2026 Srihan Yeleswarapu.
// Source available under the Lagom License (LICENSE.md).

//! The `lagom license` command: the full license text, embedded in the
//! binary at compile time (`include_str!` — the build fails if LICENSE.md
//! is missing, so a copy cannot exist without its license). Every copy of
//! the compiler — official or not — carries the terms with it, and §2 of
//! the license forbids stripping this attribution out.

use super::args::CliResult;

const LICENSE_TEXT: &str = include_str!("../../../LICENSE.md");

/// `lagom license` — print the license exactly as it ships in the repo.
pub fn cmd_license() -> CliResult {
    print!("{LICENSE_TEXT}");
    Ok(())
}
