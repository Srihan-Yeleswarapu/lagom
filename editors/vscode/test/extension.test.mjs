// The VS Code extension's validation: the manifest's every referenced file
// exists; every grammar regex compiles; the grammar's phrase sets are
// byte-equal to the web tokenizer's lists (one source of truth for the
// frozen phrases — the extension test regenerates from
// web/src/lagom-lexical.js and the tmLanguage must match exactly); and
// `lagom check --json` — the binary the extension drives — really produces
// the JSON document the test parses on both program outcomes.

import test from "node:test";
import assert from "node:assert/strict";
import { readFileSync, writeFileSync, unlinkSync, statSync } from "node:fs";
import { execFileSync } from "node:child_process";
import { fileURLToPath } from "node:url";
import path from "node:path";

const here = path.dirname(fileURLToPath(import.meta.url));
const extension = path.resolve(here, "..");
const repo = path.resolve(extension, "..", "..");

function readJson(file) {
  return JSON.parse(readFileSync(path.join(extension, file), "utf8"));
}

test("the manifest references files that exist", () => {
  const manifest = readJson("package.json");
  statSync(path.join(extension, manifest.main));
  for (const language of manifest.contributes.languages) {
    statSync(path.join(extension, language.configuration));
  }
  for (const grammar of manifest.contributes.grammars) {
    assert.equal(grammar.scopeName, "source.lagom");
    statSync(path.join(extension, grammar.path));
  }
  for (const snippets of manifest.contributes.snippets) {
    statSync(path.join(extension, snippets.path));
  }
  assert.ok(
    manifest.activationEvents.includes("onLanguage:lagom"),
    "the extension wakes for .lagom files"
  );
});

test("every grammar pattern compiles as a regular expression", () => {
  const grammar = readJson("syntaxes/lagom.tmLanguage.json");
  assert.equal(grammar.scopeName, "source.lagom");
  for (const entry of grammar.patterns) {
    assert.ok(entry.include.startsWith("#"), "top-level includes are repo refs");
    assert.ok(grammar.repository[entry.include.slice(1)], `${entry.include} exists`);
  }
  for (const [name, item] of Object.entries(grammar.repository)) {
    if (item.match) {
      assert.doesNotThrow(
        () => new RegExp(item.match),
        `${name} pattern compiles`
      );
    }
    if (item.begin) assert.doesNotThrow(() => new RegExp(item.begin));
    if (item.end) assert.doesNotThrow(() => new RegExp(item.end));
  }
});

test("the grammar's phrase sets equal the web tokenizer's lists", async () => {
  const { TYPE_PHRASES, LITERAL_WORDS, BUILTIN_WORDS, KEYWORD_PHRASES } =
    await import(path.join(repo, "web", "src", "lagom-lexical.js"));
  const grammar = readJson("syntaxes/lagom.tmLanguage.json");

  // The M2 surface must reach editors: the network built-ins and the
  // tasks-and-channels ops are in the shared builtin list.
  for (const word of [
    "send",
    "receive",
    "open listener",
    "open connection",
    "expect connection",
    "put",
    "take",
    "close",
    "port",
  ]) {
    assert.ok(BUILTIN_WORDS.includes(word), `builtin list carries ${word}`);
  }

  // Byte-equality: the same transformation over the same list must produce
  // the same regex the grammar file carries — a copy that drifts loses.
  // The web tokenizer sorts the PHRASES by length first, then escapes and
  // spaces each one (the mapped-word sort below would tie-break
  // differently); mirror it exactly.
  const expect = (list) => `(?:${alternation(list)})(?!\\w)`;
  function alternation(phrases) {
    return [...phrases]
      .sort((a, b) => b.length - a.length)
      .map((phrase) =>
        phrase
          .split(" ")
          .map((word) => word.replace(/[.*+?^${}()|[\]\\]/g, "\\$&"))
          .join("[ \\t]+")
      )
      .join("|");
  }

  assert.equal(
    grammar.repository.type.match,
    expect(TYPE_PHRASES),
    "type phrases in set-parity with the web tokenizer"
  );
  assert.equal(
    grammar.repository.literal.match,
    expect(LITERAL_WORDS),
    "literal words in set-parity"
  );
  assert.equal(
    grammar.repository.builtin.match,
    expect(BUILTIN_WORDS),
    "builtin words (say/ask + tasks + M2 network ops) in set-parity"
  );
  assert.equal(
    grammar.repository.keyword.match,
    expect(KEYWORD_PHRASES),
    "keyword phrases longest-first, in set-parity"
  );
});

