// The lesson page. The lesson id comes from the URL (/lesson/<id>), and its JSON
// file is fetched as static content. Blocks in the lesson body:
//   { type: "p", text }                             a paragraph
//   { type: "example", title, code, inputMode?, expected }
//                                                   a playground plus expected output
//   { type: "exercise", id, title?, prompt, starter, runMode?, inputMode?, checks }
//                                                   see exercise.js
// Opening the page marks the lesson visited. Running code marks it attempted.
// A reading-only lesson (no exercises) completes with "Mark complete". A lesson
// with exercises completes when all of them do.
import { loadCourse, loadLesson, neighbours, lessonHref, flattenOutline } from "./content.js";
import { h } from "./dom.js";
import { engine, runner, progress } from "./runtime.js";
import { createPlayground } from "./playground.js";
import { createExercise } from "./exercise.js";
import { exercisesOf, examplePlaygroundId, lessonFlags, resetLessonProgress } from "./status.js";
import { STATUS_LABELS, statusOf } from "./progress.js";
import { mountSidebar, mountTopicBar, progressNote, showLoadError } from "./chrome.js";
import { mountSearch } from "./search.js";

const main = document.querySelector("main");

function lessonIdFromUrl() {
  const match = /^\/lesson\/([^/]+)\/?$/.exec(location.pathname);
  return match ? decodeURIComponent(match[1]) : null;
}

async function start() {
  const id = lessonIdFromUrl();
  try {
    const course = await loadCourse();
    const position = id ? neighbours(course, id) : null;
    if (!position) throw new Error(`There is no lesson called "${id ?? ""}".`);
    const [lesson, prev, next] = await Promise.all([
      loadLesson(id),
      position.prev ? loadLesson(position.prev) : null,
      position.next ? loadLesson(position.next) : null,
    ]);
    render(course, lesson, { prev, next });
  } catch (error) {
    showLoadError(main, error);
  }
}

function render(course, lesson, { prev, next }) {
  const topic = flattenOutline(course).find((entry) => entry.id === lesson.id);
  document.title = `${lesson.title} · Learn Lagom`;
  progress.markVisited(lesson.id);

  const hasExercises = exercisesOf(lesson).length > 0;
  const statusLine = h("p", { className: "meta", role: "status" });
  const refreshSidebar = mountSidebar(course, lesson.id);
  mountTopicBar(course, topic?.topicId ?? null);
  let markButton = null;

  function refreshStatus() {
    const flags = lessonFlags(progress, lesson);
    statusLine.textContent = `Status: ${STATUS_LABELS[statusOf(flags)]}`;
    if (markButton) {
      markButton.disabled = flags.completed;
      markButton.textContent = flags.completed ? "Completed" : "Mark complete";
    }
    refreshSidebar();
  }

  const onAttempt = () => {
    progress.markAttempted(lesson.id);
    refreshStatus();
  };

  function renderExample(block, index) {
    const lines = block.code.split("\n").length;
    const playground = createPlayground(
      {
        id: examplePlaygroundId(lesson.id, index),
        initialCode: block.code,
        editable: true,
        // An example demonstrates working code; only exercises have answers
        // to check, so the Check button stays out of examples.
        runMode: "run",
        inputMode: block.inputMode ?? "none",
        heightHint: Math.min(Math.max(lines, 3), 14),
        label: `Example: ${block.title}`,
        saveDraft: true,
      },
      { engine, progress, onAttempt },
    );
    return h("section", { className: "example" },
      h("h2", {}, block.title ?? "Try it yourself"),
      playground.element,
      h("p", { className: "expected-label" }, "Expected output"),
      h("pre", { className: "expected" }, block.expected.join("\n")),
    );
  }

  function renderBlock(block, index) {
    if (block.type === "p") return h("p", {}, block.text);
    if (block.type === "example") return renderExample(block, index);
    if (block.type === "exercise") {
      return createExercise(block, { runner, engine, progress, onAttempt, onChange: refreshStatus }).element;
    }
    return null;
  }

  if (!hasExercises) {
    markButton = h("button", { type: "button", className: "mark-complete" }, "Mark complete");
    markButton.addEventListener("click", () => {
      progress.markCompleted(lesson.id);
      refreshStatus();
    });
  }

  const resetButton = h("button", { type: "button", className: "reset-lesson" }, "Clear progress for this lesson");
  resetButton.addEventListener("click", () => {
    if (!window.confirm("Clear this lesson's progress and saved code? Starter code will come back.")) return;
    resetLessonProgress(progress, lesson);
    // Reloading rebuilds every playground from its starter code.
    location.reload();
  });

  // Previous and Next. The same links appear above and below the lesson.
  const pager = () =>
    h(
      "nav",
      { className: "pager", "aria-label": "Lesson navigation" },
      prev
        ? h("a", { href: lessonHref(prev.id), className: "pager-link prev" }, h("span", { className: "pager-dir" }, "‹ Previous"), h("span", { className: "pager-title" }, prev.title))
        : h("span", { className: "pager-link pager-empty" }),
      next
        ? h("a", { href: lessonHref(next.id), className: "pager-link next" }, h("span", { className: "pager-dir" }, "Next ›"), h("span", { className: "pager-title" }, next.title))
        : h("span", { className: "pager-link pager-empty" }),
    );

  main.replaceChildren(
    h(
      "p",
      { className: "crumbs" },
      h("a", { href: "/" }, "Course"),
      " › ",
      h("a", { href: `/#${topic?.topicId ?? ""}` }, topic?.topicTitle ?? ""),
      " › ",
      lesson.title,
    ),
    h(
      "header",
      { className: "lesson-head" },
      h("h1", {}, lesson.title),
      h("p", { className: "objective" }, lesson.objective),
      h("p", { className: "meta" }, `About ${lesson.minutes} minutes`),
      statusLine,
    ),
    pager(),
    h("div", { className: "lesson-body" }, lesson.body.map(renderBlock)),
    h(
      "footer",
      { className: "lesson-foot" },
      hasExercises
        ? h("p", { className: "note" }, "Finish every exercise on this page to complete the lesson.")
        : markButton,
      resetButton,
      progressNote(progress),
    ),
    pager(),
  );
  refreshStatus();
}

mountSearch();
start();
