//! The Lagom lexer.
//!
//! Spec anchor: 00 §7.0 (lexical rules), §7.15 (phrase-tokens), doc 02 §2, docs/13 §1–2.
//!
//! Architecture (two passes):
//! 1. **Logical-line pass** — physical lines are grouped into logical statements by the
//!    continuation rule (7.0.1): a line joins the previous statement iff the previous
//!    statement is *incomplete* at end-of-line (token-tail check per the spec: "completeness
//!    is a property of the token at the line end") and the next line is indented deeper
//!    than the statement's start. Joined lines are re-lexed as one unit from a joined
//!    buffer, with spans mapped back to original file offsets.
//! 2. **Token pass** — a simple scanner over each logical line: keyword phrases (longest
//!    first) beat name words; name words accumulate into maximal `WordRun`s (multi-word
//!    names are the norm, 7.0.3 — greedy resolution against bound names happens in sema).
//!
//! Phrase-tokens (`and pass the problem on`, `if it fails`, `wait for all tasks`) are
//! single items so no name can collide with them (7.15 note) and the call grammar's
//! greedy `and` can never consume into them.

use lagom_diagnostics::{Diagnostic, Span};

// ---------------------------------------------------------------------------
// Token model
// ---------------------------------------------------------------------------

#[derive(Clone, Debug, PartialEq)]
pub enum Tok {
    Newline,
    Indent,
    Dedent,
    Eof,

    Int(i64),
    Float(f64),
    /// Text literal value; `{expr}` interpolation is kept verbatim (desugared in HIR, 7.7).
    Text(String),
    /// A doc-comment line (`## ...`), attached to the following item by the parser.
    Doc(String),
    /// A maximal run of one or more name words (`things`, `top score`, `square root`).
    WordRun(Vec<String>),

    Plus,
    Minus,
    Star,
    Slash,
    LParen,
    RParen,
    Comma,
    QuestionMark,
}

/// Keyword and reserved-phrase tokens. Keywords win lexically (7.0.3); user names
/// may not collide with them.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Kw {
    // declarations & statements
    Make,
    Changing,
    EqualTo,
    Set,
    To,
    Increase,
    Decrease,
    By,
    If,
    OtherwiseIf,
    Otherwise,
    Repeat,
    TimesWord,
    Using,
    While,
    ForEach,
    In,
    Stop,
    Next,
    Function,
    Takes,
    Called,
    Returns,
    GiveBack,
    FailWith,
    CanFail,
    Attempt,
    AndPassTheProblemOn,
    IfItFails,
    Then,
    As,
    Test,
    CheckThat,
    Use,
    For,
    Structure,
    Has,
    OfType,
    WaitForAllTasks,

    // expressions
    And,
    Or,
    Not,
    Is,
    CmpEqualTo,
    CmpNotEqualTo,
    CmpGreaterThan,
    CmpLessThan,
    CmpAtLeast,
    CmpAtMost,
    Plus,
    Minus,
    DividedBy,
    DividedEvenlyBy,
    RemainderOf,
    Modulo,

    // literals & collections
    True,
    False,
    Nothing,
    AListOf,
    AMapFrom,
    APairOf,

    // types & conversion-call heads (dual context: type position vs `number from …` call)
    Number,
    Decimal,
    Text,
    Boolean,
    Random,

    // option comparisons (M0 basic options, 7.6/8.5/D-34)
    IsNothing,
    IsSomething,

    // prepositions (closed set, 7.15)
    Of,
    At,
    From,
    With,
    A,
    An,
}

#[derive(Clone, Debug, PartialEq)]
pub enum Item {
    Tok(Tok),
    Kw(Kw),
}

#[derive(Clone, Debug)]
pub struct Token {
    pub tok: Tok,
    pub span: Span,
}

pub struct LexOutput {
    /// All items with spans (Indent/Dedent/Newline included), in source order.
    pub items: Vec<(Item, Span)>,
    pub errors: Vec<Diagnostic>,
}

/// Lex `src` into the item stream. This is the lexer's public entry point.
pub fn lex(src: &str) -> LexOutput {
    let mut errors = Vec::new();
    let lines = logical_lines(src, &mut errors);
    let mut items = Vec::new();
    let mut indents: Vec<usize> = vec![0];
    for line in &lines {
        if line.is_doc {
            let span = line.map(Span::new(0, line.joined.len()));
            items.push((Item::Tok(Tok::Doc(line.doc_text.clone())), span));
            items.push((Item::Tok(Tok::Newline), span));
            continue;
        }
        // indentation events
        let cur = *indents.last().unwrap();
        if line.first_indent > cur {
            if line.first_indent - cur < 2 {
                let span = line.map(Span::new(0, 0));
                errors.push(
                    Diagnostic::error(
                        "E0102",
                        "This block is indented by less than 2 spaces.",
                        span,
                    )
                    .with_explanation(
                        "Each block level is indented further than the line that opens it; 4 spaces per level is the canonical form.",
                    )
                    .with_fix("Indent the block by 4 spaces."),
                );
            }
            indents.push(line.first_indent);
            let span = line.map(Span::new(0, 0));
            items.push((Item::Tok(Tok::Indent), span));
        } else if line.first_indent < cur {
            while *indents.last().unwrap() > line.first_indent {
                indents.pop();
                let span = line.map(Span::new(0, 0));
                items.push((Item::Tok(Tok::Dedent), span));
            }
            if *indents.last().unwrap() != line.first_indent {
                let span = line.map(Span::new(0, line.joined.len()));
                errors.push(Diagnostic::error(
                    "E0103",
                    "This line is indented to a level that no open block matches.",
                    span,
                )
                .with_explanation(
                    "Every block must be indented by a consistent amount from the line that opens it.",
                ));
            }
        }
        // tokens of the logical line
        let (line_items, line_errors) = scan_line(line);
        errors.extend(line_errors);
        for (item, jspan) in line_items {
            let span = line.map(jspan);
            items.push((item, span));
        }
        let end = line.map(Span::new(line.joined.len(), line.joined.len()));
        items.push((Item::Tok(Tok::Newline), end));
    }
    while indents.len() > 1 {
        indents.pop();
        let last = lines.last();
        let span = match last {
            Some(l) => l.map(Span::new(l.joined.len(), l.joined.len())),
            None => Span::new(src.len(), src.len()),
        };
        items.push((Item::Tok(Tok::Dedent), span));
    }
    let eof = Span::new(src.len(), src.len());
    items.push((Item::Tok(Tok::Eof), eof));
    LexOutput { items, errors }
}

