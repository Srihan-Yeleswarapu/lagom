//! `lagom lsp` — the language server for editors (00 §26.4, doc 09 §4, M2).
//!
//! A stdio JSON-RPC process speaking the LSP subset the teaching story
//! needs: full-text sync → `publishDiagnostics` (the compiler's teaching
//! diagnostics, unaltered, plus the fix line editors can show in the
//! tooltip), `hover` (the inferred type of the name under the cursor,
//! straight from sema's span-keyed side table), and `definition`
//! (7.0.3's headline case: a multi-word run jumps to its `make`/`takes`
//! binding through the `resolved_uses` side table, with a call→callee
//! fallback for call sites).
//!
//! Positions: LSP speaks 0-based lines and UTF-16 columns; the compiler
//! maps spans as 1-based lines and character columns (`SourceFile::
//! line_col`). Both conversions live here, at the protocol edge — nothing
//! else in the workspace learns LSP units. On the BMP (what a student is
//! in practice typing) the two agree; the conversions handle astral-plane
//! text correctly anyway.
//!
//! Errors: one unparseable message is logged to stderr and the loop
//! continues — an editor session must not die on a bad write. An unknown
//! request is answered with the -32601 method-not-found error, never
//! silence (silence hangs the editor waiting for its reply).

use std::collections::HashMap;
use std::io::{BufRead, Write};

use lagom_diagnostics::{Diagnostic, Severity, Span};
use serde_json::{json, Value};

/// The checked program of a clean document — the hover/definition data.
type Checked = lagom_sema::CheckedProgram;

pub fn cmd_lsp(_rest: &[String]) -> super::args::CliResult {
    run()
}

pub fn run() -> super::args::CliResult {
    let stdin = std::io::stdin();
    let mut reader = stdin.lock();
    let mut server = Server::new();
    loop {
        let Some(body) = read_message(&mut reader) else {
            break; // clean EOF: the editor closed the pipe
        };
        let Ok(message) = serde_json::from_str::<Value>(&body) else {
            eprintln!("lagom lsp: skipping an unparseable message");
            continue;
        };
        let id = message.get("id").cloned();
        let method = message
            .get("method")
            .and_then(Value::as_str)
            .unwrap_or("");
        let params = message.get("params").cloned().unwrap_or(Value::Null);

        match method {
            "initialize" => respond(
                id,
                Some(json!({
                    "capabilities": {
                        "textDocumentSync": { "openClose": true, "change": 2 },
                        "hoverProvider": true,
                        "definitionProvider": true,
                    },
                    "serverInfo": { "name": "lagom" }
                })),
                None,
            ),
            "initialized" | "$/cancelRequest" | "$/setTrace" => {}
            "shutdown" => respond(id, Some(Value::Null), None),
            "exit" => break,
            "textDocument/didOpen" => {
                let doc = params.get("textDocument");
                let uri = doc.and_then(|d| d.get("uri")).and_then(Value::as_str);
                let text = doc.and_then(|d| d.get("text")).and_then(Value::as_str);
                if let (Some(uri), Some(text)) = (uri, text) {
                    server.open(uri, text);
                }
            }
            "textDocument/didChange" => {
                // Incremental sync (change: 2): the changes apply in order —
                // a range edit replaces its span, a range-less change is a
                // full-document replace. One reanalysis at the end of the
                // batch, never per keystroke.
                let uri = params
                    .get("textDocument")
                    .and_then(|d| d.get("uri"))
                    .and_then(Value::as_str);
                let changes = params
                    .get("contentChanges")
                    .and_then(Value::as_array);
                if let (Some(uri), Some(changes)) = (uri, changes) {
                    server.apply_changes(uri, changes);
                }
            }
            "textDocument/didClose" => {
                if let Some(uri) = params
                    .get("textDocument")
                    .and_then(|d| d.get("uri"))
                    .and_then(Value::as_str)
                {
                    server.close(uri);
                }
            }
            "textDocument/hover" => {
                let result = server.with_position(&params, |server, uri, src, position| {
                    server.hover(uri, src, &position)
                });
                respond(id, Some(result.unwrap_or(Value::Null)), None);
            }
            "textDocument/definition" => {
                let result = server.with_position(&params, |server, uri, src, position| {
                    server.definition(uri, src, &position)
                });
                respond(id, Some(result.unwrap_or(Value::Null)), None);
            }
            "textDocument/rename" => {
                let result = server.with_position_and_new_name(&params, |server, uri, src, position, new_name| {
                    server.rename(uri, src, &position, &new_name)
                });
                // A rename the server cannot stand behind answers an error,
                // never silence — silence reads as a hang in the editor.
                match result {
                    Some(Ok(changes)) => respond(id, Some(changes), None),
                    Some(Err(reason)) => respond(
                        id,
                        None,
                        Some(json!({
                            "code": -32602,
                            "message": reason,
                        })),
                    ),
                    None => respond(id, Some(Value::Null), None),
                }
            }
            "textDocument/references" => {
                let include = params
                    .get("context")
                    .and_then(|c| c.get("includeDeclaration"))
                    .and_then(Value::as_bool)
                    .unwrap_or(true);
                let result = server.with_position(&params, |server, uri, src, position| {
                    server.references(uri, src, &position, include)
                });
                respond(id, Some(result.unwrap_or(Value::Null)), None);
            }
            other => {
                // Unknown *requests* must be answered; unknown notifications
                // drop silently.
                if id.is_some() {
                    respond(
                        id,
                        None,
                        Some(json!({
                            "code": -32601,
                            "message": format!("`lagom lsp` does not implement `{other}`."),
                        })),
                    );
                }
            }
        }
    }
    Ok(())
}

