// Talks to the compiler worker (web/runner.worker.js). One engine owns one
// worker and runs at most one request at a time.
//
// Rules:
//   - Starting a request while another runs supersedes the old one. A paused
//     `ask` blocks the worker, so the only way to free it is to terminate it.
//     The superseded request resolves as "superseded" and its reply is dropped.
//   - Stop terminates the worker and resolves the run as "stopped". The next
//     request starts a fresh worker.
//   - Replies are matched by request id, so a stale reply is ignored.
//   - Interactive ask uses a SharedArrayBuffer. It needs cross-origin isolation
//     (scripts/serve-web.py sends the headers). Without it, `canInteract()` is
//     false and an interactive run gets no input.
//
// The channel layout must match runner.worker.js: Int32 control words at byte 0
// (state, length), then UTF-8 answer bytes from byte 8.
import { localOutcome, toCheckOutcome, toRunOutcome } from "./outcome.js";

const CTL_STATE = 0; // 0 waiting, 1 answered, 2 end of input
const CTL_LENGTH = 1;
const DATA_OFFSET = 8;
const DATA_BYTES = 4096;

// Longest answer the channel can carry, in UTF-8 bytes.
export const MAX_ANSWER_BYTES = DATA_BYTES;

export function canInteract() {
  return typeof SharedArrayBuffer !== "undefined" && globalThis.crossOriginIsolated === true;
}

// createWorker: () => Worker. Injected so Node tests can supply a fake.
export function createEngine({ createWorker }) {
  let worker = null;
  let pending = null; // { id, kind, source, resolve, onAsk, ctl, bytes, waitingAsk }
  let nextId = 1;
  const encoder = new TextEncoder();

  function ensureWorker() {
    if (worker) return worker;
    worker = createWorker();
    worker.onmessage = (event) => handleMessage(event.data);
    worker.onerror = (event) => {
      event.preventDefault?.();
      const message = event.message || "The compiler worker stopped unexpectedly.";
      finish(localOutcome("error", message), true);
    };
    return worker;
  }

  function terminateWorker() {
    if (worker) worker.terminate();
    worker = null;
  }

  // Resolve the pending request. With terminate, the worker is killed too.
  function finish(outcome, terminate) {
    const current = pending;
    pending = null;
    if (terminate) terminateWorker();
    current?.resolve(outcome);
  }

  function handleMessage(data) {
    if (!pending || data.id !== pending.id) return; // stale reply
    if (data.type === "ask") {
      pending.waitingAsk = true;
      pending.onAsk?.({ prompt: data.prompt, transcript: data.transcript });
      return;
    }
    if (!data.ok) {
      finish(localOutcome("error", data.error), false);
      return;
    }
    const outcome =
      pending.kind === "check"
        ? toCheckOutcome(pending.source, data.result)
        : toRunOutcome(pending.source, data.result);
    finish(outcome, false);
  }

  // Start a request. Any request already running is superseded first.
  function start(kind, source, options = {}) {
    if (pending) finish(localOutcome("superseded"), true);
    const id = nextId;
    nextId += 1;
    let resolveRequest;
    const resolved = new Promise((resolve) => {
      resolveRequest = resolve;
    });
    const w = ensureWorker();
    pending = {
      id,
      kind,
      source,
      resolve: resolveRequest,
      onAsk: options.onAsk,
      ctl: options.ctl,
      bytes: options.bytes,
      waitingAsk: false,
    };
    try {
      w.postMessage({ ...options.message, id, type: kind, source });
    } catch (error) {
      finish(localOutcome("error", String(error?.message ?? error)), true);
    }
    return resolved;
  }

  // Release the worker's wait with a state code, after any answer bytes are written.
  function release(state) {
    const { ctl } = pending;
    pending.waitingAsk = false;
    Atomics.store(ctl, CTL_STATE, state);
    Atomics.notify(ctl, CTL_STATE);
  }

  return {
    get busy() {
      return pending !== null;
    },
    get waitingForInput() {
      return pending !== null && pending.waitingAsk;
    },
    canInteract,

    check(source) {
      return start("check", source);
    },

    // Run a program. With interactive: true, each `ask` calls onAsk({ prompt,
    // transcript }) and waits for answer() or endInput(). Otherwise the `ask`
    // lines read `stdin` (one answer per line), and `ask` fails at the end.
    run(source, { interactive = false, onAsk, stdin = "" } = {}) {
      if (!interactive || !canInteract()) {
        return start("run", source, { message: { stdin } });
      }
      const channel = new SharedArrayBuffer(DATA_OFFSET + DATA_BYTES);
      return start("run", source, {
        onAsk,
        ctl: new Int32Array(channel, 0, 2),
        bytes: new Uint8Array(channel),
        message: { channel },
      });
    },

    // Send the learner's reply to the waiting ask. Returns false if no ask is
    // waiting or the reply is too long.
    answer(text) {
      if (!pending?.waitingAsk) return false;
      const encoded = encoder.encode(text);
      if (encoded.byteLength > DATA_BYTES) return false;
      pending.bytes.set(encoded, DATA_OFFSET);
      Atomics.store(pending.ctl, CTL_LENGTH, encoded.byteLength);
      release(1);
      return true;
    },

    // Tell the waiting ask that there is no more input.
    endInput() {
      if (!pending?.waitingAsk) return false;
      release(2);
      return true;
    },

    // Stop the current request. The worker is terminated, not just ignored, so
    // a runaway loop or a paused ask stops at once.
    stop() {
      if (!pending) return false;
      finish(localOutcome("stopped"), true);
      return true;
    },

    destroy() {
      if (pending) finish(localOutcome("superseded"), true);
      terminateWorker();
    },
  };
}
