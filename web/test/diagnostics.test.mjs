import { test } from "node:test";
import assert from "node:assert/strict";
import {
  fromWireDiagnostics,
  lineStarts,
  positionToOffset,
  toEditorRanges,
} from "../src/diagnostics.js";

test("lineStarts gives the UTF-16 offset where each line begins", () => {
  assert.deepEqual(lineStarts("a\nbc\n"), [0, 2, 5]);
});

test("columns count scalar values, so an emoji is one column but two UTF-16 units", () => {
  const source = "😀😀x";
  assert.equal(positionToOffset(source, [0], 1, 1), 0);
  assert.equal(positionToOffset(source, [0], 1, 3), 4);
});

test("positions past the end are clamped instead of throwing", () => {
  const source = "ab\ncd";
  const starts = lineStarts(source);
  assert.equal(positionToOffset(source, starts, 2, 99), source.length);
  assert.equal(positionToOffset(source, starts, 9, 1), 3);
});

test("a wire diagnostic becomes the canonical shape with offsets", () => {
  const source = "ab\ncdef";
  const [d] = fromWireDiagnostics(source, [
    {
      severity: "error",
      code: "E1",
      message: "bad",
      line: 2,
      column: 2,
      endLine: 2,
      endColumn: 4,
      rendered: "teaching text",
      notes: ["a note"],
    },
  ]);
  assert.equal(d.severity, "error");
  assert.equal(d.code, "E1");
  assert.equal(d.message, "bad");
  assert.equal(d.rendered, "teaching text");
  assert.deepEqual(d.notes, ["a note"]);
  assert.deepEqual(d.start, { line: 2, column: 2, offset: 4 });
  assert.deepEqual(d.end, { line: 2, column: 4, offset: 6 });
});

test("unknown severities become info, and missing rendered text falls back to message", () => {
  const [d] = fromWireDiagnostics("x", [{ severity: "note", line: 1, column: 1, message: "hi" }]);
  assert.equal(d.severity, "info");
  assert.equal(d.rendered, "hi");
  assert.equal(d.end, null);
});

test("an end before the start is clamped to the start", () => {
  const [d] = fromWireDiagnostics("abc", [
    { severity: "error", line: 1, column: 3, endLine: 1, endColumn: 1, message: "m" },
  ]);
  assert.equal(d.end.offset, d.start.offset);
});

test("missing or non-array diagnostics become an empty list", () => {
  assert.deepEqual(fromWireDiagnostics("x", undefined), []);
  assert.deepEqual(fromWireDiagnostics("x", null), []);
});

test("editor ranges use one character when there is no end, and never pass the source end", () => {
  const diagnostics = fromWireDiagnostics("ab", [{ severity: "error", line: 1, column: 3, message: "m" }]);
  const [range] = toEditorRanges(diagnostics, 2);
  assert.equal(range.from, 2);
  assert.equal(range.to, 2);
});