// ---------------------------------------------------------------------------
// Server state
// ---------------------------------------------------------------------------

struct Server {
    /// uri → source text of every open document.
    docs: HashMap<String, String>,
    /// uri → checked program, present only while the document checks clean.
    checked: HashMap<String, Checked>,
}

impl Server {
    fn new() -> Self {
        Server {
            docs: HashMap::new(),
            checked: HashMap::new(),
        }
    }

    fn open(&mut self, uri: &str, text: &str) {
        self.docs.insert(uri.to_string(), text.to_string());
        self.refresh(uri);
    }

    /// Apply a didChange batch: every content change in order, then one
    /// reanalysis. A ranged edit splices its replacement into the byte
    /// span the range points at; a range-less edit replaces the whole
    /// document (both are legal in incremental mode).
    fn apply_changes(&mut self, uri: &str, changes: &[Value]) {
        let mut text = self
            .docs
            .get(uri)
            .cloned()
            .unwrap_or_default();
        for change in changes {
            let new_text = change.get("text").and_then(Value::as_str).unwrap_or("");
            match read_range(change.get("range")) {
                None => text = new_text.to_string(),
                Some((start, end)) => {
                    let start = offset_at(&text, &start);
                    let end = offset_at(&text, &end);
                    let end = end.max(start);
                    let mut spliced = String::with_capacity(text.len() + new_text.len());
                    spliced.push_str(&text[..start]);
                    spliced.push_str(new_text);
                    spliced.push_str(&text[end..]);
                    text = spliced;
                }
            }
        }
        self.docs.insert(uri.to_string(), text.clone());
        self.refresh(uri);
    }

    fn refresh(&mut self, uri: &str) {
        let (checked, diags) = {
            let text = self.docs.get(uri).cloned().unwrap_or_default();
            analyze(&text)
        };
        if let Some(checked) = checked {
            self.checked.insert(uri.to_string(), checked);
        } else {
            self.checked.remove(uri);
        }
        if let Some(text) = self.docs.get(uri) {
            publish_diagnostics(uri, &diags, text);
        }
    }

    fn close(&mut self, uri: &str) {
        self.docs.remove(uri);
        self.checked.remove(uri);
        publish_diagnostics(uri, &[], "");
    }

