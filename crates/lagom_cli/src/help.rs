// Copyright (c) 2026 Srihan Yeleswarapu.
// Source available under the Lagom License (LICENSE.md).

//! The help and version arms (`lagom help`, `lagom version`, and the bare
//! invocation): the top-level usage text and the identity line (26.1 — the
//! package version plus the build profile, stamped from the crate itself,
//! no hand-maintained copy). A sibling like every other command module, so
//! the next command has an obvious home in `main.rs`'s dispatch.

use super::args::CliResult;

/// The CLI's identity line (26.1): the package version plus the build
/// profile, stamped from the crate itself — no hand-maintained copy.
pub const VERSION_LINE: &str = concat!(
    "lagom ",
    env!("CARGO_PKG_VERSION"),
    " (",
    env!("LAGOM_BUILD_PROFILE"),
    ")"
);

/// The top-level usage text: the command list every unknown/bare invocation
/// points at. This is the one copy; the doc and README reference it, they do
/// not duplicate it.
pub fn usage() -> String {
    "usage: lagom <command> [arguments]

commands:
  run [file] [--release]    compile and run a Lagom program (default: main.lagom)
  build [file] [-o out]     compile to a native executable
  test [file]               run this file's `test` blocks (on the interpreter)
  check [file]              parse and check; show diagnostics
  fmt [file] [--write]      print the canonically formatted source (or rewrite)
  new <name>                start a new Lagom project directory
  uninstall [--rc] [--yes]  remove an installed copy (plan only, unless --yes; --rc also cleans PATH lines in shell rc files)
  play [--seed N]           the REPL: run lines now, teach types as you go
  doc [file] [--check]      render ## doc comments to markdown (verify examples)
  add <package>             record a dependency (Lagom.toml + Lagom.lock)
  remove <package>          drop a dependency
  profile [file]            run with the teaching profiler (call counts)
  words                     print the reserved words
  explain <code>            the teaching write-up for an error code
  license                   print the license terms (the Lagom License)
"
    .to_string()
}

/// `lagom version` / `--version` / `-V`. The identity line plus the
/// copyright line: every invocation of every copy of the compiler —
/// official or redistributed — prints the Author's attribution, so no
/// matter where a binary ends up, the name travels with it.
pub fn cmd_version() -> CliResult {
    println!("{VERSION_LINE}");
    if env!("LAGOM_BUILD_COMMIT").is_empty() {
        println!("built from a local checkout (no commit stamped)");
    } else {
        println!(
            "built from commit {} (source: https://github.com/Srihan-Yeleswarapu/lagom)",
            env!("LAGOM_BUILD_COMMIT")
        );
    }
    println!(
        "Copyright (c) 2026 Srihan Yeleswarapu — source available under the Lagom License."
    );
    println!("run `lagom license` for the full terms.");
    Ok(())
}

/// `lagom help` / `--help` / `-h`.
pub fn cmd_help() -> CliResult {
    print!("{}", usage());
    print!("\nCopyright (c) 2026 Srihan Yeleswarapu — the Lagom License (`lagom license`).\n");
    Ok(())
}
