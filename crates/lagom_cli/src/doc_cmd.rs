// Copyright (c) 2026 Srihan Yeleswarapu.
// Source available under the Lagom License (LICENSE.md).

//! `lagom doc` — render `##` doc comments into a markdown page (00 §26.6:
//! docs-in-the-compiler; doc 09 §13: the rendered site with *executable
//! examples*).
//!
//! Conventions, pinned by tests:
//! - A `## …` line attaches to the next declaration (`function`,
//!   `structure`, `kind`, `test`, or `a type called …`) in the source.
//! - A `## >>> <statement>` doc line starts an *example*; **consecutive
//!   `>>>` lines are one example** (a multi-statement session — later
//!   statements see earlier bindings, exactly like `lagom play`). Any other
//!   `##` line ends the example.
//! - The example runs with the whole file in scope (the Rust doctest rule).
//! - `## = <text>` lines after a `>>>` pin what the example produces: the
//!   `say` output it printed — or, when it printed nothing, the value of
//!   its last binding (D-40 in doc 09: a `make` example pins its bound
//!   value; the REPL teaching echo is the meaning of "=").
//! - `--check` runs every example through the **play session engine**
//!   (`lagom_driver::Session` — the one evaluator); any wrong output,
//!   compile error, or unpinnable `=` fails the command with a named,
//!   line-numbered error. No doc can drift from the compiler, and no
//!   example can false-pass.

use std::path::Path;

use super::args::{read_source, source_path, CliError, CliResult};
use lagom_driver::{Session, StepOutcome};

/// One documented item: its name, its doc lines, the declaration's line.
struct DocItem {
    name: String,
    doc: Vec<String>,
    line: usize,
}

/// One executable example: the statements and the expected output lines.
struct Example {
    statements: Vec<String>,
    expected: Vec<String>,
    /// Source line of the example's first `>>>` (for check errors).
    line: usize,
}

/// What the checked example produced, in D-40's precedence order.
enum Produced {
    /// The `say` output, in order (wins when present).
    Output(Vec<String>),
    /// No output; the example's last binding, from the teaching echo.
    Value(String),
    /// No output and no binding.
    Nothing,
}

pub fn cmd_doc(rest: &[String]) -> CliResult {
    let path = source_path(rest)?;
    let src = read_source(&path)?;
    let items = extract(&src);
    if items.is_empty() {
        // `--check` must never false-pass just because nothing was
        // documented: examples can exist without an attached declaration.
        if super::args::has_flag(rest, "--check") {
            return check_examples(&src, &examples(&src));
        }
        println!(
            "no `##` doc comments found in {} — write one above a function:\n\n## Greets someone by name.\nfunction greet\n    takes text called who\n    gives back \"Hello!\"",
            path.display()
        );
        return Ok(());
    }
    let markdown = render(&path, &items);
    if super::args::has_flag(rest, "--check") {
        return check_examples(&src, &examples(&src));
    }
    print!("{markdown}");
    Ok(())
}

/// Scan the source: `##` lines accumulate; the next declaration line claims
/// them. A non-declaration line drops a pending doc (misplaced comment).
fn extract(src: &str) -> Vec<DocItem> {
    let mut items = Vec::new();
    let mut pending: Vec<String> = Vec::new();
    for (i, raw) in src.lines().enumerate() {
        let trimmed = raw.trim();
        if let Some(doc) = trimmed.strip_prefix("##") {
            pending.push(doc.trim().to_string());
            continue;
        }
        if trimmed.is_empty() || trimmed.starts_with('#') {
            continue; // blank or plain comment: does not claim the doc
        }
        if pending.is_empty() {
            continue;
        }
        if let Some(name) = declaration_name(trimmed) {
            items.push(DocItem { name, doc: std::mem::take(&mut pending), line: i + 1 });
        }
    }
    items
}

/// The declaration's display name, when the line starts one.
fn declaration_name(line: &str) -> Option<String> {
    let mut words = line.split_whitespace();
    match words.next()? {
        "function" => Some(words.collect::<Vec<_>>().join(" ")),
        "structure" | "kind" => words.next().map(|n| n.to_string()),
        "test" => Some(line.trim_start_matches("test").trim().trim_matches('"').to_string()),
        "a" if line.starts_with("a type called") => {
            // `a type called score is a number` — the alias name.
            line.split_whitespace().nth(3).map(|n| n.to_string())
        }
        _ => None,
    }
}

/// The examples in source order. Consecutive `## >>>` lines are ONE example
/// (D-40): the first line opens it, later ones append statements; any other
/// `##` line ends it. `## =` lines pin the current example's output.
fn examples(src: &str) -> Vec<Example> {
    let mut out: Vec<Example> = Vec::new();
    for (i, raw) in src.lines().enumerate() {
        let doc = match raw.trim().strip_prefix("##") {
            Some(d) => d.trim(),
            None => continue,
        };
        if let Some(stmt) = doc.strip_prefix(">>>") {
            match out.last_mut() {
                // A `=`-pinned example already ended: a new `>>>` opens one.
                Some(ex) if ex.expected.is_empty() => ex.statements.push(stmt.trim().to_string()),
                _ => out.push(Example {
                    statements: vec![stmt.trim().to_string()],
                    expected: Vec::new(),
                    line: i + 1,
                }),
            }
        } else if let Some(want) = doc.strip_prefix('=') {
            match out.last_mut() {
                Some(ex) => ex.expected.push(want.trim().to_string()),
                // A `=` with no example above cannot be checked at all.
                None => out.push(Example {
                    statements: Vec::new(),
                    expected: vec![want.trim().to_string()],
                    line: i + 1,
                }),
            }
        }
    }
    out
}

