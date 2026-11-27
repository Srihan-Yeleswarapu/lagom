// The Lagom extension: the language server (`lagom lsp`) drives diagnostics,
// hover, and go-to-definition, with layer-aware completions on the client.
// The client here is hand-rolled (one file, no dependencies): a spawned
// process speaking the JSON-RPC framing the server implements — the same
// contract `scripts/lsp-smoke.py` pins and the Rust integration test keeps
// honest. If the binary is missing or the server cannot start, the
// extension degrades to `lagom check --json` diagnostics only, so a
// textmate-only install still lints.

const vscode = require("vscode");
const child_process = require("child_process");
const path = require("path");
const completions = require("./completions");

// The fallback diagnostics (check --json, executed per document).
const LEGACY_PENDING = new Map();
let LEGACY_COLLECTION = null;

// The live server and its consumer.
let client = null;
let serverOutput = null;

function activate(context) {
  serverOutput = vscode.window.createOutputChannel("Lagom");
  context.subscriptions.push(
    serverOutput,
    new vscode.Disposable(() => stopClient())
  );

  // The server wins when it starts; otherwise the fallback stands in.
  tryStartClient(context);

  context.subscriptions.push(
    vscode.languages.registerHoverProvider("lagom", {
      provideHover(document, position) {
        if (!client) return fallbackHover(document, position);
        return client
          .request("textDocument/hover", {
            textDocument: { uri: document.uri.toString() },
            position: asLspPosition(position),
          })
          .then((hover) => (hover ? toVscodeHover(hover) : null))
          .catch(() => fallbackHover(document, position));
      },
    }),
    vscode.languages.registerDefinitionProvider("lagom", {
      provideDefinition(document, position) {
        if (!client) return null;
        return client
          .request("textDocument/definition", {
            textDocument: { uri: document.uri.toString() },
            position: asLspPosition(position),
          })
          .then((def) => {
            if (!def || !def.uri) return [];
            const target = vscode.Uri.parse(def.uri);
            return [
              new vscode.Location(
                target,
                new vscode.Range(def.range.start.line, def.range.start.character, def.range.end.line, def.range.end.character)
              ),
            ];
          })
          .catch(() => null);
      },
    }),
    vscode.languages.registerRenameProvider("lagom", {
      // The server validates the new name (word runs only, 7.0.3) and
      // answers an error message for the editor's rename dialog.
      provideRenameEdits(document, position, newName) {
        if (!client) throw new Error("the Lagom language server is not running.");
        return client
          .request("textDocument/rename", {
            textDocument: { uri: document.uri.toString() },
            position: asLspPosition(position),
            newName,
          })
          .then(asWorkspaceEdit)
          .catch((error) => {
            throw error;
          });
      },
    }),
    vscode.languages.registerReferenceProvider("lagom", {
      provideReferences(document, position, options) {
        if (!client) return null;
        return client
          .request("textDocument/references", {
            textDocument: { uri: document.uri.toString() },
            position: asLspPosition(position),
            context: { includeDeclaration: options.includeDeclaration },
          })
          .then((locations) => (locations ?? []).map(toVscodeLocation))
          .catch(() => null);
      },
    }),
    vscode.languages.registerCompletionItemProvider(
      "lagom",
      {
        provideCompletionItems() {
          // The student layer, offered as words: built-ins first (they are
          // the calls a beginner makes), then the type surface, then the
          // keyword phrases. Never anything the student layer cannot say.
          return [
            ...completions.builtins.map((word) => item(word, vscode.CompletionItemKind.Function)),
            ...completions.types.map((word) => item(word, vscode.CompletionItemKind.TypeParameter)),
            ...completions.keywords.map((word) => item(word, vscode.CompletionItemKind.Keyword)),
          ];
        },
      },
      ...completions.triggerCharacters
    )
  );

  function item(label, kind) {
    const completion = new vscode.CompletionItem(label, kind);
    completion.detail = "Lagom";
    return completion;
  }

  // ---- the language-server client --------------------------------------

  function tryStartClient(context) {
    const binary = vscode.workspace
      .getConfiguration("lagom")
      .get("check.path", "lagom");
    const child = child_process.spawn(binary, ["lsp"], {
      cwd: workspaceRoot() ?? undefined,
    });
    let ready = false;
    client = {
      request(method, params) {
        return new Promise((resolve, reject) => {
          if (!ready) return reject(new Error("server not ready"));
          const id = client.nextId++;
          client.pending.set(id, { resolve, reject });
          client.send({ jsonrpc: "2.0", id, method, params });
          // Requests the server cannot answer must not hang the editor.
          setTimeout(() => {
            if (client.pending.has(id)) {
              client.pending.delete(id);
              reject(new Error(`timeout: ${method}`));
            }
          }, 10_000);
        });
      },
      notify(method, params) {
        client.send({ jsonrpc: "2.0", method, params });
      },
      send(message) {},
      nextId: 1,
      pending: new Map(),
      child,
    };
    client.send = (message) => {
      const body = Buffer.from(JSON.stringify(message), "utf8");
      client.child.stdin.write(
        Buffer.concat([
          Buffer.from(`Content-Length: ${body.length}\r\n\r\n`, "utf8"),
          body,
        ])
      );
    };
    client.child.stdout.on("data", onServerData);
    client.child.on("error", onServerFailure);
    client.child.on("exit", onServerFailure);

    // Bootstrap: initialize → initialized, then open every already-open
    // document so the first diagnostics land without an explicit save.
    client
      .request("initialize", { capabilities: {} })
      .then(() => {
        ready = true;
        client.notify("initialized", {});
        for (const doc of vscode.workspace.textDocuments) {
          if (doc.languageId === "lagom") client.notifyDidOpen(doc);
        }
      })
      .catch(() => onServerFailure());

    const lastSent = new Map();

    client.notifyDidOpen = (doc) => {
      synchronizedDocs.set(doc.uri.toString(), doc.version);
      lastSent.set(doc.uri.toString(), doc.getText());
      client.notify("textDocument/didOpen", {
        textDocument: {
          uri: doc.uri.toString(),
          languageId: doc.languageId,
          version: doc.version,
          text: doc.getText(),
        },
      });
    };

    // The collection lives server-side from here on.
    context.subscriptions.push(
      vscode.workspace.onDidOpenTextDocument((doc) => {
        if (doc.languageId !== "lagom") return;
        if (doc.uri.scheme !== "file") return; // untitled: the fallback owns it
        if (client) client.notifyDidOpen(doc);
        else scheduleFallbackCheck(doc);
      }),
      vscode.workspace.onDidChangeTextDocument((event) => {
        const doc = event.document;
        if (doc.languageId !== "lagom" || doc.uri.scheme !== "file") return;
        const uri = doc.uri.toString();
        if (client && synchronizedDocs.has(uri)) {
          synchronizedDocs.set(uri, doc.version);
          client.notify("textDocument/didChange", {
            textDocument: { uri, version: doc.version },
            // Incremental sync with the smallest edit: common prefix and
            // suffix give the single ranged change (everything typed is a
            // splice, so the server re-parses only the final batch).
            contentChanges: [diffChange(lastSent.get(uri) ?? "", doc.getText())],
          });
          lastSent.set(uri, doc.getText());
        } else {
          scheduleFallbackCheck(doc);
        }
      }),
      vscode.workspace.onDidCloseTextDocument((doc) => {
        synchronizedDocs.delete(doc.uri.toString());
        lastSent.delete(doc.uri.toString());
        if (client) {
          client.notify("textDocument/didClose", {
            textDocument: { uri: doc.uri.toString() },
          });
        }
      })
    );

    const synchronizedDocs = new Map();

    function onServerData(chunk) {
      // The framing is header-line / blank / body; accumulate and split.
      buffer = Buffer.concat([buffer, chunk]);
      for (;;) {
        const split = buffer.indexOf("\r\n\r\n");
        if (split < 0) break;
        const head = buffer.slice(0, split).toString("utf8");
        let length = null;
        for (const line of head.split("\r\n")) {
          if (line.toLowerCase().startsWith("content-length:")) {
            length = Number(line.split(":")[1].trim());
          }
        }
        if (length === null || buffer.length < split + 4 + length) break;
        const body = buffer
          .slice(split + 4, split + 4 + length)
          .toString("utf8");
        buffer = buffer.slice(split + 4 + length);
        let message;
        try {
          message = JSON.parse(body);
        } catch {
          continue;
        }
        if (message.method === "textDocument/publishDiagnostics") {
          publishServerDiagnostics(message.params);
        } else if (message.id !== undefined && client.pending.has(message.id)) {
          const { resolve, reject } = client.pending.get(message.id);
          client.pending.delete(message.id);
          if (message.error) {
            reject(new Error(message.error.message));
          } else {
            resolve(message.result);
          }
        }
      }
    }
    let buffer = Buffer.alloc(0);

    // The one ranged change from old to new text: shared prefix and suffix
    // bound the edit; the middle is the whole difference. Positions are
    // VS Code positions (0-based, UTF-16) — LSP's own units — so the
    // conversion eats the code units, not the bytes.
    function diffChange(oldText, newText) {
      if (oldText === newText) return { text: "" };
      let start = 0;
      const max = Math.min(oldText.length, newText.length);
      while (start < max && oldText[start] === newText[start]) start += 1;
      let oldEnd = oldText.length;
      let newEnd = newText.length;
      while (oldEnd > start && newEnd > start && oldText[oldEnd - 1] === newText[newEnd - 1]) {
        oldEnd -= 1;
        newEnd -= 1;
      }
      return {
        range: {
          start: docPositions(oldText, start),
          end: docPositions(oldText, oldEnd),
        },
        text: newText.slice(start, newEnd),
      };
    }

    function docPositions(text, index) {
      let line = 0;
      let character = 0;
      for (let i = 0; i < index; i += 1) {
        if (text[i] === "\n") {
          line += 1;
          character = 0;
        } else {
          character += 1;
        }
      }
      return { line, character };
    }

    function onServerFailure(error) {
      if (client === null) return;
      const stopped = client;
      client = null;
      try {
        stopped.child.kill();
      } catch {
        // already gone
      }
      serverOutput.appendLine(
        `[${new Date().toISOString()}] the language server stopped` +
          (error?.message ? `: ${error.message}` : "") +
          " — falling back to `lagom check --json` diagnostics."
      );
      // Undo the stop: the exit race (kill → exit event) must not log twice.
      return;
    }

    function publishServerDiagnostics(params) {
      if (!params || !params.uri) return;
      const collection = diagnosticsCollection();
      const diagnostics = [];
      for (const d of params.diagnostics ?? []) {
        const item = new vscode.Diagnostic(
          new vscode.Range(
            d.range.start.line,
            d.range.start.character,
            d.range.end.line,
            d.range.end.character
          ),
          d.message,
          d.severity === 2
            ? vscode.DiagnosticSeverity.Warning
            : d.severity === 3
            ? vscode.DiagnosticSeverity.Information
            : vscode.DiagnosticSeverity.Error
        );
        item.code = d.code;
        item.source = "Lagom";
        diagnostics.push(item);
      }
      collection.set(vscode.Uri.parse(params.uri), diagnostics);
    }
  }

  function stopClient() {
    if (!client) return;
    try {
      client.child.kill();
    } catch {
      // already gone
    }
    client = null;
  }

  function asLspPosition(position) {
    // VS Code positions are 0-based UTF-16 — LSP's own units.
    return { line: position.line, character: position.character };
  }

  function asWorkspaceEdit(changes) {
    if (!changes || !changes.changes) return null;
    const edit = new vscode.WorkspaceEdit();
    for (const [uri, edits] of Object.entries(changes.changes)) {
      for (const change of edits) {
        edit.replace(
          vscode.Uri.parse(uri),
          new vscode.Range(
            change.range.start.line,
            change.range.start.character,
            change.range.end.line,
            change.range.end.character
          ),
          change.newText
        );
      }
    }
    return edit;
  }

  function toVscodeLocation(location) {
    return new vscode.Location(
      vscode.Uri.parse(location.uri),
      new vscode.Range(
        location.range.start.line,
        location.range.start.character,
        location.range.end.line,
        location.range.end.character
      )
    );
  }

  function toVscodeHover(hover) {
    return new vscode.Hover(
      new vscode.MarkdownString(hover.contents.value),
      new vscode.Range(
        hover.range.start.line,
        hover.range.start.character,
        hover.range.end.line,
        hover.range.end.character
      )
    );
  }
}

