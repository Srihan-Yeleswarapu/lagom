// Copyright (c) 2026 Srihan Yeleswarapu.
// Source available under the Lagom License (LICENSE.md).

//! Diagnostics tests (26.2, the differentiator): the five-part student
//! diagnostic — what, where (line + quoted source), why, fix, concept — and
//! the expert mode. Every diagnostic the compiler emits must render
//! source-aware: never a crash, never an unexplained failure.

use lagom_diagnostics::{render_expert, render_student, Diagnostic, SourceFile, Span, Verbosity};

fn rendered(src: &str, d: &Diagnostic) -> String {
    render_student(&SourceFile::new("prog.lagom", src), d)
}

// ---------------------------------------------------------------------------
// The five-part test (33.1.4): what / where / why / fix / concept
// ---------------------------------------------------------------------------

#[test]
fn student_render_names_what_where_why_fix_concept() {
    let src = "make x equal to 1\nsay mystery\n";
    let d = Diagnostic::error("E0344", "I do not know what `mystery` is. Make it first with `make`.", Span::new(21, 28))
        .with_explanation("Names must be made with `make` (or come from `takes`, `using`, or a loop) before they are used.")
        .with_fix("make mystery equal to … first")
        .with_concept("names");
    let out = rendered(src, &d);
    assert!(out.contains("ERROR on line 2"), "{out}");
    assert!(out.contains("say mystery"), "{out}");
    assert!(out.contains("^^^^^^"), "{out}");
    assert!(out.contains("Names must be made"), "{out}");
    assert!(out.contains("To fix, write:"), "{out}");
    assert!(out.contains("lagom explain names"), "{out}");
}

#[test]
fn caret_points_at_the_right_column() {
    let src = "make a equal to 1\nsay b\n";
    let d = Diagnostic::error("E0344", "unknown name", Span::new(22, 23));
    let out = rendered(src, &d);
    // The caret line aligns under `b` (4 spaces of indent + 4 for `say `).
    let caret_line = out.lines().find(|l| l.contains('^')).expect("caret line");
    assert_eq!(caret_line.trim_end().len(), caret_line.trim_end().trim_end_matches('^').len() + 1);
    assert!(out.contains("    say b"), "{out}");
}

#[test]
fn multiline_span_quotes_both_lines() {
    let src = "make a equal to\n1 plus\n2\n";
    let d = Diagnostic::error("E0202", "incomplete expression", Span::new(0, 22));
    let out = rendered(src, &d);
    assert!(out.contains("make a equal to"), "{out}");
    assert!(out.contains("1 plus"), "{out}");
}

#[test]
fn labels_notes_and_severity_render() {
    let src = "make x equal to 1\nset x to 2\n";
    let d = Diagnostic::warning("E0300", "rebinding an immutable name", Span::new(18, 27))
        .with_label(Span::new(0, 17), "made here (immutable)")
        .with_note("use `make changing` when a value must change");
    let out = rendered(src, &d);
    assert!(out.contains("WARNING on line 2"), "{out}");
    assert!(out.contains("made here (immutable)"), "{out}");
    assert!(out.contains("note: use `make changing`"), "{out}");
}

#[test]
fn expert_mode_is_compact_and_machine_shaped() {
    let src = "say mystery\n";
    let d = Diagnostic::error("E0344", "I do not know what `mystery` is.", Span::new(4, 11));
    let out = render_expert(&SourceFile::new("prog.lagom", src), &d);
    assert!(out.contains("E0344"), "{out}");
    assert!(out.contains("1:5"), "{out}");
    assert!(!out.contains("To fix, write:"), "{out}");
}

#[test]
fn line_col_is_one_based() {
    let f = SourceFile::new("t.lagom", "alpha\nbeta\n");
    assert_eq!(f.line_col(0), (1, 1));
    assert_eq!(f.line_col(6), (2, 1));
    assert_eq!(f.line_col(8), (2, 3));
}

// ---------------------------------------------------------------------------
// End-to-end through the front end: real diagnostics, never a crash
// ---------------------------------------------------------------------------

fn first_diagnostic(src: &str) -> Diagnostic {
    let (program, diags) = lagom_parser::parse(src);
    if !diags.is_empty() {
        return diags[0].clone();
    }
    let checked = lagom_sema::check(&program, src);
    assert!(
        !checked.diags.items.is_empty(),
        "expected a diagnostic for: {src}"
    );
    checked.diags.items[0].clone()
}

#[test]
fn parse_errors_carry_codes_spans_and_source() {
    // E0203: expected end of line — the R-4 divergence case is a *diagnostic*,
    // never a silent misparse (7.9).
    let d = first_diagnostic("say 1 then stop now");
    assert_eq!(d.code, "E0203");
    let out = rendered("say 1 then stop now", &d);
    assert!(out.contains("say 1 then stop now"), "{out}");
    assert!(out.contains("line 1"), "{out}");
}

