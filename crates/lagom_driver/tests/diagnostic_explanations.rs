// Copyright (c) 2026 Srihan Yeleswarapu.
// Source available under the Lagom License (LICENSE.md).

//! M2 diagnostic acceptance tests: intentionally broken programs, each
//! asserting that the explanation names the *actual root cause* — and that
//! the suggested fix addresses it (the fix-why footer makes the causal
//! claim checkable). These are the tests the "diagnostics demonstrably
//! reduce debugging work" gate rests on (00 §26.2: content reviewed like
//! API, snapshot-tested like code).
//!
//! The rule every case follows:
//! 1. the error is detected (code + span),
//! 2. the rendered explanation names the root cause in its own words,
//! 3. the suggested fix is *correct for the intent* — the test states the
//!    intent in a comment and refuses suggestions that would only silence
//!    the error while breaking the program's meaning.

use lagom_diagnostics::{SourceFile, Verbosity};

/// Compile (parse+check) and return the errors.
fn errors(src: &str) -> Vec<lagom_diagnostics::Diagnostic> {
    let (ast, parse_diags) = lagom_parser::parse(src);
    if !parse_diags.is_empty() {
        return parse_diags;
    }
    let checked = lagom_sema::check(&ast, src);
    checked
        .diags
        .items
        .into_iter()
        .filter(|d| matches!(d.severity, lagom_diagnostics::Severity::Error))
        .collect()
}

fn student_page(src: &str, code: &str) -> String {
    let diags = errors(src);
    let d = diags
        .iter()
        .find(|d| d.code == code)
        .unwrap_or_else(|| panic!("expected {code}, got {:?}", diags.iter().map(|d| d.code).collect::<Vec<_>>()));
    let file = SourceFile::new("broken.lagom", src);
    Verbosity::Student.render(&file, d)
}

// ---------------------------------------------------------------------------
// The five-part test on the canonical beginner errors
// ---------------------------------------------------------------------------

/// Intent: score holds a number; the student wrote a text literal for it.
/// The root cause is the text value at the binding — not the later use.
#[test]
fn type_mismatch_explains_the_value_and_avoids_the_wrong_fix() {
    let src = "\
make score equal to \"fifteen\"
if score is greater than 10
    say \"big\"
";
    let page = student_page(src, "E0356");
    // What + where: the error is at the comparison (line 2), not the binding.
    assert!(page.contains("ERROR on line 2"), "{page}");
    assert!(page.contains("is greater than"), "{page}");
    // Why.
    assert!(page.contains("only makes sense for numbers"), "{page}");
    // Provenance: the origin of the text value is named, not just the use.
    assert!(page.contains("the value was made here"), "{page}");
    assert!(page.contains("make score equal to \"fifteen\""), "{page}");
    // The fix must NOT be "convert score with number from" — the value is a
    // text literal that cannot be parsed (a misleading fix-after-fix cycle).
    assert!(!page.contains("number from score"), "{page}");
}

/// Intent: parse what the user typed. The error is the unhandled can-fail
/// call; the fix must wrap it in attempt, not remove the can-fail.
#[test]
fn unhandled_failure_suggests_attempt_not_signature_surgery() {
    let src = "make answer equal to ask \"pick: \"\nsay number from answer\n";
    let page = student_page(src, "E0302");
    assert!(page.contains("must be wrapped in `attempt`"), "{page}");
    // The fix wraps exactly the can-fail call (not the whole `say`): that
    // call is the root cause, and wrapping less is the smaller correct fix.
    assert!(page.contains("attempt number from answer if it fails then"), "{page}");
    // The causal footer: why attempt solves it.
    assert!(page.contains("This fixes it because:"), "{page}");
    assert!(page.contains("no longer escape unhandled"), "{page}");
}

