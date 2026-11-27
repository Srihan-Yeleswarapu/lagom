// Runs the Lagom compiler and interpreter off the main thread, so long or
// looping programs never freeze the page. The page stops a runaway program by
// terminating this worker.
//
// Messages in:  { id, type: "check", source }
//               { id, type: "run", source, stdin? }       fixed answers; ask fails at end
//               { id, type: "run", source, channel }        interactive ask
//                 channel: SharedArrayBuffer, layout below
// Messages out: { id, ok: true, result } | { id, ok: false, error }
//               { id, type: "ask", prompt, transcript } while a run waits
// `result` is the parsed JSON document returned by lagom_web's check/run.
//
// Interactive ask: while a run is paused on `ask`, this worker blocks on
// Atomics.wait. The page writes the answer into the shared channel and wakes
// it, or marks end of input.
import init, { check, run, run_interactive } from "./pkg/lagom_web.js";

const ready = init();

// Channel layout: Int32 control words at byte 0, then answer bytes from byte 8.
const CTL_STATE = 0; // 0 = waiting for the page, 1 = answered, 2 = end of input
const CTL_LENGTH = 1; // byte length of the answer
const CHANNEL_DATA_OFFSET = 8;

let currentId = 0;
let ctl = null;
let bytes = null;
const decoder = new TextDecoder();

// Called by the wasm module (lagom_web's `js_ask_line`). It must return a
// string, or null at end of input.
self.lagomAskLine = (prompt, transcript) => {
  Atomics.store(ctl, CTL_STATE, 0);
  self.postMessage({ id: currentId, type: "ask", prompt, transcript });
  while (Atomics.load(ctl, CTL_STATE) === 0) {
    Atomics.wait(ctl, CTL_STATE, 0);
  }
  if (Atomics.load(ctl, CTL_STATE) !== 1) return null;
  const length = Atomics.load(ctl, CTL_LENGTH);
  // TextDecoder rejects views on shared memory, so copy the answer out first.
  // slice() returns a non-shared copy.
  return decoder.decode(bytes.slice(CHANNEL_DATA_OFFSET, CHANNEL_DATA_OFFSET + length));
};

self.onmessage = async (event) => {
  const { id, type, source } = event.data;
  currentId = id;
  try {
    await ready;
    let json;
    if (type === "run" && event.data.channel) {
      ctl = new Int32Array(event.data.channel, 0, 2);
      bytes = new Uint8Array(event.data.channel);
      json = run_interactive(source);
    } else if (type === "run") {
      json = run(source, event.data.stdin ?? "");
    } else {
      json = check(source);
    }
    self.postMessage({ id, ok: true, result: JSON.parse(json) });
  } catch (error) {
    self.postMessage({ id, ok: false, error: String(error?.message ?? error) });
  }
};
