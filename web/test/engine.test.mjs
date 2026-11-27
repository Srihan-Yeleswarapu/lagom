import { test } from "node:test";
import assert from "node:assert/strict";
import { createEngine, MAX_ANSWER_BYTES } from "../src/engine.js";

// A stand-in for the compiler worker. Tests drive it by calling reply().
class FakeWorker {
  static created = [];
  constructor() {
    this.messages = [];
    this.terminated = false;
    FakeWorker.created.push(this);
  }
  postMessage(message) {
    this.messages.push(message);
  }
  terminate() {
    this.terminated = true;
  }
  reply(data) {
    this.onmessage({ data });
  }
}

function setup() {
  FakeWorker.created.length = 0;
  return createEngine({ createWorker: () => new FakeWorker() });
}

// Run a test with the page reported as cross-origin isolated, as it is when served
// by scripts/serve-web.py. The previous value is restored afterwards.
async function isolated(fn) {
  const had = Object.prototype.hasOwnProperty.call(globalThis, "crossOriginIsolated");
  const previous = globalThis.crossOriginIsolated;
  globalThis.crossOriginIsolated = true;
  try {
    await fn();
  } finally {
    if (had) globalThis.crossOriginIsolated = previous;
    else delete globalThis.crossOriginIsolated;
  }
}

test("a check resolves with a canonical check outcome", async () => {
  const engine = setup();
  const pending = engine.check("make x equal to 1");
  const worker = FakeWorker.created[0];
  assert.equal(engine.busy, true);
  worker.reply({ id: worker.messages[0].id, ok: true, result: { ok: true, diagnostics: [] } });
  const outcome = await pending;
  assert.equal(outcome.kind, "ok");
  assert.equal(engine.busy, false);
});

test("compile errors come back as errors, with diagnostics converted at the boundary", async () => {
  const engine = setup();
  const source = "say 1";
  const pending = engine.check(source);
  const worker = FakeWorker.created[0];
  worker.reply({
    id: worker.messages[0].id,
    ok: true,
    result: {
      ok: false,
      diagnostics: [{ severity: "error", line: 1, column: 5, message: "bad", rendered: "text" }],
    },
  });
  const outcome = await pending;
  assert.equal(outcome.kind, "errors");
  assert.equal(outcome.diagnostics[0].start.offset, 4);
  assert.equal(outcome.diagnostics[0].rendered, "text");
});

test("a newer request supersedes the old one, and its late reply is ignored", async () => {
  const engine = setup();
  const first = engine.check("a");
  const w1 = FakeWorker.created[0];
  const firstId = w1.messages[0].id;

  const second = engine.run("b");
  assert.equal((await first).kind, "superseded");
  assert.equal(w1.terminated, true);

  const w2 = FakeWorker.created[1];
  w1.reply({ id: firstId, ok: true, result: { ok: true, diagnostics: [] } });
  assert.equal(engine.busy, true, "a late reply from the old worker must not finish the new run");

  w2.reply({ id: w2.messages[0].id, ok: true, result: { status: "completed", output: ["b"] } });
  const outcome = await second;
  assert.equal(outcome.kind, "completed");
  assert.deepEqual(outcome.output, ["b"]);
});

test("stop terminates the worker and reports stopped, and a later request gets a fresh worker", async () => {
  const engine = setup();
  const pending = engine.run("loop");
  const worker = FakeWorker.created[0];
  assert.equal(engine.stop(), true);
  assert.equal((await pending).kind, "stopped");
  assert.equal(worker.terminated, true);
  assert.equal(engine.busy, false);
  assert.equal(engine.stop(), false);

  engine.check("x");
  assert.equal(FakeWorker.created.length, 2);
});

test("a worker error becomes an error outcome with its message", async () => {
  const engine = setup();
  const pending = engine.run("x");
  const worker = FakeWorker.created[0];
  worker.onerror({ message: "boom", preventDefault() {} });
  const outcome = await pending;
  assert.equal(outcome.kind, "error");
  assert.equal(outcome.message, "boom");
  assert.equal(worker.terminated, true);
});

test("a compiler error reply carries its message", async () => {
  const engine = setup();
  const pending = engine.run("x");
  const worker = FakeWorker.created[0];
  worker.reply({ id: worker.messages[0].id, ok: false, error: "wasm trap" });
  const outcome = await pending;
  assert.equal(outcome.kind, "error");
  assert.equal(outcome.message, "wasm trap");
});

test("a non-interactive run passes stdin as one string and no channel", () => {
  const engine = setup();
  engine.run("x", { stdin: "a\nb" });
  const message = FakeWorker.created[0].messages[0];
  assert.equal(message.stdin, "a\nb");
  assert.equal(message.channel, undefined);
  engine.destroy();
});

test("without cross-origin isolation an interactive run falls back to stdin", () => {
  const engine = setup();
  engine.run("x", { interactive: true, onAsk() {} });
  const message = FakeWorker.created[0].messages[0];
  assert.equal(message.channel, undefined);
  assert.equal(message.stdin, "");
  engine.destroy();
});

test("an interactive run shares a channel, and each answer resumes the next ask", async () => {
  await isolated(async () => {
    const engine = setup();
    const asks = [];
    const pending = engine.run("src", { interactive: true, onAsk: (ask) => asks.push(ask) });
    const worker = FakeWorker.created[0];
    const message = worker.messages[0];
    assert.ok(message.channel instanceof SharedArrayBuffer);
    const ctl = new Int32Array(message.channel, 0, 2);

    worker.reply({ id: message.id, type: "ask", prompt: "Name? ", transcript: "" });
    assert.equal(engine.waitingForInput, true);
    assert.deepEqual(asks, [{ prompt: "Name? ", transcript: "" }]);

    assert.equal(engine.answer("Ada"), true);
    assert.equal(engine.waitingForInput, false);
    assert.equal(Atomics.load(ctl, 0), 1);
    const length = Atomics.load(ctl, 1);
    assert.equal(new TextDecoder().decode(new Uint8Array(message.channel, 8, length)), "Ada");

    worker.reply({ id: message.id, type: "ask", prompt: "Again? ", transcript: "Name? " });
    assert.equal(engine.endInput(), true);
    assert.equal(Atomics.load(ctl, 0), 2);

    worker.reply({
      id: message.id,
      ok: true,
      result: { status: "panicked", output: ["Name? "], message: "end of input" },
    });
    const outcome = await pending;
    assert.equal(outcome.kind, "panicked");
    assert.equal(outcome.message, "end of input");
  });
});

test("answers are refused when no ask is waiting or the answer is too long", async () => {
  await isolated(async () => {
    const engine = setup();
    engine.run("src", { interactive: true, onAsk() {} });
    const worker = FakeWorker.created[0];
    assert.equal(engine.answer("early"), false);
    assert.equal(engine.endInput(), false);

    worker.reply({ id: worker.messages[0].id, type: "ask", prompt: "? ", transcript: "" });
    assert.equal(engine.answer("x".repeat(MAX_ANSWER_BYTES + 1)), false);
    assert.equal(engine.waitingForInput, true);
    engine.destroy();
  });
});

test("destroy supersedes a pending run and terminates the worker", async () => {
  const engine = setup();
  const pending = engine.run("x");
  engine.destroy();
  assert.equal((await pending).kind, "superseded");
  assert.equal(FakeWorker.created[0].terminated, true);
});