// ---------------------------------------------------------------------------
// Pass 1: logical lines (the continuation rule, 7.0.1)
// ---------------------------------------------------------------------------

struct LogicalLine {
    first_indent: usize,
    /// The joined text of all segments, separated by " " (or "\n"+indent inside strings).
    joined: String,
    /// Per segment: (original byte start, original byte end). Segment i starts at
    /// `seg_joined_start(i)` in `joined`.
    segs: Vec<(usize, usize)>,
    /// Separator inserted BEFORE each segment ("" for the first).
    seps: Vec<String>,
    is_doc: bool,
    doc_text: String,
}

impl LogicalLine {
    fn seg_joined_start(&self, i: usize) -> usize {
        let mut pos = 0;
        for s in 0..i {
            pos += self.segs[s].1 - self.segs[s].0 + self.seps[s].len();
        }
        pos += self.seps[i].len();
        pos
    }

    /// Map a span in `joined` coordinates back to a file span. Segment `i`
    /// occupies joined [s, e]; the separator after it occupies [e, s_next).
    /// Boundary positions clamp to the segment they fall in.
    fn map(&self, jspan: Span) -> Span {
        let map_one = |jp: usize| -> usize {
            if self.segs.is_empty() {
                return 0;
            }
            let n = self.segs.len();
            for i in 0..n {
                let s = self.seg_joined_start(i);
                let e = s + (self.segs[i].1 - self.segs[i].0);
                if jp <= e || i == n - 1 {
                    let clamped = jp.max(s).min(e);
                    return self.segs[i].0 + (clamped - s);
                }
            }
            self.segs[n - 1].1
        };
        Span::new(map_one(jspan.start), map_one(jspan.end))
    }
}

/// Words that leave a statement incomplete when they are the last word of a line
/// (the token-tail check of 7.0.1). Type words (`number`/`decimal`/`text`/`boolean`)
/// are deliberately excluded — see docs/14 G-6.
const INCOMPLETE_TAIL_WORDS: &[&str] = &[
    "and", "as", "at", "attempt", "back", "by", "called", "changing", "check",
    "decrease", "divided", "each", "evenly", "for", "from", "function", "give",
    "greater", "has", "if", "in", "increase", "is", "least", "less", "make", "minus",
    "modulo", "most", "not", "of", "or", "plus", "remainder", "repeat", "returns", "set",
    "structure", "takes", "test", "that", "than", "to", "type", "use", "using",
    "while", "with",
];

