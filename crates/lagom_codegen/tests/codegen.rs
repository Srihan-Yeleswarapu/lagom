//! Code-generation tests (22.3, one-IR-many-backends): every M0 language
//! area must lower through LIR into a Cranelift object without error, in
//! both dev and release modes.
//!
//! Behavioral end-to-end coverage lives in the driver integration tests;
//! here we pin the backend contract itself.

use lagom_codegen::compile;
use lagom_lir::LirProgram;

/// Front-end + lowering: source → LIR, the codegen crate's input.
fn lower(src: &str, dev: bool) -> LirProgram {
    let (program, diags) = lagom_parser::parse(src);
    assert!(diags.is_empty(), "parse errors: {diags:?}");
    let checked = lagom_sema::check(&program, src);
    assert!(
        !checked.diags.has_errors(),
        "check errors: {:?}",
        checked
            .diags
            .items
            .iter()
            .map(|d| format!("{}: {}", d.code, d.message))
            .collect::<Vec<_>>()
    );
    let mir = lagom_mir::lower(lagom_hir::lower(checked));
    lagom_lir::set_source(src);
    lagom_lir::lower(&mir, dev)
}

fn assert_compiles(src: &str) {
    let lir = lower(src, true);
    let object = compile(&lir, src, true, true).expect("dev object");
    assert!(!object.is_empty());
    let lir = lower(src, false);
    let object = compile(&lir, src, false, true).expect("release object");
    assert!(!object.is_empty());
}

// ---------------------------------------------------------------------------
// Every M0 language area, area by area
// ---------------------------------------------------------------------------

#[test]
fn arithmetic_and_variables() {
    assert_compiles(
        "make changing n equal to 0\nset n to n plus 6 times 7\ndecrease n by 2\nincrease n by 1\nsay n\n",
    );
}

#[test]
fn conditions_and_boolean_operators() {
    assert_compiles(
        "if 1 is less than 2 and not false\n    say \"yes\"\notherwise if 3 is at least 4 or true\n    say \"maybe\"\notherwise\n    say \"no\"\n",
    );
}

#[test]
fn all_three_loops_with_stop_and_next() {
    assert_compiles(
        "repeat 3 times using i\n    if i is equal to 1\n        next\n    say i\n\nmake changing n equal to 3\nrepeat while n is greater than 0\n    decrease n by 1\n    if n is equal to 0\n        stop\n\nrepeat for each word in a list of \"a\", \"b\"\n    say word\n",
    );
}

#[test]
fn functions_with_parameters_and_returns() {
    assert_compiles(
        "function add\n    takes number called a\n    takes number called b\n    returns a number\n    gives back a plus b\n\nsay add 2 and 3\n",
    );
}

#[test]
fn lists_and_maps_and_pairs() {
    assert_compiles(
        "make xs equal to a list of 1, 2, 3\nmake m equal to a map from \"a\" to 1, \"b\" to 2\nmake p equal to a pair of 4 and 5\nsay size of xs\nsay m at \"b\"\n",
    );
}

#[test]
fn structs_with_construction_and_fields() {
    assert_compiles(
        "structure point\n    has x of type number\n    has y of type number\n\nmake p equal to a point with x 3 and y 4\nmake a equal to x of p\nmake b equal to y of p\nsay \"{a plus b}\"\n",
    );
}

#[test]
fn strings_with_interpolation_and_text_ops() {
    assert_compiles(
        "make name equal to \"lagom\"\nsay \"hello {name}, {2 plus 2}\"\nsay uppercase of name\nsay size of name\n",
    );
}

#[test]
fn can_fail_and_attempt_with_conversion() {
    assert_compiles(
        "function halve\n    takes number called n\n    returns a decimal\n    can fail\n    if n is equal to 0\n        fail with \"cannot halve zero\"\n    gives back n divided by 2\n\nattempt halve (4) if it fails then\n    say problem\notherwise\n    say result\n\nattempt number from \"nope\" if it fails then\n    say \"caught\"\n",
    );
}

#[test]
fn module_imports_and_builtins() {
    assert_compiles("use math for square root, floor\n\nsay square root of 81\nsay floor of 2.5\n");
}

// S-12/D-32: `test` blocks are interpreter-only at M0 — LIR/native codegen
// never sees them. `lagom build` on a program with tests must still compile
// (tests are simply not in the binary).
#[test]
fn test_blocks_are_excluded_from_native_code() {
    assert_compiles(
        "function double\n    takes number called n\n    returns a number\n    gives back n times 2\n\ntest \"doubling\"\n    check that double 2 is equal to 4\n",
    );
}

// A library (functions, no top-level statements) builds a binary whose
// `lagom_main` initializes the runtime and exits 0.
#[test]
fn library_without_script_builds() {
    assert_compiles(
        "function double\n    takes number called n\n    returns a number\n    gives back n times 2\n",
    );
}

#[test]
fn random_uses_the_kernel_module() {
    assert_compiles("make x equal to random from 1 to 100\nsay x\n");
}

#[test]
fn input_output_roundtrip() {
    assert_compiles("make reply equal to ask \"your name? \"\nsay \"hi {reply}\"\n");
}
