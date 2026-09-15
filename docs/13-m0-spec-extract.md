# Phase 13 — M0 Normative Extract (spec → compiler)

*Status: complete. This is the bridge document between the frozen architecture (docs 00–12) and the Rust implementation. It states exactly what M0 implements, derived from [§7.15 of 00](00-architecture.md#715-formal-grammar-sketch) and [§33.1](00-architecture.md#331-m0--the-language-exists-and-is-real-the-gate-to-everything-else), and it records the four small spec gaps found while extracting, with their smallest resolutions (per the critical rule: no silent redesign).*

## 1. M0 statement grammar (from the frozen §7.15, M0 subset)

```ebnf
program     = { blank | toplevel } ;
toplevel    = use | function | structure | test | doccomment statement ;

block       = INDENT { statement } DEDENT ;

statement   = make | set | increase | decrease | if | repeat | stop | next
            | giveback | failwith | attempt | checkthat | use
            | assignment | exprstmt ;

make        = "make" ["changing"] name "equal to" expr ;
set         = "set" target "to" expr ;
increase    = ("increase"|"decrease") target "by" expr ;
target      = name { prep additive } ;               (* `set things at 2 to "x"`, `set score of p to 5` *)
if          = "if" expr block { "otherwise if" expr block } [ "otherwise" block ] ;
repeat      = "repeat" (count | whilec | foreach) block ;
count       = number "times" ["using" name] ;
whilec      = "while" expr ;
foreach     = name "in" expr ;                       (* pattern = name at M0 *)
giveback    = "give back" expr ;
failwith    = "fail with" expr ;
attempt     = "attempt" orexpr [attempttail] ;
attempttail = "and pass the problem on"
            | "if it fails" "then" block ["otherwise" block]
            | "as" name block ["otherwise" block] ;
checkthat   = "check that" comparison ;
use         = "use" name [ "for" name { "," name } ] ;
```

## 2. M0 expression grammar (from the frozen §7.15)

```ebnf
expr        = orexpr ;
orexpr      = andexpr { "or" andexpr } ;
andexpr     = notexpr { "and" notexpr } ;
notexpr     = ["not"] comparison ;
comparison  = additive [ ("is" cmpword | cmpword) additive ] ;
cmpword     = "equal to" | "not equal to" | "greater than" | "less than"
            | "at least" | "at most" ;
additive    = multiplicative { ("+"|"plus") multiplicative | ("-"|"minus") multiplicative } ;
multiplicative = unary { ("*"|"times") unary | ("/"|"divided by") unary
                 | "divided evenly by" unary | ("remainder of"|"modulo") unary } ;
unary       = "-" primary | primary ;
primary     = literal | name | call | "(" expr ")" | listlit | maplit | pairlit
            | interpolation | structlit | attempt | "true" | "false" | "nothing" ;

call        = flowcall { "with" name additive } ;    (* where/using suffixes are M1; `with` is needed
                                                        for struct construction and labeled args *)
flowcall    = name [additive] { prep additive } { "and" additive } ;
prep        = "of" | "at" | "from" | "to" ;
listlit     = "a list of" expr { "," expr } ;
maplit      = "a map from" primary "to" primary { "," primary "to" primary } ;
pairlit     = "a pair of" expr "and" expr ;
structlit   = "a" usertype { "with" name additive } ;   (* `a player with name "bo" and score 0` —
                                                        the second+ `with` spells `and` (7.11) *)
```

## 3. M0 semantic decisions (all traceable)

| # | Decision | Source |
|---|---|---|
| S-1 | `number` = i64, `decimal` = f64; `divided by` promotes to decimal; `divided evenly by` floors (Python pair: floor division, remainder takes divisor's sign); `remainder of a and b` is the call form | D-7, D-10, 03-semantics §3 |
| S-2 | Overflow: checked trap in both build modes (D-35 recommendation adopted for the implementation; the M1 datum may revisit) | D-35 |
| S-3 | Mixed number/decimal promotes to decimal; annotated `number` receiving decimals is a compile error; numeric literals are polymorphic at inference, defaulting to `number` (accumulate-into-`0` must type as the element type) | R-15, 8.2 |
| S-4 | `and`/`or` short-circuit and are strictly `boolean`-typed (the open question in doc 03 is resolved for M0 as proposed: strictly boolean) | doc 03 open q |
| S-5 | Equality: structural for values, recursive through containers; no classes at M0 so identity case is deferred | D-30 |
| S-6 | `can fail` calls must be handled: `attempt … if it fails then … otherwise …` (binds `problem`/`result`), `attempt … as name …`, or `attempt … and pass the problem on` (statement-only). Unhandled `can fail` call = compile error | 13.1 |
| S-7 | Text↔number: `number from text` / `decimal from text` (`can fail`), `text from number`; `random from 1 to N` (kernel `random` module, `can fail` only in that its args must be sane — actually pure student API returning number) — both in the implicit `standard`/`math`/`random` modules | D-39, 33.1 |
| S-8 | Value semantics: list/map/struct assignment copies (lists/maps copy-on-write internally; observable behavior = copy) | 9.2 |
| S-9 | `say` prints numbers without decimal point (integral), decimals shortest-roundtrip, text raw, boolean `true`/`false`, lists `[a, b, c]`, maps `{k: v}`, struct fields; interpolation `{expr}` formats identically | 7.7, 8.2, D-34 printing |
| S-10 | Failure value at student level is a `text` message. Unhandled failure at runtime: print the LOM failure report (dev builds) and exit non-zero | 13.1, D-32 |
| S-11 | Modules: one file = one module; `use math for square root, floor` imports names into scope; the implicit `standard` module is always in scope | 7.13, 19.1 |
| S-12 | Tests: `test "name" …` blocks + `check that <comparison>` — run by `lagom test` on the interpreter; failing check = failed test | 5.1, 26.3 |
| S-13 | Options at M0: `first of list` returns `T?`; `say` prints value or `nothing` (D-34); `is nothing`/`is something` comparisons legal; full pattern forms are M1 | 7.6, 8.5, 5.1 |
| S-14 | Greedy resolution applies the 7.0.3 rule to *callees and reads* (implementation clarification): a multi-word callee splits at a known-function prefix (`say square root of 16` → `say(square root(16))`); a non-function callee with exactly one prepositional argument is a *read* (`name of p` = field read, `things at 2` = index read) — flowing reads and calls are one grammar, resolved by what is in scope. | 7.0.3, 7.9, 7.11 |

## 4. Spec gaps found during extraction (documented, smallest resolutions)

Per the critical rule, these are the *only* places where implementation extraction hit text that did not fully determine behavior. Each is recorded here and in `docs/14-m0-contradictions.md`; none required changing the frozen grammar.

| # | Gap | Smallest resolution | Affects |
|---|---|---|---|
| G-1 | §7.15 `foreach = "for each" pattern "in" expr` allows tuple/struct patterns, but M0 has no destructuring; the grammar's `pattern` also covers `match` (M1). The `repeat for each word, position in words` index variant (7.5) needs a concrete M0 form. | M0 implements `pattern` = single name (plain) and the two-name index form `repeat for each word, position in words` as its own statement production. Full patterns arrive with `match` (M1). | lexer/parser/sema |
| G-2 | `say` of a `decimal` format is explicitly deferred ("exact fractional-text formatting … spec at M1", doc 03). The calculator project needs a number now. | M0 formats decimals shortest-roundtrip (Rust `{}` for f64); integral values print without the `.0` only for `number`; a decimal integral value prints as written by shortest-roundtrip (e.g. `2.5`, `5` is `number` so prints `5`). Documented as provisional until doc 03 freezes. | runtime/format |
| G-3 | The grammar's `count = number "times" ["using" name]` reads a literal `number` token; `repeat n times using i` (count from a variable) is not derivable, yet the guessing-game stress program uses a variable round count. | The literal is what is normative at M0; variable counts are expressible with `repeat while` + a mutable counter (spec-faithful). The implementation *also* accepts a comparison-level head (`repeat n times using i`) behind an error message pointing at the `while` form — no silent grammar extension, the frozen grammar remains the parse. Wait — that is a silent extension. **Resolution: implement the frozen form only.** Variable counts use `repeat while`. | parser |
| G-4 | `check that <comparison>` — the comparison must be boolean-typed; the failing check's runtime message needs a rendered form of the operands. | Render `check that` failures as `check failed: <rendered comparison> (left = …, right = …)` with source span. | tests runtime |

## 5. Crate map (from doc 08 §9, M0 subset)

| Crate | Owns | Spec anchor |
|---|---|---|
| `lagom_diagnostics` | Span, Diagnostic, codes, student/normal/expert rendering | 21.4, 26.2 |
| `lagom_lexer` | UTF-8 scan, indentation scanner, continuation rule, phrase-tokens | 7.0, 7.15 |
| `lagom_ast` | Arena AST, lossless trivia, spans on every node | 21.1 |
| `lagom_parser` | Recursive descent, one bounded backtrack (`takes` filler — M1), error recovery | 21.1 |
| `lagom_sema` | Greedy multi-word names (7.0.3), local type inference (D-11), capability check, handling check | 8.1, 13.1 |
| `lagom_hir` | Desugaring: clauses→sigs, interpolation→format, word ops→ops; spans retained (HIR span contract, doc 08 §2) | 22.1 |
| `lagom_mir` | Typed, straight-line-with-branches form; drop elaboration | 22.2 |
| `lagom_lom` | Event ring, provenance records, failure report (interpreter hook) | 26.5, D-32 |
| `lagom_interp` | MIR tree-walking evaluator (REPL/REPL-grade tests, comptime path later) | 21.3 |
| `lagom_lir` | Backend-neutral: explicit CFG, machine-ish types | 22.3 |
| `lagom_codegen` | LIR → Cranelift module → object → link | 21.2, D-8 |
| `lagom_rt` | The runtime staticlib linked into every binary (format, text, list/map boxing, i64-checked arithmetic traps, LOM ring shim) | 21.1 |
| `lagom_driver` | Pipeline orchestration: source→binary, source→run, source→tests | 21.5 |
| `lagom_cli` | `lagom run/build/test/check/fmt/words/explain/new` | 26.1 |

## 6. M0 validation set

The four validation programs (33.1) live in `validation/`: calculator, guessing game, quiz, text adventure — each must `lagom run` and each is exercised by the integration tests. The LOM acceptance test seeds a failure in the guessing game and asserts the report names the failing value's origin.
