// Evaluates an exercise's checks against a learner's source.
//
// A check is one of:
//   { id, description, kind: "compiles" }
//       passes when the program has no compile errors
//   { id, description, kind: "output", stdin?: string[], expect: string[] }
//       passes when the program completes and its output lines equal `expect`
//       exactly. `output` lists every line the compiler reports, including ask
//       prompts (answers are not part of the output). Answers come from `stdin`.
//   { id, description, kind: "source", matches: string, flags?: string }
//       passes when the source matches the regular expression
//
// `runner` is { check(source), run(source, { stdin }) }, where each returns a
// canonical outcome (see outcome.js). Keeping it injectable lets Node tests run
// the same checks as the browser does.

export async function evaluateChecks(exercise, source, runner) {
  const results = [];
  const compile = await runner.check(source);
  // The compiler never reached the program: say why, rather than "does not compile".
  if (compile.kind === "superseded" || compile.kind === "error") {
    return exercise.checks.map((check) => ({
      id: check.id,
      description: check.description,
      passed: false,
      superseded: compile.kind === "superseded",
      detail: compile.label,
    }));
  }
  const compiles = compile.kind === "ok";

  for (const check of exercise.checks) {
    results.push(await evaluateOne(check, source, compiles, runner));
  }
  return results;
}

async function evaluateOne(check, source, compiles, runner) {
  const base = { id: check.id, description: check.description };

  if (check.kind === "compiles") {
    return { ...base, passed: compiles, detail: compiles ? null : "The program does not compile yet." };
  }

  if (check.kind === "source") {
    let regex;
    try {
      regex = new RegExp(check.matches, check.flags ?? "m");
    } catch (error) {
      return { ...base, passed: false, detail: `Bad check definition: ${error.message}` };
    }
    const passed = regex.test(source);
    return { ...base, passed, detail: passed ? null : "The code does not contain the required part yet." };
  }

  if (check.kind === "output") {
    if (!compiles) {
      return { ...base, passed: false, detail: "The program does not compile yet." };
    }
    const outcome = await runner.run(source, { stdin: (check.stdin ?? []).join("\n") });
    if (outcome.kind === "superseded") {
      return { ...base, passed: false, superseded: true, detail: outcome.label };
    }
    if (outcome.kind !== "completed") {
      const reason = outcome.message ? ` ${outcome.message}` : "";
      return { ...base, passed: false, detail: `${outcome.label}${reason}` };
    }
    const passed = sameLines(outcome.output, check.expect);
    return {
      ...base,
      passed,
      detail: passed ? null : `Expected ${JSON.stringify(check.expect)} but got ${JSON.stringify(outcome.output)}.`,
    };
  }

  return { ...base, passed: false, detail: `Unknown check kind "${check.kind}".` };
}

function sameLines(actual, expected) {
  return actual.length === expected.length && actual.every((line, i) => line === expected[i]);
}

// True when every check passed. An exercise with no checks is never complete.
export function allPassed(results) {
  return results.length > 0 && results.every((result) => result.passed);
}
