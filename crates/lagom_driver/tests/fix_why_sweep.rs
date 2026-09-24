//! The fix-why sweep (M2's causal guarantee, whole-surface): every code the
//! compiler actually emits is triggered by an intentionally broken program
//! (probed against the real binary), rendered through the real student
//! renderer, and checked for the one property that can rot silently: a
//! fix-why footer that merely restates the error line — a tautology that
//! looks like a justification and teaches nothing. The renderer's fallback
//! guard is tested separately; THIS file pins the hand-written explicit
//! whys, so a future `with_fix_why("…the same sentence as the message…")`
//! fails CI here.
//!
//! Coverage (every code with a `with_fix` in lexer/parser/sema, plus every
//! code reachable from this broken-program corpus):
//! - lexer  E0101 E0102 E0105 E0108  (indentation / quote / unknown word)
//! - parser E0201–E0206, E0208–E0210 (structure/ambiguity/conversion forms)
//! - sema   E0208, E0302, E0330–E0394 minus E0376 (the whole language check)
//! E0376 is covered by construction (a duplicate finalizer parses to the
//! code) though it carries no fix; X-codes have no emitters (the explain
//! index's R-lesson rows are runtime failure reports, not compile errors).

use lagom_diagnostics::{SourceFile, Severity, Verbosity};