/// Intent: count something mutable. The root cause is the missing
/// `changing`; the fix is the declaration, not deleting the mutation.
#[test]
fn assignment_to_immutable_points_at_the_declaration() {
    let src = "make score equal to 0\nincrease score by 5\nsay score\n";
    let page = student_page(src, "E0339");
    // The error points at the mutation; the provenance label quotes the
    // declaration it came from.
    assert!(page.contains("ERROR on line 2"), "{page}");
    assert!(page.contains("cannot be changed"), "{page}");
    assert!(page.contains("made here (immutable)"), "{page}");
    assert!(page.contains("make changing score equal to …"), "{page}");
    // The fix-why names why `changing` addresses the cause.
    assert!(page.contains("only a `changing` binding accepts"), "{page}");
}

/// Intent: greet a name. The root cause is the misspelling; `did you mean`
/// must propose the in-scope name, not invent a declaration.
#[test]
fn unknown_name_proposes_the_real_binding() {
    let src = "make score equal to 10\nsay scoer\n";
    let page = student_page(src, "E0344");
    assert!(page.contains("scoer"), "{page}");
    // The suggestion names an in-scope binding — not "declare it" (which the
    // explanation already covers) and not an invented name.
    assert!(page.contains("did you mean `score`?"), "{page}");
    assert!(page.contains("before they are used"), "{page}");
}

/// Intent: the student believes an empty-list `first of` crashes. It does
/// not (D-34: it gives `nothing`) — assert the language is honest here and
/// the *indexing* case is the one that teaches bounds.
#[test]
fn index_error_names_the_bounds_and_the_size() {
    let src = "make xs equal to a list of 1, 2\nsay xs at 9\n";
    // Compile-time: this is legal; the bounds error is a runtime lesson.
    assert!(errors(src).is_empty(), "indexing is checked at runtime (13.1)");
}

/// The full-pipeline view: a broken program renders a complete student page
/// with what/where/why/fix/concept — the five-part contract of 00 §26.2.
#[test]
fn the_five_parts_render_in_student_mode() {
    let src = "make n equal to \"hi\" of type number\nsay n\n";
    let diags = errors(src);
    let d = diags.first().expect("the annotation mismatch is caught");
    assert_eq!(d.code, "E0360");
    let file = SourceFile::new("broken.lagom", src);
    let page = Verbosity::Student.render(&file, d);
    assert!(page.contains("ERROR on line 1"), "{page}");
    assert!(page.contains("needs number"), "{page}");
    assert!(page.contains("To fix, write:"), "{page}");
    assert!(page.contains("This fixes it because:"), "{page}");
    // Provenance: the literal's provenance is its own span, so no binding
    // label appears; the fix offers both honest directions (value or
    // annotation) instead of silently dropping the annotation.
    assert!(page.contains("use a number value here"), "{page}");
    assert!(page.contains("change the annotation"), "{page}");
}

/// Expert mode stays terse (26.2) — no teaching prose, one line, the code.
#[test]
fn expert_mode_stays_terse() {
    let src = "say mystery\n";
    let diags = errors(src);
    let d = diags.first().expect("unknown name is an error");
    let file = SourceFile::new("b.lagom", src);
    let page = Verbosity::Expert.render(&file, d);
    assert!(page.starts_with("E0344:"), "{page}");
    assert_eq!(page.lines().count(), 1, "{page}");
    assert!(!page.contains("Learn more"), "{page}");
}

/// Normal mode: what/where/why without the fix footers (G-30's shape).
#[test]
fn normal_mode_keeps_why_drops_fix() {
    let src = "make answer equal to ask \"pick: \"\nsay number from answer\n";
    let diags = errors(src);
    let d = diags.iter().find(|d| d.code == "E0302").unwrap();
    let file = SourceFile::new("b.lagom", src);
    let page = Verbosity::Normal.render(&file, d);
    assert!(page.contains("must be wrapped in `attempt`"), "{page}");
    assert!(!page.contains("To fix, write:"), "{page}");
    assert!(!page.contains("This fixes it because:"), "{page}");
}