    /// Run `f` with the document state for a hover/definition request.
    fn with_position<R>(
        &self,
        params: &Value,
        f: impl FnOnce(&Self, &str, &str, Position) -> R,
    ) -> Option<R> {
        let uri = params
            .get("textDocument")
            .and_then(|d| d.get("uri"))
            .and_then(Value::as_str)?;
        let src = self.docs.get(uri)?;
        let position = Position::from_json(params.get("position"))?;
        Some(f(self, uri, src, position))
    }

    /// Rename additionally carries `newName`, validated by the server —
    /// the honest bar: whatever lands must re-parse as the same kind of
    /// name run (words separated by single spaces, no punctuation).
    fn with_position_and_new_name<R>(
        &self,
        params: &Value,
        f: impl FnOnce(&Self, &str, &str, Position, &str) -> R,
    ) -> Option<R> {
        // `newName` is looked up before the closure so the inner one stays
        // an honest R-returning callback.
        let new_name = params.get("newName").and_then(Value::as_str)?;
        self.with_position(params, |server, uri, src, position| {
            f(server, uri, src, position, new_name)
        })
    }

    fn hover(&self, uri: &str, src: &str, position: &Position) -> Value {
        let Some(checked) = self.checked.get(uri) else {
            return Value::Null; // hover on a broken program has no types to show
        };
        let offset = offset_at(src, position);
        // The narrowest typed span containing the offset is the token the
        // student is pointing at; an outer span would describe the whole
        // expression the name sits inside.
        let mut best: Option<(Span, &lagom_sema::Type)> = None;
        for (span, ty) in &checked.node_types {
            if span.start <= offset && offset < span.end {
                let better = match best {
                    None => true,
                    Some((b, _)) => span.end - span.start < b.end - b.start,
                };
                if better {
                    best = Some((*span, ty));
                }
            }
        }
        match best {
            None => Value::Null,
            Some((span, ty)) => {
                // The name run's span may include the separator space after
                // it; the hover reads better on the trimmed form.
                let label = safe_slice(src, span.start, span.end).trim().to_string();
                json!({
                    "contents": {
                        "kind": "markdown",
                        "value": format!("`{label}` is a `{}`", ty.display()),
                    },
                    "range": range_of(src, span),
                })
            }
        }
    }

    fn definition(&self, uri: &str, src: &str, position: &Position) -> Value {
        let Some(checked) = self.checked.get(uri) else {
            return Value::Null;
        };
        let offset = offset_at(src, position);
        self.find_target(uri, src, checked, offset)
            .map(|target| location(uri, src, target.decl_span()))
            .unwrap_or(Value::Null)
    }

    /// The entity under the cursor: the shared shape behind definition,
    /// rename, and find-references — they must agree on the target, or
    /// hover says one thing and rename changes another.
    fn find_target(
        &self,
        uri: &str,
        src: &str,
        checked: &Checked,
        offset: usize,
    ) -> Option<Target> {
        let _ = uri;
        // 0. The cursor sits on a *declaration* (the `make`/`takes` binding,
        //    a function header): the decl spans are the resolved_uses
        //    values, so the overlay check comes first — references/rename
        //    started from the declaration must find the same entity a use
        //    click finds.
        let decl_hits: Vec<&Span> = checked
            .resolved_uses
            .values()
            .filter(|decl| decl.start <= offset && offset < decl.end)
            .collect();
        if let Some(decl) = decl_hits.iter().min_by_key(|s| s.end - s.start) {
            return Some(Target::Use(**decl));
        }

        // 1. A use the checker resolved to a binding or a function value:
        //    the honest edge (the whole-run case, single- or multi-word).
        if let Some(decl) = smallest_containing(&checked.resolved_uses, offset)
            .and_then(|use_span| checked.resolved_uses.get(use_span))
        {
            return Some(Target::Use(*decl));
        }


        // 2. A call: the callee-name text names a checked function (LOM's
        //    provenance anchor spans cover the call's name run).
        let callee_hit = checked
            .callee_spans
            .values()
            .filter(|s| s.start <= offset && offset < s.end)
            .min_by_key(|s| s.end - s.start)
            .copied();
        if let Some(callee_span) = callee_hit {
            let name = safe_slice(src, callee_span.start, callee_span.end);
            let wanted = name.trim();
            if let Some(decl) = decl_span_of(checked, wanted) {
                return Some(Target::Callable { decl });
            }
        }

        // 3. Bare word under the cursor matching a callable's name (first-
        //    class function references, header clicks). Single words only:
        //    multi-word runs belong to edges 1 and 2, and guessing run
        //    boundaries here would risk the wrong jump.
        if let Some(word) = word_at(src, offset) {
            if let Some(decl) = decl_span_of(checked, &word) {
                return Some(Target::Callable { decl });
            }
        }
        None
    }

