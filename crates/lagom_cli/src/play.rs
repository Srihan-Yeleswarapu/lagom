//! `lagom play` — the REPL (00 §26.4, doc 09 §18): incremental evaluation on
//! the interpreter backend, teaching mode showing each new binding with its
//! type and value, and `#replay` to re-run the accepted history
//! deterministically (26.5's replay story, session-scoped).

use std::io::{BufRead, Write};

use super::args::{CliError, CliResult};
use lagom_driver::{Session, StepOutcome};

/// The banner: what the mode shows and the three session commands.
const BANNER: &str = "lagom play — type Lagom, one statement at a time.\n  Each line runs right away; new bindings print as `name is a <type> = <value>`.\n  Session commands: #replay (re-run everything), #history (step/value history), #exit";

pub fn cmd_play(rest: &[String]) -> CliResult {
    // A seed makes the session replayable: explicit `#seed N`, else a
    // clock-derived one (printed, so it can be re-pinned).
    let seed: u64 = rest
        .iter()
        .position(|a| a == "--seed")
        .and_then(|i| rest.get(i + 1))
        .and_then(|s| s.parse().ok())
        .unwrap_or_else(|| {
            let d = std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .map(|d| d.subsec_nanos() as u64 ^ d.as_secs())
                .unwrap_or(0x2545_F491_4F6C_DD1D);
            d | 1
        });
    println!("{BANNER}");
    println!("  (seed {seed} — pass `--seed {seed}` to replay this session)\n");
    let mut session = Session::with_seed(seed);
    let stdin = std::io::stdin();
    let mut lines = stdin.lock().lines();
    let mut stdout = std::io::stdout();
    loop {
        print!(">> ");
        let _ = stdout.flush();
        let Some(line) = lines.next() else {
            break; // EOF (piped input): end the session cleanly.
        };
        let line = line.unwrap_or_default();
        match line.trim() {
            "" => continue,
            "#exit" | "#quit" | ":q" => break,
            "#history" => {
                print!("{}", session.render_history());
                continue;
            }
            "#replay" => {
                let out = session.replay();
                println!("-- replay of the session (seed {seed}) --");
                for l in &out {
                    println!("{l}");
                }
                if out.is_empty() {
                    println!("(no output)");
                }
                continue;
            }
            cmd if cmd.starts_with('#') => {
                println!("unknown session command — try #replay, #history, or #exit");
                continue;
            }
            _ => {}
        }
        match session.submit(&line) {
            StepOutcome::Accepted { echo, output } => {
                for l in output {
                    println!("{l}");
                }
                for l in echo {
                    println!("  {l}");
                }
            }
            StepOutcome::Rejected { rendered } => {
                eprint!("{rendered}");
                // A rejected line must not poison the transcript.
                let _ = stdout.flush();
            }
        }
    }
    Ok(())
}

/// The REPL reads stdin itself (it is the interactive surface), so unlike
/// `run --trace` it never pre-reads the piped lines. Piped sessions work:
/// each line evaluates in turn and EOF ends the session.
pub fn _doc() {}

// Keep the error import honest: play reports through stdout/stderr and
// returns Ok; CliError stays imported for the signature's future needs.
#[allow(unused)]
fn _assert_error_used(e: CliError) -> CliError {
    e
}
