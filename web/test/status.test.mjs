import { test } from "node:test";
import assert from "node:assert/strict";
import { createProgressStore } from "../src/progress.js";
import { examplePlaygroundId, lessonFlags, resetLessonProgress } from "../src/status.js";

class MemoryStorage {
  constructor() {
    this.map = new Map();
  }
  getItem(key) {
    return this.map.has(key) ? this.map.get(key) : null;
  }
  setItem(key, value) {
    this.map.set(key, String(value));
  }
  removeItem(key) {
    this.map.delete(key);
  }
}

const reading = {
  id: "basics.print",
  body: [{ type: "p", text: "Read me." }],
};

const withExercises = {
  id: "basics.ask",
  body: [
    { type: "p", text: "Ask things." },
    { type: "example", code: "say \"hi\"\n" },
    { type: "exercise", id: "basics.ask.colour" },
    { type: "exercise", id: "basics.ask.food" },
  ],
};

test("a reading-only lesson is complete only after Mark complete", () => {
  const progress = createProgressStore(new MemoryStorage());
  progress.markVisited(reading.id);
  assert.deepEqual(lessonFlags(progress, reading), { visited: true, attempted: false, completed: false });
  progress.markCompleted(reading.id);
  assert.equal(lessonFlags(progress, reading).completed, true);
});

test("a lesson with exercises is complete when every exercise is complete", () => {
  const progress = createProgressStore(new MemoryStorage());
  progress.markCompleted("basics.ask.colour");
  assert.equal(lessonFlags(progress, withExercises).completed, false);
  progress.markCompleted("basics.ask.food");
  assert.equal(lessonFlags(progress, withExercises).completed, true);
});

test("reset clears flags, exercise completion, and drafts, and leaves other lessons alone", () => {
  const storage = new MemoryStorage();
  const progress = createProgressStore(storage);
  progress.markVisited(withExercises.id);
  progress.markAttempted(withExercises.id);
  progress.markCompleted("basics.ask.colour");
  progress.saveDraft("basics.ask.colour", "say 1");
  progress.saveDraft(examplePlaygroundId(withExercises.id, 1), "say 2");
  progress.markCompleted(reading.id);

  resetLessonProgress(progress, withExercises);

  assert.deepEqual(lessonFlags(progress, withExercises), { visited: false, attempted: false, completed: false });
  assert.equal(progress.get("basics.ask.colour").completed, false);
  assert.equal(progress.getDraft("basics.ask.colour"), null);
  assert.equal(progress.getDraft(examplePlaygroundId(withExercises.id, 1)), null);
  assert.equal(progress.get(reading.id).completed, true);
});

test("example draft ids include the body position so two examples never share a draft", () => {
  assert.notEqual(examplePlaygroundId("a", 1), examplePlaygroundId("a", 3));
});
