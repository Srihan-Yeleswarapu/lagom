import { test } from "node:test";
import assert from "node:assert/strict";
import { createProgressStore, DRAFTS_KEY, PROGRESS_KEY } from "../src/progress.js";

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

const NONE = { visited: false, attempted: false, completed: false };

test("an unknown lesson has no progress", () => {
  assert.deepEqual(createProgressStore(new MemoryStorage()).get("basics.print"), NONE);
});

test("visited, attempted, and completed are separate flags", () => {
  const store = createProgressStore(new MemoryStorage());
  store.markVisited("basics.print");
  assert.deepEqual(store.get("basics.print"), { visited: true, attempted: false, completed: false });
  store.markAttempted("basics.print");
  assert.deepEqual(store.get("basics.print"), { visited: true, attempted: true, completed: false });
});

test("completing a lesson does not imply it was visited", () => {
  const store = createProgressStore(new MemoryStorage());
  store.markCompleted("basics.print");
  assert.deepEqual(store.get("basics.print"), { visited: false, attempted: false, completed: true });
});

test("progress survives a new store on the same storage", () => {
  const storage = new MemoryStorage();
  createProgressStore(storage).markCompleted("variables.make");
  assert.equal(createProgressStore(storage).get("variables.make").completed, true);
});

test("clearLesson removes one lesson's flags and leaves drafts alone", () => {
  const store = createProgressStore(new MemoryStorage());
  store.markCompleted("a");
  store.markVisited("b");
  store.saveDraft("a", "say 1");
  store.clearLesson("a");
  assert.deepEqual(store.get("a"), NONE);
  assert.equal(store.get("b").visited, true);
  assert.equal(store.getDraft("a"), "say 1");
});

test("drafts are keyed by playground id and can be removed", () => {
  const store = createProgressStore(new MemoryStorage());
  assert.equal(store.getDraft("p1"), null);
  store.saveDraft("p1", "one");
  store.saveDraft("p2", "two");
  assert.equal(store.getDraft("p1"), "one");
  store.removeDraft("p1");
  assert.equal(store.getDraft("p1"), null);
  assert.equal(store.getDraft("p2"), "two");
});

test("corrupt stored data reads as empty instead of throwing", () => {
  const storage = new MemoryStorage();
  storage.setItem(PROGRESS_KEY, "{not json");
  storage.setItem(DRAFTS_KEY, "[1,2");
  const store = createProgressStore(storage);
  assert.deepEqual(store.get("basics.print"), NONE);
  assert.equal(store.getDraft("p1"), null);
  store.markVisited("basics.print");
  assert.equal(store.get("basics.print").visited, true);
});

test("a failing write reports non-persistent progress", () => {
  const storage = new MemoryStorage();
  storage.setItem = () => {
    throw new Error("QuotaExceededError");
  };
  const store = createProgressStore(storage);
  assert.equal(store.persistent, true);
  store.markVisited("basics.print");
  assert.equal(store.persistent, false);
});

test("export then import merges flags and keeps existing drafts", () => {
  const source = createProgressStore(new MemoryStorage());
  source.markCompleted("basics.print");
  source.saveDraft("p1", "from source");
  source.saveDraft("p2", "only in source");
  const exported = source.exportJson();

  const target = createProgressStore(new MemoryStorage());
  target.markVisited("basics.print");
  target.saveDraft("p1", "mine");
  const counts = target.importJson(exported);

  assert.deepEqual(target.get("basics.print"), { visited: true, attempted: false, completed: true });
  assert.equal(target.getDraft("p1"), "mine");
  assert.equal(target.getDraft("p2"), "only in source");
  assert.deepEqual(counts, { lessons: 1, drafts: 1 });
});

test("importing never removes a completion that is already here", () => {
  const target = createProgressStore(new MemoryStorage());
  target.markCompleted("basics.print");
  const empty = createProgressStore(new MemoryStorage()).exportJson();
  target.importJson(empty);
  assert.equal(target.get("basics.print").completed, true);
});

test("importing rejects files that are not a progress export", () => {
  const store = createProgressStore(new MemoryStorage());
  assert.throws(() => store.importJson('{"format":"other","version":1}'), /not a Lagom progress export/);
  assert.throws(() => store.importJson('{"format":"lagom-progress","version":2}'), /not a Lagom progress export/);
  assert.throws(() => store.importJson("not json"), SyntaxError);
});