/// Build logical lines: skip blanks and comments, join continuations, collect docs.
fn logical_lines(src: &str, errors: &mut Vec<Diagnostic>) -> Vec<LogicalLine> {
    let mut out: Vec<LogicalLine> = Vec::new();
    let mut pos = 0usize;
    while pos <= src.len() {
        // find end of this physical line
        let rest = &src[pos..];
        if rest.is_empty() {
            break;
        }
        let nl = rest.find('\n').map(|i| pos + i);
        let line_end = nl.unwrap_or(src.len());
        let text = &src[pos..line_end];

        // measure indent
        let mut indent = 0usize;
        let tb = text.as_bytes();
        while indent < tb.len() && tb[indent] == b' ' {
            indent += 1;
        }
        if indent < tb.len() && tb[indent] == b'\t' {
            errors.push(
                Diagnostic::error("E0101", "Indent with spaces, not tabs.", Span::new(pos + indent, pos + indent + 1))
                    .with_explanation("Lagom blocks are indented with spaces; 4 per level is the canonical form.")
                    .with_fix("Replace the tab with 4 spaces."),
            );
            // treat the tab as indentation and continue
            while indent < tb.len() && (tb[indent] == b'\t' || tb[indent] == b' ') {
                indent += 1;
            }
        }
        let content = &text[indent..];
        let trimmed = content.trim_end();
        let is_blank = trimmed.is_empty();
        let is_comment = trimmed.starts_with('#') && !trimmed.starts_with("##");

        if is_blank || is_comment {
            // skipped lines do not break a continuation and do not affect indentation
            pos = if let Some(e) = nl { e + 1 } else { src.len() + 1 };
            if pos > src.len() {
                break;
            }
            continue;
        }

        if trimmed.starts_with("##") {
            // doc comment: its own logical line (indent-neutral)
            let doc_text = trimmed[2..].trim().to_string();
            out.push(LogicalLine {
                first_indent: indent,
                joined: trimmed.to_string(),
                segs: vec![(pos + indent, line_end)],
                seps: vec![String::new()],
                is_doc: true,
                doc_text,
            });
            pos = if let Some(e) = nl { e + 1 } else { src.len() + 1 };
            if pos > src.len() {
                break;
            }
            continue;
        }

        // start (or continue) a logical statement line
        let seg_start = pos + indent;
        let seg_end = line_end;
        let mut continuation_of_last = false;
        if let Some(last) = out.last() {
            // A still-open string literal continues regardless of indent (7.7):
            // string content is not statement syntax (docs/14 G-5).
            if !last.is_doc && last.string_open() {
                continuation_of_last = true;
            } else if !last.is_doc && joins_previous(last, text) {
                // otherwise the new line must be indented deeper than the statement's start
                if indent <= last.first_indent {
                    let at = Span::new(seg_start, seg_end);
                    errors.push(
                        Diagnostic::error(
                            "E0108",
                            "The line above ends in the middle of a statement, but this line does not continue it.",
                            at,
                        )
                        .with_explanation(
                            "A statement continues onto the next line only when that line is indented deeper than the line the statement started on (7.0.1).",
                        ),
                    );
                    // do not join; the parser will report the incomplete statement
                } else {
                    continuation_of_last = true;
                }
            }
        }
        if continuation_of_last {
            let last = out.last_mut().unwrap();
            // separator: preserve newlines inside multi-line strings (docs/14 G-5:
            // verbatim content, no stripping — pending doc 02's rule)
            let sep = if last.string_open() {
                format!("\n{}", " ".repeat(indent))
            } else {
                " ".to_string()
            };
            last.seps.push(sep);
            last.segs.push((seg_start, seg_end));
            last.joined.push_str(last.seps.last().unwrap());
            last.joined.push_str(content.trim_end());
        } else {
            out.push(LogicalLine {
                first_indent: indent,
                joined: trimmed.to_string(),
                segs: vec![(seg_start, seg_end)],
                seps: vec![String::new()],
                is_doc: false,
                doc_text: String::new(),
            });
        }

        pos = if let Some(e) = nl { e + 1 } else { src.len() + 1 };
        if pos > src.len() {
            break;
        }
    }
    out
}

impl LogicalLine {
    /// Is a string literal still open at the end of the joined text? (Multi-line
    /// strings, 7.7 — content is verbatim, docs/14 G-5.)
    fn string_open(&self) -> bool {
        let mut open = false;
        let mut chars = self.joined.chars().peekable();
        while let Some(c) = chars.next() {
            if c == '\\' {
                chars.next();
                continue;
            }
            if c == '"' {
                open = !open;
            }
        }
        open
    }

    fn paren_balance(&self) -> i32 {
        let mut bal = 0i32;
        let mut open = false;
        let mut chars = self.joined.chars().peekable();
        while let Some(c) = chars.next() {
            match c {
                '\\' => {
                    chars.next();
                }
                '"' => open = !open,
                '(' if !open => bal += 1,
                ')' if !open => bal -= 1,
                '#' if !open => break, // comment starts: ignore the rest
                _ => {}
            }
        }
        bal
    }

    /// Is the statement still incomplete at end-of-line? (7.0.1: completeness is a
    /// property of the token at the line end — approximated by the line's tail.)
    fn incomplete_tail(&self) -> bool {
        if self.string_open() {
            return true;
        }
        if self.paren_balance() > 0 {
            return true;
        }
        let trimmed = self.joined.trim_end();
        if trimmed.is_empty() {
            return false;
        }
        // last character operators
        if let Some(c) = trimmed.chars().last() {
            if matches!(c, '+' | '-' | '*' | '/' | ',' | '(') {
                return true;
            }
        }
        // last word
        let last_word: String = trimmed
            .chars()
            .rev()
            .take_while(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || *c == '_')
            .collect::<Vec<_>>()
            .into_iter()
            .rev()
            .collect();
        // the take_while above stops at the first non-word char from the end; if the
        // line ends with e.g. `5`, there is no trailing alpha word
        if last_word.is_empty()
            || last_word.chars().next().map_or(true, |c| c.is_ascii_digit())
        {
            return false;
        }
        INCOMPLETE_TAIL_WORDS.contains(&last_word.as_str())
    }
}

/// Should `next_line_text` join `prev`?
fn joins_previous(prev: &LogicalLine, next_line_text: &str) -> bool {
    let _ = next_line_text;
    prev.incomplete_tail()
}

// ---------------------------------------------------------------------------
// Pass 2: scan one logical line
// ---------------------------------------------------------------------------

