// Copyright (c) 2026 Srihan Yeleswarapu.
// Source available under the Lagom License (LICENSE.md).

//! The interpreter back end (tests; LOM failure reports): run the program's
//! script body or its `test` blocks on the MIR, in dev mode — the LOM probes
//! are live and the failure report is produced on unhandled failure. Shares
//! the front end with the native backend, so semantics cannot drift.
//!
//! Every entry point is a projection of one execution core (`run_full`),
//! which front-ends, runs once, and hands back everything a caller could
//! want — host (output), outcome, and the instrumented interpreter (the LOM
//! ring). The projections exist so each caller reads as its intent; none of
//! them runs the program twice.

use crate::pipeline::{self, Frontend, FrontendError};
use lagom_mir;

/// The execution core: run an already-front-ended program **once** and
/// return the host (buffered output), the run outcome, and the interpreter
/// itself — which in dev mode owns the LOM event ring (26.5's probes) and
/// the entry frame's final named locals (26.4's REPL data). Every other
/// runner in this module is a projection of this one; none runs the program
/// twice, and callers that already front-ended (an entry check, a compile
/// timer) hand the `Frontend` here instead of paying for it again.
pub fn run_fe(
    fe: Frontend,
    src: &str,
    stdin: Vec<String>,
    seed: Option<u64>,
) -> (lagom_interp::Host, lagom_interp::RunOutcome, lagom_interp::Interp) {
    let mut fe = fe;
    let mut it = lagom_interp::Interp::dev(&mut fe.mir);
    it.host.stdin = stdin;
    if let Some(seed) = seed {
        it.host = lagom_interp::Host::with_seed(it.host.stdin.clone(), seed);
    }
    let outcome = it.run(&fe.mir, src);
    let host = std::mem::replace(&mut it.host, lagom_interp::Host::new(Vec::new()));
    (host, outcome, it)
}

/// One execution of the program from source: front end it, run it once on
/// the core. `Err` reports the front-end rejection for the caller to render.
pub fn run_full(
    src: &str,
    stdin: Vec<String>,
    seed: Option<u64>,
) -> Result<(lagom_interp::Host, lagom_interp::RunOutcome, lagom_interp::Interp), FrontendError>
{
    let fe = pipeline::frontend(src)?;
    Ok(run_fe(fe, src, stdin, seed))
}

/// Run the program's script body on the interpreter (the caller checked the
/// source already; a front-end error here is a caller bug).
pub fn run_interpreted(
    src: &str,
    stdin: Vec<String>,
    seed: Option<u64>,
) -> (lagom_interp::Host, lagom_interp::RunOutcome) {
    let Ok((host, outcome, _)) = run_full(src, stdin, seed) else {
        panic!("front-end error reaching the interpreter")
    };
    (host, outcome)
}

/// The trace projection of one execution: the LOM event timeline —
/// `--trace`'s step/value history (26.5: what the program did, in order,
/// with the values it bound) — together with the run's output and outcome,
/// from that one execution. Interpreter and native share the renderer
/// through `lagom_mir`.
pub fn run_traced_fe(
    fe: Frontend,
    src: &str,
    stdin: Vec<String>,
    seed: Option<u64>,
) -> (
    lagom_mir::TraceReport,
    lagom_interp::Host,
    lagom_interp::RunOutcome,
) {
    let (host, outcome, it) = run_fe(fe, src, stdin, seed);
    let ring = it.ring().expect("dev interpreter carries the LOM ring");
    let ended = match &outcome {
        lagom_interp::RunOutcome::Completed => lagom_mir::TraceEnd::Completed,
        lagom_interp::RunOutcome::Failed { message, .. } => {
            lagom_mir::TraceEnd::Failed(message.clone())
        }
        lagom_interp::RunOutcome::Panicked { message, .. } => {
            lagom_mir::TraceEnd::Panicked(message.clone())
        }
    };
    (lagom_mir::trace_report(ring, &ended, src), host, outcome)
}

/// The trace timeline from source — the projection any caller that wants
/// just the post-mortem view uses (the tests, and anything without an
/// entry/has_entry check to do first). `None` when the front end rejects
/// the source.
pub fn run_traced(src: &str, stdin: Vec<String>, seed: Option<u64>) -> Option<lagom_mir::TraceReport> {
    let fe = pipeline::frontend(src).ok()?;
    Some(run_traced_fe(fe, src, stdin, seed).0)
}

/// Whether the program needs stdin: the interpreter's host pre-reads stdin
/// (it cannot interact mid-run), so reading it eagerly blocks on a terminal
/// until EOF. The CLI therefore asks this before running with `--trace`.
///
/// Known limitation, accepted for M0: this is a source-text heuristic — the
/// word `ask` anywhere (a comment, a string literal) counts. The cost of a
/// false positive is only that piped/redirected input is consumed; a false
/// negative would hang an interactive run, so the heuristic errs visible.
pub fn needs_input(src: &str) -> bool {
    // The word `ask` (comments, strings, any word position) — matching the
    // documented heuristic. Not a substring test: `task` must not read
    // stdin (14.2's `start a task` made the substring form hang every task
    // program on the traced path).
    src.split(|c: char| !c.is_alphanumeric()).any(|w| w == "ask")
}

/// Per-function call counts from the LOM ring of one dev run — `lagom
/// profile`'s data (26.5's events, M2's teaching profiler). The counts come
/// from the run the caller just showed (pass the returned `Interp`), not
/// from a second execution. `None` when the front end rejects the source.
pub fn call_counts(
    it: &lagom_interp::Interp,
) -> Option<Vec<(String, u64)>> {
    let ring = it.ring()?;
    let mut counts: Vec<(String, u64)> = Vec::new();
    for event in ring.events() {
        if let lagom_mir::LomEvent::FunctionEntry { function, .. } = event {
            match counts.iter_mut().find(|(n, _)| n == function) {
                Some((_, c)) => *c += 1,
                None => counts.push((function.clone(), 1)),
            }
        }
    }
    Some(counts)
}

/// Run every `test` block (S-12) on the interpreter, in declaration order.
pub fn run_tests_interpreted(src: &str, seed: Option<u64>) -> Vec<lagom_interp::TestOutcome> {
    let Ok(mut fe) = pipeline::frontend(src) else {
        panic!("front-end error reaching the interpreter")
    };
    let mut it = lagom_interp::Interp::dev(&mut fe.mir);
    if let Some(seed) = seed {
        it.host = lagom_interp::Host::with_seed(Vec::new(), seed);
    }
    it.run_tests(&fe.mir, src)
}