function deactivate() {
  if (LEGACY_COLLECTION) {
    LEGACY_COLLECTION.dispose();
    LEGACY_COLLECTION = null;
  }
}

// ---------------------------------------------------------------------------
// The diagnostics collection (one, shared by server and fallback)
// ---------------------------------------------------------------------------

function diagnosticsCollection() {
  if (!LEGACY_COLLECTION) {
    LEGACY_COLLECTION = vscode.languages.createDiagnosticCollection("Lagom");
  }
  return LEGACY_COLLECTION;
}

function fallbackHover(document, position) {
  const preview = document.getText(document.getWordRangeAtPosition(position));
  return preview
    ? new vscode.Hover(`\`${preview}\` — build the compiler for full types (\`cargo install --path crates/lagom_cli\`).`)
    : null;
}

function scheduleFallbackCheck(document) {
  const delay = vscode.workspace
    .getConfiguration("lagom")
    .get("check.debounce", 300);
  clearPending(document);
  LEGACY_PENDING.set(
    document.uri.toString(),
    setTimeout(() => {
      LEGACY_PENDING.delete(document.uri.toString());
      legacyCheck(document);
    }, delay)
  );
}

function clearPending(document) {
  const old = LEGACY_PENDING.get(document.uri.toString());
  if (old) clearTimeout(old);
  LEGACY_PENDING.delete(document.uri.toString());
}

