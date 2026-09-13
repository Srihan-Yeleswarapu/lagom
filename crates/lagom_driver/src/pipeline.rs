//! The shared front end (doc 08's stage discipline): source → parse →
//! check → HIR → MIR → (verify). One module owns the pipeline so the stages
//! are composed exactly once, and every backend — native, interpreter,
//! tests — consumes the same output, so diagnostics and semantics cannot
//! drift.

use lagom_diagnostics::{Diagnostic, SourceFile, Verbosity};
use lagom_mir::MirProgram;

/// Everything the back ends consume, produced by the shared front end.
pub struct Frontend {
    pub mir: MirProgram,
    /// Whether the program has a script body (top-level statements) — the
    /// thing `lagom run` executes. `false` for test-only or library files.
    pub has_entry: bool,
    /// How many `test` blocks the program declares (0 when none).
    pub test_count: usize,
}

/// A compilation that stopped before code could run.
#[derive(Debug)]
pub enum FrontendError {
    /// Source-level errors (parse or check) — rendered for the user.
    Diagnostics(Vec<Diagnostic>),
    /// An internal invariant failure (verifier, lowering) — never the
    /// user's fault; reported honestly, with the stage named.
    Internal { stage: &'static str, message: String },
    /// A toolchain/environment failure (missing runtime rlib, rustc,
    /// unwritable output) — the user's setup, not a compiler bug.
    Tool(String),
}

/// Parse and check a program; stops at the first failing stage so the user
/// sees the errors of one stage at a time (doc 08's stage discipline).
pub fn frontend(src: &str) -> Result<Frontend, FrontendError> {
    let (program, parse_diags) = lagom_parser::parse(src);
    // The parser only emits errors, so any diagnostic blocks the pipeline.
    if !parse_diags.is_empty() {
        return Err(FrontendError::Diagnostics(parse_diags));
    }
    let checked = lagom_sema::check(&program, src);
    let diags = &checked.diags.items;
    if diags.iter().any(|d| is_error(d)) {
        // Keep only errors and the notes attached to them: warnings during a
        // failing compile are noise.
        let errors: Vec<Diagnostic> = diags.iter().filter(|d| is_error(d)).cloned().collect();
        return Err(FrontendError::Diagnostics(errors));
    }
    let hir = lagom_hir::lower(checked);
    let mir = lagom_mir::lower(hir);
    if let Err(message) = lagom_mir::verify(&mir) {
        return Err(FrontendError::Internal { stage: "MIR verify", message });
    }
    let has_entry = mir
        .items
        .iter()
        .any(|i| matches!(i, lagom_mir::MirItem::Main(f) if f.is_entry));
    let test_count = mir
        .items
        .iter()
        .filter(|i| matches!(i, lagom_mir::MirItem::Test(_)))
        .count();
    Ok(Frontend { mir, has_entry, test_count })
}

fn is_error(d: &Diagnostic) -> bool {
    matches!(d.severity, lagom_diagnostics::Severity::Error)
}

/// Render diagnostics the way the student CLI shows them (00 §26.2).
pub fn render_diagnostics(file_name: &str, src: &str, diags: &[Diagnostic]) -> String {
    render_diagnostics_in(file_name, src, diags, Verbosity::Student)
}

/// Render diagnostics in a chosen verbosity mode (00 §26.2, docs/14 G-23).
pub fn render_diagnostics_in(
    file_name: &str,
    src: &str,
    diags: &[Diagnostic],
    mode: Verbosity,
) -> String {
    let file = SourceFile::new(file_name, src);
    let mut out = String::new();
    for d in diags {
        out.push_str(&mode.render(&file, d));
        out.push('\n');
    }
    out
}
