// Copyright (c) 2026 Srihan Yeleswarapu.
// Source available under the Lagom License (LICENSE.md).

//! Diagnostic core: spans, diagnostics, error codes, student-mode rendering.
//!
//! Spec anchor: 00 §21.4 (diagnostics pipeline), §26.2 (teaching diagnostics).
//! Every stage attaches spans from the first token it consumes — spans are
//! first-class from day one (implementation brief).

use std::fmt;

/// A byte range in a single source file.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Default)]
pub struct Span {
    pub start: usize,
    pub end: usize,
}

impl Span {
    pub fn new(start: usize, end: usize) -> Self {
        Span { start, end }
    }

    /// Smallest span covering both.
    pub fn to(self, other: Span) -> Span {
        Span {
            start: self.start.min(other.start),
            end: self.end.max(other.end),
        }
    }

    pub fn join(spans: impl IntoIterator<Item = Span>) -> Span {
        spans.into_iter().fold(Span::default(), |acc, s| acc.to(s))
    }
}

/// A diagnostic, with the five-part teaching shape available (what/where/why/fix/concept).
#[derive(Clone, Debug)]
pub struct Diagnostic {
    pub code: &'static str,
    pub severity: Severity,
    pub message: String,
    pub span: Span,
    /// The "why" — plain-words explanation of the rule behind the error.
    pub explanation: Option<String>,
    /// The "fix" — a concrete suggested rewrite, rendered as lagom source.
    pub fix: Option<String>,
    /// The M2 causal guarantee: *why the fix solves the original problem*
    /// (never rendered as a bare "this makes it compile" — when absent, the
    /// renderer falls back to the explanation, and student mode never
    /// invents a justification the checker cannot back).
    pub fix_why: Option<String>,
    /// The "concept" — the `lagom explain` code the error links to.
    pub concept: Option<&'static str>,
    /// Secondary spans with labels (e.g. "created here as text").
    pub labels: Vec<(Span, String)>,
    pub notes: Vec<String>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Severity {
    Error,
    Warning,
    Note,
}

impl fmt::Display for Severity {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Severity::Error => write!(f, "ERROR"),
            Severity::Warning => write!(f, "WARNING"),
            Severity::Note => write!(f, "NOTE"),
        }
    }
}

impl Diagnostic {
    pub fn error(code: &'static str, message: impl Into<String>, span: Span) -> Self {
        Diagnostic {
            code,
            severity: Severity::Error,
            message: message.into(),
            span,
            explanation: None,
            fix: None,
            fix_why: None,
            concept: None,
            labels: Vec::new(),
            notes: Vec::new(),
        }
    }

    pub fn warning(code: &'static str, message: impl Into<String>, span: Span) -> Self {
        Diagnostic {
            severity: Severity::Warning,
            ..Diagnostic::error(code, message, span)
        }
    }

    pub fn with_explanation(mut self, why: impl Into<String>) -> Self {
        self.explanation = Some(why.into());
        self
    }

    pub fn with_fix(mut self, fix: impl Into<String>) -> Self {
        self.fix = Some(fix.into());
        self
    }

    /// Attach the causal justification for the suggested fix (M2's rule:
    /// a suggestion ships only with the reason it solves the original
    /// problem, so the student can verify it — and so a fix-after-fix
    /// cycle is visible before it starts).
    pub fn with_fix_why(mut self, why: impl Into<String>) -> Self {
        self.fix_why = Some(why.into());
        self
    }

    pub fn with_concept(mut self, concept: &'static str) -> Self {
        self.concept = Some(concept);
        self
    }

    pub fn with_label(mut self, span: Span, label: impl Into<String>) -> Self {
        self.labels.push((span, label.into()));
        self
    }

    pub fn with_note(mut self, note: impl Into<String>) -> Self {
        self.notes.push(note.into());
        self
    }
}

/// A source file being diagnosed: holds the text so the renderer can quote it.
pub struct SourceFile {
    pub name: String,
    pub text: String,
    /// Byte offset of the start of each line, for span → line/col mapping.
    line_starts: Vec<usize>,
}

