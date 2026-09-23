//! The `lagom play` session engine (00 §26.4, doc 09 §18): incremental
//! evaluation over the interpreter backend — the one evaluator, reused.
//!
//! Design points, each pinned by a test:
//! - **One session, many statements.** The session accumulates the source
//!   accepted so far; every new statement is recompiled *with* the session's
//!   declarations (functions, structures, kinds, aliases) so later lines can
//!   call earlier functions — exactly how a student uses a REPL.
//! - **Teaching mode**: after each accepted statement the statement's new
//!   bindings are reported with their types (`score is a number = 5`), and a
//!   `say` statement's output is echoed. Types come from the MIR locals of
//!   the entry frame; values from the run — one source of truth.
//! - **Deterministic replay**: the session carries a fixed seed; re-running
//!   the accepted history is byte-identical (26.5's story, session-scoped).
//! - **Errors never kill the session**: a rejected statement renders as
//!   student diagnostics and the previous session state persists (the
//!   student edits and retries — the classic playground loop), and the
//!   `#history` view records the attempt as rejected, so the session's
//!   story — attempts included — is the one the student lived.
//! - **`#history` is the step/value view** (doc 09 §18): every submission
//!   with what it bound (`name is a T = value`), what it printed, and a
//!   marked rejection per failed attempt — rendered by the session engine
//!   with the same strings the live echo printed (one renderer).

use crate::pipeline::{frontend, render_diagnostics_in, FrontendError};
use crate::run_full;
use lagom_diagnostics::Verbosity;
use lagom_sema::Type;

/// One statement's result.
#[derive(Debug, Clone, PartialEq)]
pub enum StepOutcome {
    /// Accepted: the teaching echo (per-binding `name is a T = value`) plus
    /// any `say` output the statement produced, in order.
    Accepted { echo: Vec<String>, output: Vec<String> },
    /// Rejected: the rendered student diagnostics. The session is unchanged.
    Rejected { rendered: String },
}

/// One entry in the session's step/value history: what was submitted and
/// what came of it. Accepted steps keep the exact teaching strings the live
/// REPL printed (one renderer); rejected steps keep a one-line summary of
/// the rendered diagnostics, so a mistake is recognizable without reprinting
/// the whole page.
#[derive(Debug, Clone, PartialEq)]
pub struct HistoryStep {
    pub statement: String,
    pub outcome: HistoryOutcome,
}

/// What came of one submitted statement.
#[derive(Debug, Clone, PartialEq)]
pub enum HistoryOutcome {
    /// The teaching echo and the `say` output, verbatim from the live step.
    Accepted { echo: Vec<String>, output: Vec<String> },
    /// The first message line of the rendered diagnostics.
    Rejected { summary: String },
}

/// A `lagom play` session: the accepted source so far plus the seed.
pub struct Session {
    accepted: String,
    seed: u64,
    /// Every submit in order — accepted and rejected — for `#history`.
    history: Vec<HistoryStep>,
}

impl Session {
    /// A fresh session with an explicit seed (replay-deterministic).
    pub fn with_seed(seed: u64) -> Session {
        Session { accepted: String::new(), seed, history: Vec::new() }
    }

    /// The accepted source so far (the replayable history).
    pub fn accepted(&self) -> &str {
        &self.accepted
    }

