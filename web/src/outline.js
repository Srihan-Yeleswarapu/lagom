// The course outline (/). It lists every lesson in order, grouped by topic,
// with each lesson's state. It also exports and imports local progress, so a
// learner can move it to another browser.
import { loadCourse, loadAllLessons, lessonHref } from "./content.js";
import { h } from "./dom.js";
import { progress } from "./runtime.js";
import { lessonFlags } from "./status.js";
import { STATUS_LABELS, statusOf } from "./progress.js";
import { mountSidebar, mountTopicBar, progressNote, showLoadError } from "./chrome.js";
import { mountSearch } from "./search.js";

const main = document.querySelector("main");

async function start() {
  try {
    const course = await loadCourse();
    const refreshSidebar = mountSidebar(course, null);
    mountTopicBar(course, null);
    const lessons = await loadAllLessons(course);
    render(course, lessons, refreshSidebar);
  } catch (error) {
    showLoadError(main, error);
  }
}

function render(course, lessons, refreshSidebar) {
  const list = h("div", { className: "topics" });
  const importStatus = h("p", { className: "meta", role: "status" });

  function renderList() {
    list.replaceChildren(...course.topics.map((topic) => topicSection(topic, lessons)));
  }

  function exportProgress() {
    const url = URL.createObjectURL(new Blob([progress.exportJson()], { type: "application/json" }));
    const link = h("a", { href: url, download: "lagom-progress.json" });
    document.body.append(link);
    link.click();
    link.remove();
    setTimeout(() => URL.revokeObjectURL(url), 1000);
  }

  async function importFile(event) {
    const input = event.target;
    const file = input.files?.[0];
    if (!file) return;
    try {
      const counts = progress.importJson(await file.text());
      importStatus.textContent = `Imported progress for ${counts.lessons} lessons and ${counts.drafts} saved drafts. Progress already in this browser is kept.`;
      renderList();
      refreshSidebar();
    } catch (error) {
      importStatus.textContent = `Could not import that file. ${error.message}`;
    }
    input.value = "";
  }

  const fileInput = h("input", {
    type: "file",
    accept: "application/json,.json",
    className: "sr-only",
    id: "import-file",
    "aria-label": "Progress file to import",
    onchange: importFile,
  });

  document.title = `${course.title}`;
  main.replaceChildren(
    h(
      "header",
      { className: "page-head" },
      h("h1", {}, course.title),
      h("p", { className: "lead" }, "Short lessons that teach Lagom one idea at a time. Each lesson has a playground you can run right there."),
    ),
    list,
    h(
      "section",
      { className: "progress-tools", "aria-labelledby": "progress-heading" },
      h("h2", { id: "progress-heading" }, "Your progress"),
      progressNote(progress),
      h(
        "div",
        { className: "row" },
        h("button", { type: "button", onclick: exportProgress }, "Export progress"),
        h("label", { className: "button-like", htmlFor: "import-file" }, "Import progress…"),
        fileInput,
      ),
      importStatus,
    ),
    h("p", { className: "practice-link" }, h("a", { href: "/practice" }, "Open the practice playground")),
  );
  renderList();
}

function topicSection(topic, lessons) {
  return h(
    "section",
    { className: "topic", id: topic.id },
    h("h2", {}, topic.title),
    h(
      "ol",
      { className: "lessons" },
      topic.lessons.map((id) => lessonItem(lessons[id])),
    ),
  );
}

function lessonItem(lesson) {
  const state = statusOf(lessonFlags(progress, lesson));
  return h(
    "li",
    { className: `lesson lesson-${state}` },
    h("a", { href: lessonHref(lesson.id), className: "lesson-title" }, lesson.title),
    h("p", { className: "objective" }, lesson.objective),
    h(
      "p",
      { className: "meta" },
      `About ${lesson.minutes} min · `,
      h("span", { className: "state" }, STATUS_LABELS[state]),
    ),
  );
}

mountSearch();
start();