impl SourceFile {
    pub fn new(name: impl Into<String>, text: impl Into<String>) -> Self {
        let text = text.into();
        let mut line_starts = vec![0];
        for (i, b) in text.bytes().enumerate() {
            if b == b'\n' {
                line_starts.push(i + 1);
            }
        }
        SourceFile {
            name: name.into(),
            text,
            line_starts,
        }
    }

    /// 1-based line/column (column in characters, for humans).
    pub fn line_col(&self, offset: usize) -> (usize, usize) {
        let line = match self.line_starts.binary_search(&offset) {
            Ok(l) => l,
            Err(l) => l - 1,
        };
        let start = self.line_starts[line];
        // Clamp the offset to a char boundary (a span may end inside a
        // multi-byte character) and to the text length.
        let mut end = offset.min(self.text.len());
        while end > start && !self.text.is_char_boundary(end) {
            end -= 1;
        }
        let col = self.text[start..end].chars().count() + 1;
        (line + 1, col)
    }
}

/// Render a diagnostic in student mode: the five-part teaching format (00 §26.2).
pub fn render_student(file: &SourceFile, d: &Diagnostic) -> String {
    let mut out = String::new();
    let (line, _col) = file.line_col(d.span.start);
    let _ = fmt::Write::write_fmt(
        &mut out,
        format_args!("{} on line {}\n\n{}\n\n", d.severity, line, d.message),
    );
    quote_span(&mut out, file, d.span, true);
    for (span, label) in &d.labels {
        let (l, _) = file.line_col(span.start);
        if l == line {
            continue; // primary span already shown
        }
        out.push('\n');
        out.push_str(label);
        out.push_str(":\n");
        quote_span(&mut out, file, *span, false);
    }
    if let Some(why) = &d.explanation {
        out.push('\n');
        out.push_str(why);
        out.push('\n');
    }
    if let Some(fix) = &d.fix {
        out.push_str("\nTo fix, write:\n\n    ");
        out.push_str(fix);
        out.push_str("\n");
        // The fix-why footer (M2): every suggestion carries the reason it
        // addresses the root cause. Without it, a suggestion that merely
        // silences the error would look identical to one that repairs it.
        if let Some(why) = &d.fix_why {
            let _ = fmt::Write::write_fmt(&mut out, format_args!("\nThis fixes it because: {why}\n"));
        } else if let Some(why) = &d.explanation {
            // Fall back to the rule itself — still a causal statement, never
            // a bare "the compiler stops complaining" — but never a bare
            // restatement of the error line above: a tautology teaches
            // nothing, so no footer prints at all.
            let norm = |t: &str| -> String {
                t.chars()
                    .map(|c| if c.is_alphanumeric() { c } else { ' ' })
                    .collect::<String>()
                    .split_whitespace()
                    .collect::<Vec<_>>()
                    .join(" ")
                    .to_lowercase()
            };
            if norm(why) != norm(&d.message) {
                let _ =
                    fmt::Write::write_fmt(&mut out, format_args!("\nThis fixes it because: {why}\n"));
            }
        }
    }
    if let Some(concept) = &d.concept {
        let _ = fmt::Write::write_fmt(
            &mut out,
            format_args!("\nLearn more: run `lagom explain {}`\n", concept),
        );
    }
    for note in &d.notes {
        let _ = fmt::Write::write_fmt(&mut out, format_args!("\nnote: {}\n", note));
    }
    out
}

/// Quote the source line(s) for a span with a caret marker.
fn quote_span(out: &mut String, file: &SourceFile, span: Span, primary: bool) {
    let (line, col) = file.line_col(span.start);
    let (end_line, _) = file.line_col(span.end.saturating_sub(1).max(span.start));
    for l in line..=end_line.max(line) {
        let start = file.line_starts[l - 1];
        let end = file
            .line_starts
            .get(l)
            .copied()
            .unwrap_or(file.text.len());
        let text = file.text[start..end].trim_end_matches(['\n', '\r']);
        let _ = fmt::Write::write_fmt(out, format_args!("    {}\n", text));
        if primary && l == line {
            let col_chars = text
                .chars()
                .take(col - 1)
                .map(|c| if c == '\t' { '\t' } else { ' ' })
                .count();
            let width = span_len_chars(file, span, start);
            let _ = fmt::Write::write_fmt(
                out,
                format_args!("    {}{}\n", " ".repeat(col_chars), "^".repeat(width.max(1))),
            );
        }
    }
}