    /// The spans in this document that name the entity `target` names.
    fn spans_of_target(&self, src: &str, checked: &Checked, target: &Target) -> Vec<Span> {
        let decl = target.decl_span();
        // Every resolved use landing on the declaration — binding reads,
        // first-class function values, multi-word runs alike.
        let mut spans: Vec<Span> = checked
            .resolved_uses
            .iter()
            .filter(|(_, use_decl)| *use_decl == &decl)
            .map(|(use_span, _)| *use_span)
            .collect();
        // Callables also collect their call sites: a call whose callee
        // span's text equals the callable's own displayed name (the text
        // the decl range carries). For bindings there is no callable with
        // that declaration span, so this arm contributes nothing.
        let own_name = safe_slice(src, decl.start, decl.end).trim().to_string();
        let is_callable = decl_span_of(checked, &own_name) == Some(decl);
        if is_callable {
            for callee_span in checked.callee_spans.values() {
                let text = safe_slice(src, callee_span.start, callee_span.end);
                if text.trim() == own_name {
                    spans.push(*callee_span);
                }
            }
        }
        // `Span` carries no ordering; the start offset orders and dedups
        // (adjacent duplicates after the key sort are the only repeats).
        spans.sort_by_key(|s| s.start);
        spans.dedup_by_key(|s| s.start);
        spans
    }

    fn references(
        &self,
        uri: &str,
        src: &str,
        position: &Position,
        include_declaration: bool,
    ) -> Value {
        let Some(checked) = self.checked.get(uri) else {
            return Value::Null;
        };
        let offset = offset_at(src, position);
        let Some(target) = self.find_target(uri, src, checked, offset) else {
            return Value::Null;
        };
        let mut locations = Vec::new();
        if include_declaration {
            locations.push(location(uri, src, target.decl_span()));
        }
        for span in self.spans_of_target(src, checked, &target) {
            locations.push(location(uri, src, span));
        }
        Value::Array(locations)
    }

    fn rename(
        &self,
        uri: &str,
        src: &str,
        position: &Position,
        new_name: &str,
    ) -> Result<Value, String> {
        let Some(checked) = self.checked.get(uri) else {
            return Err("this document does not check clean, so rename would guess — fix the errors first.".to_string());
        };
        let offset = offset_at(src, position);
        let Some(target) = self.find_target(uri, src, checked, offset) else {
            return Err(
                "rename follows what the checker resolved — match pattern names have no table yet."
                    .to_string(),
            );
        };
        let normalized: Vec<&str> = new_name.split_whitespace().collect();
        if normalized.is_empty()
            || !normalized
                .iter()
                .all(|w| !w.is_empty() && w.chars().all(|c| c.is_ascii_alphanumeric() || c == '_'))
        {
            return Err(
                "names are word runs: letters, digits and single spaces only (7.0.3).".to_string(),
            );
        }
        let replacement = normalized.join(" ");
        let mut edits = vec![edit_of(uri, src, target.decl_span(), &replacement)];
        for span in self.spans_of_target(src, checked, &target) {
            edits.push(edit_of(uri, src, span, &replacement));
        }
        Ok(json!({ "changes": { uri: edits } }))
    }
}

/// The entity the cursor points at: a resolved *use* (whose declaration
/// stands alone) or a callable (named only through its declaration's text
/// — the spans_of_target funnel reads the name there, uniformly for both
/// call sites and first-class references).
enum Target {
    Use(Span),
    Callable { decl: Span },
}

