// Local learning progress. Everything lives in this browser's localStorage (or
// any object with getItem/setItem/removeItem, which keeps this testable).
// Nothing is sent to a server.
//
// Three states, kept separate on purpose:
//   visited    the lesson page was opened
//   attempted  the learner ran or checked code in one of its playgrounds
//   completed  an exercise's checks all passed, or the learner pressed
//              "Mark complete" on a reading-only lesson
//
// Drafts are saved source per playground id, so a revisit can show the
// learner's own edit. "Reset to starter code" removes the draft. Progress is
// separate, and "Clear progress" on a lesson removes its three flags.

export const PROGRESS_KEY = "lagom.progress.v1";
export const DRAFTS_KEY = "lagom.drafts.v1";
const EXPORT_FORMAT = "lagom-progress";
const EXPORT_VERSION = 1;

const EMPTY = { visited: false, attempted: false, completed: false };

// One label for a lesson's state. A lesson shows its highest state, so the
// outline shows "Completed" rather than all three flags at once.
export const STATUS_LABELS = {
  "not-started": "Not started",
  visited: "Visited",
  attempted: "Attempted",
  completed: "Completed",
};

export function statusOf(flags) {
  if (flags?.completed) return "completed";
  if (flags?.attempted) return "attempted";
  if (flags?.visited) return "visited";
  return "not-started";
}

export function createProgressStore(storage) {
  let persistent = true;

  function read(key, fallback) {
    try {
      const raw = storage.getItem(key);
      if (raw === null || raw === undefined) return fallback;
      const parsed = JSON.parse(raw);
      return parsed && typeof parsed === "object" ? parsed : fallback;
    } catch {
      return fallback;
    }
  }

  function write(key, value) {
    try {
      storage.setItem(key, JSON.stringify(value));
    } catch {
      // Storage full or blocked. Progress still works for this page visit.
      persistent = false;
    }
  }

  function lessons() {
    return read(PROGRESS_KEY, {}).lessons ?? {};
  }

  function setFlag(lessonId, flag) {
    const all = lessons();
    const current = { ...EMPTY, ...all[lessonId] };
    if (current[flag]) return current;
    current[flag] = true;
    write(PROGRESS_KEY, { lessons: { ...all, [lessonId]: current } });
    return current;
  }

  return {
    // True when the last write succeeded. Otherwise the UI should warn that
    // progress will not survive a reload.
    get persistent() {
      return persistent;
    },

    get(lessonId) {
      return { ...EMPTY, ...lessons()[lessonId] };
    },

    markVisited: (lessonId) => setFlag(lessonId, "visited"),
    markAttempted: (lessonId) => setFlag(lessonId, "attempted"),
    markCompleted: (lessonId) => setFlag(lessonId, "completed"),

    // Clear the three flags for one lesson. Drafts are left alone.
    clearLesson(lessonId) {
      const all = { ...lessons() };
      delete all[lessonId];
      write(PROGRESS_KEY, { lessons: all });
    },

    getDraft(playgroundId) {
      const value = read(DRAFTS_KEY, {})[playgroundId];
      return typeof value === "string" ? value : null;
    },

    saveDraft(playgroundId, source) {
      write(DRAFTS_KEY, { ...read(DRAFTS_KEY, {}), [playgroundId]: source });
    },

    removeDraft(playgroundId) {
      const drafts = { ...read(DRAFTS_KEY, {}) };
      delete drafts[playgroundId];
      write(DRAFTS_KEY, drafts);
    },

    // A JSON document the learner can save and import on another browser.
    exportJson() {
      return JSON.stringify(
        {
          format: EXPORT_FORMAT,
          version: EXPORT_VERSION,
          lessons: lessons(),
          drafts: read(DRAFTS_KEY, {}),
        },
        null,
        2,
      );
    },

    // Merge an exported document into this browser. Flags are OR-ed, so nothing
    // already completed is lost. A draft is taken only where this browser has
    // none. Returns { lessons, drafts } counts, or throws on a bad document.
    importJson(text) {
      const doc = JSON.parse(text);
      if (!doc || doc.format !== EXPORT_FORMAT || doc.version !== EXPORT_VERSION) {
        throw new Error("This file is not a Lagom progress export.");
      }
      const incoming = doc.lessons && typeof doc.lessons === "object" ? doc.lessons : {};
      const incomingDrafts = doc.drafts && typeof doc.drafts === "object" ? doc.drafts : {};

      const merged = { ...lessons() };
      let lessonCount = 0;
      for (const [id, flags] of Object.entries(incoming)) {
        if (!flags || typeof flags !== "object") continue;
        const current = { ...EMPTY, ...merged[id] };
        merged[id] = {
          visited: current.visited || flags.visited === true,
          attempted: current.attempted || flags.attempted === true,
          completed: current.completed || flags.completed === true,
        };
        lessonCount += 1;
      }
      write(PROGRESS_KEY, { lessons: merged });

      const drafts = { ...read(DRAFTS_KEY, {}) };
      let draftCount = 0;
      for (const [id, source] of Object.entries(incomingDrafts)) {
        if (typeof source === "string" && drafts[id] === undefined) {
          drafts[id] = source;
          draftCount += 1;
        }
      }
      write(DRAFTS_KEY, drafts);
      return { lessons: lessonCount, drafts: draftCount };
    },
  };
}
