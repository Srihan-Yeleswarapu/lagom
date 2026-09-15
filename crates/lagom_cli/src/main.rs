//! The `lagom` CLI (M0) — the Go single-binary model (00 §26.1).
//!
//! `main.rs` is dispatch only: arg parsing, the command table, and the
//! exit-code mapping. Every command is a module sibling with one `cmd_*`
//! entry point (`help` owns the usage text and version line; `words` owns
//! its flag handling); shared plumbing (the error type, the source-file
//! convention, stdin, failure rendering) lives in `args`. `doc|play|add|
//! profile` are M1+ (doc 09/10); unknown commands say so honestly and point
//! at the command list.

mod args;
mod check;
mod doc_cmd;
mod explain;
mod explain_cmd;
mod fmt;
mod fmt_cmd;
mod help;
mod new;
mod packages;
mod play;
mod profile;
mod run;
mod template;
mod test;
mod words;

use std::process::ExitCode;

use args::CliError;
use help::usage;

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

fn dispatch(args: &[String]) -> Result<(), CliError> {
    let Some(cmd) = args.first() else {
        return Err(CliError::Message(usage()));
    };
    let rest = &args[1..];
    match cmd.as_str() {
        "run" => run::cmd_run(rest),
        "play" => play::cmd_play(rest),
        "doc" => doc_cmd::cmd_doc(rest),
        "add" => packages::cmd_add(rest),
        "remove" => packages::cmd_remove(rest),
        "profile" => profile::cmd_profile(rest),
        "build" => run::cmd_build(rest),
        "test" => test::cmd_test(rest),
        "check" => check::cmd_check(rest),
        "fmt" => fmt_cmd::cmd_fmt(rest),
        "new" => new::cmd_new(rest),
        "words" => words::cmd_words(rest),
        "explain" => explain_cmd::cmd_explain(rest),
        "version" | "--version" | "-V" => help::cmd_version(),
        "help" | "--help" | "-h" => help::cmd_help(),
        other => Err(CliError::Message(format!(
            "unknown command `{other}`.\n\n{}",
            usage()
        ))),
    }
}
