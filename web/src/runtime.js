// The one compiler engine and progress store a page uses. Every playground on a
// page shares the engine, so there is one worker to start and stop.
//
// Because the engine supersedes a request when a new one starts, starting a run
// in one playground ends a run in another. The second playground shows
// "Replaced by a newer run." Automatic problem checks never supersede anything;
// they wait until the engine is idle.
import { createEngine } from "./engine.js";
import { createProgressStore } from "./progress.js";

export const engine = createEngine({
  createWorker: () =>
    new Worker(new URL("../runner.worker.js", import.meta.url), { type: "module" }),
});

// The same surface the exercise checks use: check(source) and run(source, { stdin }).
export const runner = {
  check: (source) => engine.check(source),
  run: (source, { stdin = "" } = {}) => engine.run(source, { stdin }),
};

// localStorage can be blocked (some privacy modes). Fall back to memory, so the
// site still works for the visit; progress then lasts only until the page closes.
function browserStorage() {
  try {
    const storage = window.localStorage;
    storage.getItem("lagom.probe");
    return storage;
  } catch {
    const values = new Map();
    return {
      getItem: (key) => (values.has(key) ? values.get(key) : null),
      setItem: (key, value) => values.set(key, String(value)),
      removeItem: (key) => values.delete(key),
    };
  }
}

export const progress = createProgressStore(browserStorage());
