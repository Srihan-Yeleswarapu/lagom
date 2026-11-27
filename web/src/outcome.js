// Turns the worker's JSON into the outcome the UI shows. Compiler diagnostics
// are converted to the canonical Diagnostic shape here, through diagnostics.js,
// so no other code reads the compiler's wire fields.
//
// Run kinds:
//   completed      the program finished
//   compile-error  it did not compile; see diagnostics
//   panicked       it started, then hit a runtime error (for example, end of input)
//   failed         it stopped on a Lagom-level failure (`fail with`)
//   stopped        the learner pressed Stop
//   superseded     a newer request replaced it (ignore it)
//   error          the compiler itself could not run (worker or wasm error)
// Check kinds: "ok" (no errors; warnings allowed) and "errors".
import { fromWireDiagnostics } from "./diagnostics.js";

export const RUN_LABELS = {
  completed: "Finished.",
  "compile-error": "Did not compile.",
  panicked: "Stopped with an error.",
  failed: "The program failed.",
  stopped: "Stopped by you.",
  superseded: "Replaced by a newer run.",
  error: "The compiler could not run.",
};

export function toCheckOutcome(source, result) {
  const diagnostics = fromWireDiagnostics(source, result?.diagnostics);
  const hasError = diagnostics.some((d) => d.severity === "error");
  return {
    kind: result?.ok && !hasError ? "ok" : "errors",
    diagnostics,
  };
}

export function toRunOutcome(source, result) {
  const diagnostics = fromWireDiagnostics(source, result?.diagnostics);
  const kind = ["completed", "compile-error", "panicked", "failed"].includes(result?.status)
    ? result.status
    : "error";
  return {
    kind,
    label: RUN_LABELS[kind],
    output: Array.isArray(result?.output) ? result.output.map(String) : [],
    message: typeof result?.message === "string" ? result.message : null,
    report: typeof result?.report === "string" ? result.report : null,
    diagnostics,
  };
}

// Outcomes the engine makes itself, without a compiler result.
export function localOutcome(kind, message = null) {
  return {
    kind,
    label: RUN_LABELS[kind] ?? RUN_LABELS.error,
    output: [],
    message,
    report: null,
    diagnostics: [],
  };
}
