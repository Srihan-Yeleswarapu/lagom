// Course content: the outline (content/course.json) and one JSON file per lesson
// (content/lessons/<id>.json). The browser fetches these as static files, so
// the site needs no server. JSON was chosen over Markdown or MDX because the
// lessons hold structured blocks (examples with expected output, exercises
// with checks) that a plain Markdown file cannot express cleanly, and because
// JSON needs no build step.
//
// The outline helpers are pure, so the Node tests can run them. The loaders
// use absolute paths, because lesson URLs (/lesson/<id>) sit one level deep.
const CONTENT = "/content";

// Every lesson in outline order, with the topic it belongs to.
export function flattenOutline(course) {
  return course.topics.flatMap((topic) =>
    topic.lessons.map((id) => ({ id, topicId: topic.id, topicTitle: topic.title })),
  );
}

// { prev, next } lesson ids around a lesson, or null if it is not in the outline.
export function neighbours(course, id) {
  const order = flattenOutline(course).map((entry) => entry.id);
  const at = order.indexOf(id);
  if (at < 0) return null;
  return {
    prev: at > 0 ? order[at - 1] : null,
    next: at < order.length - 1 ? order[at + 1] : null,
  };
}

export function lessonHref(id) {
  return `/lesson/${encodeURIComponent(id)}`;
}

async function fetchJson(url) {
  const response = await fetch(url);
  if (!response.ok) throw new Error(`Could not load ${url} (${response.status}).`);
  return response.json();
}

export function loadCourse() {
  return fetchJson(`${CONTENT}/course.json`);
}

export function loadLesson(id) {
  return fetchJson(`${CONTENT}/lessons/${encodeURIComponent(id)}.json`);
}

// Every lesson in the outline, keyed by id.
export async function loadAllLessons(course) {
  const entries = await Promise.all(
    flattenOutline(course).map(async ({ id }) => [id, await loadLesson(id)]),
  );
  return Object.fromEntries(entries);
}
