// An exercise: a prompt, a playground seeded with the starter code, and a list
// of checks. "Check my answer" runs every check against the learner's code.
// When all of them pass, the exercise is recorded as completed. That stays
// even if the learner later edits the code into a failing state. The lesson's
// "Clear progress" removes it.
//
// Checks run through the shared engine, and starting a run supersedes the one
// in progress. So a check never starts while the engine is busy, and it never
// cuts another playground's run short.
import { createPlayground } from "./playground.js";
import { evaluateChecks, allPassed } from "./checks.js";
import { h } from "./dom.js";

export function createExercise(exercise, { runner, engine, progress, onAttempt, onChange }) {
  let completed = progress.get(exercise.id).completed;

  const badge = h("span", { className: "ex-badge", hidden: !completed }, "Completed");
  const summary = h("p", { className: "ex-summary", role: "status" }, "Press Check my answer to test your code.");
  const checkButton = h("button", { type: "button", className: "ex-check" }, "Check my answer");
  const checkList = h(
    "ul",
    { className: "ex-checks", "aria-label": "Checks" },
    exercise.checks.map((check) =>
      h(
        "li",
        { className: "ex-pending", dataset: { checkId: check.id } },
        h("span", { className: "ex-mark", "aria-hidden": "true" }, "•"),
        h(
          "span",
          { className: "ex-text" },
          h("span", { className: "sr-only" }, "Not checked yet: "),
          check.description,
        ),
      ),
    ),
  );

  const playground = createPlayground(
    {
      id: exercise.id,
      initialCode: exercise.starter,
      editable: true,
      runMode: exercise.runMode ?? "both",
      inputMode: exercise.inputMode ?? "none",
      heightHint: 6,
      label: `Code for: ${exercise.prompt}`,
      saveDraft: true,
    },
    { engine, progress, onAttempt },
  );

  function setBusy(busy) {
    checkButton.disabled = busy;
    checkButton.textContent = busy ? "Checking…" : "Check my answer";
  }

  function renderResults(results) {
    checkList.replaceChildren(
      ...results.map((result) =>
        h(
          "li",
          { className: result.passed ? "ex-pass" : "ex-fail", dataset: { checkId: result.id } },
          h("span", { className: "ex-mark", "aria-hidden": "true" }, result.passed ? "✓" : "✗"),
          h(
            "span",
            { className: "ex-text" },
            h("span", { className: "sr-only" }, result.passed ? "Passed: " : "Not passed: "),
            result.description,
            result.detail ? h("span", { className: "ex-detail" }, ` ${result.detail}`) : null,
          ),
        ),
      ),
    );
  }

  async function checkAnswer() {
    if (engine.busy) {
      summary.textContent = "Wait for the current run to finish, then check again.";
      return;
    }
    progress.markAttempted(exercise.id);
    onAttempt?.();
    setBusy(true);
    summary.textContent = "Checking…";
    const results = await evaluateChecks(exercise, playground.getSource(), runner);
    setBusy(false);

    if (results.some((result) => result.superseded)) {
      summary.textContent = "Your check was replaced by a newer run. Check again.";
      return;
    }
    renderResults(results);
    const passing = results.filter((result) => result.passed).length;
    if (allPassed(results)) {
      summary.textContent = "All checks pass. Exercise complete.";
      if (!completed) {
        progress.markCompleted(exercise.id);
        completed = true;
        badge.hidden = false;
        onChange?.();
      }
    } else {
      summary.textContent = `${passing} of ${results.length} ${results.length === 1 ? "check" : "checks"} pass.`;
    }
  }

  checkButton.addEventListener("click", checkAnswer);

  const element = h(
    "section",
    { className: "exercise", "aria-labelledby": `ex-${exercise.id}-title` },
    h("h2", { id: `ex-${exercise.id}-title` }, exercise.title ?? "Exercise", " ", badge),
    h("p", { className: "ex-prompt" }, exercise.prompt),
    playground.element,
    h("div", { className: "ex-actions" }, checkButton, summary),
    checkList,
  );

  return { element, focus: () => playground.focus() };
}
