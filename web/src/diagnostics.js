// The one place where compiler diagnostics become the UI's Diagnostic shape.
//
// Position conventions (documented here because they decide where highlights
// land):
//   line    1-based. Lines are separated by "\n".
//   column  1-based, counted in Unicode scalar values (code points) within the
//           line. The compiler reports this unit. So an emoji is one column,
//           not the two UTF-16 code units JavaScript sees.
//   end     the compiler's end position is exclusive, and so is ours.
//   offset  0-based UTF-16 code-unit index into the editor text. CodeMirror
//           uses this unit, so the conversion happens here, once.
//
// Canonical Diagnostic:
//   {
//     severity: "error" | "warning" | "info",
//     code: string | null,
//     message: string,
//     start: { line, column, offset },
//     end: { line, column, offset } | null,
//     rendered: string,      // the compiler's teaching text, shown verbatim
//     notes: string[],
//     fix: string | null,
//   }
//
// Nothing else in the UI reads the compiler's wire fields directly.

const SEVERITY = { error: "error", warning: "warning" };

// Offsets (UTF-16) at which each line starts.
export function lineStarts(source) {
  const starts = [0];
  for (let i = 0; i < source.length; i += 1) {
    if (source.charCodeAt(i) === 10) starts.push(i + 1);
  }
  return starts;
}

// Convert a 1-based line and 1-based scalar column into a UTF-16 offset.
// Out-of-range positions are clamped to the line's end, or to the end of the
// source, so a stale diagnostic never throws.
export function positionToOffset(source, starts, line, column) {
  const index = Math.min(Math.max(line, 1), starts.length) - 1;
  let offset = starts[index];
  const lineEnd = index + 1 < starts.length ? starts[index + 1] - 1 : source.length;
  for (let scalar = 1; scalar < column && offset < lineEnd; scalar += 1) {
    const codePoint = source.codePointAt(offset);
    offset += codePoint > 0xffff ? 2 : 1;
  }
  return Math.min(offset, source.length);
}

function toPosition(source, starts, line, column) {
  return {
    line,
    column,
    offset: positionToOffset(source, starts, line, column),
  };
}

// Convert one compiler diagnostic. `wire` is the object from lagom_web's JSON.
export function fromWireDiagnostic(source, starts, wire) {
  const start = toPosition(source, starts, wire.line, wire.column);
  const end =
    wire.endLine != null && wire.endColumn != null
      ? toPosition(source, starts, wire.endLine, wire.endColumn)
      : null;
  if (end && end.offset < start.offset) end.offset = start.offset;
  return {
    severity: SEVERITY[wire.severity] ?? "info",
    code: wire.code ?? null,
    message: wire.message ?? "",
    start,
    end,
    rendered: wire.rendered ?? wire.message ?? "",
    notes: Array.isArray(wire.notes) ? wire.notes.map(String) : [],
    fix: wire.fix ?? null,
  };
}

// Convert a list of compiler diagnostics. Missing or non-array input becomes [].
export function fromWireDiagnostics(source, wire) {
  if (!Array.isArray(wire)) return [];
  const starts = lineStarts(source);
  return wire.map((item) => fromWireDiagnostic(source, starts, item));
}

// Editor ranges for CodeMirror: { from, to, severity, message }. A diagnostic
// without an end gets a one-character range at its start.
export function toEditorRanges(diagnostics, sourceLength) {
  return diagnostics.map((d) => {
    const from = Math.min(d.start.offset, sourceLength);
    const to = d.end ? Math.min(d.end.offset, sourceLength) : Math.min(from + 1, sourceLength);
    return { from, to: Math.max(to, from), severity: d.severity, message: d.message };
  });
}