/// Render the markdown page.
fn render(path: &Path, items: &[DocItem]) -> String {
    let mut out = String::new();
    out.push_str(&format!("# Documentation — {}\n\n", path.display()));
    out.push_str(&format!("{} documented items.\n\n", items.len()));
    for item in items {
        out.push_str(&format!("## `{}`\n\n", item.name));
        out.push_str(&format!("*line {}*\n\n", item.line));
        for line in &item.doc {
            // Example lines render as fenced lagom code, not prose.
            if let Some(stmt) = line.strip_prefix(">>>") {
                out.push_str("``` lagom\n");
                out.push_str(stmt.trim());
                out.push_str("\n```\n\n");
            } else if line.starts_with('=') {
                // Expected output renders as a comment on the example.
                out.push_str(&format!("<!-- says: {} -->\n\n", line[1..].trim()));
            } else {
                out.push_str(line);
                out.push('\n');
                out.push('\n');
            }
        }
    }
    out
}

/// `--check`: run every example through the play session engine with the
/// file's declarations in scope, and compare what it produced (D-40) with
/// the `=` pins. Failures name the example's file line; nothing false-passes.
fn check_examples(src: &str, examples: &[Example]) -> CliResult {
    if examples.is_empty() {
        println!("doc check: no examples to verify.");
        return Ok(());
    }
    let mut failed = 0;
    for (i, ex) in examples.iter().enumerate() {
        match check_one(src, ex) {
            Ok(()) => println!("doc check: example {} ok (line {}).", i + 1, ex.line),
            Err(message) => {
                failed += 1;
                println!("doc check: example {} (line {}):\n{message}", i + 1, ex.line);
            }
        }
    }
    if failed > 0 {
        Err(CliError::Message(format!("doc check: {failed} example(s) failed")))
    } else {
        println!("doc check: {} example(s) ok.", examples.len());
        Ok(())
    }
}

/// Check one example. The file is the example's scope (declarations first);
/// the example's statements run on top via the session engine, collecting
/// each statement's own output and bindings.
fn check_one(src: &str, ex: &Example) -> Result<(), String> {
    // A `=` with no statements above it has nothing to run.
    if ex.statements.is_empty() {
        return Err(
            "  `## =` with no `## >>>` example above it — write the example first.".to_string(),
        );
    }
    let mut session = Session::with_seed(1);
    // The file itself is submitted as the scope. Compile errors here belong
    // to the doc's source file, not the example.
    match session.submit(src) {
        StepOutcome::Accepted { .. } => {}
        StepOutcome::Rejected { rendered } => {
            return Err(format!(
                "  the file itself does not compile, so the example cannot run:\n{}",
                indent(&rendered)
            ));
        }
    }
    // Run the example, collecting what IT produces: per-statement output,
    // then bindings from the teaching echo (name is a T = value), D-40.
    let mut output: Vec<String> = Vec::new();
    let mut bindings: Vec<String> = Vec::new();
    for stmt in &ex.statements {
        match session.submit(stmt) {
            StepOutcome::Accepted { echo, output: said } => {
                output.extend(said);
                bindings.extend(echo);
            }
            StepOutcome::Rejected { rendered } => {
                return Err(format!("  the example does not compile:\n{}", indent(&rendered)));
            }
        }
    }
    let produced = match (output.as_slice(), bindings.last()) {
        ([], Some(echo)) => match echo.split(" = ").nth(1) {
            Some(value) => Produced::Value(value.to_string()),
            None => Produced::Nothing,
        },
        // A bare call prints nothing and binds nothing: its answer was
        // discarded, so a `=` pin has nothing to attach to (D-40).
        ([], None) => Produced::Nothing,
        (said, _) => Produced::Output(said.to_vec()),
    };
    // Pins are checked when present (the Rust doctest rule); an unpinned
    // example still had to compile and run to get here.
    match (&produced, ex.expected.as_slice()) {
        (_, []) => Ok(()),
        (Produced::Output(actual), expected) => {
            if actual == expected {
                Ok(())
            } else {
                Err(format!(
                    "  output mismatch:\n  expected: {expected:?}\n  actual:   {actual:?}"
                ))
            }
        }
        (Produced::Value(actual), [expected]) => {
            if actual == expected {
                Ok(())
            } else {
                Err(format!(
                    "  value mismatch:\n  expected: {expected:?}\n  actual:   {actual:?}"
                ))
            }
        }
        (Produced::Value(actual), expected) => Err(format!(
            "  the example produced one value ({actual:?}) but {} `=` lines pin it — one `=` per produced line.",
            expected.len()
        )),
        (Produced::Nothing, expected) => Err(format!(
            "  the example prints nothing and binds nothing, but {} `=` line(s) pin output. The example's bare call ran and discarded its answer — end the example with `say <value>`, or pin the value a `make` produces (D-40).",
            expected.len()
        )),
    }
}

fn indent(s: &str) -> String {
    s.lines().map(|l| format!("    {l}")).collect::<Vec<_>>().join("\n")
}