    /// Submit one statement (or declaration, or indented block). On success
    /// the text joins the session and the outcome carries the teaching echo
    /// and output; on failure the session is unchanged and the rendered
    /// diagnostics come back.
    pub fn submit(&mut self, statement: &str) -> StepOutcome {
        let mut candidate = String::with_capacity(self.accepted.len() + statement.len() + 2);
        candidate.push_str(&self.accepted);
        candidate.push_str(statement);
        if !candidate.ends_with('\n') {
            candidate.push('\n');
        }
        // The BEFORE set comes from the accepted history only — the candidate
        // already contains the new statement, so diffing against it would
        // swallow the echo.
        let names_before = self.binding_names_hist();
        match run_full(&candidate, Vec::new(), Some(self.seed)) {
            Ok((host, outcome, interp)) => match outcome {
                lagom_interp::RunOutcome::Completed => {
                    // The teaching echo: the statement's NEW bindings (name
                    // is a T = value), in declaration order. `say` output is
                    // echoed separately, so a `make` line teaches its type
                    // and a `say` line shows its value.
                    let mut echo = Vec::new();
                    for (name, ty, printed) in interp.entry_scope() {
                        if !names_before.contains(name) {
                            echo.push(format!("{name} is a {} = {}", type_word(ty), printed));
                        }
                    }
                    let _ = host;
                    self.accepted.push_str(statement);
                    if !self.accepted.ends_with('\n') {
                        self.accepted.push('\n');
                    }
                    self.history.push(HistoryStep {
                        statement: statement.to_string(),
                        outcome: HistoryOutcome::Accepted {
                            echo: echo.clone(),
                            output: host.stdout.clone(),
                        },
                    });
                    StepOutcome::Accepted { echo, output: host.stdout }
                }
                other => {
                    let (message, report) = match other {
                        lagom_interp::RunOutcome::Failed { message, report } => (message, report),
                        lagom_interp::RunOutcome::Panicked { message, report } => (message, report),
                        lagom_interp::RunOutcome::Completed => unreachable!(),
                    };
                    let rendered = render_runtime_failure(&message, &report);
                    self.history.push(HistoryStep {
                        statement: statement.to_string(),
                        outcome: HistoryOutcome::Rejected { summary: first_line(&rendered) },
                    });
                    StepOutcome::Rejected { rendered }
                }
            },
            Err(e) => {
                let rendered = render_rejection(&candidate, &e);
                self.history.push(HistoryStep {
                    statement: statement.to_string(),
                    outcome: HistoryOutcome::Rejected { summary: first_line(&rendered) },
                });
                StepOutcome::Rejected { rendered }
            }
        }
    }

    /// The session as a step/value history (doc 09 §18): what was submitted,
    /// what it bound (`name is a T = value`, the teaching view), what it
    /// printed, and which submissions were rejected. One renderer — the
    /// accepted steps show the exact strings the live REPL echoed.
    pub fn history(&self) -> Vec<HistoryStep> {
        self.history.clone()
    }

    /// The rendered `#history` page: numbered steps over the session's life,
    /// rejected ones marked. Empty when nothing was submitted yet.
    pub fn render_history(&self) -> String {
        if self.history.is_empty() {
            return "(nothing yet)\n".to_string();
        }
        let mut out = String::new();
        for (i, step) in self.history.iter().enumerate() {
            let n = i + 1;
            match &step.outcome {
                HistoryOutcome::Accepted { echo, output } => {
                    out.push_str(&format!("{n:>3}. {}", indent_body(&step.statement, n)));
                    for l in output {
                        out.push_str(&format!("      | {l}\n"));
                    }
                    for l in echo {
                        out.push_str(&format!("      | {l}\n"));
                    }
                }
                HistoryOutcome::Rejected { summary } => {
                    out.push_str(&format!("{n:>3}. × {} — REJECTED: {summary}\n", step.statement.trim_end()));
                }
            }
        }
        out
    }

    /// Replay the accepted history from scratch with the session's seed:
    /// the output must be identical to the live run (determinism, 26.5).
    /// Returns the `say` output of the whole session, in order.
    pub fn replay(&self) -> Vec<String> {
        match run_full(&self.accepted, Vec::new(), Some(self.seed)) {
            Ok((host, lagom_interp::RunOutcome::Completed, _)) => host.stdout,
            _ => Vec::new(),
        }
    }

/// The named bindings the ACCEPTED history's entry frame declares — the
/// BEFORE set for the echo diff.
    fn binding_names_hist(&self) -> Vec<String> {
        let fe = match frontend(&self.accepted) {
            Ok(fe) => fe,
            Err(_) => return Vec::new(),
        };
        entry_names(&fe.mir)
    }
}