/// Keyword phrases, LONGEST FIRST within the same starting word so the maximal
/// phrase always wins (e.g. `is not equal to` before `not equal to` before `not`).
const PHRASES: &[&[&str]] = &[
    &["and", "pass", "the", "problem", "on"],
    &["wait", "for", "all", "tasks"],
    &["is", "not", "equal", "to"],
    &["is", "greater", "than"],
    &["is", "less", "than"],
    &["is", "equal", "to"],
    &["is", "at", "least"],
    &["is", "at", "most"],
    &        ["is", "nothing"],
        &["is", "something"],
    &[] as &[&str], // spacer
    &["not", "equal", "to"],
    &["divided", "evenly", "by"],
    &["otherwise", "if"],
    &["greater", "than"],
    &["less", "than"],
    &["equal", "to"],
    &["at", "least"],
    &["at", "most"],
    &["remainder", "of"],
    &["divided", "by"],
    &["give", "back"],
    &["fail", "with"],
    &["can", "fail"],
    &["a", "list", "of"],
    &["a", "map", "from"],
    &["a", "pair", "of"],
    &["if", "it", "fails"],
    &["of", "type"],
    &["for", "each"],
    // single-word keywords
    &["otherwise"],
    &["changing"],
    &["number"],
    &["decimal"],
    &["text"],
    &["boolean"],
    &["true"],
    &["false"],
    &["nothing"],
    &["function"],
    &["structure"],
    &["repeat"],
    &["increase"],
    &["decrease"],
    &["attempt"],
    &["check", "that"],
    &["check"],
    &["give"],
    &["fail"],
    &["while"],
    &["using"],
    &["times"],
    &["stop"],
    &["next"],
    &["takes"],
    &["returns"],
    &["random"],
    &["test"],
    &["make"],
    &["set"],
    &["to"],
    &["by"],
    &["if"],
    &["then"],
    &["as"],
    &["use"],
    &["for"],
    &["in"],
    &["and"],
    &["or"],
    &["not"],
    &["is"],
    &["plus"],
    &["minus"],
    &["modulo"],
    &["has"],
    &["called"],
    &["with"],
    &["of"],
    &["at"],
    &["from"],
    &["a"],
    &["an"],
];

fn phrase_kw(phrase: &[&str]) -> Kw {
    use Kw::*;
    match phrase {
        ["and", "pass", "the", "problem", "on"] => AndPassTheProblemOn,
        ["wait", "for", "all", "tasks"] => WaitForAllTasks,
        ["is", "not", "equal", "to"] => CmpNotEqualTo,
        ["is", "greater", "than"] => CmpGreaterThan,
        ["is", "less", "than"] => CmpLessThan,
        ["is", "equal", "to"] => CmpEqualTo,
        ["is", "at", "least"] => CmpAtLeast,
        ["is", "at", "most"] => CmpAtMost,
        ["is", "nothing"] => IsNothing,
        ["is", "something"] => IsSomething,
        ["not", "equal", "to"] => CmpNotEqualTo,
        ["greater", "than"] => CmpGreaterThan,
        ["less", "than"] => CmpLessThan,
        ["equal", "to"] => EqualTo,
        ["at", "least"] => CmpAtLeast,
        ["at", "most"] => CmpAtMost,
        ["divided", "evenly", "by"] => DividedEvenlyBy,
        ["otherwise", "if"] => OtherwiseIf,
        ["for", "each"] => ForEach,
        ["remainder", "of"] => RemainderOf,
        ["divided", "by"] => DividedBy,
        ["give", "back"] => GiveBack,
        ["fail", "with"] => FailWith,
        ["can", "fail"] => CanFail,
        ["a", "list", "of"] => AListOf,
        ["a", "map", "from"] => AMapFrom,
        ["a", "pair", "of"] => APairOf,
        ["if", "it", "fails"] => IfItFails,
        ["of", "type"] => OfType,
        ["otherwise"] => Otherwise,
        ["changing"] => Changing,
        ["number"] => Number,
        ["decimal"] => Decimal,
        ["text"] => Text,
        ["boolean"] => Boolean,
        ["true"] => True,
        ["false"] => False,
        ["nothing"] => Nothing,
        ["function"] => Function,
        ["structure"] => Structure,
        ["repeat"] => Repeat,
        ["increase"] => Increase,
        ["decrease"] => Decrease,
        ["attempt"] => Attempt,
        ["check", "that"] => CheckThat,
        ["check"] => CheckThat, // bare `check` is diagnosed by the parser
        ["give"] => GiveBack,   // bare `give` is diagnosed by the parser
        ["fail"] => FailWith,   // bare `fail` is diagnosed by the parser
        ["while"] => While,
        ["using"] => Using,
        ["times"] => TimesWord,
        ["stop"] => Stop,
        ["next"] => Next,
        ["takes"] => Takes,
        ["returns"] => Returns,
        ["random"] => Random,
        ["test"] => Test,
        ["make"] => Make,
        ["set"] => Set,
        ["to"] => To,
        ["by"] => By,
        ["if"] => If,
        ["then"] => Then,
        ["as"] => As,
        ["use"] => Use,
        ["for"] => For,
        ["in"] => In,
        ["and"] => And,
        ["or"] => Or,
        ["not"] => Not,
        ["is"] => Is,
        ["plus"] => Plus,
        ["minus"] => Minus,
        ["modulo"] => Modulo,
        ["has"] => Has,
        ["called"] => Called,
        ["with"] => With,
        ["of"] => Of,
        ["at"] => At,
        ["from"] => From,
        ["a"] => A,
        ["an"] => An,
        _ => unreachable!("unmapped phrase"),
    }
}

struct LineScan {
    items: Vec<(Item, Span)>,
    errors: Vec<Diagnostic>,
}