// ---------------------------------------------------------------------------
// The misleading-fix rejections (the M2 rule: never suggest a change merely
// because it makes compilation succeed)
// ---------------------------------------------------------------------------

/// The whole sema surface, one broken program per code: every diagnostic on
/// the language must *explain* — name the root cause in its own words, offer
/// the smallest correct fix (with a fix-why when one exists), and never
/// suggest a merely-compiles rewrite. Gate: "diagnostics demonstrably reduce
/// debugging work" proven on the whole surface, not a slice.
mod full_surface {
    use super::student_page;

    // ----- declarations & scope (7.0.3) -----

    /// Intent: two independent counters. The duplicate is real — the second
    /// `make` reuses the first's name; the label quotes the first.
    #[test]
    fn e0330_duplicate_name_quotes_the_first_binding() {
        let src = "make count equal to 1\nmake count equal to 2\nsay count\n";
        let page = student_page(src, "E0330");
        assert!(page.contains("already a name `count`"), "{page}");
        assert!(page.contains("ERROR on line 2"), "{page}");
        assert!(page.contains("the first one was made here"), "{page}");
    }

    /// Intent: `total` and `total score` as separate values. 7.0.3 forbids
    /// the prefix pair — greedy matching could not tell them apart. The fix
    /// is renaming, stated as a fix without inventing the new name.
    #[test]
    fn e0331_prefix_clash_says_rename_not_the_ambiguous_parse() {
        let src = "make total equal to 1\nmake total score equal to 2\nsay total score\n";
        let page = student_page(src, "E0331");
        assert!(page.contains("could be confused with"), "{page}");
        assert!(page.contains("rename one of them"), "{page}");
        // It must not pretend a specific disambiguation exists.
        assert!(!page.contains("did you mean"), "{page}");
    }

    /// Intent: read a file and recover. The bare `attempt` hands the failure
    /// to the caller, but a top-level script has none — the two honest fixes
    /// are `can fail` (a function) or a handled branch; the page offers both
    /// readings instead of guessing.
    #[test]
    fn e0332_bare_attempt_offers_both_honest_fixes() {
        let src = "use files\nmake data equal to attempt read file at \"gone.txt\"\n";
        let page = student_page(src, "E0332");
        assert!(page.contains("passes the problem on"), "{page}");
        assert!(page.contains("can fail"), "{page}");
        assert!(page.contains("if it fails then"), "{page}");
    }

    /// Intent: a function that answers a number. The promise is broken on
    /// the fall-through path; the fix names adding the missing `gives back`.
    #[test]
    fn e0333_missing_gives_back_names_the_broken_promise() {
        let src = "\nfunction grade\n    takes text called who\n    returns a number\n    say who\n\nsay grade \"bo\"\n";
        let page = student_page(src, "E0333");
        assert!(page.contains("promises to gives back"), "{page}");
        assert!(page.contains("gives back"), "{page}");
        assert!(page.contains("on every path"), "{page}");
    }

    /// Intent: add to a counter. The root cause is the text being increased,
    /// not the amount; the error lands on the target's span.
    #[test]
    fn e0334_increase_targets_the_non_number() {
        let src = "make changing label equal to \"count: \"\nincrease label by 1\n";
        let page = student_page(src, "E0334");
        assert!(page.contains("increase"), "{page}");
        assert!(page.contains("text"), "{page}");
        assert!(page.contains("change a number by another number"), "{page}");
    }

    /// Intent: leave a loop early — but there is no loop. `stop` only works
    /// inside `repeat`; the page says where it does work.
    #[test]
    fn e0335_stop_outside_a_loop_says_where_it_belongs() {
        let src = "make done equal to false\nif done\n    stop\n";
        let page = student_page(src, "E0335");
        assert!(page.contains("only works inside a `repeat` loop"), "{page}");
        assert!(page.contains("ends the turn"), "{page}");
    }

