// Small DOM pieces shared by the outline, lesson, and practice pages.
import { h } from "./dom.js";
import { loadAllLessons, lessonHref } from "./content.js";
import { lessonFlags } from "./status.js";
import { statusOf } from "./progress.js";
import { progress } from "./runtime.js";

// Says where progress lives. The learner should know it is local and can be lost.
export function progressNote(progress) {
  return h(
    "p",
    { className: "note" },
    "Your progress and saved code are kept only in this browser. Clearing site data or using a private window can lose them. Use Export progress on the course outline to keep a copy.",
    progress.persistent
      ? null
      : h("strong", {}, " This browser is not saving data, so progress will not survive a reload."),
  );
}

export function showLoadError(container, error) {
  container.replaceChildren(
    h("p", { className: "error", role: "alert" }, `This page could not load. ${error.message}`),
  );
}

// The row of topic tabs under the header (like W3Schools' category bar). Each
// tab opens the first lesson of its topic. The topic that holds the current
// page is marked with aria-current. Pass null on the outline page.
export function mountTopicBar(course, activeTopicId) {
  const bar = document.querySelector("#topic-bar");
  if (!bar) return;
  bar.replaceChildren(
    h(
      "ul",
      { className: "topic-tabs" },
      course.topics.map((topic) =>
        h(
          "li",
          {},
          h(
            "a",
            {
              href: lessonHref(topic.lessons[0]),
              className: "topic-tab",
              "aria-current": topic.id === activeTopicId ? "page" : null,
            },
            topic.title,
          ),
        ),
      ),
    ),
  );
}

// The lesson list shown beside every page. Each lesson shows its state, and the
// lesson open now is marked with aria-current.
function sidebarTopics(course, lessons, activeId) {
  return course.topics.map((topic) =>
    h(
      "section",
      { className: "side-topic" },
      h("h2", {}, topic.title),
      h(
        "ol",
        { className: "side-lessons" },
        topic.lessons.map((id) => {
          const lesson = lessons[id];
          const state = statusOf(lessonFlags(progress, lesson));
          return h(
            "li",
            { className: `side-lesson lesson-${state}` },
            h(
              "a",
              {
                href: lessonHref(id),
                className: "side-link",
                "aria-current": id === activeId ? "page" : null,
              },
              lesson.title,
            ),
          );
        }),
      ),
    ),
  );
}

// Keeps the lesson being read in view. Every lesson link is a full page load,
// so the sidebar would otherwise jump back to the top on each click. The lesson
// before the active one is put at the top of the sidebar, with the active
// lesson (the green highlighted one) directly below it, as close to where the
// reader was as the list allows. Runs once after the lessons load, so later
// refreshes never move a list the reader has scrolled by hand.
function scrollToActive(aside, details) {
  if (!details.open) return;
  const current = details.querySelector('.side-link[aria-current="page"]');
  if (!current) return;
  const rows = Array.from(details.querySelectorAll(".side-lesson"));
  const index = rows.findIndex((row) => row.contains(current));
  if (index < 0) return;
  // Where an element sits in the sidebar's own scroll space, so the answer does
  // not depend on where the sidebar is scrolled right now.
  const topOf = (element) =>
    element.getBoundingClientRect().top - aside.getBoundingClientRect().top + aside.scrollTop;
  const hasPrevious = index > 0;
  const anchor = hasPrevious ? rows[index - 1] : rows[index];
  const gap = hasPrevious ? 8 : 16;
  aside.scrollTop = Math.max(0, topOf(anchor) - gap);
}

// Fills the sidebar and returns refresh(), which redraws the lesson states after
// progress changes. On narrow screens the list is a disclosure that starts closed.
// On wide screens it is always open.
export function mountSidebar(course, activeId) {
  const aside = document.querySelector("#sidebar");
  if (!aside) return () => {};
  const wide = window.matchMedia("(min-width: 900px)");
  let lessons = null;
  const details = h("details", { className: "side-details" });
  const summary = h("summary", { className: "side-summary" }, "Lessons");
  details.append(summary);

  const fill = () => {
    details.querySelectorAll(".side-topic").forEach((node) => node.remove());
    if (lessons) details.append(...sidebarTopics(course, lessons, activeId));
  };
  const syncOpen = () => {
    details.open = wide.matches;
  };

  aside.replaceChildren(details);
  syncOpen();
  wide.addEventListener("change", syncOpen);

  loadAllLessons(course).then(
    (loaded) => {
      lessons = loaded;
      fill();
      scrollToActive(aside, details);
    },
    (error) => {
      details.append(h("p", { className: "error", role: "alert" }, `Lessons could not load. ${error.message}`));
    },
  );
  return fill;
}