impl Target {
    fn decl_span(&self) -> Span {
        match self {
            Target::Use(span) => *span,
            Target::Callable { decl, .. } => *decl,
        }
    }
}

fn edit_of(uri: &str, src: &str, span: Span, replacement: &str) -> Value {
    let _ = uri;
    json!({ "range": range_of(src, span), "newText": replacement })
}

/// The smallest span containing the offset.
fn smallest_containing<V>(map: &HashMap<Span, V>, offset: usize) -> Option<&Span> {
    map.keys()
        .filter(|s| s.start <= offset && offset < s.end)
        .min_by_key(|s| s.end - s.start)
}

/// A callable's declaration span by its displayed name — the one index the
/// checker's side tables do not keep, built from the checked items.
fn decl_span_of(checked: &Checked, name: &str) -> Option<Span> {
    for item in &checked.items {
        match item {
            lagom_sema::CheckedItem::Function(f) => {
                if f.name == name {
                    return Some(f.name_span);
                }
            }
            lagom_sema::CheckedItem::Class {
                methods,
                constructions,
                ..
            } => {
                for (method_name, f) in methods {
                    if method_name == name || f.name == name {
                        return Some(f.name_span);
                    }
                }
                for f in constructions {
                    if f.name == name {
                        return Some(f.name_span);
                    }
                }
            }
            lagom_sema::CheckedItem::Test(f) => {
                if f.name == name {
                    return Some(f.name_span);
                }
            }
            _ => {}
        }
    }
    None
}

/// The single word at (or past) the offset.
fn word_at(src: &str, offset: usize) -> Option<String> {
    let bytes = src.as_bytes();
    let is_word = |b: u8| b.is_ascii_alphanumeric() || b == b'_';
    let mut start = offset.min(src.len());
    while start > 0 && is_word(bytes[start - 1]) {
        start -= 1;
    }
    let mut end = offset.min(src.len());
    while end < bytes.len() && is_word(bytes[end]) {
        end += 1;
    }
    if start == end {
        None
    } else {
        Some(src[start..end].to_string())
    }
}

// ---------------------------------------------------------------------------
// Analysis + diagnostic mapping
// ---------------------------------------------------------------------------

/// One analysis pass of a document: the checked program when the front end
/// is clean (`None` otherwise — hover on a broken program shows nothing
/// rather than a type the checker cannot stand behind), and the
/// diagnostics to publish (parse errors, check errors, or warnings).
fn analyze(src: &str) -> (Option<Checked>, Vec<Diagnostic>) {
    let (program, parse_diags) = lagom_parser::parse(src);
    if !parse_diags.is_empty() {
        return (None, parse_diags);
    }
    let checked = lagom_sema::check(&program, src);
    let items = checked.diags.items.clone();
    if items.iter().any(|d| d.severity == Severity::Error) {
        // Errors (with their notes) only — warnings during a failing check
        // are noise, matching the pipeline's own filter.
        return (
            None,
            items
                .into_iter()
                .filter(|d| d.severity == Severity::Error)
                .collect(),
        );
    }
    let warnings: Vec<Diagnostic> = items
        .into_iter()
        .filter(|d| d.severity == Severity::Warning)
        .collect();
    // The rest of the pipeline (HIR → MIR → verify) never fails on clean
    // check except internally — and an internal failure is a document
    // event the editor must see. The first check's side tables stay alive
    // for hover/definition; the verification pass re-runs `check` and
    // hands it to the lowering. Small documents make the duplicate pass
    // invisible; correctness first.
    let verify_check = lagom_sema::check(&program, src);
    let mir = lagom_mir::lower(lagom_hir::lower(verify_check));
    if let Err(message) = lagom_mir::verify(&mir) {
        return (
            None,
            vec![Diagnostic {
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
            }],
        );
    }
    (Some(checked), warnings)
}

