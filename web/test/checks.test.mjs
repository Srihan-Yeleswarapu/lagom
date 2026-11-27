import { test } from "node:test";
import assert from "node:assert/strict";
import { allPassed, evaluateChecks } from "../src/checks.js";

// A runner that records what it was asked to run.
function fakeRunner({ compiles = true, outcome } = {}) {
  const runs = [];
  return {
    runs,
    check: async () => ({ kind: compiles ? "ok" : "errors", diagnostics: [] }),
    run: async (source, options) => {
      runs.push({ source, stdin: options.stdin });
      return outcome ?? { kind: "completed", label: "Finished.", output: ["Score: 5"], message: null };
    },
  };
}

const exercise = {
  checks: [
    { id: "compiles", description: "compiles", kind: "compiles" },
    { id: "uses", description: "uses increase", kind: "source", matches: "^increase score by 5$", flags: "m" },
    { id: "output", description: "prints", kind: "output", stdin: ["blue"], expect: ["Score: 5"] },
  ],
};

test("all checks pass for a correct program", async () => {
  const runner = fakeRunner();
  const results = await evaluateChecks(exercise, "increase score by 5", runner);
  assert.deepEqual(
    results.map((r) => [r.id, r.passed]),
    [
      ["compiles", true],
      ["uses", true],
      ["output", true],
    ],
  );
  assert.equal(allPassed(results), true);
});

test("the stdin lines are passed to the run joined by newlines", async () => {
  const runner = fakeRunner();
  await evaluateChecks(
    { checks: [{ id: "o", description: "", kind: "output", stdin: ["a", "b"], expect: ["Score: 5"] }] },
    "src",
    runner,
  );
  assert.deepEqual(runner.runs, [{ source: "src", stdin: "a\nb" }]);
});

test("output checks do not run a program that does not compile", async () => {
  const runner = fakeRunner({ compiles: false });
  const results = await evaluateChecks(exercise, "bad", runner);
  assert.equal(runner.runs.length, 0);
  assert.equal(results[2].passed, false);
  assert.match(results[2].detail, /does not compile/);
  assert.equal(allPassed(results), false);
});

test("a run that does not complete fails the output check with the reason", async () => {
  const runner = fakeRunner({
    outcome: { kind: "panicked", label: "Stopped with an error.", output: [], message: "end of input" },
  });
  const [, , output] = await evaluateChecks(exercise, "increase score by 5", runner);
  assert.equal(output.passed, false);
  assert.match(output.detail, /Stopped with an error\. end of input/);
});

test("a wrong output shows what was expected and what was produced", async () => {
  const runner = fakeRunner({
    outcome: { kind: "completed", label: "Finished.", output: ["Score: 4"], message: null },
  });
  const [, , output] = await evaluateChecks(exercise, "increase score by 5", runner);
  assert.equal(output.passed, false);
  assert.match(output.detail, /"Score: 5"/);
  assert.match(output.detail, /"Score: 4"/);
});

test("a source check reports a bad regular expression instead of throwing", async () => {
  const [result] = await evaluateChecks(
    { checks: [{ id: "bad", description: "", kind: "source", matches: "(" }] },
    "x",
    fakeRunner(),
  );
  assert.equal(result.passed, false);
  assert.match(result.detail, /Bad check definition/);
});

test("an unknown check kind fails rather than passing silently", async () => {
  const [result] = await evaluateChecks({ checks: [{ id: "x", description: "", kind: "magic" }] }, "x", fakeRunner());
  assert.equal(result.passed, false);
});

test("an exercise with no checks is never complete", () => {
  assert.equal(allPassed([]), false);
  assert.equal(allPassed([{ passed: true }, { passed: false }]), false);
});