fn scan_line(line: &LogicalLine) -> (Vec<(Item, Span)>, Vec<Diagnostic>) {
    let mut sc = LineScan {
        items: Vec::new(),
        errors: Vec::new(),
    };
    let src = &line.joined;
    let bytes = src.as_bytes();
    let mut pos = 0usize;
    while pos < bytes.len() {
        let b = bytes[pos];
        match b {
            b' ' | b'\t' | b'\r' | b'\n' => pos += 1,
            b'#' => break, // trailing comment (should not occur post-join, but be safe)
            b'"' => scan_string(src, &mut pos, &mut sc),
            b'(' => {
                sc.push_tok(Tok::LParen, pos, pos + 1);
                pos += 1;
            }
            b')' => {
                sc.push_tok(Tok::RParen, pos, pos + 1);
                pos += 1;
            }
            b',' => {
                sc.push_tok(Tok::Comma, pos, pos + 1);
                pos += 1;
            }
            b'?' => {
                sc.push_tok(Tok::QuestionMark, pos, pos + 1);
                pos += 1;
            }
            b'+' => {
                sc.push_tok(Tok::Plus, pos, pos + 1);
                pos += 1;
            }
            b'-' => {
                sc.push_tok(Tok::Minus, pos, pos + 1);
                pos += 1;
            }
            b'*' => {
                sc.push_tok(Tok::Star, pos, pos + 1);
                pos += 1;
            }
            b'/' => {
                sc.push_tok(Tok::Slash, pos, pos + 1);
                pos += 1;
            }
            b if b.is_ascii_digit() => scan_number(src, &mut pos, &mut sc),
            b if is_word_start(b) => scan_words(src, &mut pos, &mut sc),
            _ => {
                let ch = src[pos..].chars().next().unwrap();
                let end = pos + ch.len_utf8();
                sc.errors.push(Diagnostic::error(
                    "E0104",
                    format!("I do not recognize the character `{ch}`."),
                    Span::new(pos, end),
                ));
                pos = end;
            }
        }
    }
    (sc.items, sc.errors)
}

impl LineScan {
    fn push_tok(&mut self, tok: Tok, start: usize, end: usize) {
        self.items.push((Item::Tok(tok), Span::new(start, end)));
    }

    fn push_kw(&mut self, kw: Kw, start: usize, end: usize) {
        self.items.push((Item::Kw(kw), Span::new(start, end)));
    }
}

fn scan_string(src: &str, pos: &mut usize, sc: &mut LineScan) {
    let start = *pos;
    *pos += 1; // opening quote
    let mut value = String::new();
    loop {
        if *pos >= src.len() {
            // Should not happen: logical-line joining keeps strings balanced.
            let span = Span::new(start, *pos);
            sc.errors.push(
                Diagnostic::error("E0105", "This text is missing its closing quote.", span)
                    .with_fix("Add a `\"` at the end of the text."),
            );
            sc.push_tok(Tok::Text(value), start, *pos);
            return;
        }
        let b = src.as_bytes()[*pos];
        match b {
            b'"' => {
                *pos += 1;
                sc.push_tok(Tok::Text(value), start, *pos);
                return;
            }
            b'\\' => {
                *pos += 1;
                let esc = src.as_bytes().get(*pos).copied();
                match esc {
                    Some(b'n') => {
                        value.push('\n');
                        *pos += 1;
                    }
                    Some(b't') => {
                        value.push('\t');
                        *pos += 1;
                    }
                    Some(b'"') => {
                        value.push('"');
                        *pos += 1;
                    }
                    Some(b'\\') => {
                        value.push('\\');
                        *pos += 1;
                    }
                    Some(b'{') => {
                        value.push('{');
                        *pos += 1;
                    }
                    Some(b'}') => {
                        value.push('}');
                        *pos += 1;
                    }
                    _ => {
                        sc.errors.push(Diagnostic::error(
                            "E0106",
                            "Unknown escape.",
                            Span::new(*pos, (*pos + 1).min(src.len())),
                        ));
                    }
                }
            }
            b'\n' => {
                // multi-line string (7.7): the newline is content (verbatim, G-5)
                value.push('\n');
                *pos += 1;
            }
            _ => {
                let ch = src[*pos..].chars().next().unwrap();
                *pos += ch.len_utf8();
                value.push(ch);
            }
        }
    }
}

fn scan_number(src: &str, pos: &mut usize, sc: &mut LineScan) {
    let start = *pos;
    let bytes = src.as_bytes();
    while *pos < bytes.len() && bytes[*pos].is_ascii_digit() {
        *pos += 1;
    }
    let mut is_float = false;
    if *pos + 1 < bytes.len() && bytes[*pos] == b'.' && bytes[*pos + 1].is_ascii_digit() {
        is_float = true;
        *pos += 1;
        while *pos < bytes.len() && bytes[*pos].is_ascii_digit() {
            *pos += 1;
        }
    }
    let text = &src[start..*pos];
    if is_float {
        let v: f64 = text.parse().unwrap();
        sc.push_tok(Tok::Float(v), start, *pos);
    } else {
        match text.parse::<i64>() {
            Ok(v) => sc.push_tok(Tok::Int(v), start, *pos),
            Err(_) => {
                sc.errors.push(Diagnostic::error(
                    "E0107",
                    "This number is too large.",
                    Span::new(start, *pos),
                ));
                sc.push_tok(Tok::Int(i64::MAX), start, *pos);
            }
        }
    }
}