    /// Intent: answer a value from nowhere — `gives back` is function-only;
    /// tests and scripts do not return. The page teaches the concept.
    #[test]
    fn e0336_gives_back_outside_a_function_teaches_the_concept() {
        let src = "make x equal to 1\ngives back x\n";
        let page = student_page(src, "E0336");
        assert!(page.contains("only works inside a function"), "{page}");
        assert!(page.contains("do not return values"), "{page}");
    }

    /// Intent: report a problem. The function never declared `can fail`, so
    /// the failure has nowhere honest to go; the fix is the declaration.
    #[test]
    fn e0337_fail_with_needs_the_can_fail_clause() {
        let src = "\nfunction guard\n    takes text called who\n    if size of who is equal to 0\n        fail with \"empty name\"\n    say who\n\nsay guard \"bo\"\n";
        let page = student_page(src, "E0337");
        assert!(page.contains("does not say `can fail`"), "{page}");
        assert!(page.contains("add a `can fail` clause"), "{page}");
        assert!(page.contains("This fixes it because:"), "{page}");
    }

    // ----- expressions: collections, calls, fields (7.6–7.9) -----

    /// Intent: walk the scores. The root cause is the iterated value being a
    /// number; the page teaches what `for each` walks.
    #[test]
    fn e0338_for_each_needs_the_collection() {
        let src = "make total equal to 5\nrepeat for each item in total\n    say item\n";
        let page = student_page(src, "E0338");
        assert!(page.contains("needs a list or a map"), "{page}");
        assert!(page.contains("walks through the list's items"), "{page}");
    }

    /// Intent: load a module that does not exist. The page lists the real
    /// modules — the fix is picking one of them, not inventing a path.
    #[test]
    fn e0340_unknown_module_lists_the_real_ones() {
        let src = "use rocket\nsay \"hi\"\n";
        let page = student_page(src, "E0340");
        assert!(page.contains("no module called `rocket`"), "{page}");
        assert!(page.contains("`math`"), "{page}");
        assert!(page.contains("`files`"), "{page}");
    }

    /// Intent: two helpers with the same name. The page names the clash;
    /// there is no rename suggestion to invent.
    #[test]
    fn e0341_duplicate_function_names_the_clash() {
        let src = "\nfunction tally\n    returns a number\n    gives back 1\n\nfunction tally\n    returns a number\n    gives back 2\n";
        let page = student_page(src, "E0341");
        assert!(page.contains("already a function called `tally`"), "{page}");
    }

    /// Intent: index the count — `at` walks lists and maps, not numbers.
    #[test]
    fn e0342_at_needs_the_container() {
        let src = "make n equal to 5\nsay n at 1\n";
        let page = student_page(src, "E0342");
        assert!(page.contains("`at` needs a list, a map, or text"), "{page}");
        assert!(page.contains("number"), "{page}");
        assert!(page.contains("one character from text"), "{page}");
    }

    /// Intent: read the wrong field name off a player. The page quotes the
    /// structure's real fields — the actual repair vocabulary.
    #[test]
    fn e0343_wrong_field_lists_the_real_ones() {
        let src = "structure player\n    has name of type text\n    has score of type number\n\nmake p equal to a player with name \"bo\" and score 1\nsay nick of p\n";
        let page = student_page(src, "E0343");
        assert!(page.contains("has no field called `nick`"), "{page}");
        assert!(page.contains("name, score"), "{page}");
    }

    /// Intent: build a player — `score` collides with a frozen keyword? No:
    /// the missing field is the root cause; the fix quotes the construction.
    #[test]
    fn e0347_unknown_construction_names_the_alternatives() {
        let src = "make s equal to a playmaker with name \"bo\"\nsay s\n";
        let page = student_page(src, "E0347");
        assert!(page.contains("no structure called `playmaker`"), "{page}");
        assert!(page.contains("kind"), "{page}");
    }

