// Smoke test for the browser build: loads web/pkg/lagom_web_bg.wasm in Node and
// runs real Lagom programs through the exported check/run API.
// Usage: node scripts/smoke-web.mjs   (after scripts/build-web.sh)
import { readFileSync, mkdtempSync, writeFileSync } from "node:fs";
import { tmpdir } from "node:os";
import { join, dirname } from "node:path";
import { fileURLToPath, pathToFileURL } from "node:url";
import assert from "node:assert/strict";

const root = join(dirname(fileURLToPath(import.meta.url)), "..");
// The glue is an ES module written as .js; copy it as .mjs so Node loads it as ESM.
const dir = mkdtempSync(join(tmpdir(), "lagom-web-"));
const glue = join(dir, "lagom_web.mjs");
writeFileSync(glue, readFileSync(join(root, "web/pkg/lagom_web.js"), "utf8"));
const lagom = await import(pathToFileURL(glue).href);
lagom.initSync({ module: readFileSync(join(root, "web/pkg/lagom_web_bg.wasm")) });

const run = (src, stdin = "") => JSON.parse(lagom.run(src, stdin));
const check = (src) => JSON.parse(lagom.check(src));

let passed = 0;
function test(name, fn) {
  fn();
  passed++;
  console.log(`ok - ${name}`);
}

test("hello world prints", () => {
  const r = run('say "Hello, world"\n');
  assert.equal(r.status, "completed");
  assert.deepEqual(r.output, ["Hello, world"]);
});

test("variables, arithmetic and interpolation", () => {
  const r = run('make total equal to 2 plus 3\nsay "total is {total}"\n');
  assert.equal(r.status, "completed", JSON.stringify(r));
  assert.equal(r.output.at(-1), "total is 5");
});

test("ask reads from supplied stdin", () => {
  const r = run('make name equal to ask "Name? "\nsay "Hi {name}"\n', "Grace\n");
  assert.equal(r.status, "completed", JSON.stringify(r));
  assert.equal(r.output.at(-1), "Hi Grace");
});

test("ask with no stdin fails with end-of-input message", () => {
  const r = run('make name equal to ask "Name? "\nsay name\n', "");
  assert.equal(r.status, "panicked", JSON.stringify(r));
  assert.match(r.message, /end of input/);
});

test("compile error carries a span and student rendering", () => {
  const r = run('say "ok"\nsay undefined_name\n');
  assert.equal(r.status, "compile-error");
  const d = r.diagnostics[0];
  assert.equal(d.severity, "error");
  assert.equal(d.line, 2);
  assert.ok(d.rendered.length > 0);
});

test("check reports ok for valid and not ok for invalid source", () => {
  assert.equal(check('say "fine"\n').ok, true);
  assert.equal(check("say (\n").ok, false);
});

console.log(`\n${passed} smoke tests passed`);
