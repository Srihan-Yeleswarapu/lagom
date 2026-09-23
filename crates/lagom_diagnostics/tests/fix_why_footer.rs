//! The renderer's fix-why footer must be a cause, never a tautology: the
//! fallback (when a diagnostic carries no explicit fix-why) prints the rule
//! only when it is not a word-for-word restatement of the error line.
//! Every case here carries a fix — the footer prints only alongside one.

use lagom_diagnostics::{render_student, Diagnostic, SourceFile, Span};

/// E0337 is the regression case: it had no explicit fix-why and its
/// explanation restated the message, so the footer repeated the error.
#[test]
fn fallback_that_repeats_the_error_prints_no_footer() {
    let file = SourceFile::new("t.lagom", "function f\n    fail with \"x\"");
    let d = Diagnostic::error("E0337", "This function does not say `can fail`.", Span::new(0, 1))
        .with_explanation("This function does not say `can fail`.")
        .with_fix("add a `can fail` clause");
    let page = render_student(&file, &d);
    assert!(
        !page.contains("This fixes it because:"),
        "a restated rule is a tautology: {page}"
    );
}

/// A fallback that is genuinely causal still prints.
#[test]
fn fallback_that_is_not_a_restatement_still_prints() {
    let file = SourceFile::new("t.lagom", "say 1");
    let d = Diagnostic::error("E0356", "needs number, but got text.", Span::new(0, 1))
        .with_explanation("Numbers and text are different kinds of values; a number cannot hold text.")
        .with_fix("use a number here");
    let page = render_student(&file, &d);
    assert!(page.contains("This fixes it because:"), "{page}");
    assert!(page.contains("different kinds of values"), "{page}");
}

/// Punctuation and case differences still count as a restatement — the
/// tautology check normalizes words, so `wrong!` vs `wrong?` collapses.
#[test]
fn restatement_is_detected_after_normalization() {
    let file = SourceFile::new("t.lagom", "say 1");
    let d = Diagnostic::error("E9999", "the value is wrong!", Span::new(0, 1))
        .with_explanation("The value. Is wrong?")
        .with_fix("use the right value");
    let page = render_student(&file, &d);
    assert!(!page.contains("This fixes it because:"), "{page}");
}

/// An explicit fix-why always wins and always prints — the guard only
/// guards the fallback.
#[test]
fn explicit_fix_why_is_never_suppressed() {
    let file = SourceFile::new("t.lagom", "say 1");
    let d = Diagnostic::error("E9998", "the value is wrong!", Span::new(0, 1))
        .with_explanation("the value is wrong")
        .with_fix("the right fix")
        .with_fix_why("the fix repairs the value");
    let page = render_student(&file, &d);
    assert!(page.contains("This fixes it because: the fix repairs the value"), "{page}");
}