/// The most informative single line of a rendered diagnostics page — the
/// summary a history entry keeps (the full page was already shown live).
/// The `ERROR on line N` header is skipped: the message below it names the
/// cause.
fn first_line(rendered: &str) -> String {
    let lines: Vec<&str> = rendered.lines().map(str::trim).filter(|l| !l.is_empty()).collect();
    lines
        .iter()
        .find(|l| !l.starts_with("ERROR on line"))
        .or(lines.first())
        .map(|l| (*l).to_string())
        .unwrap_or_else(|| "error".to_string())
}

/// A multi-line statement indented under its history number.
fn indent_body(statement: &str, n: usize) -> String {
    let pad = " ".repeat(n.to_string().len() + 4);
    let mut lines = statement.lines();
    let first = lines.next().unwrap_or("").to_string();
    let mut out = format!("{first}\n");
    for l in lines {
        out.push_str(&format!("{pad}{l}\n"));
    }
    out
}

/// The entry function's named locals (no `%temp`).
fn entry_names(mir: &lagom_mir::MirProgram) -> Vec<String> {
    mir.items
        .iter()
        .filter_map(|i| match i {
            lagom_mir::MirItem::Main(f) if f.is_entry => Some(
                f.locals
                    .iter()
                    .filter(|l| !l.name.starts_with('%'))
                    .map(|l| l.name.clone())
                    .collect::<Vec<_>>(),
            ),
            _ => None,
        })
        .next()
        .unwrap_or_default()
}

/// The student word for a type (7.10's display) with its article stripped
/// when the echo already supplies one: `score is a number`, but
/// `xs is a list of number` — never the doubled `is a a list of …`.
fn type_word(t: &Type) -> String {
    let d = t.display();
    match d.strip_prefix("a ") {
        Some(rest) => rest.to_string(),
        None => d,
    }
}

fn render_rejection(candidate: &str, e: &FrontendError) -> String {
    match e {
        FrontendError::Diagnostics(diags) => {
            render_diagnostics_in("play.lagom", candidate, diags, Verbosity::Student)
        }
        FrontendError::Internal { stage, message } => format!(
            "internal compiler error in {stage}: {message}\nThis is a compiler bug, not your fault — please report it."
        ),
        FrontendError::Tool(message) => message.clone(),
    }
}

