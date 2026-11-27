#!/usr/bin/env python3
"""A scripted stdio session against `lagom lsp` — the honest smoke test.

initialize → didOpen (→ publishDiagnostics) → hover → definition → shutdown →
exit, all in one stdio session; the server exits on `exit`, so the session is
read to completion and every id-addressed message is checked by hand. Run it
after building the release CLI: `python3 scripts/lsp-smoke.py`."""

import json
import subprocess
import sys

BINARY = sys.argv[1] if len(sys.argv) > 1 else "target/release/lagom"

# The block form of `if` takes no `then` (then is the one-line form), and
# the body indents under the `if`.
PROGRAM = '''make top score equal to 10

if top score is greater than 5
    say "high"
'''

FAILURES = []


def expect(condition, what):
    if condition:
        print(f"  ok: {what}")
    else:
        FAILURES.append(what)
        print(f"  FAIL: {what}")


def frame(message):
    body = json.dumps(message).encode()
    return b"Content-Length: " + str(len(body)).encode() + b"\r\n\r\n" + body


def request(id, method, params=None):
    message = {"jsonrpc": "2.0", "id": id, "method": method}
    if params is not None:
        message["params"] = params
    return message


def parse(stdout):
    data = stdout
    while b"\r\n\r\n" in data:
        head, data = data.split(b"\r\n\r\n", 1)
        length = None
        for header_line in head.split(b"\r\n"):
            if header_line.lower().startswith(b"content-length:"):
                length = int(header_line.split(b":")[1])
        if length is None or len(data) < length:
            break
        body, data = data[:length], data[length:]
        yield json.loads(body)


def main():
    uri = "file:///smoke/main.lagom"
    messages = [
        frame(request(1, "initialize", {"capabilities": {}})),
        frame({"jsonrpc": "2.0", "method": "initialized", "params": {}}),
        frame(request(2, "textDocument/didOpen", {
            "textDocument": {"uri": uri, "languageId": "lagom", "version": 1,
                             "text": PROGRAM},
        })),
        # Point inside the multi-word name `top score` in the `if` line.
        frame(request(3, "textDocument/hover", {
            "textDocument": {"uri": uri},
            "position": {"line": 2, "character": 9},
        })),
        frame(request(4, "textDocument/definition", {
            "textDocument": {"uri": uri},
            "position": {"line": 2, "character": 9},
        })),
        frame(request(5, "shutdown")),
        frame({"jsonrpc": "2.0", "method": "exit"}),
    ]
    proc = subprocess.run(
        [BINARY, "lsp"],
        input=b"".join(messages),
        capture_output=True,
        timeout=30,
    )
    published = None
    hover = None
    definition = None
    init = None
    for message in parse(proc.stdout):
        id = message.get("id")
        if id == 1:
            init = message
        elif "method" in message and message["method"] == "textDocument/publishDiagnostics":
            published = message["params"]
        elif id == 3:
            hover = message["result"]
        elif id == 4:
            definition = message["result"]

    print("- initialize")
    expect(init is not None and "result" in init, "initialize answers")
    if init:
        expect(
            init["result"]["capabilities"].get("hoverProvider") is True,
            "hover capability declared",
        )

    print("- diagnostics")
    expect(published is not None, "didOpen published diagnostics")
    if published:
        expect(published["uri"] == uri, "the publish names the document")
        expect(published["diagnostics"] == [], "a clean program publishes an empty list")

    print("- hover")
    expect(hover is not None and hover is not None, "hover answers")
    if hover:
        value = hover["contents"]["value"]
        expect("top score" in value and "number" in value, f"hover types the run: {value!r}")

    print("- definition")
    expect(definition is not None, "definition answers")
    if definition:
        line = definition["range"]["start"]["line"]
        target = PROGRAM.split("\n")[line]
        expect(
            target.startswith("make top score"),
            f"goto-def lands on the binding: {target.strip()!r}",
        )

    if FAILURES:
        print("LSP SMOKE FAILED:", FAILURES)
        sys.exit(1)
    print("LSP SMOKE OK")


if __name__ == "__main__":
    main()
