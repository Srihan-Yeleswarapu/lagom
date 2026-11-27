// Lagom's lexical shape, as a line-by-line tokenizer. The rules mirror
// crates/lagom_lexer: `#` starts a comment, `##` a doc comment; strings are
// "..." with `{expr}` interpolation; keyword phrases are matched longest first.
//
// This file has no DOM or CodeMirror imports, so the node tests can drive it
// with a small fake stream. lagom-language.js wraps it for CodeMirror.
//
// Highlighting is a v1 tokenizer, not a grammar. It colours keywords exactly as
// the lexer sees them, so ordinary words such as `set` or `with` get the keyword
// colour too. Names of variables and functions are not coloured separately.

export const TYPE_PHRASES = [
  "number",
  "decimal",
  "text",
  "boolean",
  "a list of",
  "a map from",
  "a pair of",
  "a channel of",
];

export const LITERAL_WORDS = ["true", "false", "nothing"];

export const BUILTIN_WORDS = [
  "say",
  "ask",
  // Tasks & channels (14.2) and M2's network built-ins — `lagom words`
  // lists them, so the highlighter colours them the way the compiler sees
  // them: callable built-ins, not reserved words.
  "send",
  "receive",
  "open listener",
  "open connection",
  "expect connection",
  "put",
  "take",
  "close",
  "port",
];

export const KEYWORD_PHRASES = [
  "and pass the problem on",
  "wait for all tasks",
  "start a task",
  "keep going",
  "is not equal to",
  "is greater than",
  "is less than",
  "is equal to",
  "is at least",
  "is at most",
  "is nothing",
  "is something",
  "not equal to",
  "divided evenly by",
  "otherwise if",
  "greater than",
  "less than",
  "equal to",
  "at least",
  "at most",
  "remainder of",
  "divided by",
  "gives back",
  "fail with",
  "can fail",
  "if it fails",
  "of type",
  "for each",
  "something with value",
  "giving back",
  "is a",
  "or nothing",
  "a new",
  "before last reference disappears",
  "check that",
  "class",
  "can",
  "construction",
  "kind",
  "match",
  "when",
  "taking",
  "where",
  "it",
  "something",
  "otherwise",
  "changing",
  "function",
  "structure",
  "repeat",
  "increase",
  "decrease",
  "attempt",
  "check",
  "gives",
  "fail",
  "while",
  "using",
  "extends",
  "interface",
  "does",
  "times",
  "stop",
  "next",
  "takes",
  "returns",
  "random",
  "test",
  "make",
  "set",
  "to",
  "by",
  "if",
  "then",
  "as",
  "use",
  "for",
  "in",
  "and",
  "or",
  "not",
  "is",
  "plus",
  "minus",
  "modulo",
  "has",
  "called",
  "with",
  "of",
  "at",
  "from",
  "a",
  "an",
];

function escapeRegExp(text) {
  return text.replace(/[.*+?^${}()|[\]\\]/g, "\\$&");
}

// Longest phrase first, so `is not equal to` wins over `is`. Words inside a
// phrase may be separated by spaces or tabs. Exported so the VS Code
// extension's test can regenerate the grammar's phrase sets from this one
// source of truth (editors/vscode/test).
export function alternation(phrases) {
  return [...phrases]
    .sort((a, b) => b.length - a.length)
    .map((phrase) => phrase.split(" ").map(escapeRegExp).join("[ \\t]+"))
    .join("|");
}

// Anchored at the current position, with a trailing word boundary.
const TYPE_RE = new RegExp(`^(?:${alternation(TYPE_PHRASES)})(?!\\w)`);
const LITERAL_RE = new RegExp(`^(?:${alternation(LITERAL_WORDS)})(?!\\w)`);
const BUILTIN_RE = new RegExp(`^(?:${alternation(BUILTIN_WORDS)})(?!\\w)`);
const KEYWORD_RE = new RegExp(`^(?:${alternation(KEYWORD_PHRASES)})(?!\\w)`);
const NUMBER_RE = /^\d+(?:\.\d+)?(?!\w)/;
const WORD_RE = /^\w+/;
const STRING_TEXT_RE = /^(?:[^"\\{\n]|\\.)+/;
const INTERPOLATION_RE = /^\{[^{}\n]*\}?/;

export function startState() {
  return { inString: false };
}

export function copyState(state) {
  return { inString: state.inString };
}

// Token names: "doc", "comment", "string", "interp", "number", "typeName",
// "atom", "keyword", "builtin", or null for plain text. The names are
// deliberately close to the CodeMirror tag names used in lagom-language.js.
export function token(stream, state) {
  if (stream.sol()) state.inString = false;

  if (state.inString) {
    if (stream.match(INTERPOLATION_RE)) return "interp";
    if (stream.eat('"')) {
      state.inString = false;
      return "string";
    }
    // Always consume something so the stream makes progress, even on a lone
    // backslash at the end of a line.
    if (!stream.match(STRING_TEXT_RE)) stream.next();
    return "string";
  }

  if (stream.match("##")) {
    stream.skipToEnd();
    return "doc";
  }
  if (stream.match("#")) {
    stream.skipToEnd();
    return "comment";
  }
  if (stream.eat('"')) {
    state.inString = true;
    return "string";
  }
  if (stream.eatSpace()) return null;

  // Keywords start only at a word boundary. An identifier is consumed whole
  // below, so `answer` never matches the keyword `a` in its middle.
  const previous = stream.pos > 0 ? stream.string.charAt(stream.pos - 1) : "";
  const atWordStart = !/\w/.test(previous);
  if (atWordStart) {
    if (stream.match(NUMBER_RE)) return "number";
    if (stream.match(TYPE_RE)) return "typeName";
    if (stream.match(LITERAL_RE)) return "atom";
    if (stream.match(BUILTIN_RE)) return "builtin";
    if (stream.match(KEYWORD_RE)) return "keyword";
  }
  if (stream.match(WORD_RE)) return null;

  stream.next();
  return null;
}

// Exposed for tests: the regular expressions are part of the rule set.
export const PATTERNS = { TYPE_RE, LITERAL_RE, BUILTIN_RE, KEYWORD_RE, NUMBER_RE };