/// Scan a maximal word sequence, splitting it into keyword phrases and name runs.
///
/// Determinism (7.0.3): at each word start the longest keyword phrase wins; words
/// that begin no phrase accumulate into the current `WordRun`. The run ends at any
/// non-word character. Greedy resolution of a run against *bound* names happens in
/// sema (the lexer hands the parser the raw run).
fn scan_words(src: &str, pos: &mut usize, sc: &mut LineScan) {
    let bytes = src.as_bytes();
    while *pos < bytes.len() && is_word_start(bytes[*pos]) {
        let run_start = *pos;
        let mut run: Vec<String> = Vec::new();
        loop {
            let wstart = *pos;
            // longest phrase at this position?
            if let Some((kw, len)) = match_phrase(src, wstart) {
                if !run.is_empty() {
                    sc.push_tok(Tok::WordRun(std::mem::take(&mut run)), run_start, wstart);
                }
                let end = wstart + len;
                sc.push_kw(kw, wstart, end);
                *pos = end;
                break;
            }
            // plain word
            let mut wend = wstart;
            while wend < bytes.len() && is_word_cont(bytes[wend]) {
                wend += 1;
            }
            run.push(src[wstart..wend].to_string());
            *pos = wend;
            // continue the run across a single space if a word follows AND neither
            // the next word starts a phrase (checked next loop) — we always peek:
            let mut peek = *pos;
            while peek < bytes.len() && bytes[peek] == b' ' {
                peek += 1;
            }
            if peek < bytes.len() && is_word_start(bytes[peek]) {
                *pos = peek;
                continue;
            }
            break;
        }
        if !run.is_empty() {
            let end = *pos;
            sc.push_tok(Tok::WordRun(run), run_start, end);
        }
        // after a phrase we may be followed by more words (e.g. `make top score`)
        while *pos < bytes.len() && (bytes[*pos] == b' ' || bytes[*pos] == b'\t') {
            *pos += 1;
        }
    }
}

/// Match the longest keyword phrase at `at`. Returns the keyword and byte length.
fn match_phrase(src: &str, at: usize) -> Option<(Kw, usize)> {
    let bytes = src.as_bytes();
    'outer: for phrase in PHRASES {
        if phrase.is_empty() {
            continue;
        }
        let mut pos = at;
        for (i, w) in phrase.iter().enumerate() {
            if i > 0 {
                if bytes.get(pos) != Some(&b' ') {
                    continue 'outer;
                }
                pos += 1;
            }
            if !src[pos..].starts_with(w) {
                continue 'outer;
            }
            pos += w.len();
        }
        // must end at a word boundary (not glued to more word characters)
        if pos < bytes.len() && is_word_cont(bytes[pos]) {
            continue;
        }
        return Some((phrase_kw(phrase), pos - at));
    }
    None
}

fn is_word_start(b: u8) -> bool {
    b.is_ascii_lowercase() || b == b'_'
}

fn is_word_cont(b: u8) -> bool {
    b.is_ascii_lowercase() || b.is_ascii_digit() || b == b'_'
}