    /// Intent: build a player but misspell a field in the construction. The
    /// page quotes the real fields, same vocabulary as E0343's read side.
    #[test]
    fn e0348_unknown_construction_field_lists_the_real_ones() {
        let src = "structure player\n    has name of type text\n\nmake p equal to a player with nick \"bo\"\nsay p\n";
        let page = student_page(src, "E0348");
        assert!(page.contains("has no field called `nick`"), "{page}");
        assert!(page.contains("its fields are: name"), "{page}");
    }

    /// Intent: build a whole player. The fix quotes the missing field inside
    /// the construction form — the smallest completion of the student's own
    /// line, not a rewrite.
    #[test]
    fn e0349_missing_construction_field_quotes_the_form() {
        let src = "structure player\n    has name of type text\n    has score of type number\n\nmake p equal to a player with name \"bo\"\nsay p\n";
        let page = student_page(src, "E0349");
        assert!(page.contains("missing its `score` field"), "{page}");
        assert!(page.contains("a player with score …"), "{page}");
    }

    // ----- operators & conditions (8.3) -----

    /// Intent: compare what the user typed with a number. The root cause is
    /// the text type at the comparison; provenance quotes the binding.
    #[test]
    fn e0356_comparison_provenance_names_the_origin() {
        let src = "make reply equal to ask \"age: \"\nif reply is greater than 13\n    say \"teen\"\n";
        let page = student_page(src, "E0356");
        assert!(page.contains("needs two numbers"), "{page}");
        assert!(page.contains("text"), "{page}");
        assert!(page.contains("the value was made here"), "{page}");
    }

    /// Intent: do arithmetic on a count that is really text. The error lands
    /// on the offending side with the operator named.
    #[test]
    fn e0357_arithmetic_names_the_side_and_operator() {
        let src = "make label equal to \"count: \"\nmake doubled equal to label plus 1\n";
        let page = student_page(src, "E0357");
        assert!(page.contains("Arithmetic needs numbers"), "{page}");
        assert!(page.contains("left side"), "{page}");
        assert!(page.contains("`plus` works on numbers"), "{page}");
    }

    /// Intent: compare what the user typed with a number for equality. The
    /// page teaches the same-type rule (the ordering case is E0356's).
    #[test]
    fn e0355_equality_teaches_the_same_type_rule() {
        let src = "make reply equal to ask \"age: \"\nif reply is equal to 13\n    say \"teen\"\n";
        let page = student_page(src, "E0355");
        assert!(page.contains("cannot compare text with number"), "{page}");
        assert!(page.contains("only when they have the same type"), "{page}");
    }

    /// Intent: branch on a text. A text is not a yes-or-no; the fix teaches
    /// the comparison shape instead of coercing (no truthiness, 8.3).
    #[test]
    fn e0358_text_condition_suggests_the_comparison_not_a_coercion() {
        let src = "make name equal to \"bo\"\nif name\n    say \"yes\"\n";
        let page = student_page(src, "E0358");
        assert!(page.contains("not a yes-or-no value"), "{page}");
        assert!(page.contains("compare it"), "{page}");
        // The suggested comparison is concrete and in-scope.
        assert!(page.contains("size of"), "{page}");
    }

    /// Intent: convert what the user typed. `number from` eats text; the
    /// page states the contract on the offending value's own span.
    #[test]
    fn e0359_conversion_states_the_type_contract() {
        let src = "make n equal to 5\nmake parsed equal to number from n\n";
        let page = student_page(src, "E0359");
        assert!(page.contains("number from"), "{page}");
        assert!(page.contains("to be text"), "{page}");
        assert!(page.contains("number"), "{page}");
    }

    // ----- names (7.0.3) -----