fn publish_diagnostics(uri: &str, diags: &[Diagnostic], src: &str) {
    let items: Vec<Value> = diags
        .iter()
        .map(|d| {
            let mut message = d.message.clone();
            if let Some(fix) = &d.fix {
                message.push_str(&format!("\n\nTo fix, write: {fix}"));
            }
            json!({
                "range": range_of(src, d.span),
                "severity": severity_code(d.severity),
                "code": d.code,
                "message": message,
                "source": "Lagom",
            })
        })
        .collect();
    notify(
        "textDocument/publishDiagnostics",
        json!({ "uri": uri, "diagnostics": items }),
    );
}

fn severity_code(s: Severity) -> u8 {
    match s {
        Severity::Error => 1,
        Severity::Warning => 2,
        Severity::Note => 3,
    }
}

// ---------------------------------------------------------------------------
// Position conversion (the only LSP-unit code in the workspace)
// ---------------------------------------------------------------------------

struct Position {
    line: usize,
    /// UTF-16 code units, as LSP speaks.
    character: usize,
}

/// An LSP {start, end} range read into positions.
fn read_range(range: Option<&Value>) -> Option<(Position, Position)> {
    let range = range?.as_object()?;
    let start = Position::from_json(range.get("start"))?;
    let end = Position::from_json(range.get("end"))?;
    Some((start, end))
}

impl Position {
    fn from_json(p: Option<&Value>) -> Option<Position> {
        let p = p?.as_object()?;
        Some(Position {
            line: p.get("line")?.as_u64()? as usize,
            character: p.get("character")?.as_u64()? as usize,
        })
    }
}

/// Byte offset of an LSP position in this source.
fn offset_at(src: &str, p: &Position) -> usize {
    let mut line = 0usize;
    let mut offset = 0usize;
    for text in src.split_inclusive('\n') {
        if line == p.line {
            let mut utf16 = 0usize;
            for ch in text.chars() {
                if utf16 >= p.character {
                    break;
                }
                utf16 += ch.len_utf16();
                offset += ch.len_utf8();
            }
            return offset;
        }
        offset += text.len();
        line += 1;
    }
    offset.min(src.len())
}

/// The LSP range of a compiler span: 0-based lines, UTF-16 columns.
fn range_of(src: &str, span: Span) -> Value {
    json!({
        "start": point_of(src, span.start),
        "end": point_of(src, span.end),
    })
}

/// The LSP point of a byte offset.
fn point_of(src: &str, offset: usize) -> Value {
    let mut o = offset.min(src.len());
    while o > 0 && !src.is_char_boundary(o) {
        o -= 1;
    }
    let mut line = 0usize;
    let mut utf16 = 0usize;
    for (i, ch) in src.char_indices() {
        if i >= o {
            break;
        }
        if ch == '\n' {
            line += 1;
            utf16 = 0;
        } else {
            utf16 += ch.len_utf16();
        }
    }
    json!({ "line": line, "character": utf16 })
}

fn location(uri: &str, src: &str, span: Span) -> Value {
    json!({ "uri": uri, "range": range_of(src, span) })
}