fn render_runtime_failure(message: &str, report: &Option<String>) -> String {
    let mut out = String::new();
    out.push_str(message);
    if let Some(report) = report {
        out.push_str("\n\n");
        out.push_str(report);
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn session_accumulates_and_echoes_types() {
        let mut s = Session::with_seed(7);
        let out = s.submit("make score equal to 5");
        match out {
            StepOutcome::Accepted { echo, output } => {
                assert!(echo.iter().any(|l| l.contains("score") && l.contains("number") && l.contains("5")), "{echo:?}");
                assert!(output.is_empty());
            }
            other => panic!("expected accepted, got {other:?}"),
        }
        // The second line can use the first: the session carries state.
        let out = s.submit("make doubled equal to score plus 1");
        assert!(matches!(out, StepOutcome::Accepted { .. }), "{out:?}");
        let out = s.submit("say doubled");
        match out {
            StepOutcome::Accepted { output, .. } => assert_eq!(output, vec!["6"]),
            other => panic!("expected accepted, got {other:?}"),
        }
    }

    #[test]
    fn declarations_persist_and_later_lines_call_them() {
        let mut s = Session::with_seed(7);
        assert!(matches!(
            s.submit("\nfunction double\n    takes number called n\n    gives back n times 2"),
            StepOutcome::Accepted { .. }
        ));
        match s.submit("say double 21") {
            StepOutcome::Accepted { output, echo } => {
                assert_eq!(output, vec!["42"]);
                assert!(echo.is_empty(), "a call line binds nothing: {echo:?}");
            }
            other => panic!("expected accepted, got {other:?}"),
        }
    }

    #[test]
    fn rejection_keeps_the_session_alive() {
        let mut s = Session::with_seed(7);
        assert!(matches!(s.submit("make score equal to 5"), StepOutcome::Accepted { .. }));
        // A bad line: rejected with student diagnostics, session unchanged.
        let bad = s.submit("say scoer");
        match bad {
            StepOutcome::Rejected { rendered } => {
                // Student mode: the lesson, not the code — the did-you-mean
                // names the in-scope binding from the session.
                assert!(rendered.contains("scoer"), "{rendered}");
                assert!(rendered.contains("did you mean `score`?"), "{rendered}");
            }
            other => panic!("expected rejected, got {other:?}"),
        }
        // The next good line still sees `score` — the bad line left no trace.
        match s.submit("say score") {
            StepOutcome::Accepted { output, .. } => assert_eq!(output, vec!["5"]),
            other => panic!("expected accepted, got {other:?}"),
        }
    }

    #[test]
    fn replay_is_deterministic() {
        let mut s = Session::with_seed(11);
        s.submit("make changing rolls equal to a list of nothing, nothing, nothing");
        s.submit("say rolls");
        // Replaying the accepted history from scratch gives the same output.
        assert_eq!(s.replay().len(), 1);
        let a = s.replay();
        let b = s.replay();
        assert_eq!(a, b);
    }

    #[test]
    fn runtime_failure_renders_the_lom_report() {
        let mut s = Session::with_seed(7);
        match s.submit("make xs equal to a list of 1, 2\nsay xs at 9") {
            StepOutcome::Rejected { rendered } => {
                // The runtime lesson rides along (the M2 report teaches).
                assert!(rendered.contains("at 9") || rendered.contains("index"), "{rendered}");
            }
            other => panic!("expected rejected, got {other:?}"),
        }
    }

    // ----- #history: the step/value view (doc 09 §18) -----

    /// The requested acceptance case: a make, a say, and a rejected
    /// statement. History must show bindings with types, the output, and
    /// the rejection — the session as the student lived it.
    #[test]
    fn history_shows_bindings_output_and_rejections() {
        let mut s = Session::with_seed(7);
        assert!(matches!(s.submit("make score equal to 5"), StepOutcome::Accepted { .. }));
        assert!(matches!(
            s.submit("say \"{score plus 1}\""),
            StepOutcome::Accepted { .. }
        ));
        assert!(matches!(s.submit("say scoer"), StepOutcome::Rejected { .. }));
        let page = s.render_history();
        let steps = s.history();
        assert_eq!(steps.len(), 3, "{page}");
        // Step 1: the binding, with its type — the teaching view, verbatim.
        assert!(page.contains("1. make score equal to 5"), "{page}");
        assert!(page.contains("| score is a number = 5"), "{page}");
        // Step 2: the output the statement produced.
        assert!(page.contains("2. say \"{score plus 1}\""), "{page}");
        assert!(page.contains("| 6"), "{page}");
        // Step 3: recognizable as rejected, with the cause named.
        assert!(page.contains("REJECTED"), "{page}");
        assert!(page.contains("I do not know what `scoer` is"), "{page}");
    }

    /// Accepted steps keep the exact strings the live echo printed — one
    /// renderer, so history cannot drift from the teaching view.
    #[test]
    fn history_echo_matches_the_live_teaching_strings() {
        let mut s = Session::with_seed(7);
        let StepOutcome::Accepted { echo, output } = s.submit("make score equal to 5") else {
            panic!("expected accepted");
        };
        let HistoryOutcome::Accepted { echo: h_echo, output: h_output } =
            &s.history()[0].outcome
        else {
            panic!("expected accepted history step");
        };
        assert_eq!(&echo, h_echo);
        assert_eq!(&output, h_output);
    }

    /// An empty session's history says so rather than printing nothing.
    #[test]
    fn empty_history_says_nothing_yet() {
        let s = Session::with_seed(7);
        assert_eq!(s.render_history(), "(nothing yet)\n");
        assert!(s.history().is_empty());
    }

    /// A rejected statement never enters the accepted source — replay and
    /// later statements behave exactly as before the attempt.
    #[test]
    fn rejected_attempt_leaves_accepted_source_untouched() {
        let mut s = Session::with_seed(7);
        s.submit("make score equal to 5");
        s.submit("say scoer");
        assert_eq!(s.accepted(), "make score equal to 5\n");
        assert_eq!(s.replay(), Vec::<String>::new());
    }
}