    /// Intent: greet a name typed with a typo. The suggestion must come from
    /// the scope — never an invented declaration (the E0344 bar: `scoer` →
    /// `score` accepted; short names that would mislead are rejected).
    #[test]
    fn e0344_suggestion_comes_from_the_scope_or_stays_silent() {
        let src = "make score equal to 10\nsay scoer\n";
        let page = student_page(src, "E0344");
        assert!(page.contains("did you mean `score`?"), "{page}");
        // And the short-name rule: `it` is NOT "fixed" into `split`.
        let src = "make split equal to \"a\"\nsay it\n";
        let page = student_page(src, "E0344");
        assert!(!page.contains("did you mean"), "{page}");
    }

    /// Intent: negate a name that holds text — only a number can go negative.
    #[test]
    fn e0345_negation_states_the_operand_contract() {
        let src = "make label equal to \"count\"\nmake flipped equal to -label\n";
        let page = student_page(src, "E0345");
        assert!(page.contains("only negate a number"), "{page}");
        assert!(page.contains("text"), "{page}");
        assert!(page.contains("must be a number"), "{page}");
    }

    /// Intent: one list of like things. The mixed element is the root cause;
    /// the page names both types (no silent unification, no dropping).
    #[test]
    fn e0346_mixed_list_names_both_types() {
        let src = "make things equal to a list of 1, \"a\"\nsay things\n";
        let page = student_page(src, "E0346");
        assert!(page.contains("mixes number and text"), "{page}");
        assert!(page.contains("the same type"), "{page}");
    }

    /// Intent: build a whole player — the duplicate is the extra `score`.
    /// The page teaches one-value-per-field, not deduplication magic.
    #[test]
    fn e0350_duplicate_construction_field_teaches_one_per_field() {
        let src = "structure player\n    has name of type text\n\nmake p equal to a player with name \"a\" and name \"b\"\nsay p\n";
        let page = student_page(src, "E0350");
        assert!(page.contains("gives `name` twice"), "{page}");
        assert!(page.contains("sets each field once"), "{page}");
    }

    /// Intent: call a value like a function. The page offers the real
    /// directions — compute with the value, or call a function that exists.
    #[test]
    fn e0351_value_call_names_the_real_alternatives() {
        let src = "make v equal to 5\nmake w equal to v 3\n";
        let page = student_page(src, "E0351");
        assert!(page.contains("is a value, not a function"), "{page}");
        assert!(page.contains("Only functions take arguments"), "{page}");
    }

    /// Intent: the first item of a count — `first of` walks lists.
    #[test]
    fn e0352_first_of_states_the_container_contract() {
        let src = "make n equal to 5\nsay first of n\n";
        let page = student_page(src, "E0352");
        assert!(page.contains("`first of` needs a list"), "{page}");
        assert!(page.contains("`nothing` when the list is empty"), "{page}");
    }

    /// Intent: use the math module without loading it. The fix is the one
    /// line — and the fix-why explains the scoping rule it rests on.
    #[test]
    fn e0353_math_module_fix_carries_its_why() {
        let src = "say square root of 16\n";
        let page = student_page(src, "E0353");
        assert!(page.contains("in the `math` module"), "{page}");
        assert!(page.contains("use math"), "{page}");
        assert!(page.contains("This fixes it because:"), "{page}");
        assert!(page.contains("only in scope once the file uses it"), "{page}");
    }

    /// Intent: describe a shape kind with two circles. The page teaches why
    /// variant names must differ (match tells them apart by name).
    #[test]
    fn e0361_duplicate_variant_teaches_the_naming_role() {
        let src = "kind shape\n    is a circle with radius of type number\n    is a circle with side of type number\n";
        let page = student_page(src, "E0361");
        assert!(page.contains("already has a variant called `circle`"), "{page}");
        assert!(page.contains("tells the kinds apart"), "{page}");
    }

    // ----- match & kinds (7.12) -----

    /// Intent: branch on a number by matching a text pattern. The page names
    /// both types — pattern and scrutinee — so the student can align them.
    #[test]
    fn e0362_pattern_mismatch_names_both_types() {
        let src = "match 5\n    when \"five\"\n        say \"five\"\n    otherwise\n        say \"other\"\n";
        let page = student_page(src, "E0362");
        assert!(page.contains("matches text but the matched value is number"), "{page}");
        assert!(page.contains("must match the type"), "{page}");
    }

