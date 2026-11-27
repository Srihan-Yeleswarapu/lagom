//! The `lagom lsp` end-to-end contract, spoken over real stdio: the framing
//! (`Content-Length`) matters as much as the JSON, so the test drives the
//! binary with raw bytes exactly like the editor client does — initialize,
//! open a document, hover the multi-word name run, jump to its definition,
//! publish an error document, shut down.

use std::io::{BufRead, BufReader, Read as _, Write as _};
use std::process::{Child, Command, Stdio};

const CLEAN_PROGRAM: &str = "make top score equal to 10\n\nif top score is greater than 5\n    say \"high\"\n";

const RENAME_PROGRAM: &str = "\nfunction double\n    takes number called n\n    gives back n times 2\n\nmake changing tally equal to 0\nrepeat 3 times using i\n    set tally to double i\nsay tally\n";

const BROKEN_PROGRAM: &str = "make changing total equal to 0\nsays nothing\n";

struct Session {
    child: Child,
    reader: BufReader<std::process::ChildStdout>,
    next_id: i64,
}

impl Session {
    fn start() -> Session {
        let mut child = Command::new(env!("CARGO_BIN_EXE_lagom"))
            .arg("lsp")
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::null())
            .spawn()
            .expect("spawn the server");
        let stdout = child.stdout.take().expect("server stdout");
        Session {
            child,
            reader: BufReader::new(stdout),
            next_id: 1,
        }
    }

    fn write(&mut self, body: &str) {
        let stdin = self.child.stdin.as_mut().expect("server stdin");
        let payload = format!("Content-Length: {}\r\n\r\n{}", body.len(), body);
        stdin.write_all(payload.as_bytes()).expect("write to the server");
        stdin.flush().expect("flush");
    }

    fn request(&mut self, method: &str, params: &serde_json::Value) {
        let request = serde_json::json!({
            "jsonrpc": "2.0", "id": self.next_id, "method": method, "params": params
        });
        self.next_id += 1;
        self.write(&request.to_string());
    }

    fn notify(&mut self, method: &str, params: &serde_json::Value) {
        let request = serde_json::json!({ "jsonrpc": "2.0", "method": method, "params": params });
        self.write(&request.to_string());
    }

    fn read_message(&mut self) -> serde_json::Value {
        let mut length = None;
        let mut line = String::new();
        loop {
            line.clear();
            let n = self.reader.read_line(&mut line).expect("read headers");
            assert!(n > 0, "the server closed the pipe mid-session");
            let trimmed = line.trim_end();
            if trimmed.is_empty() {
                break;
            }
            if let Some(value) = trimmed.strip_prefix("Content-Length: ") {
                length = value.trim().parse::<usize>().ok();
            }
        }
        let mut body = vec![0u8; length.expect("a Content-Length header")];
        self.reader.read_exact(&mut body).expect("read the body");
        serde_json::from_slice(&body).expect("the server speaks JSON")
    }

    /// Read notifications (keeping the publishDiagnostics params) until the
    /// response to the request just sent arrives.
    fn response_with_publishes(
        &mut self,
        publishes: &mut Vec<serde_json::Value>,
    ) -> serde_json::Value {
        let wanted = self.next_id - 1;
        loop {
            let message = self.read_message();
            if message["method"] == "textDocument/publishDiagnostics" {
                publishes.push(message["params"].clone());
                continue;
            }
            if message.get("id").and_then(serde_json::Value::as_i64) == Some(wanted) {
                return message;
            }
        }
    }

    fn initialize(&mut self) {
        self.request("initialize", &serde_json::json!({"capabilities": {}}));
        let response = self.response_with_publishes(&mut Vec::new());
        assert!(
            response["result"]["capabilities"]["hoverProvider"].as_bool() == Some(true),
            "hover capability declared: {response}"
        );
        self.notify("initialized", &serde_json::json!({}));
    }
}

