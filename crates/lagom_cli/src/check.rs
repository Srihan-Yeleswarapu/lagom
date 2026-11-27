//! `lagom check` — parse and check; show diagnostics, human or machine.
//!
//! The human form is the teaching rendering (00 §26.2). `--json` emits the
//! editor document — the same field shape `crates/lagom_web::check` returns
//! (the browser playground's contract): one JSON object on stdout in every
//! outcome, diagnostics spelled `code/severity/message/line/column/endLine/
//! endColumn/rendered/explanation/fix/concept/notes` (1-based span starts).
//! The exit code carries pass/fail, not the output channel, so editor
//! tooling never has to parse stderr to learn what is wrong.

use super::args::{read_source, render_failure, source_path, CliError, CliResult};
use lagom_driver::FrontendError;
use lagom_diagnostics::{Diagnostic, Severity, SourceFile, Verbosity};

pub fn cmd_check(rest: &[String]) -> CliResult {
    let json = rest.iter().any(|a| a == "--json");
    let rest: Vec<String> = rest.iter().filter(|a| *a != "--json").cloned().collect();
    let path = source_path(&rest)?;
    let src = read_source(&path)?;
    let file_name = path.display().to_string();

    if !json {
        return match lagom_driver::frontend(&src) {
            Ok(_) => {
                println!("{} checks out.", path.display());
                Ok(())
            }
            Err(e) => Err(render_failure(&path, &src, &e)),
        };
    }

    // JSON mode: always one document on stdout; the exit code says pass/fail.
    let (diags, ok) = match lagom_driver::frontend(&src) {
        Ok(_) => (None, true),
        Err(FrontendError::Diagnostics(list)) => (Some(list), false),
        Err(FrontendError::Internal { stage, message }) => (
            Some(vec![Diagnostic {
                code: "internal",
                severity: Severity::Error,
                message: format!("internal compiler error in {stage}: {message}"),
                span: lagom_diagnostics::Span::default(),
                explanation: None,
                fix: None,
                fix_why: None,
                concept: None,
                labels: Vec::new(),
                notes: Vec::new(),
            }]),
            false,
        ),
        Err(FrontendError::Tool(message)) => (
            Some(vec![Diagnostic {
                code: "environment",
                severity: Severity::Error,
                message,
                span: lagom_diagnostics::Span::default(),
                explanation: None,
                fix: None,
                fix_why: None,
                concept: None,
                labels: Vec::new(),
                notes: Vec::new(),
            }]),
            false,
        ),
    };

    let file = SourceFile::new(&file_name, &src);
    let items: Vec<String> = diags
        .unwrap_or_default()
        .iter()
        .map(|d| diag_json(&file, d))
        .collect();
    println!(
        r#"{{"path": {},"ok": {},"diagnostics": [{}]}}"#,
        json_string(&file_name),
        if ok { "true" } else { "false" },
        items.join(", ")
    );
    if ok {
        Ok(())
    } else {
        Err(CliError::Exit(1))
    }
}

/// One diagnostic as the editor document's object — the field shape
/// `crates/lagom_web::check` documents, byte for byte in spirit (camelCase,
/// 1-based line/column, student-mode `rendered`).
fn diag_json(file: &SourceFile, d: &Diagnostic) -> String {
    let (line, column) = file.line_col(d.span.start);
    let (end_line, end_column) = file.line_col(d.span.end);
    let field = |key: &str, value: Option<&String>| match value {
        Some(v) => format!(r#""{key}": {}"#, json_string(v)),
        None => format!(r#""{key}": null"#),
    };
    let notes: Vec<String> = d.notes.iter().map(|n| json_string(n)).collect();
    // The fields join with commas by construction — one place owns the
    // separator, so no field can be born separator-less.
    let fields = vec![
        format!(r#""code": {}"#, json_string(&d.code)),
        format!(r#""severity": {}"#, json_string(severity_name(d.severity))),
        format!(r#""message": {}"#, json_string(&d.message)),
        format!(r#""line": {line}"#),
        format!(r#""column": {column}"#),
        format!(r#""endLine": {end_line}"#),
        format!(r#""endColumn": {end_column}"#),
        // `concept` is `&'static str` in the diagnostic model; the JSON
        // field shape carries it as a string either way.
        field("rendered", Some(&Verbosity::Student.render(file, d))),
        field("explanation", d.explanation.as_ref()),
        field("fix", d.fix.as_ref()),
        field("concept", d.concept.map(|c| c.to_string()).as_ref()),
        format!(r#""notes": [{}]"#, notes.join(", ")),
    ];
    format!("{{{}}}", fields.join(", "))
}

fn severity_name(s: Severity) -> &'static str {
    match s {
        Severity::Error => "error",
        Severity::Warning => "warning",
        Severity::Note => "note",
    }
}

/// A JSON string literal: escape what JSON requires, pass the rest through.
fn json_string(s: &str) -> String {
    let mut out = String::with_capacity(s.len() + 2);
    out.push('"');
    for ch in s.chars() {
        match ch {
            '"' => out.push_str("\\\""),
            '\\' => out.push_str("\\\\"),
            '\n' => out.push_str("\\n"),
            '\r' => out.push_str("\\r"),
            '\t' => out.push_str("\\t"),
            c if (c as u32) < 0x20 => out.push_str(&format!("\\u{:04x}", c as u32)),
            c => out.push(c),
        }
    }
    out.push('"');
    out
}