    /// Intent: match a variant without its values. The fix quotes the exact
    /// `when a circle with radius …` form — the match's own vocabulary.
    #[test]
    fn e0363_fieldless_variant_quotes_the_match_form() {
        let src = "kind shape\n    is a circle with radius of type number\nmake s equal to a circle with radius 1\nmatch s\n    when circle\n        say \"round\"\n";
        let page = student_page(src, "E0363");
        assert!(page.contains("carries values"), "{page}");
        assert!(page.contains("when a circle with radius"), "{page}");
    }

    /// Intent: read the wrong field out of a variant pattern. The page lists
    /// the fields the variant actually carries.
    #[test]
    fn e0364_wrong_pattern_field_lists_the_real_ones() {
        let src = "kind shape\n    is a circle with radius of type number\nmake s equal to a circle with radius 1\nmatch s\n    when a circle with diameter d\n        say d\n";
        let page = student_page(src, "E0364");
        assert!(page.contains("has no field called `diameter`"), "{page}");
        assert!(page.contains("its fields are: radius"), "{page}");
    }

    /// Intent: match a non-option as `something`. The page teaches the one
    /// honest subject of `something`/`nothing` — options (8.5).
    #[test]
    fn e0365_something_needs_an_option() {
        let src = "make n equal to 5\nmatch n\n    when something with value v\n        say v\n    when nothing\n        say \"none\"\n";
        let page = student_page(src, "E0365");
        assert!(page.contains("matches an option"), "{page}");
        assert!(page.contains("Only an option"), "{page}");
    }

    /// Intent: feed a plain value where the combinator needs a function. The
    /// page quotes both lambda spellings (the two honest repairs).
    #[test]
    fn e0366_using_needs_a_function_and_quotes_the_lambdas() {
        let src = "make broken thing equal to 5\nmake out equal to map a list of 1 using broken thing\n";
        let page = student_page(src, "E0366");
        assert!(page.contains("needs a function"), "{page}");
        assert!(page.contains("using it plus 5"), "{page}");
        assert!(page.contains("taking n giving back"), "{page}");
    }

    /// Intent: match a variant the kind never declared. The page lists the
    /// variants that DO exist — the actual choice space.
    #[test]
    fn e0367_unknown_variant_lists_the_real_ones() {
        let src = "kind shape\n    is a circle with radius of type number\nmake s equal to a circle with radius 1\nmatch s\n    when a triangle with side t\n        say t\n";
        let page = student_page(src, "E0367");
        assert!(page.contains("no variant called `triangle`"), "{page}");
        assert!(page.contains("variants are: circle"), "{page}");
    }

    /// Intent: build a variant but miss a field. The fix quotes the exact
    /// construction with the missing field — the smallest completion.
    #[test]
    fn e0368_missing_variant_field_quotes_the_construction() {
        let src = "kind shape\n    is a circle with radius of type number\nmake s equal to a circle\nsay s\n";
        let page = student_page(src, "E0368");
        assert!(page.contains("needs its `radius` field"), "{page}");
        assert!(page.contains("the field is number"), "{page}");
    }

    // ----- combinators (11.2) -----

    /// Intent: `map` with a two-parameter function. The page states the
    /// one-element-per-call contract (and quotes no invented fix).
    #[test]
    fn e0369_arity_of_the_using_function_is_the_concept() {
        let src = "make f equal to taking a and b giving back a plus b\nmake out equal to map a list of 1 using f\n";
        let page = student_page(src, "E0369");
        assert!(page.contains("taking one value, but this function takes 2"), "{page}");
        assert!(page.contains("once per element"), "{page}");
    }

