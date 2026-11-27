// The practice page (/practice). It is the same playground the lessons use, on
// its own: free editing, Run, and interactive ask. There are no answers to
// check here, so the Check button stays out. The draft is kept in this browser.
import { h } from "./dom.js";
import { engine, progress } from "./runtime.js";
import { createPlayground } from "./playground.js";
import { loadCourse } from "./content.js";
import { mountSidebar, mountTopicBar, progressNote } from "./chrome.js";
import { mountSearch } from "./search.js";

const STARTER = 'say "Hello, world!"\n';

const playground = createPlayground(
  {
    id: "practice.scratch",
    initialCode: STARTER,
    editable: true,
    runMode: "run",
    inputMode: "ask",
    heightHint: 14,
    label: "Practice program",
    saveDraft: true,
  },
  { engine, progress },
);

document.querySelector("main").replaceChildren(
  h(
    "header",
    { className: "page-head" },
    h("p", { className: "crumb" }, h("a", { href: "/" }, "Course outline")),
    h("h1", {}, "Practice"),
    h("p", { className: "lead" }, "Write any Lagom program here. Run it or try ask. Your code is saved in this browser."),
  ),
  playground.element,
  progressNote(progress),
);

mountSearch();

loadCourse().then(
  (course) => {
    mountTopicBar(course, null);
    return mountSidebar(course, null);
  },
  (error) =>
    document.querySelector("#sidebar").replaceChildren(
      h("p", { className: "error", role: "alert" }, `Lessons could not load. ${error.message}`),
    ),
);