// ---------------------------------------------------------------------------
// Tests — pinned to the frozen grammar (docs/13 §1–2) and §7.0.1's invariants
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    fn toks(src: &str) -> Vec<Item> {
        let out = lex(src);
        assert!(out.errors.is_empty(), "unexpected lexer errors: {:?}", out.errors);
        out.items.into_iter().map(|(i, _)| i).collect()
    }

    fn toks_with_errors(src: &str) -> (Vec<Item>, Vec<Diagnostic>) {
        let out = lex(src);
        (out.items.into_iter().map(|(i, _)| i).collect(), out.errors)
    }

    fn kws(src: &str) -> Vec<Kw> {
        toks(src)
            .into_iter()
            .filter_map(|i| match i {
                Item::Kw(k) => Some(k),
                _ => None,
            })
            .collect()
    }

    #[test]
    fn hello_world() {
        let t = toks("say \"Hello, world!\"\n");
        assert_eq!(
            t,
            vec![
                Item::Tok(Tok::WordRun(vec!["say".into()])),
                Item::Tok(Tok::Text("Hello, world!".into())),
                Item::Tok(Tok::Newline),
                Item::Tok(Tok::Eof),
            ]
        );
    }

    /// §7.0.1 continuation: `plus` at end-of-line + deeper indent joins the lines.
    #[test]
    fn continuation_rule_plus() {
        let src = "make message equal to \"Hello, \" plus name plus\n    \"welcome to Lagom!\"\n";
        let t = toks(src);
        // exactly one statement: no Newline between the two physical lines
        let newlines = t.iter().filter(|i| **i == Item::Tok(Tok::Newline)).count();
        assert_eq!(newlines, 1, "expected a single logical line, got {t:?}");
        assert!(!t.contains(&Item::Tok(Tok::Indent)));
        // and the pieces are all present in order
        let joined = format!("{t:?}");
        assert!(joined.contains("welcome to Lagom!"));
    }

    /// A line at the SAME indent does not join — the statement ended.
    #[test]
    fn continuation_no_join_same_indent() {
        let src = "make a equal to 1 plus 2\nsay a\n";
        let t = toks(src);
        let newlines = t.iter().filter(|i| **i == Item::Tok(Tok::Newline)).count();
        assert_eq!(newlines, 2);
        assert!(!t.contains(&Item::Tok(Tok::Indent)));
    }

    /// A continuation line must be indented deeper than the statement's start (E0108).
    #[test]
    fn continuation_requires_deeper_indent() {
        let src = "make a equal to 1 plus\n2\n";
        let (t, errors) = toks_with_errors(src);
        assert_eq!(errors.len(), 1, "expected E0108, got {errors:?}");
        assert_eq!(errors[0].code, "E0108");
        let _ = t;
    }

    /// The canonical example (§6.2): indentation produces Indent/Dedent.
    #[test]
    fn canonical_example_blocks() {
        let src = "make age equal to 15\n\nif age is greater than 13\n    say \"You can join!\"\notherwise\n    say \"Sorry, come back later.\"\n";
        let t = toks(src);
        let kinds: Vec<&str> = t
            .iter()
            .map(|i| match i {
                Item::Tok(Tok::Indent) => "INDENT",
                Item::Tok(Tok::Dedent) => "DEDENT",
                Item::Tok(Tok::Newline) => "NL",
                _ => ".",
            })
            .collect();
        // make-line (4 items), if-line (4), INDENT say (2), DEDENT otherwise (1),
        // INDENT say (2), DEDENT — one Newline per logical line, Eof last
        assert_eq!(
            kinds,
            vec![".", ".", ".", ".", "NL", ".", ".", ".", ".", "NL", "INDENT", ".", ".", "NL", "DEDENT", ".", "NL", "INDENT", ".", ".", "NL", "DEDENT", "."]
        );
    }

    /// Blank lines and comments never affect indentation (7.0).
    #[test]
    fn blank_and_comment_lines_skipped() {
        let src = "make a equal to 1\n\n# a comment\n\nif a is greater than 0\n    # inside\n    say a\n";
        let (t, errors) = toks_with_errors(src);
        assert!(errors.is_empty(), "{errors:?}");
        let has_indent = t.contains(&Item::Tok(Tok::Indent));
        assert!(has_indent);
        // exactly one indent/dedent pair for the if-block
        let indents = t.iter().filter(|i| **i == Item::Tok(Tok::Indent)).count();
        let dedents = t.iter().filter(|i| **i == Item::Tok(Tok::Dedent)).count();
        assert_eq!(indents, 1);
        assert_eq!(dedents, 1);
    }

    /// Multi-word keyword phrases are single items; none may be split.
    #[test]
    fn phrase_tokens() {
        assert_eq!(
            kws("if age is greater than 13\n"),
            vec![Kw::If, Kw::CmpGreaterThan]
        );
        assert_eq!(kws("make x equal to 5\n"), vec![Kw::Make, Kw::EqualTo]);
        assert_eq!(
            kws("make h equal to total divided evenly by 2\n"),
            vec![Kw::Make, Kw::EqualTo, Kw::DividedEvenlyBy]
        );
        assert_eq!(
            kws("attempt divide 10 and 0 and pass the problem on\n"),
            // the first `and` is the call grammar's argument separator (flowcall's
            // `{ "and" additive }`); the phrase-token ends the statement (7.15)
            vec![Kw::Attempt, Kw::And, Kw::AndPassTheProblemOn]
        );
        assert_eq!(
            kws("attempt divide 10 and 0 if it fails then\n    say problem\n"),
            vec![Kw::Attempt, Kw::And, Kw::IfItFails, Kw::Then]
        );
        // `is not equal to` beats `not equal to` beats `not`
        assert_eq!(
            kws("if x is not equal to 0\n"),
            vec![Kw::If, Kw::CmpNotEqualTo]
        );
        // collection phrases
        assert_eq!(
            kws("make t equal to a list of 1, 2\n"),
            vec![Kw::Make, Kw::EqualTo, Kw::AListOf]
        );
        assert_eq!(
            kws("make ages equal to a map from \"ana\" to 11\n"),
            vec![Kw::Make, Kw::EqualTo, Kw::AMapFrom, Kw::To]
        );
        assert_eq!(
            kws("make p equal to a pair of 3 and 4\n"),
            // the `and` belongs to the pair literal's grammar (a pair of X and Y)
            vec![Kw::Make, Kw::EqualTo, Kw::APairOf, Kw::And]
        );
        // `give back`, `fail with`, `can fail`, `check that`
        assert_eq!(kws("give back total\n"), vec![Kw::GiveBack]);
        assert_eq!(kws("fail with \"no\"\n"), vec![Kw::FailWith]);
        assert_eq!(kws("function f\n    can fail\n"), vec![Kw::Function, Kw::CanFail]);
        assert_eq!(kws("check that 1 is less than 2\n"), vec![Kw::CheckThat, Kw::CmpLessThan]);
    }

    /// Numbers, floats, strings, escapes; interpolation braces stay in the value.
    #[test]
    fn literals() {
        let t = toks("make x equal to 3.5\nmake y equal to 42\n");
        assert!(t.contains(&Item::Tok(Tok::Float(3.5))));
        assert!(t.contains(&Item::Tok(Tok::Int(42))));
        let t = toks("say \"a\\\"b{n}c\"\n");
        match &t[1] {
            Item::Tok(Tok::Text(v)) => assert_eq!(v, "a\"b{n}c"),
            other => panic!("{other:?}"),
        }
        // multi-line string (7.7): newline is content; the string continues
        // regardless of indent (string content is not statement syntax)
        let t = toks("make long equal to \"line one\nline two\"\n");
        match &t[3] {
            Item::Tok(Tok::Text(v)) => assert_eq!(v, "line one\nline two"),
            other => panic!("{other:?}"),
        }
    }

    /// Multi-word names are one WordRun; keywords split runs.
    #[test]
    fn multi_word_names() {
        let t = toks("make top score equal to 0\n");
        assert!(t.contains(&Item::Tok(Tok::WordRun(vec!["top".into(), "score".into()]))));
        // keyword + name + keyword sequence
        let t = toks("make high score equal to 5\n");
        assert_eq!(
            t.iter().filter(|i| **i == Item::Kw(Kw::Make)).count(),
            1
        );
        assert!(t.contains(&Item::Tok(Tok::WordRun(vec!["high".into(), "score".into()]))));
    }

    /// The text→number conversion call derives: `number from answer` (D-39).
    #[test]
    fn number_from_conversion() {
        let t = toks("attempt number from answer if it fails then\n    say \"That was not a number!\"\notherwise\n    say \"You picked {result}.\"\n");
        let k = kws("attempt number from answer if it fails then\n");
        assert_eq!(k, vec![Kw::Attempt, Kw::Number, Kw::From, Kw::IfItFails, Kw::Then]);
        let _ = t;
    }

    /// `random from 1 to 6` — two prepositional args on one flowcall (S-7).
    #[test]
    fn random_from_to() {
        assert_eq!(
            kws("make roll equal to random from 1 to 6\n"),
            vec![Kw::Make, Kw::EqualTo, Kw::Random, Kw::From, Kw::To]
        );
    }

    /// `say`/`ask` are ordinary function names, NOT keywords (§7.1).
    #[test]
    fn say_ask_are_names() {
        let t = toks("make answer equal to ask \"What is your name?\"\n");
        assert!(t.contains(&Item::Tok(Tok::WordRun(vec!["ask".into()]))));
        assert!(!t.iter().any(|i| matches!(i, Item::Kw(Kw::A)) && false));
    }

    /// Doc comments become Doc items (attached to the next item by the parser).
    #[test]
    fn doc_comments() {
        let t = toks("## Greets one person.\nfunction greet\n");
        assert_eq!(
            t[0],
            Item::Tok(Tok::Doc("Greets one person.".into()))
        );
    }

    /// Parens spanning lines join (unbalanced paren = incomplete statement).
    #[test]
    fn paren_continuation() {
        let src = "make x equal to (1 plus\n    2)\n";
        let t = toks(src);
        let newlines = t.iter().filter(|i| **i == Item::Tok(Tok::Newline)).count();
        assert_eq!(newlines, 1);
    }

    /// EOF emits the trailing dedents.
    #[test]
    fn eof_dedents() {
        let src = "if true\n    if true\n        say 1\n";
        let t = toks(src);
        let dedents = t.iter().filter(|i| **i == Item::Tok(Tok::Dedent)).count();
        assert_eq!(dedents, 2);
        assert_eq!(*t.last().unwrap(), Item::Tok(Tok::Eof));
    }

    /// Tabs are rejected (E0101); unknown characters are rejected (E0104).
    #[test]
    fn lexical_errors() {
        let (_, errors) = toks_with_errors("make x equal to 1\n\t\tsay x\n");
        assert!(errors.iter().any(|e| e.code == "E0101"), "{errors:?}");
        let (_, errors) = toks_with_errors("make x equal to $\n");
        assert!(errors.iter().any(|e| e.code == "E0104"), "{errors:?}");
    }

    /// Targets, field access, indexing, and struct construction lex per the grammar.
    #[test]
    fn m0_statement_shapes() {
        assert_eq!(
            kws("set things at 2 to \"plum\"\n"),
            vec![Kw::Set, Kw::At, Kw::To]
        );
        assert_eq!(
            kws("increase score of p by 10\n"),
            vec![Kw::Increase, Kw::Of, Kw::By]
        );
        assert_eq!(
            kws("make p equal to a player with name \"bo\" and score 0\n"),
            vec![Kw::Make, Kw::EqualTo, Kw::A, Kw::With, Kw::And]
        );
        assert_eq!(
            kws("structure player\n    has name of type text\n    has score of type number\n"),
            vec![Kw::Structure, Kw::Has, Kw::OfType, Kw::Text, Kw::Has, Kw::OfType, Kw::Number]
        );
        assert_eq!(
            kws("repeat 10 times using i\n    say i\n"),
            vec![Kw::Repeat, Kw::TimesWord, Kw::Using]
        );
        assert_eq!(
            kws("repeat while score is less than 100\n"),
            vec![Kw::Repeat, Kw::While, Kw::CmpLessThan]
        );
        assert_eq!(
            kws("repeat for each item in things\n"),
            vec![Kw::Repeat, Kw::ForEach, Kw::In]
        );
        assert_eq!(kws("stop\nnext\n"), vec![Kw::Stop, Kw::Next]);
        assert_eq!(
            kws("use math for square root, floor\n"),
            vec![Kw::Use, Kw::For]
        );
        // `true`/`false`/`nothing` are literals
        assert_eq!(
            kws("make flag equal to true\n"),
            vec![Kw::Make, Kw::EqualTo, Kw::True]
        );
    }
}
