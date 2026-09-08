//! The `lagom` CLI (M0) — the Go single-binary model (00 §26.1).
//!
//! `main.rs` is the face only: argument dispatch and the exit-code mapping.
//! Each command lives in its own module; shared plumbing (the error type,
//! the source-file convention, stdin, failure rendering) lives in `args`.
//! `doc|play|add|profile` are M1+ (doc 09/10); unknown commands say so
//! honestly and point at the command list.

mod args;
mod check;
mod explain;
mod explain_cmd;
mod fmt;
mod fmt_cmd;
mod new;
mod run;
mod template;
mod test;
mod words;

use std::process::ExitCode;

use args::CliError;

/// The CLI's identity line (26.1): the package version plus the build
/// profile, stamped from the crate itself — no hand-maintained copy.
const VERSION_LINE: &str = concat!(
    "lagom ",
    env!("CARGO_PKG_VERSION"),
    " (",
    env!("LAGOM_BUILD_PROFILE"),
    ")"
);

fn main() -> ExitCode {
    let args: Vec<String> = std::env::args().skip(1).collect();
    match dispatch(&args) {
        Ok(()) => ExitCode::SUCCESS,
        Err(CliError::Message(msg)) => {
            eprintln!("{msg}");
            ExitCode::from(1)
        }
        Err(CliError::Compile(msg)) => {
            eprint!("{msg}");
            ExitCode::from(1)
        }
        // The user's toolchain/environment, not a compiler bug — say so
        // plainly and exit 1.
        Err(CliError::Tool(msg)) => {
            eprintln!("{msg}");
            ExitCode::from(1)
        }
        // Native exit codes propagate (unhandled failure = 1, panic = 101).
        Err(CliError::Exit(code)) => ExitCode::from(code),
    }
}

fn usage() -> String {
    "usage: lagom <command> [arguments]

commands:
  run [file] [--release]    compile and run a Lagom program (default: main.lagom)
  build [file] [-o out]     compile to a native executable
  test [file]               run this file's `test` blocks (on the interpreter)
  check [file]              parse and check; show diagnostics
  fmt [file] [--write]      print the canonically formatted source (or rewrite)
  new <name>                start a new Lagom project directory
  words                     print the reserved words
  explain <code>            the teaching write-up for an error code
"
    .to_string()
}

fn dispatch(args: &[String]) -> Result<(), CliError> {
    let Some(cmd) = args.first() else {
        return Err(CliError::Message(usage()));
    };
    let rest = &args[1..];
    match cmd.as_str() {
        "run" => run::cmd_run(rest),
        "build" => run::cmd_build(rest),
        "test" => test::cmd_test(rest),
        "check" => check::cmd_check(rest),
        "fmt" => fmt_cmd::cmd_fmt(rest),
        "new" => new::cmd_new(rest),
        "words" => {
            println!("{}", words::RESERVED_LISTING);
            Ok(())
        }
        "explain" => explain_cmd::cmd_explain(rest),
        "version" | "--version" | "-V" => {
            println!("{VERSION_LINE}");
            Ok(())
        }
        "help" | "--help" | "-h" => {
            print!("{}", usage());
            Ok(())
        }
        other => Err(CliError::Message(format!(
            "unknown command `{other}`.\n\n{}",
            usage()
        ))),
    }
}
