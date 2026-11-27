// The reusable playground. It holds the editor, Check / Run / Stop / Reset, the
// output panel (with the interactive transcript), and the problems panel. It
// takes a configuration object and does not know whether it sits in a lesson
// example, an exercise, or a practice page.
//
// config (plain data):
//   id          stable id. It keys the saved draft, so keep it stable.
//   initialCode the starter code. "Reset to starter code" restores it.
//   editable    false makes the editor read-only.
//   runMode     "check": Check only. "run": Run only. "both": both.
//   inputMode   "none": ask gets no answers, so the program ends at its first
//               ask. "ask": each ask pauses for an answer typed in the panel.
//   heightHint  editor height in lines (default 8).
//   label       accessible name for the editor.
//   saveDraft   true keeps the learner's edits in this browser.
//
// runtime: { engine, progress } from runtime.js.
// onAttempt: called when the learner starts a run or a manual check.
//
// Problems update when the learner pauses typing. That automatic check waits
// while the engine is busy, so it never interrupts a run. Ctrl/Cmd+Enter runs
// the program, or checks in a check-only playground.
import { createEditor } from "../vendor/editor.bundle.js";
import { canInteract } from "./engine.js";
import { h } from "./dom.js";
import { toEditorRanges } from "./diagnostics.js";
import { buildTranscript } from "./transcript.js";

const CHECK_DELAY_MS = 800;
const DRAFT_DELAY_MS = 400;
// Outcomes where the compiler reached the program (so output and diagnostics are real).
const RAN_KINDS = ["completed", "compile-error", "panicked", "failed"];
let instances = 0;