fn span_len_chars(file: &SourceFile, span: Span, line_start: usize) -> usize {
    let s = span.start.max(line_start);
    let e = span.end.min(file.text.len());
    // Spans can end inside a multi-byte character (a diagnostic may cover a
    // whole line); clamp both ends to char boundaries before slicing.
    let mut end = e;
    while end > s && !file.text.is_char_boundary(end) {
        end -= 1;
    }
    let mut start = s;
    while start < end && !file.text.is_char_boundary(start) {
        start += 1;
    }
    file.text[start..end].chars().count().clamp(1, 40)
}

/// Render in expert mode: terse, code-first (00 §26.2).
pub fn render_expert(file: &SourceFile, d: &Diagnostic) -> String {
    let (line, col) = file.line_col(d.span.start);
    format!(
        "{}: {} at {}:{}:{}\n",
        d.code, d.message, file.name, line, col
    )
}

/// A diagnostic verbosity mode (00 §26.2: `student`/`normal`/`expert`).
/// Student mode is the default for new projects; the mode is per-invocation
/// via `--student`/`--normal`/`--expert` and per-project via `Lagom.toml`.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
pub enum Verbosity {
    /// The full five-part teaching format (what/where/why/fix/concept).
    #[default]
    Student,
    /// The teaching format's what/where — no fix suggestion or concept link.
    Normal,
    /// Terse, code-first: `E0003: message at file:line:col`.
    Expert,
}

impl Verbosity {
    /// Parse a `--diagnostics` flag value.
    pub fn parse(name: &str) -> Option<Verbosity> {
        match name {
            "student" => Some(Verbosity::Student),
            "normal" => Some(Verbosity::Normal),
            "expert" => Some(Verbosity::Expert),
            _ => None,
        }
    }

    /// Render one diagnostic in this mode.
    pub fn render(self, file: &SourceFile, d: &Diagnostic) -> String {
        match self {
            Verbosity::Student => render_student(file, d),
            Verbosity::Normal => render_normal(file, d),
            Verbosity::Expert => render_expert(file, d),
        }
    }
}

/// Render in normal mode: the teaching format's what/where without the
/// fix/concept footers. doc 09 names the mode but leaves the exact shape
/// open (its open-questions list defers the default-mode policy to M1 with
/// user data); this is the smallest honest reading: same teaching prose,
/// minus the two footers a learner still needs (see docs/14 G-30).
pub fn render_normal(file: &SourceFile, d: &Diagnostic) -> String {
    let mut out = String::new();
    let (line, _col) = file.line_col(d.span.start);
    let _ = fmt::Write::write_fmt(
        &mut out,
        format_args!("{} on line {}\n\n{}\n\n", d.severity, line, d.message),
    );
    quote_span(&mut out, file, d.span, true);
    for (span, label) in &d.labels {
        let (l, _) = file.line_col(span.start);
        if l == line {
            continue;
        }
        out.push('\n');
        out.push_str(label);
        out.push_str(":\n");
        quote_span(&mut out, file, *span, false);
    }
    if let Some(why) = &d.explanation {
        out.push('\n');
        out.push_str(why);
        out.push('\n');
    }
    for note in &d.notes {
        let _ = fmt::Write::write_fmt(&mut out, format_args!("\nnote: {}\n", note));
    }
    out
}

/// The diagnostic bundle for one compilation.
#[derive(Default)]
pub struct Diagnostics {
    pub items: Vec<Diagnostic>,
}

impl Diagnostics {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn push(&mut self, d: Diagnostic) {
        self.items.push(d);
    }

    pub fn has_errors(&self) -> bool {
        self.items
            .iter()
            .any(|d| d.severity == Severity::Error)
    }

    pub fn error_count(&self) -> usize {
        self.items
            .iter()
            .filter(|d| d.severity == Severity::Error)
            .count()
    }
}
