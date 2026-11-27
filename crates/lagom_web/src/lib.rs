//! The browser face of the Lagom compiler and teaching interpreter.
//!
//! Everything here runs in-process: `check` and `run` take Lagom source as a
//! string and never touch the network. The pipeline is the same one
//! `lagom_driver` runs for `lagom run --trace` (parse → check → HIR → MIR →
//! verify → interpreter, dev mode so LOM failure reports are produced). The
//! driver itself is not used because it links the native Cranelift backend,
//! which has no meaning on `wasm32-unknown-unknown`.
//!
//! Both entry points return a JSON document as a string; `web/runner.worker.js`
//! parses it and the `web/src` modules render it.
//!
//! This is deliberately not `compile(source) -> { bytes, diagnostics }`: the
//! module produces no bytecode or Wasm. It runs programs on the interpreter,
//! because the native backend (Cranelift) cannot target `wasm32-unknown-unknown`. Keeping the boundary to strings means the module needs
//! no serialization glue and the same functions are unit-tested natively.

use lagom_diagnostics::{Diagnostic, Severity, SourceFile, Span, Verbosity};
use lagom_mir::MirProgram;
use serde::Serialize;
use wasm_bindgen::prelude::*;

/// The file name used in diagnostics for code typed into the browser editor.
const FILE_NAME: &str = "main.lagom";

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct DiagnosticJson {
    code: String,
    severity: &'static str,
    message: String,
    /// 1-based line and column of the span start.
    line: usize,
    column: usize,
    /// 1-based line and column of the span end (exclusive).
    end_line: usize,
    end_column: usize,
    /// The student-mode teaching rendering (what / where / why / fix).
    rendered: String,
    explanation: Option<String>,
    fix: Option<String>,
    concept: Option<String>,
    notes: Vec<String>,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct CheckJson {
    ok: bool,
    diagnostics: Vec<DiagnosticJson>,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct RunJson {
    /// `completed` | `failed` | `panicked` | `compile-error`.
    status: &'static str,
    /// Every line the program printed with `say` (and `ask` prompts), in order.
    output: Vec<String>,
    /// The failure message for `failed` / `panicked`.
    message: Option<String>,
    /// The LOM failure report (dev build) for `failed` / `panicked`.
    report: Option<String>,
    /// Compile-time diagnostics (errors for `compile-error`, warnings otherwise).
    diagnostics: Vec<DiagnosticJson>,
}

fn severity_name(s: Severity) -> &'static str {
    match s {
        Severity::Error => "error",
        Severity::Warning => "warning",
        Severity::Note => "note",
    }
}

fn is_error(d: &Diagnostic) -> bool {
    matches!(d.severity, Severity::Error)
}

fn to_json(file: &SourceFile, d: &Diagnostic) -> DiagnosticJson {
    let (line, column) = file.line_col(d.span.start);
    let (end_line, end_column) = file.line_col(d.span.end);
    DiagnosticJson {
        code: d.code.to_string(),
        severity: severity_name(d.severity),
        message: d.message.clone(),
        line,
        column,
        end_line,
        end_column,
        rendered: Verbosity::Student.render(file, d),
        explanation: d.explanation.clone(),
        fix: d.fix.clone(),
        concept: d.concept.map(str::to_string),
        notes: d.notes.clone(),
    }
}

/// Run the front end. On success returns the MIR and the non-fatal diagnostics
/// (warnings); on failure returns the errors. Mirrors `lagom_driver::pipeline`.
fn compile(src: &str) -> Result<(MirProgram, Vec<Diagnostic>), Vec<Diagnostic>> {
    let (program, parse_diags) = lagom_parser::parse(src);
    // The parser only emits errors, so any diagnostic stops the pipeline.
    if !parse_diags.is_empty() {
        return Err(parse_diags);
    }
    let checked = lagom_sema::check(&program, src);
    let diags = checked.diags.items.clone();
    if diags.iter().any(is_error) {
        return Err(diags.into_iter().filter(is_error).collect());
    }
    let mir = lagom_mir::lower(lagom_hir::lower(checked));
    if let Err(message) = lagom_mir::verify(&mir) {
        return Err(vec![Diagnostic {
            code: "internal",
            severity: Severity::Error,
            message: format!("internal compiler error (MIR verify): {message}"),
            span: Span::default(),
            explanation: None,
            fix: None,
            fix_why: None,
            concept: None,
            labels: Vec::new(),
            notes: Vec::new(),
        }]);
    }
    Ok((mir, diags))
}

fn to_json_list(src: &str, diags: &[Diagnostic]) -> Vec<DiagnosticJson> {
    let file = SourceFile::new(FILE_NAME, src);
    diags.iter().map(|d| to_json(&file, d)).collect()
}

fn to_json_string<T: Serialize>(value: &T) -> String {
    serde_json::to_string(value).expect("serializing plain structs cannot fail")
}

/// Type-check a program without running it.
///
/// Returns JSON: `{ ok: boolean, diagnostics: [...] }`. `ok` is false when any
/// error was found; warnings alone keep it true.
#[wasm_bindgen]
pub fn check(source: &str) -> String {
    let json = match compile(source) {
        Ok((_, warnings)) => CheckJson {
            ok: true,
            diagnostics: to_json_list(source, &warnings),
        },
        Err(errors) => CheckJson {
            ok: false,
            diagnostics: to_json_list(source, &errors),
        },
    };
    to_json_string(&json)
}

/// Compile and run a program on the Lagom interpreter.
///
/// `stdin` is the text the program's `ask` statements read, one answer per
/// line. The browser cannot block a wasm call on user input, so the answers are
/// supplied up front; an `ask` with no answer left fails with a clear message.
///
/// Returns JSON: `{ status, output, message, report, diagnostics }`.
#[wasm_bindgen]
pub fn run(source: &str, stdin: &str) -> String {
    let lines = stdin.lines().map(str::to_string).collect();
    to_json_string(&execute(source, lines, None))
}

#[wasm_bindgen]
extern "C" {
    /// Provided by the page's worker (`web/runner.worker.js`). Shows `prompt`,
    /// blocks until the user answers, and returns the answer, or `null` at end
    /// of input. `transcript` is the program output so far.
    #[wasm_bindgen(js_name = lagomAskLine)]
    fn js_ask_line(prompt: &str, transcript: &str) -> Option<String>;
}

/// Compile and run a program, pausing on each `ask` to get the answer from the
/// page.
///
/// The browser worker blocks inside this call (`Atomics.wait`) while the user
/// types an answer, so each `ask` pauses the program until it is answered. An
/// `ask` after the user says "no more input" fails with `end of input`.
///
/// Returns the same JSON shape as [`run`].
#[wasm_bindgen]
pub fn run_interactive(source: &str) -> String {
    let hook: lagom_interp::AskHook = Box::new(|prompt, transcript| {
        let joined = transcript.join("\n");
        js_ask_line(prompt, &joined)
    });
    to_json_string(&execute(source, Vec::new(), Some(hook)))
}

/// Shared run path. `stdin` answers are used first; then `ask_hook`, if given.
fn execute(source: &str, stdin: Vec<String>, ask_hook: Option<lagom_interp::AskHook>) -> RunJson {
    match compile(source) {
        Err(errors) => RunJson {
            status: "compile-error",
            output: Vec::new(),
            message: None,
            report: None,
            diagnostics: to_json_list(source, &errors),
        },
        Ok((mut mir, warnings)) => {
            let has_entry = mir.items.iter().any(|i| {
                matches!(i, lagom_mir::MirItem::Main(f) if f.is_entry)
            });
            let diagnostics = to_json_list(source, &warnings);
            if !has_entry {
                RunJson {
                    status: "completed",
                    output: Vec::new(),
                    message: Some("This program has no top-level statements to run.".into()),
                    report: None,
                    diagnostics,
                }
            } else {
                let mut it = lagom_interp::Interp::dev(&mut mir);
                it.host.stdin = stdin;
                it.host.ask_hook = ask_hook;
                let outcome = it.run(&mir, source);
                let output = it.host.stdout.clone();
                let (status, message, report) = match outcome {
                    lagom_interp::RunOutcome::Completed => ("completed", None, None),
                    lagom_interp::RunOutcome::Failed { message, report } => {
                        ("failed", Some(message), report)
                    }
                    lagom_interp::RunOutcome::Panicked { message, report } => {
                        ("panicked", Some(message), report)
                    }
                };
                RunJson { status, output, message, report, diagnostics }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::Value;

    fn parse(s: &str) -> Value {
        serde_json::from_str(s).expect("valid JSON")
    }

    #[test]
    fn runs_hello_world() {
        let v = parse(&run("say \"Hello from Lagom\"\n", ""));
        assert_eq!(v["status"], "completed");
        assert_eq!(v["output"][0], "Hello from Lagom");
    }

    #[test]
    fn feeds_ask_from_stdin_lines() {
        let src = "make name equal to ask \"Who is it? \"\nsay \"Hello, {name}\"\n";
        let v = parse(&run(src, "Ada\n"));
        assert_eq!(v["status"], "completed", "{v}");
        let last = v["output"].as_array().unwrap().last().unwrap();
        assert_eq!(last, "Hello, Ada");
    }

    #[test]
    fn ask_without_input_fails_with_message() {
        let v = parse(&run("make name equal to ask \"Who? \"\nsay name\n", ""));
        assert_eq!(v["status"], "panicked", "{v}");
        assert!(v["message"].as_str().unwrap().contains("end of input"));
    }

    #[test]
    fn reports_compile_error_with_position() {
        let src = "say \"ok\"\nsay undefined_name\n";
        let v = parse(&run(src, ""));
        assert_eq!(v["status"], "compile-error", "{v}");
        let d = &v["diagnostics"][0];
        assert_eq!(d["severity"], "error");
        assert_eq!(d["line"], 2);
        assert!(!d["rendered"].as_str().unwrap().is_empty());
    }

    #[test]
    fn check_accepts_valid_and_rejects_invalid() {
        assert_eq!(parse(&check("say \"hi\"\n"))["ok"], true);
        let bad = parse(&check("say (\n"));
        assert_eq!(bad["ok"], false);
        assert!(!bad["diagnostics"].as_array().unwrap().is_empty());
    }

    /// A hook that answers from a queue, like the page does, and records each
    /// (prompt, transcript) pair it was asked with.
    fn scripted(answers: Vec<&'static str>, seen: std::rc::Rc<std::cell::RefCell<Vec<String>>>) -> lagom_interp::AskHook {
        let mut queue: std::collections::VecDeque<&'static str> = answers.into();
        Box::new(move |prompt, transcript| {
            seen.borrow_mut().push(format!("{prompt}|{}", transcript.len()));
            queue.pop_front().map(str::to_string)
        })
    }

    #[test]
    fn interactive_asks_drive_later_output() {
        let src = "make a equal to ask \"First? \"\nmake b equal to ask \"Second? \"\nmake total equal to a plus b\nsay \"sum is {total}\"\n";
        let seen = std::rc::Rc::new(std::cell::RefCell::new(Vec::new()));
        let v = serde_json::to_value(execute(src, Vec::new(), Some(scripted(vec!["2", "3"], seen.clone())))).unwrap();
        assert_eq!(v["status"], "completed", "{v}");
        // `ask` gives text, so `plus` joins the two answers: "2" then "3".
        assert_eq!(v["output"].as_array().unwrap().last().unwrap(), "sum is 23");
        assert_eq!(seen.borrow().len(), 2);
    }

    #[test]
    fn interactive_ask_after_no_more_input_fails() {
        let src = "make a equal to ask \"First? \"\nmake b equal to ask \"Second? \"\nsay b\n";
        let seen = std::rc::Rc::new(std::cell::RefCell::new(Vec::new()));
        let v = serde_json::to_value(execute(src, Vec::new(), Some(scripted(vec!["only"], seen)))).unwrap();
        assert_eq!(v["status"], "panicked", "{v}");
        assert!(v["message"].as_str().unwrap().contains("end of input"));
    }

    #[test]
    fn empty_program_has_nothing_to_run() {
        let v = parse(&run("", ""));
        assert_eq!(v["status"], "completed");
        assert!(v["output"].as_array().unwrap().is_empty());
    }
}