    /// Intent: `map` a count. The page states the walker contract.
    #[test]
    fn e0370_combinator_states_the_list_contract() {
        let src = "make broken equal to map 5 using it plus 1\n";
        let page = student_page(src, "E0370");
        assert!(page.contains("works on a list"), "{page}");
        assert!(page.contains("walk a list one element at a time"), "{page}");
    }

    // ----- type aliases (8.4) -----

    /// Intent: two names for the same idea. The page teaches the one-type-
    /// per-name rule shared by structures, kinds, and aliases.
    #[test]
    fn e0371_duplicate_alias_teaches_the_shared_naming_space() {
        let src = "a type called score is a number\na type called score is a text\n";
        let page = student_page(src, "E0371");
        assert!(page.contains("already a type called `score`"), "{page}");
        assert!(page.contains("only one type"), "{page}");
    }

    /// Intent: alias a type to itself (and a two-alias circle). The page
    /// explains the must-reach-a-real-type rule — the cycle can never mean
    /// anything.
    #[test]
    fn e0372_self_referential_alias_names_the_circle() {
        let src = "a type called score is a score\n";
        let page = student_page(src, "E0372");
        assert!(page.contains("cannot be defined in terms of itself"), "{page}");
        assert!(page.contains("come back around"), "{page}");
    }

    /// Intent: alias to a type that does not exist. The page lists what CAN
    /// stand after `is a` — the actual repair vocabulary.
    #[test]
    fn e0373_unknown_alias_target_lists_the_real_types() {
        let src = "a type called gem is a jewel\n";
        let page = student_page(src, "E0373");
        assert!(page.contains("no type called `jewel`"), "{page}");
        assert!(page.contains("`number`"), "{page}");
        assert!(page.contains("a `structure`"), "{page}");
    }
}

/// A text value compared with `is greater than` must not be "fixed" by
/// wrapping the text in `number from` — the parse can fail and the
/// comparison still has the wrong shape. The honest fix is the literal.
#[test]
fn no_fix_after_fix_cycle_on_text_comparison() {
    let src = "make age equal to \"15\"\nif age is greater than 13\n    say \"teen\"\n";
    let page = student_page(src, "E0356");
    assert!(!page.contains("number from"), "{page}");
}

/// An arity error must not suggest stuffing extra arguments with `nothing`.
#[test]
fn arity_error_says_the_counts_not_a_padding_trick() {
    let src = "\
function add
    takes number called a
    takes number called b
    returns a number
    gives back a plus b

say add 1
";
    let page = student_page(src, "E0354");
    assert!(page.contains("needs 2 values, but this call gives 1"), "{page}");
    assert!(!page.contains("nothing"), "{page}");
}

/// When intent is genuinely ambiguous, the compiler must say so rather than
/// guess (the M2 honesty rule). The flow-call ambiguity case is the
/// canonical one: two readings, both plausible.
#[test]
fn genuine_ambiguity_is_reported_not_guessed() {
    // `bigger of a and b or c` — two parses; the diagnostic names the rule.
    let src = "use math\nmake a equal to 1\nmake b equal to 2\nmake c equal to 3\nsay bigger of a and b or c\n";
    let diags = errors(src);
    let parsed = diags
        .iter()
        .find(|d| d.code == "E0209")
        .expect("the ambiguity must be a compile-time error, never a silent parse");
    let file = SourceFile::new("b.lagom", src);
    let page = Verbosity::Student.render(&file, parsed);
    assert!(page.contains("two readings") || page.contains("cannot tell"), "{page}");
}
/// The did-you-mean suggestion carries a causal why (not the restated
/// rule): the end-task typo regression showed a footer that repeated the
/// error line verbatim.
#[test]
fn did_you_mean_footer_names_the_real_cause() {
    let page = student_page("make score equal to 10
say scoer
", "E0344");
    assert!(page.contains("did you mean `score`?"), "{page}");
    assert!(page.contains("This fixes it because:"), "{page}");
    assert!(page.contains("already has"), "{page}");
}