export function createPlayground(config, { engine, progress, onAttempt } = {}) {
  const runMode = config.runMode ?? "both";
  const canCheck = runMode === "check" || runMode === "both";
  const canRun = runMode === "run" || runMode === "both";
  const interactive = config.inputMode === "ask";
  const editable = config.editable !== false;
  const persistDraft = Boolean(config.saveDraft && editable && progress);
  const askId = `pg-ask-${++instances}`;

  let ownsRun = false; // this widget started the run in progress
  let asks = []; // { prompt, transcript, answer } for each ask in the current run
  let checkTimer = null;
  let draftTimer = null;
  let pendingDraft = null;

  const savedDraft = persistDraft ? progress.getDraft(config.id) : null;
  const usingDraft = savedDraft !== null;

  // ---- DOM ----
  const status = h("p", { className: "pg-status", role: "status", tabindex: "-1" });
  const note = h("p", { className: "pg-note" }, usingDraft
    ? "Showing your saved edits. Reset to starter code to see the original."
    : null);
  const transcript = h("div", { className: "pg-transcript", role: "log", "aria-label": "Program output" });
  const result = h("div", { className: "pg-result", hidden: true });
  const askLabel = h("label", { className: "pg-ask-label", for: askId });
  const askInput = h("input", { id: askId, type: "text", autocomplete: "off", spellcheck: "false" });
  const askForm = h(
    "form",
    {
      className: "pg-ask",
      hidden: true,
      onsubmit: (event) => {
        event.preventDefault();
        sendAnswer(askInput.value);
      },
    },
    askLabel,
    askInput,
    h("button", { type: "submit", className: "pg-send" }, "Send"),
    h("button", { type: "button", className: "pg-end", onclick: endInput }, "No more input"),
  );
  const problems = h("ul", { className: "pg-problems", role: "list" },
    h("li", { className: "pg-empty" }, "Problems appear here once the code is checked."));
  const runButton = h("button", {
    type: "button",
    className: "pg-run",
    title: "Run (Ctrl+Enter or Cmd+Enter)",
    onclick: () => runProgram(),
  }, "Run");
  const checkButton = h("button", { type: "button", className: "pg-check", onclick: () => runCheck() }, "Check");
  const stopButton = h("button", { type: "button", className: "pg-stop", onclick: stop }, "Stop");
  const resetButton = h("button", { type: "button", className: "pg-reset", onclick: reset }, "Reset to starter code");
  runButton.hidden = !canRun;
  checkButton.hidden = !canCheck;
  resetButton.hidden = !editable;
  stopButton.disabled = true;

  const editorHost = h("div", { className: "pg-editor" });
  editorHost.style.height = `${(config.heightHint ?? 8) * 1.5}em`;

  const root = h(
    "section",
    { className: "playground", "aria-label": config.label ?? "Lagom playground" },
    h("div", { className: "pg-toolbar" }, runButton, checkButton, stopButton, resetButton),
    status,
    editorHost,
    h(
      "div",
      { className: "pg-panels" },
      h("section", { className: "pg-output" },
        h("h4", {}, "Output"), transcript, result, askForm),
      h("section", { className: "pg-problems-panel" },
        h("h4", {}, "Problems"), problems),
    ),
    note,
  );

  const editor = createEditor({
    parent: editorHost,
    doc: usingDraft ? savedDraft : config.initialCode,
    editable,
    label: config.label ?? "Lagom code",
    onChange: handleEdit,
    onRun: () => (canRun ? runProgram() : runCheck()),
  });

  // ---- Status and transcript ----
  function setStatus(text) {
    status.textContent = text;
  }

  function setRunning(running) {
    runButton.disabled = running;
    checkButton.disabled = running;
    resetButton.disabled = running;
    stopButton.disabled = !running;
  }

  function showTranscript(entries) {
    const lines = entries.map((entry) =>
      h("div", { className: `pg-line pg-line-${entry.kind}` },
        entry.kind === "answer" ? `› ${entry.text}` : entry.text));
    if (lines.length === 0) lines.push(h("div", { className: "pg-empty" }, "No output yet."));
    transcript.replaceChildren(...lines);
    transcript.scrollTop = transcript.scrollHeight;
  }

  // The output so far while an ask is pending: everything the worker sent before
  // the last prompt, then the prompt itself.
  function liveOutput() {
    const last = asks.at(-1);
    if (!last) return [];
    return [...(last.transcript ? last.transcript.split("\n") : []), last.prompt];
  }

  function showResult(outcome) {
    const parts = [h("strong", {}, outcome.label)];
    if (outcome.message) parts.push(h("p", { className: "pg-message" }, outcome.message));
    if (outcome.report) parts.push(h("pre", { className: "pg-report" }, outcome.report));
    result.dataset.kind = outcome.kind;
    result.replaceChildren(...parts);
    result.hidden = false;
  }

  // ---- Problems ----
  // Each diagnostic: text from the compiler (rendered, shown verbatim), with a
  // button that moves the cursor to its start. The gutter and underline use the
  // same offsets.
  function showProblems(diagnostics, source) {
    editor.setDiagnostics(toEditorRanges(diagnostics, source.length));
    if (diagnostics.length === 0) {
      problems.replaceChildren(h("li", { className: "pg-empty" }, "No problems."));
      return;
    }
    problems.replaceChildren(...diagnostics.map((d) =>
      h("li", { className: `pg-problem pg-${d.severity}` },
        h("button", {
          type: "button",
          className: "pg-jump",
          onclick: () => editor.jumpTo(d.start.offset),
        }, `${d.severity}: line ${d.start.line}, column ${d.start.column}`),
        h("pre", { className: "pg-rendered" }, d.rendered))));
  }

  // ---- Check ----
  function scheduleCheck() {
    clearTimeout(checkTimer);
    checkTimer = setTimeout(() => runCheck({ auto: true }), CHECK_DELAY_MS);
  }

  async function runCheck({ auto = false } = {}) {
    if (ownsRun) return;
    if (auto && engine.busy) {
      scheduleCheck();
      return;
    }
    if (!auto) {
      onAttempt?.();
      setStatus("Checking…");
    }
    const source = editor.getText();
    const outcome = await engine.check(source);
    // The engine has one worker, so another playground's check or run can
    // replace this one. An automatic check tries again later. A manual one
    // tells the learner.
    if (outcome.kind === "superseded") {
      if (auto) scheduleCheck();
      else setStatus("Replaced by a newer run. Check again.");
      return;
    }
    if (editor.getText() !== source) {
      if (!auto) setStatus("The code changed while it was being checked. Check again.");
      return;
    }
    if (outcome.kind === "error" || outcome.kind === "stopped") {
      if (!auto) setStatus(outcome.label);
      if (outcome.kind === "error") showResult(outcome);
      problems.replaceChildren(h("li", { className: "pg-empty" }, `Could not check the code: ${outcome.label}`));
      return;
    }
    showProblems(outcome.diagnostics, source);
    if (!auto) {
      const errors = outcome.diagnostics.filter((d) => d.severity === "error").length;
      setStatus(errors === 0 ? "No problems found."
        : `${errors} ${errors === 1 ? "problem" : "problems"} found.`);
    }
  }

  // ---- Run ----
  async function runProgram() {
    if (ownsRun) return;
    onAttempt?.();
    const source = editor.getText();
    ownsRun = true;
    asks = [];
    clearTimeout(checkTimer);
    result.hidden = true;
    showTranscript([]);
    setRunning(true);
    setStatus(interactive && !canInteract()
      ? "Running. Typed input is off here: start the page with scripts/serve-web.py."
      : "Running…");

    const outcome = await engine.run(source, { interactive, onAsk: showAsk });
    ownsRun = false;
    hideAsk();
    setRunning(false);

    if (outcome.kind === "superseded") {
      showTranscript(buildTranscript(liveOutput(), asks));
      setStatus(outcome.label);
      return;
    }
    const lines = RAN_KINDS.includes(outcome.kind) ? outcome.output : liveOutput();
    showTranscript(buildTranscript(lines, asks));
    showResult(outcome);
    if (RAN_KINDS.includes(outcome.kind)) showProblems(outcome.diagnostics, source);
    setStatus(outcome.label);
  }

  function stop() {
    if (ownsRun) engine.stop();
  }

  // ---- Ask ----
  function showAsk({ prompt, transcript: before }) {
    asks.push({ prompt, transcript: before, answer: undefined });
    showTranscript(buildTranscript(liveOutput(), asks));
    askLabel.textContent = prompt;
    askInput.value = "";
    askForm.hidden = false;
    setStatus("Waiting for your answer. Type it and press Send, or press No more input.");
    askInput.focus();
  }

  function sendAnswer(text) {
    if (!engine.answer(text)) {
      setStatus("Your answer could not be sent. Try a shorter answer.");
      return;
    }
    finishAsk(text);
  }

  function endInput() {
    if (engine.endInput()) finishAsk(null);
  }

  function finishAsk(answer) {
    asks[asks.length - 1].answer = answer;
    hideAsk();
    showTranscript(buildTranscript(liveOutput(), asks));
    setStatus("Running…");
    status.focus();
  }

  function hideAsk() {
    askForm.hidden = true;
  }

  // ---- Editing, drafts, reset ----
  function handleEdit(text) {
    editor.setDiagnostics([]);
    problems.replaceChildren(h("li", { className: "pg-empty" }, "Problems update when you pause typing."));
    scheduleCheck();
    if (persistDraft) queueDraft(text);
  }

  function queueDraft(text) {
    pendingDraft = text;
    clearTimeout(draftTimer);
    draftTimer = setTimeout(flushDraft, DRAFT_DELAY_MS);
  }

  function flushDraft() {
    clearTimeout(draftTimer);
    if (pendingDraft === null) return;
    const text = pendingDraft;
    pendingDraft = null;
    if (text === config.initialCode) progress.removeDraft(config.id);
    else progress.saveDraft(config.id, text);
  }

  if (persistDraft) window.addEventListener("pagehide", flushDraft);

  function reset() {
    if (ownsRun) return;
    clearTimeout(draftTimer);
    pendingDraft = null;
    if (persistDraft) progress.removeDraft(config.id);
    editor.setText(config.initialCode);
    note.textContent = "";
    result.hidden = true;
    showTranscript([]);
    setStatus("Reset to the starter code.");
    scheduleCheck();
  }

  scheduleCheck();

  return {
    element: root,
    getSource: () => editor.getText(),
    focus: () => editor.focus(),
  };
}