#[test]
fn incremental_changes_drive_reanalysis_and_the_publish_stops_erroring() {
    let mut session = Session::start();
    session.initialize();
    // The document opens broken: `says nothing` (no such callable).
    session.notify(
        "textDocument/didOpen",
        &serde_json::json!({
            "textDocument": {
                "uri": "file:///incr.lagom",
                "languageId": "lagom",
                "version": 1,
                "text": "says nothing\n"
            }
        }),
    );
    session.request("textDocument/hover", &serde_json::json!({
        "textDocument": {"uri": "file:///incr.lagom"},
        "position": {"line": 0, "character": 2}
    }));
    let mut erroring_publishes = Vec::new();
    let hover = session.response_with_publishes(&mut erroring_publishes);
    assert!(hover["result"] == serde_json::Value::Null, "no types on a broken doc");
    assert!(
        erroring_publishes.iter().any(|p| !p["diagnostics"].as_array().expect("items").is_empty()),
        "the broken document publishes errors: {erroring_publishes:?}"
    );

    // One *ranged* edit fixes the call: `says` → `say`. The editor sends
    // only the range; the server reanalyses and publishes an empty list.
    session.notify(
        "textDocument/didChange",
        &serde_json::json!({
            "textDocument": {"uri": "file:///incr.lagom", "version": 2},
            "contentChanges": [{
                // Replace the whole word `says` (chars 0..4) with `say` —
                // the exercise-the-replacement-path edit.
                "range": {"start": {"line": 0, "character": 0}, "end": {"line": 0, "character": 4}},
                "text": "say"
            }]
        }),
    );
    session.request("textDocument/hover", &serde_json::json!({
        "textDocument": {"uri": "file:///incr.lagom"},
        "position": {"line": 0, "character": 2}
    }));
    let mut clean_publishes = Vec::new();
    let _hover = session.response_with_publishes(&mut clean_publishes);
    let last = clean_publishes.last().expect("the clean publish");
    assert_eq!(
        last["diagnostics"].as_array().expect("items").len(),
        0,
        // The empty list is the *proof the edit landed*: only a splice into
        // exactly `says` → `say` makes the document check clean.
        "the fixed document publishes an empty list: {last}"
    );
    shutdown(&mut session);
}

fn shutdown(session: &mut Session) {
    session.request("shutdown", &serde_json::Value::Null);
    let response = session.response_with_publishes(&mut Vec::new());
    assert!(
        response["result"] == serde_json::Value::Null,
        "shutdown answers null: {response}"
    );
    session.notify("exit", &serde_json::Value::Null);
    session.child.wait().expect("the server exits cleanly");
}

#[test]
fn rename_rewrites_the_function_and_every_call() {
    let mut session = Session::start();
    session.initialize();
    session.notify(
        "textDocument/didOpen",
        &serde_json::json!({
            "textDocument": {
                "uri": "file:///rename.lagom",
                "languageId": "lagom",
                "version": 1,
                "text": RENAME_PROGRAM
            }
        }),
    );
    // The leading blank line puts the header on line 1 and the call
    // `double i` on line 7; character 19 sits inside `double`.
    session.request(
        "textDocument/rename",
        &serde_json::json!({
            "textDocument": {"uri": "file:///rename.lagom"},
            "position": {"line": 7, "character": 19},
            "newName": "triple"
        }),
    );
    let mut publishes = Vec::new();
    let response = session.response_with_publishes(&mut publishes);
    let changes = response["result"]["changes"]
        .as_object()
        .expect("changes map");
    let edits = changes
        .values()
        .next()
        .and_then(|list| list.as_array())
        .expect("edits");
    let lines: Vec<usize> = edits
        .iter()
        .map(|edit| edit["range"]["start"]["line"].as_i64().expect("line") as usize)
        .collect();
    assert_eq!(edits.len(), 2, "the declaration + the call: {edits:?}");
    // `say tally` is a different name — no edit there.
    assert!(lines.contains(&1), "the header line: {lines:?}");
    assert!(lines.contains(&7), "the call line: {lines:?}");
    for edit in edits {
        assert_eq!(
            edit["newText"].as_str(),
            Some("triple"),
            "the replacement carries the new name"
        );
    }
    // The honest bar for a bad name: the server refuses with a message.
    session.request(
        "textDocument/rename",
        &serde_json::json!({
            "textDocument": {"uri": "file:///rename.lagom"},
            "position": {"line": 7, "character": 19},
            "newName": "has, punctuation"
        }),
    );
    let refused = session.response_with_publishes(&mut publishes);
    assert!(
        refused["error"]["message"]
            .as_str()
            .map(|m| m.contains("word runs"))
            .unwrap_or(false),
        "a punctuation rename is refused with the rule: {refused}"
    );
    shutdown(&mut session);
}

