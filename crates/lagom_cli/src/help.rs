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
  play [--seed N]           the REPL: run lines now, teach types as you go
  doc [file] [--check]      render ## doc comments to markdown (verify examples)
  add <package>             record a dependency (Lagom.toml + Lagom.lock)
  remove <package>          drop a dependency
  profile [file]            run with the teaching profiler (call counts)
  words                     print the reserved words
  explain <code>            the teaching write-up for an error code
"
    .to_string()
}

/// `lagom version` / `--version` / `-V`.
pub fn cmd_version() -> CliResult {
    println!("{VERSION_LINE}");
    Ok(())
}

/// `lagom help` / `--help` / `-h`.
pub fn cmd_help() -> CliResult {
    print!("{}", usage());
    Ok(())
}