/// One broken program per code, each probed to emit exactly that code.
fn trigger(code: &str) -> &'static str {
    match code {
        // ----- lexer: indentation, quotes, unknown words -----
        "E0101" => "say 5 @ x\n\tif true\n        say 1\n",
        "E0102" => "if true\n say 1\n",
        "E0105" => "say \"oops\n",
        "E0108" => "make sum equal to 1 +\n2\nsay sum\n",

        // ----- parser: structure & ambiguity -----
        "E0201" => "make score 10\n",
        "E0202" => "if x is greater than 3\nsay \"a\"\n",
        "E0203" => "say greet who, \"!\"\n",
        "E0204" => "repeat sometimes\n    say 1\n",
        "E0205" => "make x equal to number\n",
        "E0206" => "say \"oops {name\"\n",
        "E0208" => "set to 5\n",
        "E0209" => "make x equal to bigger of a and b or c\n",
        "E0210" => "make count equal to 2\nsay count is less than 3\n",

        // ----- sema: the whole language check (E0330–E0394) -----
        "E0302" => "make answer equal to ask \"pick: \"\nsay number from answer\n",
        "E0330" => "make count equal to 1\nmake count equal to 2\nsay count\n",
        "E0331" => "make total equal to 1\nmake total score equal to 2\nsay total score\n",
        "E0332" => "use files\nmake data equal to attempt read file at \"gone.txt\"\n",
        "E0333" => "function grade\n    takes text called who\n    returns a number\n    say who\n\nsay grade \"bo\"\n",
        "E0334" => "make changing label equal to \"count: \"\nincrease label by 1\n",
        "E0335" => "make done equal to false\nif done\n    stop\n",
        "E0336" => "make x equal to 1\ngives back x\n",
        "E0337" => "function guard\n    takes text called who\n    if size of who is equal to 0\n        fail with \"empty name\"\n    say who\n\nsay guard \"bo\"\n",
        "E0338" => "make total equal to 5\nrepeat for each item in total\n    say item\n",
        "E0339" => "make score equal to 0\nincrease score by 5\nsay score\n",
        "E0340" => "use rocket\nsay \"hi\"\n",
        "E0341" => "function tally\n    returns a number\n    gives back 1\n\nfunction tally\n    returns a number\n    gives back 2\n",
        "E0342" => "make n equal to 5\nsay n at 1\n",
        "E0343" => "structure player\n    has name of type text\n    has score of type number\n\nmake p equal to a player with name \"bo\" and score 1\nsay nick of p\n",
        "E0344" => "make score equal to 10\nsay scoer\n",
        "E0345" => "make label equal to \"count\"\nmake flipped equal to -label\n",
        "E0346" => "make things equal to a list of 1, \"a\"\nsay things\n",
        "E0347" => "make s equal to a playmaker with name \"bo\"\nsay s\n",
        "E0348" => "structure player\n    has name of type text\n\nmake p equal to a player with nick \"bo\"\nsay p\n",
        "E0349" => "structure player\n    has name of type text\n    has score of type number\n\nmake p equal to a player with name \"bo\"\nsay p\n",
        "E0350" => "structure player\n    has name of type text\n\nmake p equal to a player with name \"a\" and name \"b\"\nsay p\n",
        "E0351" => "make v equal to 5\nmake w equal to v 3\n",
        "E0352" => "make n equal to 5\nsay first of n\n",
        "E0353" => "say square root of 16\n",
        "E0354" => "function add\n    takes number called a\n    takes number called b\n    returns a number\n    gives back a plus b\n\nsay add 1\n",
        "E0355" => "make reply equal to ask \"age: \"\nif reply is equal to 13\n    say \"teen\"\n",
        "E0356" => "make score equal to \"fifteen\"\nif score is greater than 10\n    say \"big\"\n",
        "E0357" => "make label equal to \"count: \"\nmake doubled equal to label plus 1\n",
        "E0358" => "make name equal to \"bo\"\nif name\n    say \"yes\"\n",
        "E0359" => "make n equal to 5\nmake parsed equal to number from n\n",
        "E0360" => "make n equal to \"hi\" of type number\nsay n\n",
        "E0361" => "kind shape\n    is a circle with radius of type number\n    is a circle with side of type number\n",
        "E0362" => "match 5\n    when \"five\"\n        say \"five\"\n    otherwise\n        say \"other\"\n",
        "E0363" => "kind shape\n    is a circle with radius of type number\nmake s equal to a circle with radius 1\nmatch s\n    when circle\n        say \"round\"\n",
        "E0364" => "kind shape\n    is a circle with radius of type number\nmake s equal to a circle with radius 1\nmatch s\n    when a circle with diameter d\n        say d\n",
        "E0365" => "make n equal to 5\nmatch n\n    when something with value v\n        say v\n    when nothing\n        say \"none\"\n",
        "E0366" => "make broken thing equal to 5\nmake out equal to map a list of 1 using broken thing\n",
        "E0367" => "kind shape\n    is a circle with radius of type number\nmake s equal to a circle with radius 1\nmatch s\n    when a triangle with side t\n        say t\n",
        "E0368" => "kind shape\n    is a circle with radius of type number\nmake s equal to a circle\nsay s\n",
        "E0369" => "make f equal to taking a and b giving back a plus b\nmake out equal to map a list of 1 using f\n",
        "E0370" => "make broken equal to map 5 using it plus 1\n",
        "E0371" => "a type called score is a number\na type called score is a text\n",
        "E0372" => "a type called score is a score\n",
        "E0373" => "a type called gem is a jewel\n",
        "E0374" => "class counter\n    has count of type number\n    can bump\n        set count of myself to 1\n\nmake c equal to 5\nbump c\n",
        "E0375" => "class window\n    has width of type number\n    construction\n        takes number called width\n        set width of myself to width\n\nmake w equal to a new window with height 800\nsay width of w\n",
        "E0376" => "class thing\n    before last reference disappears\n        say 1\n    before last reference disappears\n        say 2\nsay 1\n",
        "E0377" => "use files\n\nclass holder\n    has gem of type text\n    before last reference disappears\n        append file \"log\" at \"log.txt\"\nsay \"go\"\n",
        "E0378" => "interface printable\n    can print\n\nclass sled does vehicle and printable\n    can slide\n        say \"sliding\"\n\nmake s equal to a new sled\nsay s\n",
        "E0379" => "class sled does vehicle\n    can slide\n        say \"sliding\"\n\nmake s equal to a new sled\nsay s\n",
        "E0380" => "class sled extends vehicle\n    can slide\n        say \"sliding\"\n\nmake s equal to a new sled\nsay s\n",
        "E0381" => "interface printable\n    can print\n        say \"printed\"\n\nclass badge does printable and printable\n    can print\n        say \"badge\"\n\nmake b equal to a new badge\nsay b\n",
        "E0382" => "interface walker\n    can go\n        takes number called steps\n        say \"walking\"\n\ninterface runner\n    can go\n        takes text called path\n        say \"running\"\n\nclass bot does walker and runner\n    has label of type text\n\nmake b equal to a new bot with label \"x\"\nsay b\n",
        "E0383" => "interface comparable\n    can compare to\n        takes anything called other\n        gives back true\n\nfunction biggest item\n    takes a list of some type that does comparable called items\n    returns some type\n    gives back first of items\n\nmake words equal to a list of \"b\", \"a\"\nsay biggest item words\n",
        "E0384" => "interface printable\n    can print\n        say \"printed\"\n\nclass badge does printable\n    can print\n        say \"badge\"\n\nfunction show\n    takes some type that does printable called thing\n    echo of thing\n\nmake b equal to a new badge\nsay show b\n",
        "E0390" => "wait for all tasks\nsay \"done\"\n",
        "E0391" => "make m equal to a channel of text\nsend \"x\", m\n",
        "E0393" => "make m equal to 5\nsend \"x\" to m\n",
        "E0394" => "make m equal to 5\nreceive from m\n",
        other => panic!("no broken-program trigger registered for {other} — add one to the corpus"),
    }
}