test("the completions carry the student layer's words", async () => {
  const { TYPE_PHRASES, BUILTIN_WORDS, KEYWORD_PHRASES } = await import(
    path.join(repo, "web", "src", "lagom-lexical.js")
  );
  const completions = (await import(path.join(extension, "completions.js"))).default;
  const setOf = (list) => [...list].sort();
  assert.deepEqual(setOf(completions.builtins), setOf(BUILTIN_WORDS), "builtins parity");
  assert.deepEqual(setOf(completions.types), setOf(TYPE_PHRASES), "type phrases parity");
  assert.deepEqual(setOf(completions.keywords), setOf(KEYWORD_PHRASES), "keyword phrases parity");
  // The layer rule: the student surface cannot say `unsafe`, so no
  // completion may ever suggest it.
  for (const list of [completions.builtins, completions.types, completions.keywords]) {
    assert.ok(!list.some((w) => w.includes("unsafe")), "no unsafe completion");
  }
});

test("every snippet has prefix, body, and description", () => {
  const snippets = readJson("snippets/lagom.json");
  assert.ok(Object.keys(snippets).length >= 10, "the teaching snippets ship");
  for (const [name, snippet] of Object.entries(snippets)) {
    assert.ok(typeof snippet.prefix === "string" && snippet.prefix, `${name} prefix`);
    assert.ok(Array.isArray(snippet.body) && snippet.body.length, `${name} body`);
    assert.ok(
      typeof snippet.description === "string" && snippet.description,
      `${name} description`
    );
  }
});

test("`lagom check --json` produces the document the extension parses", (t) => {
  const binary = path.join(repo, "target", "release", "lagom");
  try {
    statSync(binary);
  } catch {
    // `assert.skip` does not exist — node:test's context-owned skip does.
    t.skip("build the release CLI first (cargo build --release -p lagom_cli)");
    return;
  }

  const good = execFileSync(binary, ["check", path.join(repo, "validation", "smoke.lagom"), "--json"], {
    encoding: "utf8",
  });
  const okReport = JSON.parse(good);
  assert.equal(okReport.ok, true);
  assert.deepEqual(okReport.diagnostics, []);

  const badPath = path.join(repo, "target", "tmp-lagom-bad.lagom");
  const badSource = "say it\n";
  try {
    writeFileSync(badPath, badSource);
    let badOkCallFailed = false;
    let badStdout = "";
    try {
      execFileSync(binary, ["check", badPath, "--json"], { encoding: "utf8" });
      assert.fail("an erroring program must exit non-zero");
    } catch (e) {
      badOkCallFailed = true;
      badStdout = e.stdout;
    }
    assert.ok(badOkCallFailed, "the bad program must exit non-zero");
    const report = JSON.parse(badStdout);
    assert.equal(report.ok, false);
    assert.ok(
      report.diagnostics.some((d) => d.severity === "error"),
      "the erroring document carries a severity"
    );
    for (const d of report.diagnostics) {
      assert.ok(typeof d.line === "number" && d.line >= 1, "1-based line");
      assert.ok(typeof d.code === "string" && d.code, "a code to explain");
    }
  } finally {
    try {
      unlinkSync(badPath);
    } catch {
      // nothing to clean up
    }
  }
});