#[test]
fn references_collect_uses_without_confusing_the_say() {
    let mut session = Session::start();
    session.initialize();
    session.notify(
        "textDocument/didOpen",
        &serde_json::json!({
            "textDocument": {
                "uri": "file:///refs.lagom",
                "languageId": "lagom",
                "version": 1,
                "text": "make changing top score equal to 0\nincrease top score by 3\nsay top score\n"
            }
        }),
    );
    session.request(
        "textDocument/references",
        &serde_json::json!({
            "textDocument": {"uri": "file:///refs.lagom"},
            // Inside the multi-word name `top score` on the make line.
            "position": {"line": 0, "character": 15},
            "context": {"includeDeclaration": true}
        }),
    );
    let mut publishes = Vec::new();
    let binding = session.response_with_publishes(&mut publishes);
    let locations = binding["result"]
        .as_array()
        .expect("locations");
    let lines: Vec<usize> = locations
        .iter()
        .map(|location| location["range"]["start"]["line"].as_i64().expect("line") as usize)
        .collect();
    assert_eq!(lines, vec![0, 1, 2], "decl + both uses: {lines:?}");
    shutdown(&mut session);
}

#[test]
fn hover_types_the_multi_word_run_and_definition_lands_on_the_binding() {
    // Cursor inside `top score` in the `if` line (line 2, character 9).
    let mut session = Session::start();
    session.initialize();
    session.notify(
        "textDocument/didOpen",
        &serde_json::json!({
            "textDocument": {
                "uri": "file:///hover.lagom",
                "languageId": "lagom",
                "version": 1,
                "text": CLEAN_PROGRAM
            }
        }),
    );
    session.request(
        "textDocument/hover",
        &serde_json::json!({
            "textDocument": {"uri": "file:///hover.lagom"},
            "position": {"line": 2, "character": 9}
        }),
    );
    let hover = session.response_with_publishes(&mut Vec::new())["result"].clone();
    let value = hover["contents"]["value"]
        .as_str()
        .expect("hover markdown")
        .to_string();
    assert!(
        value.contains("top score") && value.contains("number"),
        "hover types the run with the inferred type: {value}"
    );
    session.request(
        "textDocument/definition",
        &serde_json::json!({
            "textDocument": {"uri": "file:///hover.lagom"},
            "position": {"line": 2, "character": 9}
        }),
    );
    let definition = session.response_with_publishes(&mut Vec::new())["result"].clone();
    let line = definition["range"]["start"]["line"]
        .as_i64()
        .expect("definition range");
    assert_eq!(
        CLEAN_PROGRAM.split('\n').nth(line as usize),
        Some("make top score equal to 10"),
        "goto-def lands on the `make` that binds top score"
    );
    shutdown(&mut session);
}

#[test]
fn an_erroring_document_publishes_the_teaching_diagnostics() {
    let mut session = Session::start();
    session.initialize();
    session.notify(
        "textDocument/didOpen",
        &serde_json::json!({
            "textDocument": {
                "uri": "file:///broken.lagom",
                "languageId": "lagom",
                "version": 1,
                "text": BROKEN_PROGRAM
            }
        }),
    );
    session.request("textDocument/hover", &serde_json::json!({
        "textDocument": {"uri": "file:///broken.lagom"},
        "position": {"line": 1, "character": 4}
    }));
    let mut publishes = Vec::new();
    let response = session.response_with_publishes(&mut publishes);
    // Hover on a broken program answers null (no types to stand behind).
    assert!(response["result"] == serde_json::Value::Null, "{response}");
    assert!(
        !publishes.is_empty(),
        "an erroring document must publish on open"
    );
    let items = publishes[publishes.len() - 1]["diagnostics"]
        .as_array()
        .expect("diagnostics array");
    assert!(
        items.iter().any(|d| d["severity"].as_i64() == Some(1)),
        "an error arrives as severity 1: {items:?}"
    );
    assert!(
        items.iter().all(|d| d["source"].as_str() == Some("Lagom")),
        "the source names the compiler: {items:?}"
    );
    shutdown(&mut session);
}