function legacyCheck(document) {
  if (
    document.languageId !== "lagom" ||
    document.uri.scheme !== "file" ||
    !vscode.workspace.getConfiguration("lagom").get("check.enabled", true)
  ) {
    return;
  }
  const binary = vscode.workspace
    .getConfiguration("lagom")
    .get("check.path", "lagom");
  const cwd =
    vscode.workspace.getWorkspaceFolder(document.uri)?.uri.fsPath ??
    path.dirname(document.uri.fsPath);
  child_process.execFile(
    binary,
    ["check", document.uri.fsPath, "--json"],
    { cwd, maxBuffer: 10 * 1024 * 1024 },
    (error, stdout, stderr) => {
      if (error && error.code === "ENOENT") {
        serverOutput?.appendLine(
          `[${new Date().toISOString()}] the \`lagom\` binary was not found at "${binary}". ` +
            "Install the compiler, or set `lagom.check.path` to its full path."
        );
        serverOutput?.show(true);
        return;
      }
      let report;
      try {
        report = JSON.parse(stdout);
      } catch (parseError) {
        serverOutput?.appendLine(
          `[${new Date().toISOString()}] could not parse \`lagom check --json\` output: ${parseError}` +
            (stderr ? `\n${stderr}` : "")
        );
        return;
      }
      publishLegacy(document, report);
    }
  );
}

function publishLegacy(document, report) {
  const diagnostics = [];
  for (const d of report.diagnostics) {
    const range = document.isValidPosition(
      new vscode.Position(Math.max(0, d.endLine - 1), Math.max(0, d.endColumn - 1))
    )
      ? new vscode.Range(
          Math.max(0, d.line - 1),
          Math.max(0, d.column - 1),
          Math.max(0, d.endLine - 1),
          Math.max(0, d.endColumn - 1)
        )
      : // An empty document or a span past its end: anchor at the top.
        new vscode.Range(0, 0, 0, 1);
    const item = new vscode.Diagnostic(
      range,
      d.fix ? `${d.message}\n\nTo fix, write: ${d.fix}` : d.message,
      d.severity === "warning"
        ? vscode.DiagnosticSeverity.Warning
        : d.severity === "note"
        ? vscode.DiagnosticSeverity.Information
        : vscode.DiagnosticSeverity.Error
    );
    item.code = d.code;
    item.source = "Lagom";
    diagnostics.push(item);
  }
  diagnosticsCollection().set(document.uri, diagnostics);
}

module.exports = { activate, deactivate };
