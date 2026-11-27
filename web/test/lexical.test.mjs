import { test } from "node:test";
import assert from "node:assert/strict";
import { readFileSync, readdirSync } from "node:fs";
import { dirname, join } from "node:path";
import { fileURLToPath } from "node:url";
import { startState, token } from "../src/lagom-lexical.js";

// The subset of CodeMirror's StringStream that the tokenizer uses.
class FakeStream {
  constructor(string) {
    this.string = string;
    this.pos = 0;
  }
  sol() {
    return this.pos === 0;
  }
  peek() {
    return this.string.charAt(this.pos) || undefined;
  }
  next() {
    if (this.pos < this.string.length) return this.string.charAt(this.pos++);
    return undefined;
  }
  eat(match) {
    const ch = this.peek();
    const ok = typeof match === "string" ? ch === match : ch !== undefined && match.test(ch);
    if (ok) {
      this.pos += 1;
      return ch;
    }
    return undefined;
  }
  eatSpace() {
    const start = this.pos;
    while (this.peek() !== undefined && /\s/.test(this.peek())) this.pos += 1;
    return this.pos > start;
  }
  skipToEnd() {
    this.pos = this.string.length;
  }
  match(pattern) {
    if (typeof pattern === "string") {
      if (!this.string.startsWith(pattern, this.pos)) return null;
      this.pos += pattern.length;
      return pattern;
    }
    const found = this.string.slice(this.pos).match(pattern);
    if (!found || found.index !== 0) return null;
    this.pos += found[0].length;
    return found[0];
  }
}

function tokenize(line) {
  const stream = new FakeStream(line);
  const state = startState();
  const tokens = [];
  while (stream.pos < line.length) {
    const start = stream.pos;
    const type = token(stream, state);
    assert.ok(stream.pos > start, `tokenizer made no progress at column ${start} of ${JSON.stringify(line)}`);
    tokens.push({ text: line.slice(start, stream.pos), type });
  }
  return tokens;
}

// Only the tokens that carry a highlight, as [type, text] pairs.
function styled(line) {
  return tokenize(line)
    .filter((t) => t.type !== null)
    .map((t) => [t.type, t.text]);
}

test("a # comment runs to the end of the line, and ## is a doc comment", () => {
  assert.deepEqual(styled("# a note"), [["comment", "# a note"]]);
  assert.deepEqual(styled("## Adds two numbers"), [["doc", "## Adds two numbers"]]);
});

test("strings highlight their text and interpolations separately", () => {
  assert.deepEqual(styled('say "Hi {name}!"'), [
    ["builtin", "say"],
    ["string", '"'],
    ["string", "Hi "],
    ["interp", "{name}"],
    ["string", "!"],
    ["string", '"'],
  ]);
});

test("longest keyword phrase wins, and identifiers are not split", () => {
  assert.deepEqual(styled("make changing score equal to 10"), [
    ["keyword", "make"],
    ["keyword", "changing"],
    ["keyword", "equal to"],
    ["number", "10"],
  ]);
  assert.deepEqual(styled("if age is at least 13"), [
    ["keyword", "if"],
    ["keyword", "is at least"],
    ["number", "13"],
  ]);
  assert.deepEqual(styled("answer"), []);
  assert.deepEqual(styled("isolated"), []);
});

test("multi-word type names, literals, and decimals", () => {
  assert.deepEqual(styled("taking a list of text"), [
    ["keyword", "taking"],
    ["typeName", "a list of"],
    ["typeName", "text"],
  ]);
  assert.deepEqual(styled("set done to true"), [
    ["keyword", "set"],
    ["keyword", "to"],
    ["atom", "true"],
  ]);
  assert.deepEqual(styled("3.5"), [["number", "3.5"]]);
});

test("every line of every lesson example is covered exactly by its tokens", () => {
  const lessonDir = join(dirname(fileURLToPath(import.meta.url)), "..", "content", "lessons");
  for (const file of readdirSync(lessonDir)) {
    const lesson = JSON.parse(readFileSync(join(lessonDir, file), "utf8"));
    for (const block of lesson.body) {
      const code = block.type === "example" ? block.code : block.type === "exercise" ? block.solution : null;
      if (!code) continue;
      for (const line of code.split("\n")) {
        const joined = tokenize(line)
          .map((t) => t.text)
          .join("");
        assert.equal(joined, line, `${file}: tokens do not cover ${JSON.stringify(line)}`);
      }
    }
  }
});