/// Every code the compiler emits at check time. Keep in step with the
/// emitters; the test below fails if a trigger does not produce its code.
const EMITTED: &[&str] = &[
    "E0101", "E0102", "E0105", "E0108",
    "E0201", "E0202", "E0203", "E0204", "E0205", "E0206", "E0208", "E0209", "E0210",
    "E0302",
    "E0330", "E0331", "E0332", "E0333", "E0334", "E0335", "E0336", "E0337", "E0338",
    "E0339", "E0340", "E0341", "E0342", "E0343", "E0344", "E0345", "E0346", "E0347",
    "E0348", "E0349", "E0350", "E0351", "E0352", "E0353", "E0354", "E0355", "E0356",
    "E0357", "E0358", "E0359", "E0360", "E0361", "E0362", "E0363", "E0364", "E0365",
    "E0366", "E0367", "E0368", "E0369", "E0370", "E0371", "E0372", "E0373", "E0374",
    "E0375", "E0376", "E0377", "E0378", "E0379", "E0380", "E0381", "E0382", "E0383",
    "E0384", "E0390", "E0391", "E0393", "E0394",
];

/// All errors a source produces across the whole front end.
fn all_errors(src: &str) -> Vec<lagom_diagnostics::Diagnostic> {
    let (ast, parse_diags) = lagom_parser::parse(src);
    if !parse_diags.is_empty() {
        return parse_diags
            .into_iter()
            .filter(|d| matches!(d.severity, Severity::Error))
            .collect();
    }
    lagom_sema::check(&ast, src)
        .diags
        .items
        .into_iter()
        .filter(|d| matches!(d.severity, Severity::Error))
        .collect()
}

/// The rendered student page for `code`'s diagnostic.
fn page_for(src: &str, code: &str) -> String {
    let diags = all_errors(src);
    let d = diags
        .iter()
        .find(|d| d.code == code)
        .unwrap_or_else(|| {
            panic!(
                "trigger for {code} did not produce it (got {:?}); fix the corpus entry",
                diags.iter().map(|d| d.code).collect::<Vec<_>>()
            )
        });
    let file = SourceFile::new("broken.lagom", src);
    Verbosity::Student.render(&file, d)
}

/// Normalized word bag: punctuation removed, lowercase, sorted words.
fn bag(t: &str) -> Vec<String> {
    let mut words: Vec<String> = t
        .chars()
        .map(|c| if c.is_alphanumeric() { c } else { ' ' })
        .collect::<String>()
        .split_whitespace()
        .map(|w| w.to_lowercase())
        .collect();
    words.sort();
    words
}

/// Jaccard-style containment: does `a` mostly repeat inside `b`?
/// 1.0 = a perfect word-for-word restatement (a tautology).
fn restatement_score(a: &str, b: &str) -> f64 {
    let (wa, wb) = (bag(a), bag(b));
    if wa.is_empty() {
        return 0.0;
    }
    let overlap = wa.iter().filter(|w| wb.contains(w)).count();
    overlap as f64 / wa.len() as f64
}