fn safe_slice(src: &str, start: usize, end: usize) -> String {
    let clamp = |mut o: usize| {
        while o > 0 && !src.is_char_boundary(o) {
            o -= 1;
        }
        o
    };
    let start = clamp(start);
    let end = clamp(end).max(start);
    src[start..end].to_string()
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod analysis_tests {
    use super::*;

    #[test]
    fn analyze_produces_side_tables_for_a_clean_program() {
        // The block form of `if` takes no `then` (`then` is the one-line
        // form), and the body indents under the `if`.
        let src = "make top score equal to 10\n\nif top score is greater than 5\n    say \"high\"\n";
        let (checked, diags) = analyze(src);
        let some = checked.expect("a clean program yields its side tables");
        assert!(diags.is_empty(), "clean: {diags:?}");
        assert!(
            !some.node_types.is_empty(),
            "the span-keyed type table is populated"
        );
        assert!(
            some.resolved_uses
                .values()
                .any(|decl| src[decl.start..].starts_with("top score")),
            "resolved_uses carries the run → decl edge; got {:?}",
            some.resolved_uses
        );
    }

    #[test]
    fn a_ranged_change_splices_its_span() {
        // Insert, replace, delete, and a multi-line edit, in one batch —
        // the server reanalyses once at the end.
        let mut server = Server::new();
        server.open(
            "file:///t.lagom",
            "make a equal to 1\nmake b equal to 2\n",
        );
        // The `2` on line 1 is character 16; the `1` on line 0 is 16 too.
        let changes = vec![
            serde_json::json!({
                "range": {"start": {"line": 1, "character": 16}, "end": {"line": 1, "character": 17}},
                "text": "20"
            }),
            serde_json::json!({
                "range": {"start": {"line": 0, "character": 16}, "end": {"line": 0, "character": 17}},
                "text": ""
            }),
        ];
        server.apply_changes("file:///t.lagom", &changes);
        let text = server.docs.get("file:///t.lagom").expect("document").as_str();
        assert_eq!(text, "make a equal to \nmake b equal to 20\n", "replace 2 with 20, delete the 1");
    }

    #[test]
    fn a_change_without_range_replaces_the_document() {
        let mut server = Server::new();
        server.open("file:///t.lagom", "say \"hi\"\n");
        server.apply_changes(
            "file:///t.lagom",
            &[serde_json::json!({ "text": "say \"bye\"\n" })],
        );
        assert_eq!(
            server.docs.get("file:///t.lagom").map(String::as_str),
            Some("say \"bye\"\n")
        );
    }

    #[test]
    fn a_change_for_an_unknown_document_starts_fresh() {
        let mut server = Server::new();
        server.apply_changes(
            "file:///never.lagom",
            &[serde_json::json!({
                "range": {"start": {"line": 0, "character": 0}, "end": {"line": 0, "character": 0}},
                "text": "say \"x\"\n"
            })],
        );
        assert_eq!(
            server.docs.get("file:///never.lagom").map(String::as_str),
            Some("say \"x\"\n")
        );
    }

    #[test]
    fn offset_at_reads_lsp_positions() {
        let src = "ab\nmake x equal to 1\n";
        // line 1 starts at offset 3; `character: 7` walks `mk`… to the
        // space after `x` (offset 10, characters and bytes agree in ASCII).
        assert_eq!(offset_at(src, &Position { line: 1, character: 7 }), 10);
        assert_eq!(offset_at(src, &Position { line: 0, character: 1 }), 1);
        // A line past the end clamps to the text length.
        assert_eq!(offset_at("x", &Position { line: 5, character: 0 }), 1);
    }
}

// ---------------------------------------------------------------------------
// Framing
// ---------------------------------------------------------------------------

fn read_message(reader: &mut impl BufRead) -> Option<String> {
    let mut length = None;
    loop {
        let mut line = String::new();
        let n = reader.read_line(&mut line).ok()?;
        if n == 0 {
            return None;
        }
        let line = line.trim_end();
        if line.is_empty() {
            break;
        }
        if let Some(value) = line.strip_prefix("Content-Length:") {
            length = value.trim().parse::<usize>().ok();
        }
    }
    let length = length?;
    let mut buf = vec![0u8; length];
    reader.read_exact(&mut buf).ok()?;
    Some(String::from_utf8_lossy(&buf).into_owned())
}

fn write_message(body: &Value) {
    let Ok(text) = serde_json::to_string(body) else {
        return;
    };
    let mut out = std::io::stdout().lock();
    let _ = write!(out, "Content-Length: {}\r\n\r\n{}", text.len(), text);
    let _ = out.flush();
    // stdout stays exclusively the LSP channel; human chatter is stderr's.
}

fn respond(id: Option<Value>, result: Option<Value>, error: Option<Value>) {
    let Some(id) = id else { return };
    let mut body = json!({ "jsonrpc": "2.0", "id": id });
    if let Some(result) = result {
        body.as_object_mut()
            .unwrap()
            .insert("result".into(), result);
    }
    if let Some(error) = error {
        body.as_object_mut().unwrap().insert("error".into(), error);
    }
    write_message(&body);
}

fn notify(method: &str, params: Value) {
    let body = json!({ "jsonrpc": "2.0", "method": method, "params": params });
    write_message(&body);
}


