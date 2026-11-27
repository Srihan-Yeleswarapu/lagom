import { test } from "node:test";
import assert from "node:assert/strict";
import { buildTranscript, transcriptText } from "../src/transcript.js";

test("a program without asks is just its output", () => {
  assert.deepEqual(buildTranscript(["one", "two"]), [
    { kind: "say", text: "one" },
    { kind: "say", text: "two" },
  ]);
});

test("an answered ask places the answer between its prompt and the next output", () => {
  const entries = buildTranscript(["What is your name? ", "Hello, Ada!"], [
    { prompt: "What is your name? ", transcript: "", answer: "Ada" },
  ]);
  assert.deepEqual(entries, [
    { kind: "prompt", text: "What is your name? " },
    { kind: "answer", text: "Ada" },
    { kind: "say", text: "Hello, Ada!" },
  ]);
});

test("several asks keep their own output in order", () => {
  const output = ["Hi", "Q1? ", "Q2? ", "Done"];
  const entries = buildTranscript(output, [
    { prompt: "Q1? ", transcript: "Hi", answer: "a" },
    { prompt: "Q2? ", transcript: "Hi\nQ1? ", answer: "b" },
  ]);
  assert.deepEqual(
    entries.map((e) => [e.kind, e.text]),
    [
      ["say", "Hi"],
      ["prompt", "Q1? "],
      ["answer", "a"],
      ["prompt", "Q2? "],
      ["answer", "b"],
      ["say", "Done"],
    ],
  );
});

test("a say line that contains a newline still matches its transcript", () => {
  const entries = buildTranscript(["a\nb", "Q? "], [{ prompt: "Q? ", transcript: "a\nb", answer: "c" }]);
  assert.deepEqual(
    entries.map((e) => [e.kind, e.text]),
    [
      ["say", "a\nb"],
      ["prompt", "Q? "],
      ["answer", "c"],
    ],
  );
});

test("a waiting ask shows its prompt before the compiler has returned output", () => {
  const entries = buildTranscript([], [{ prompt: "Name? ", transcript: "", answer: undefined }]);
  assert.deepEqual(entries, [{ kind: "prompt", text: "Name? " }]);
});

test("end of input is shown as a clear note, not as an answer", () => {
  const entries = buildTranscript(["Name? "], [{ prompt: "Name? ", transcript: "", answer: null }]);
  assert.deepEqual(
    entries.map((e) => e.kind),
    ["prompt", "end"],
  );
  assert.match(entries[1].text, /No more input/);
});

test("transcriptText joins the entries one per line", () => {
  assert.equal(
    transcriptText([
      { kind: "prompt", text: "Name? " },
      { kind: "answer", text: "Ada" },
    ]),
    "Name? \nAda",
  );
});
