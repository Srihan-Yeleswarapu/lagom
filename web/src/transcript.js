// Builds the transcript the output panel shows for a run.
//
// The compiler's `output` array holds every `say` line and every `ask` prompt,
// in order, but never the learner's answers. Each ask also reports the joined
// output that came before its prompt. That lets each answer be placed after its
// own prompt, even when a `say` line contains a newline.
//
// Input:
//   output: string[]                 lines the program produced so far
//   asks:   [{ prompt, transcript, answer }]
//           transcript: joined output before this prompt ("" when none)
//           answer:     string (the learner's reply), null (end of input),
//                       or undefined (still waiting)
//
// Output entries, in order:
//   { kind: "say" | "prompt" | "answer" | "end", text }
// "end" is the note shown when input ended. A prompt is also shown while it
// waits, even before the compiler has returned output.
export function buildTranscript(output, asks = []) {
  const entries = [];
  let pos = 0;

  for (const ask of asks) {
    const at = findPromptIndex(output, ask.transcript, pos);
    if (at === null) {
      // The output does not contain this prompt yet (a live, waiting ask).
      pushSays(entries, output.slice(pos));
      pos = output.length;
    } else {
      pushSays(entries, output.slice(pos, at));
      pos = at;
      if (output[pos] === ask.prompt) pos += 1;
    }
    entries.push({ kind: "prompt", text: ask.prompt });
    if (typeof ask.answer === "string") {
      entries.push({ kind: "answer", text: ask.answer });
    } else if (ask.answer === null) {
      entries.push({ kind: "end", text: "No more input. The program needs another answer." });
    }
  }

  pushSays(entries, output.slice(pos));
  return entries;
}

// Smallest n >= from such that output.slice(0, n) joins to `transcript`.
// Returns null if there is none.
function findPromptIndex(output, transcript, from) {
  for (let n = from; n <= output.length; n += 1) {
    if (output.slice(0, n).join("\n") === transcript) return n;
  }
  return null;
}

function pushSays(entries, lines) {
  for (const text of lines) entries.push({ kind: "say", text });
}

// Plain text of the transcript, one line per entry (for a live region or a copy).
export function transcriptText(entries) {
  return entries.map((entry) => entry.text).join("\n");
}
