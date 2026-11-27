import { test } from "node:test";
import assert from "node:assert/strict";
import { readFileSync, readdirSync } from "node:fs";
import { dirname, join } from "node:path";
import { fileURLToPath } from "node:url";
import { compilerCheck, compilerRun } from "./compiler.mjs";
import { toCheckOutcome, toRunOutcome } from "../src/outcome.js";
import { allPassed, evaluateChecks } from "../src/checks.js";

// The real compiler, through the same outcome conversion the browser uses.
const runner = {
  check: async (source) => toCheckOutcome(source, compilerCheck(source)),
  run: async (source, { stdin }) => toRunOutcome(source, compilerRun(source, stdin)),
};

const contentDir = join(dirname(fileURLToPath(import.meta.url)), "..", "content");
const course = JSON.parse(readFileSync(join(contentDir, "course.json"), "utf8"));
const lessonDir = join(contentDir, "lessons");

const lessons = Object.fromEntries(
  readdirSync(lessonDir)
    .filter((file) => file.endsWith(".json"))
    .map((file) => [file.slice(0, -".json".length), JSON.parse(readFileSync(join(lessonDir, file), "utf8"))]),
);

const RUN_MODES = ["check", "run", "both"];
const INPUT_MODES = ["none", "ask"];
const CHECK_KINDS = ["compiles", "output", "source"];

const blocksOf = (type) =>
  Object.entries(lessons).flatMap(([id, lesson]) =>
    lesson.body.filter((block) => block.type === type).map((block) => ({ lessonId: id, block })),
  );

test("the outline and the lesson files are the same set, and each file names itself", () => {
  const outline = course.topics.flatMap((topic) => topic.lessons);
  assert.equal(new Set(outline).size, outline.length, "a lesson is listed twice in the outline");
  assert.deepEqual([...outline].sort(), Object.keys(lessons).sort());
  for (const [id, lesson] of Object.entries(lessons)) {
    assert.equal(lesson.id, id, `lessons/${id}.json has id ${lesson.id}`);
  }
});

test("every lesson has the required fields and known block types", () => {
  for (const [id, lesson] of Object.entries(lessons)) {
    assert.ok(lesson.title, `${id}: title`);
    assert.ok(lesson.objective, `${id}: objective`);
    assert.ok(Number.isInteger(lesson.minutes) && lesson.minutes > 0, `${id}: minutes`);
    assert.ok(Array.isArray(lesson.body) && lesson.body.length > 0, `${id}: body`);
    for (const block of lesson.body) {
      assert.ok(["p", "example", "exercise"].includes(block.type), `${id}: block type ${block.type}`);
    }
  }
  for (const { lessonId, block } of blocksOf("example")) {
    assert.ok(INPUT_MODES.includes(block.inputMode), `${lessonId}: example inputMode`);
    assert.ok(Array.isArray(block.expected), `${lessonId}: example expected`);
  }
  for (const { lessonId, block } of blocksOf("exercise")) {
    assert.ok(RUN_MODES.includes(block.runMode), `${lessonId}: exercise runMode`);
    assert.ok(INPUT_MODES.includes(block.inputMode), `${lessonId}: exercise inputMode`);
    assert.ok(block.prompt && block.starter && block.solution, `${lessonId}: exercise fields`);
    assert.ok(block.checks.length > 0, `${lessonId}: exercise needs checks`);
    const ids = block.checks.map((check) => check.id);
    assert.equal(new Set(ids).size, ids.length, `${block.id}: duplicate check ids`);
    for (const check of block.checks) {
      assert.ok(CHECK_KINDS.includes(check.kind), `${block.id}: check kind ${check.kind}`);
    }
  }
});

test("exercise ids are unique across the course", () => {
  const ids = blocksOf("exercise").map(({ block }) => block.id);
  assert.equal(new Set(ids).size, ids.length);
});

test("every example prints exactly its expected output", async () => {
  for (const { lessonId, block } of blocksOf("example")) {
    const check = await runner.check(block.code);
    assert.equal(check.kind, "ok", `${lessonId}: example does not compile`);
    const outcome = await runner.run(block.code, { stdin: (block.stdin ?? []).join("\n") });
    assert.equal(outcome.kind, "completed", `${lessonId}: example did not complete (${outcome.label})`);
    assert.deepEqual(outcome.output, block.expected, `${lessonId}: example output`);
  }
});

test("every exercise's solution passes all of its checks", async () => {
  for (const { lessonId, block } of blocksOf("exercise")) {
    const results = await evaluateChecks(block, block.solution, runner);
    const failed = results.filter((r) => !r.passed).map((r) => `${r.id}: ${r.detail}`);
    assert.ok(allPassed(results), `${block.id} solution fails: ${failed.join("; ")}`);
  }
});

test("every exercise's starter fails at least one check, so there is something to do", async () => {
  for (const { block } of blocksOf("exercise")) {
    const results = await evaluateChecks(block, block.starter, runner);
    assert.equal(allPassed(results), false, `${block.id}: starter already passes every check`);
  }
});

test("no starter is its own solution, so the exercise never hands over the answer", () => {
  for (const { block } of blocksOf("exercise")) {
    assert.notEqual(block.starter, block.solution, `${block.id}: the starter equals the solution`);
  }
});

test("no example is an exercise's solution word for word, so the lesson above never shows the answer", () => {
  for (const [lessonId, lesson] of Object.entries(lessons)) {
    const solutions = lesson.body
      .filter((block) => block.type === "exercise")
      .map((block) => block.solution);
    for (const block of lesson.body) {
      if (block.type !== "example") continue;
      for (const solution of solutions) {
        assert.notEqual(block.code, solution, `${lessonId}: an example is an exercise's exact solution`);
      }
    }
  }
});

test("the example right above an exercise never prints the exercise's expected output", () => {
  for (const [lessonId, lesson] of Object.entries(lessons)) {
    lesson.body.forEach((block, index) => {
      if (block.type !== "exercise") return;
      const before = lesson.body.slice(0, index).filter((b) => b.type === "example");
      const last = before[before.length - 1];
      if (!last) return;
      const output = block.checks.find((check) => check.kind === "output");
      if (!output) return;
      assert.notDeepEqual(
        last.expected,
        output.expect,
        `${block.id}: the example right above prints exactly the exercise's expected output`,
      );
    });
  }
});
