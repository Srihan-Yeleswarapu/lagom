// CodeMirror 6 language for Lagom. The tokenizer lives in lagom-lexical.js so
// the node tests can run it without CodeMirror. This file only maps its token
// names onto highlight tags.
import { StreamLanguage } from "@codemirror/language";
import { tags } from "@lezer/highlight";
import { startState, copyState, token } from "./lagom-lexical.js";

export const lagomLanguage = StreamLanguage.define({
  name: "lagom",
  startState,
  copyState,
  token,
  languageData: {
    commentTokens: { line: "#" },
  },
  tokenTable: {
    doc: tags.docComment,
    interp: tags.special(tags.variableName),
    builtin: tags.function(tags.variableName),
  },
});
