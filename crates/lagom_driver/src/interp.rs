//! The interpreter back end (tests; LOM failure reports): run the program's
//! script body or its `test` blocks on the MIR, in dev mode — the LOM probes
//! are live and the failure report is produced on unhandled failure. Shares
//! the front end with the native backend, so semantics cannot drift.

use crate::pipeline::frontend;

/// Run the program's script body on the interpreter.
pub fn run_interpreted(
    src: &str,
    stdin: Vec<String>,
    seed: Option<u64>,
) -> (lagom_interp::Host, lagom_interp::RunOutcome) {
    let mut fe = expect_frontend(src);
    let mut it = lagom_interp::Interp::dev(&mut fe.mir);
    it.host.stdin = stdin;
    if let Some(seed) = seed {
        it.host = lagom_interp::Host::with_seed(it.host.stdin.clone(), seed);
    }
    let outcome = it.run(&fe.mir, src);
    (it.host, outcome)
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
    src.contains("ask")
}

/// Run every `test` block (S-12) on the interpreter, in declaration order.
pub fn run_tests_interpreted(src: &str, seed: Option<u64>) -> Vec<lagom_interp::TestOutcome> {
    let mut fe = expect_frontend(src);
    let mut it = lagom_interp::Interp::dev(&mut fe.mir);
    if let Some(seed) = seed {
        it.host = lagom_interp::Host::with_seed(Vec::new(), seed);
    }
    it.run_tests(&fe.mir, src)
}

/// The CLI checks the source before reaching these paths, so a front-end
/// error here is a caller bug, not a user-facing condition.
fn expect_frontend(src: &str) -> crate::pipeline::Frontend {
    match frontend(src) {
        Ok(fe) => fe,
        Err(e) => panic!("front-end error reaching the interpreter: {e:?}"),
    }
}
