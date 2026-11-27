import { test } from "node:test";
import assert from "node:assert/strict";
import { readFileSync, readdirSync } from "node:fs";
import { dirname, join } from "node:path";
import { fileURLToPath } from "node:url";
import { searchLessons } from "../src/search.js";

const contentDir = join(dirname(fileURLToPath(import.meta.url)), "..", "content");
const course = JSON.parse(readFileSync(join(contentDir, "course.json"), "utf8"));
const lessonDir = join(contentDir, "lessons");
const lessons = Object.fromEntries(
  readdirSync(lessonDir)
    .filter((file) => file.endsWith(".json"))
    .map((file) => [file.slice(0, -".json".length), JSON.parse(readFileSync(join(lessonDir, file), "utf8"))]),
);

test("an empty or blank query finds nothing", () => {
  assert.deepEqual(searchLessons(course, lessons, ""), []);
  assert.deepEqual(searchLessons(course, lessons, "   "), []);
});

test("a title word finds its lesson, and matching ignores case", () => {
  const hits = searchLessons(course, lessons, "TRANSFORM");
  assert.equal(hits[0].id, "functions.closures");
});

test("every word must match, so an unrelated second word removes the hit", () => {
  assert.deepEqual(searchLessons(course, lessons, "closures zebra"), []);
});

test("a topic title matches its lessons", () => {
  const ids = searchLessons(course, lessons, "functions", 50).map((hit) => hit.id);
  assert.ok(ids.includes("functions.define"));
  assert.ok(ids.includes("functions.closures"));
});

test("title matches rank above objective-only matches", () => {
  const hits = searchLessons(course, lessons, "list", 50);
  const title = hits.findIndex((hit) => lessons[hit.id].title.toLowerCase().includes("list"));
  const objectiveOnly = hits.findIndex((hit) => !lessons[hit.id].title.toLowerCase().includes("list"));
  assert.ok(title >= 0, "a lesson with list in its title is found");
  if (objectiveOnly >= 0) assert.ok(title < objectiveOnly);
});

test("results are capped at the limit and carry the lesson title and topic", () => {
  const hits = searchLessons(course, lessons, "a", 3);
  assert.ok(hits.length <= 3);
  for (const hit of hits) {
    assert.equal(hit.title, lessons[hit.id].title);
    assert.ok(hit.topicTitle);
  }
});

test("a query that matches nothing returns no results", () => {
  assert.deepEqual(searchLessons(course, lessons, "qqqzzz"), []);
});
