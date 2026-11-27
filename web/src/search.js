// Lesson search in the header. It matches lesson titles, objectives and topic
// titles. Every word of the query must match one of those fields. Lesson
// bodies are not searched, so results stay quick and predictable.
import { h } from "./dom.js";
import { flattenOutline, lessonHref, loadAllLessons, loadCourse } from "./content.js";

const MAX_RESULTS = 8;

// Matching lessons for a query, best first. A word found in the title counts
// double, and a word found in the objective or topic counts once. Ties keep
// outline order. The function is pure, so the Node tests can run it.
export function searchLessons(course, lessons, query, limit = MAX_RESULTS) {
  const words = query.trim().toLowerCase().split(/\s+/).filter(Boolean);
  if (words.length === 0) return [];
  const hits = [];
  for (const { id, topicTitle } of flattenOutline(course)) {
    const lesson = lessons[id];
    if (!lesson) continue;
    const title = lesson.title.toLowerCase();
    const rest = `${lesson.objective} ${topicTitle}`.toLowerCase();
    if (!words.every((word) => title.includes(word) || rest.includes(word))) continue;
    const score = words.reduce(
      (total, word) => total + (title.includes(word) ? 2 : 0) + (rest.includes(word) ? 1 : 0),
      0,
    );
    hits.push({ id, title: lesson.title, topicTitle, score });
  }
  return hits.sort((a, b) => b.score - a.score).slice(0, limit);
}

// Puts the search box into #site-search. Lesson text loads the first time the
// learner types, so pages that never search do not fetch every lesson.
export function mountSearch() {
  const host = document.querySelector("#site-search");
  if (!host) return;

  let course = null;
  let lessons = null;
  let ready = null;

  const input = h("input", {
    type: "search",
    id: "site-search-input",
    placeholder: "Search lessons",
    autocomplete: "off",
    "aria-label": "Search lessons",
    "aria-controls": "site-search-results",
    "aria-expanded": "false",
  });
  const results = h("ul", { id: "site-search-results", className: "search-results", "aria-live": "polite", hidden: true });

  function ensureLessons() {
    ready ??= loadCourse()
      .then(async (loaded) => {
        lessons = await loadAllLessons(loaded);
        course = loaded;
      })
      .catch((error) => {
        ready = null; // let the next keystroke try again
        throw error;
      });
    return ready;
  }

  function hide() {
    results.hidden = true;
    input.setAttribute("aria-expanded", "false");
  }

  function show(query) {
    const hits = searchLessons(course, lessons, query);
    results.replaceChildren(
      ...(hits.length
        ? hits.map((hit) =>
            h(
              "li",
              {},
              h("a", { href: lessonHref(hit.id) }, hit.title),
              h("span", { className: "search-topic" }, hit.topicTitle),
            ),
          )
        : [h("li", { className: "search-empty" }, `No lessons match “${query.trim()}”.`)]),
    );
    results.hidden = false;
    input.setAttribute("aria-expanded", "true");
  }

  input.addEventListener("input", async () => {
    if (!input.value.trim()) return hide();
    try {
      await ensureLessons();
    } catch {
      results.replaceChildren(h("li", { className: "search-empty" }, "Search is unavailable right now."));
      results.hidden = false;
      return;
    }
    show(input.value);
  });

  input.addEventListener("keydown", (event) => {
    if (event.key === "Escape") {
      input.value = "";
      hide();
    } else if (event.key === "Enter") {
      const first = results.querySelector("a");
      if (first) {
        event.preventDefault();
        location.href = first.href;
      }
    }
  });

  // Clicking anywhere else closes the results.
  document.addEventListener("click", (event) => {
    if (!host.contains(event.target)) hide();
  });

  host.replaceChildren(h("div", { className: "site-search-box", role: "search" }, input, results));
}