#[test]
fn sema_errors_carry_codes_and_teaching_text() {
    // Unknown name (E0344): the what/where/why lesson.
    let d = first_diagnostic("say mystery");
    assert_eq!(d.code, "E0344");
    assert!(d.message.contains("mystery"), "{}", d.message);
    // Unhandled can-fail call (E0302): names the fix shapes. (E0333 is the
    // missing-`gives back` rule; the unhandled-failure rule has always been
    // E0302 — see sema's `unhandled_can_fail_call_is_compile_error`.)
    let d = first_diagnostic(
        "function f\n    takes number called n\n    returns a number\n    can fail\n    fail with \"no\"\n\nmake x equal to f 1",
    );
    assert_eq!(d.code, "E0302");
    assert!(d.explanation.is_some(), "E0302 explains why");
    // `fail with` without `can fail` (E0337).
    let d = first_diagnostic("function f\n    fail with \"no\"");
    assert_eq!(d.code, "E0337");
    // Wrong arity (E0354).
    let d = first_diagnostic(
        "function add\n    takes number called a\n    takes number called b\n    returns a number\n    gives back a plus b\n\nsay add 1",
    );
    assert_eq!(d.code, "E0354");
}

#[test]
fn every_diagnostic_renders_without_panicking() {
    // A battery of invalid programs: the compiler must answer each with a
    // rendered diagnostic (the 33.1.4 contract), not a crash.
    let bad = [
        "say 1 then stop now",
        "make equal to 3",
        "say (",
        "if 1\n    say 1",
        "repeat times using i\n    say i",
        "function f\n    takes number called n\n    gives back n\n\nsay f 1, 2, 3",
        "make x equal to a list of 1\nsay x at \"no\"",
        "structure s\n    has n of type unknown",
        "make x equal to mystery plus 1",
        "attempt 1 if it fails then\n    say problem",
    ];
    for src in bad {
        let (program, parse_diags) = lagom_parser::parse(src);
        if !parse_diags.is_empty() {
            for d in &parse_diags {
                let _ = rendered(src, d);
            }
            continue;
        }
        let checked = lagom_sema::check(&program, src);
        for d in &checked.diags.items {
            let _ = rendered(src, d);
            let _ = render_expert(&SourceFile::new("p.lagom", src), d);
        }
    }
}

#[test]
fn renderer_survives_multibyte_characters() {
    // The em-dash regression from the campaign: caret math must count
    // characters, not bytes, or the renderer panics on non-ASCII source.
    let src = "say \"café — the dash — here\"\nmake x equal to mystery\n";
    let d = Diagnostic::error("E0344", "unknown name", Span::new(44, 51));
    let out = rendered(src, &d); // must not panic
    assert!(out.contains("line 2"), "{out}");
}

#[test]
fn verbosity_modes_render_the_same_diagnostic_three_ways() {
    // docs/14 G-23: the three §26.2 modes are selectable, and the shapes are
    // distinct — student carries the fix/concept footers, normal drops them,
    // expert is the terse code-first one-liner.
    let src = "make x equal to mystery\n";
    let (program, parse_diags) = lagom_parser::parse(src);
    assert!(parse_diags.is_empty());
    let checked = lagom_sema::check(&program, src);
    let d = checked
        .diags
        .items
        .iter()
        .find(|d| d.severity == lagom_diagnostics::Severity::Error)
        .expect("unknown name must be an error");
    let file = SourceFile::new("p.lagom", src);

    let student = Verbosity::Student.render(&file, d);
    let normal = Verbosity::Normal.render(&file, d);
    let expert = Verbosity::Expert.render(&file, d);

    // Student: full teaching format (what/where/why, then fix and concept).
    assert!(student.contains("ERROR on line"), "{student}");
    assert!(student.contains("Make it first"), "{student}");
    // Normal: the teaching what/where/why, no fix footer, no concept link.
    assert!(normal.contains("ERROR on line"), "{normal}");
    assert!(!normal.contains("To fix, write:"), "{normal}");
    assert!(!normal.contains("Learn more"), "{normal}");
    // Expert: terse, code-first.
    assert!(expert.starts_with(d.code), "{expert}");
    assert!(expert.contains("p.lagom:1:"), "{expert}");

    // The parser helper resolves the flag spellings (and nothing else).
    assert_eq!(Verbosity::parse("student"), Some(Verbosity::Student));
    assert_eq!(Verbosity::parse("normal"), Some(Verbosity::Normal));
    assert_eq!(Verbosity::parse("expert"), Some(Verbosity::Expert));
    assert_eq!(Verbosity::parse("loud"), None);
    // Student is the default (§26.2: default for new projects).
    assert_eq!(Verbosity::default(), Verbosity::Student);
}