/// The whole point of this file: render every emitted code's diagnostic and
/// assert that any printed fix-why footer carries causal content beyond the
/// error line and the explanation. A footer that a reader could have
/// produced by copying the error is a tautology — it must not ship.
#[test]
fn fix_why_footers_add_causes_not_restatements() {
    let mut failures = Vec::new();
    for code in EMITTED {
        let page = page_for(trigger(code), code);
        let Some(footer) = page.split("This fixes it because: ").nth(1) else {
            continue; // no fix or no footer — nothing to check
        };
        let footer = footer.lines().next().unwrap_or("").trim();
        if footer.is_empty() {
            failures.push(format!("{code}: empty fix-why footer"));
            continue;
        }
        // The footer must not restate the error line …
        let message_line = page
            .lines()
            .nth(2)
            .unwrap_or("")
            .trim()
            .to_string();
        let against_message = restatement_score(footer, &message_line);
        // … and must not restate the explanation paragraph either (a
        // paraphrase of the rule is still not a justification of the fix).
        let explanation = page
            .split("\n\n")
            .find(|p| {
                !p.contains("ERROR on line")
                    && !p.starts_with("    ")
                    && !p.starts_with("To fix")
                    && !p.starts_with("This fixes")
                    && !p.starts_with("Learn more")
                    && !p.starts_with("note:")
                    && !p.is_empty()
                    && !p.lines().next().unwrap_or("").contains(" on line ")
            })
            .unwrap_or("");
        let against_explanation = restatement_score(footer, explanation);
        if against_message >= 0.9 || against_explanation >= 0.9 {
            failures.push(format!(
                "{code}: fix-why restates the error ({:.0}% of message words, {:.0}% of explanation words):\n    message:     {message_line}\n    explanation: {}\n    footer:      {footer}",
                against_message * 100.0,
                against_explanation * 100.0,
                explanation.lines().next().unwrap_or("")
            ));
        }
    }
    assert!(
        failures.is_empty(),
        "tautological fix-why footers found:\n\n{}",
        failures.join("\n\n")
    );
}

/// The corpus itself cannot rot: every code with an explicit fix in the
/// compiler must print a footer (the M2 rule — a suggestion ships with its
/// reason), so a silently dropped explanation shows up here, not in a
/// student's terminal.
#[test]
fn every_fixed_diagnostic_prints_a_footer() {
    // Codes whose fix legitimately renders without a footer: the fix is an
    // example fragment (`For example: …`) or a pointer, and the page carries
    // the reasoning elsewhere (notes/explanation), reviewed in-place here.
    const FOOTERLESS: &[&str] = &["E0101", "E0102", "E0105", "E0108", "E0205", "E0207", "E0210"];
    let mut missing = Vec::new();
    for code in EMITTED {
        if FOOTERLESS.contains(&code) {
            continue;
        }
        let page = page_for(trigger(code), code);
        if page.contains("To fix, write:") && !page.contains("This fixes it because:") {
            missing.push(format!("{code}: has a fix but no fix-why footer:\n{page}"));
        }
    }
    assert!(
        missing.is_empty(),
        "fixed diagnostics without a causal footer:\n\n{}",
        missing.join("\n\n")
    );
}

/// The detector itself cannot rot: a word-for-word copy must score as a
/// restatement, and a footer with genuinely different words must not. If
/// this ever fails, the sweep's verdicts mean nothing.
#[test]
fn the_tautology_detector_detects_tautologies() {
    let error_line = "This player is missing its `score` field.";
    let tautology = "this player is missing its score field";
    assert!(
        restatement_score(tautology, error_line) >= 0.9,
        "a word-for-word copy must be detected"
    );
    let causal =
        "every field needs a value before anything can read the object";
    assert!(
        restatement_score(causal, error_line) < 0.5,
        "a real why must not be flagged"
    );
    assert_eq!(restatement_score("", error_line), 0.0);
}

/// One honest end-to-end page, asserted in full, so the sweep's mechanics
/// are visible: E0337's footer must carry the causal content (the caller's
/// knowledge) that the fix rests on — not the error line restated.
#[test]
fn the_footer_contract_is_visible_on_a_real_page() {
    let page = page_for(trigger("E0337"), "E0337");
    assert!(page.contains("does not say `can fail`"), "{page}");
    assert!(page.contains("This fixes it because:"), "{page}");
    assert!(
        page.contains("honest to go"),
        "footer lost its causal content:\n{page}"
    );
}
