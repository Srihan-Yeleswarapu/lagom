// The one CodeMirror entry point. esbuild bundles this file into
// web/vendor/editor.bundle.js, so the site needs no CDN or node_modules at
// runtime.
//
// The editor knows nothing about Lagom's compiler. It takes plain
// { from, to, severity, message } ranges as UTF-16 offsets. The diagnostics
// adapter in diagnostics.js produces those ranges.
import { Annotation, EditorState } from "@codemirror/state";
import {
  EditorView,
  keymap,
  lineNumbers,
  highlightActiveLineGutter,
  drawSelection,
} from "@codemirror/view";
import { defaultKeymap, history, historyKeymap } from "@codemirror/commands";
import { syntaxHighlighting, HighlightStyle } from "@codemirror/language";
import { lintGutter, setDiagnostics } from "@codemirror/lint";
import { tags } from "@lezer/highlight";
import { lagomLanguage } from "./lagom-language.js";

// Marks edits made by the page itself (setText), so they do not count as the
// learner typing. onChange is not called for them.
const external = Annotation.define();

const highlightStyle = HighlightStyle.define([
  { tag: tags.keyword, color: "#7c2d9a", fontWeight: "600" },
  { tag: tags.typeName, color: "#0b6e6e" },
  { tag: tags.atom, color: "#9a4b00" },
  { tag: tags.number, color: "#9a4b00" },
  { tag: tags.string, color: "#2c6e1f" },
  { tag: tags.special(tags.variableName), color: "#1d4ed8", fontWeight: "600" },
  { tag: tags.function(tags.variableName), color: "#1d4ed8", fontWeight: "600" },
  { tag: tags.comment, color: "#6b7280", fontStyle: "italic" },
  { tag: tags.docComment, color: "#4b5563", fontStyle: "italic" },
]);

// Options:
//   parent      element to mount into
//   doc         initial text
//   editable    false makes the text read-only but still selectable
//   label       accessible name for the text area
//   onChange    called with the new text after each edit
//   onRun       called on Mod-Enter (Ctrl+Enter, or Cmd+Enter on a Mac)
//
// Tab moves focus out of the editor rather than inserting a tab, so keyboard
// users are never trapped.
export function createEditor({ parent, doc, editable, label, onChange, onRun }) {
  const runKeymap = keymap.of([
    {
      key: "Mod-Enter",
      preventDefault: true,
      run: () => {
        onRun?.();
        return true;
      },
    },
  ]);

  const view = new EditorView({
    parent,
    doc,
    extensions: [
      runKeymap,
      lineNumbers(),
      highlightActiveLineGutter(),
      history(),
      drawSelection(),
      keymap.of([...defaultKeymap, ...historyKeymap]),
      lintGutter(),
      lagomLanguage,
      syntaxHighlighting(highlightStyle),
      EditorState.readOnly.of(!editable),
      EditorView.editable.of(editable),
      EditorView.contentAttributes.of({ "aria-label": label }),
      EditorView.updateListener.of((update) => {
        if (!update.docChanged) return;
        if (update.transactions.some((tr) => tr.annotation(external))) return;
        onChange?.(update.state.doc.toString());
      }),
      EditorView.theme({
        "&": { height: "100%", fontSize: "14px" },
        ".cm-scroller": { fontFamily: "ui-monospace, SFMono-Regular, Menlo, monospace" },
      }),
    ],
  });

  return {
    getText: () => view.state.doc.toString(),
    // Replace the whole text. Does not call onChange, so programmatic resets
    // do not count as edits.
    setText(text) {
      view.dispatch({
        changes: { from: 0, to: view.state.doc.length, insert: text },
        annotations: external.of(true),
      });
    },
    // ranges: [{ from, to, severity: "error" | "warning" | "info", message }]
    setDiagnostics(ranges) {
      const length = view.state.doc.length;
      const clean = ranges.map((range) => {
        const from = Math.min(Math.max(0, range.from), length);
        const to = Math.min(Math.max(from, range.to), length);
        return { from, to, severity: range.severity, message: range.message };
      });
      view.dispatch(setDiagnostics(view.state, clean));
    },
    // Move the caret to an offset and scroll it into view.
    jumpTo(offset) {
      const at = Math.min(Math.max(0, offset), view.state.doc.length);
      view.dispatch({ selection: { anchor: at }, scrollIntoView: true });
      view.focus();
    },
    focus: () => view.focus(),
    destroy: () => view.destroy(),
  };
}
