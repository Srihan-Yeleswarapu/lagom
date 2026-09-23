# Lagom — Language Architecture & Design Proposal

> **Simple syntax, not a simple language.**
>
> A 10-year-old should be able to write `say "Hello!"`. An expert should be able to write a database engine, a compiler, or an operating-system component — in the same language, without leaving it.

*Version: 0.1.0-draft (Phase 1 deliverable — design only, no implementation).*
*Status: proposal for review. Every major decision records alternatives and tradeoffs in the [Decision Log](#34-decision-log).*

**Lagom** is Swedish for *just enough* — the design philosophy of the language in one word.

---

## Table of contents

1. [Language names](#1-language-names)
2. [Vision](#2-vision)
3. [Design principles](#3-design-principles)
4. [Target users](#4-target-users)
5. [Feature matrix](#5-feature-matrix)
6. [Syntax philosophy](#6-syntax-philosophy)
7. [Proposed syntax](#7-proposed-syntax)
8. [Type system](#8-type-system)
9. [Memory management](#9-memory-management)
10. [OOP model](#10-oop-model)
11. [Functional programming](#11-functional-programming)
12. [Generics](#12-generics)
13. [Error handling](#13-error-handling)
14. [Concurrency model](#14-concurrency-model)
15. [Async model](#15-async-model)
16. [Metaprogramming](#16-metaprogramming)
17. [FFI design](#17-ffi-design)
18. [Unsafe / low-level design](#18-unsafe--low-level-design)
19. [Standard library](#19-standard-library)
20. [Package manager](#20-package-manager)
21. [Compiler architecture](#21-compiler-architecture)
22. [IR design](#22-ir-design)
23. [Backend strategy](#23-backend-strategy)
24. [Compilation-speed strategy](#24-compilation-speed-strategy)
25. [Runtime-performance strategy](#25-runtime-performance-strategy)
26. [Tooling strategy](#26-tooling-strategy)
27. [Educational progression](#27-educational-progression)
28. [Security model](#28-security-model)
29. [Self-hosting strategy](#29-self-hosting-strategy)
30. [Comparison against existing languages](#30-comparison-against-existing-languages)
31. [Unresolved questions](#31-major-unresolved-questions)
32. [Major risks](#32-major-risks)
33. [MVP proposal](#33-mvp-proposal)
34. [Decision log](#34-decision-log)

---

## 0. How to read this document: layers

Every feature in this proposal carries a **layer tag**. Layers answer one question: *when does a programmer meet this?* They are presentation layers over one coherent language — never separate dialects.

| Tag | Layer | Who meets it | Rule of thumb |
|---|---|---|---|
| `student` | Student | First lessons (ages ~10+) | Can be taught in one sentence, no jargon |
| `intermediate` | Intermediate | ~1 year in, or a motivated beginner | Standard CS concepts: types, interfaces, generics |
| `advanced` | Advanced | Experienced programmers | Concurrency, async, metaprogramming, FFI |
| `expert` | Expert | Systems programmers | Ownership, allocators, atomics, SIMD, ABI, asm |

Hard rules that follow from the layer system:

- **A beginner should encounter less complexity, not a less capable language.** Student-layer code is always valid expert-layer code; expert code uses the same semantics with more explicit control.
- Capabilities are never *absent* from the student layer by design failure — they are simply *not shown*. The compiler can always explain more when asked (`help`).
- Nothing above `student` may change the meaning of `student` code. Layers are strictly additive.
- Diagnostics, docs, and tooling filter by layer: the student mode of the compiler never prints the words "allocator" or "monomorphization" unless the student asks.

---

## 1. Language names

**Primary name: `Lagom`.** Chosen by the project owner. Swedish for *"just enough"* — directly expressing the philosophy: simple surface, complete capability. File extension: `.lagom`. CLI: `lagom`. Manifest: `Lagom.toml`.

Pronunciation: /ˈlɑː.ɡɔm/ ("LAH-gom").

Considered and rejected as primary names: *Braid* (collision with Braid, the game), *Onwards* (awkward as a CLI), *Sprout* (sounds like a toy — the exact perception this language must fight). The `.lagom` extension is unclaimed on major TLDs and has no significant prior-art collision in the programming-language space as of 2026. Trademark screening before 1.0 is listed in [§31 unresolved questions](#31-major-unresolved-questions).

---

## 2. Vision

Lagom is a general-purpose, statically typed, natively compiled programming language whose surface syntax reads like structured English while its underlying computational power is essentially unrestricted. A student transitions from Scratch or Code.org directly into Lagom and — without ever changing languages — progresses through variables and loops, functions and data structures, object-oriented and functional design, concurrency, memory control, and systems programming, all the way to writing a compiler in Lagom that compiles Lagom. The language hides *punctuation and ceremony*, never *concepts*: every syntactic nicety corresponds to a real, transferable programming concept. Safety is the default (memory-safe, type-safe, no null crashes, no data races), and power is opt-in (`unsafe`, ownership, allocators) rather than the price of admission. Compilation is fast enough that the compiler feels like a calculator, not a batch job — because a language for learners must respond at learner speed.

**What Lagom is not:** not a toy, not Python-with-English-words, not a natural-language programming system, not "Scratch text mode". It is a deterministic, formally specified, statically compiled systems language.

---

## 3. Design principles

The ranked goals from the project brief, restated as principles that recur throughout this document:

1. **Exceptional readability.** Code reads like the idea it expresses. `if age is greater than 13` is preferred over `if age > 13`. [§6](#6-syntax-philosophy)
2. **Exceptional beginner accessibility.** The first hour requires zero concepts beyond *do this, then that*. [§27](#27-educational-progression)
3. **Extremely low syntactic complexity.** Small keyword set, no operator zoo, one block form (indentation), one function form (capability clauses). [§7](#7-proposed-syntax)
4. **Strong transfer to other languages.** Every Lagom concept maps 1:1 onto a concept in Python, TypeScript, Java, Go, C, C++, Rust, or Swift — the syntax differs, the idea does not. [§27](#27-educational-progression), [§30](#30-comparison-against-existing-languages)
5. **Strong static analysis and teaching diagnostics.** The compiler is the first teacher. Errors say *what, where, why, how to fix, what to learn*. [§26](#26-tooling-strategy), [§31 unresolved questions](#31-major-unresolved-questions) for diagnostic-mode details.
6. **Very fast compilation.** Sub-second interactive builds for student-scale programs; incremental rebuilds for large programs. [§24](#24-compilation-speed-strategy)
7. **Extremely strong runtime performance.** Native code, zero-cost abstractions where possible; performance claims are measured, not asserted. [§25](#25-runtime-performance-strategy)
8. **Memory safety by default.** No use-after-free, no dangling pointers, no null dereferences, no data races in safe Lagom. [§9](#9-memory-management)
9. **Low-level programming when explicitly desired.** `unsafe` is a door, not a wall. [§18](#18-unsafe--low-level-design)
10. **Excellent tooling.** Formatter, LSP, debugger, profiler, test runner, package manager, REPL — designed together with the language, not after. [§26](#26-tooling-strategy)

**Non-goals** (equally binding):

- **Not** minimal character count. Lagom code is often *longer* than C; that is acceptable.
- **Not** natural-language understanding. No AI, no fuzzy parsing; the grammar is small and formally specified.
- **Not** being "easier than Python" by hiding real concepts. Variables, types, control flow, and memory are *taught*, not hidden.
- **Not** binary compatibility with C++ or seamless interop with arbitrary object systems. C ABI interop is the supported boundary (like Rust, Go, Swift, Zig).
- **Not** REPL-only execution as the primary model. The primary artifact is a native binary; the REPL is a teaching tool.
- **Not** trying to keep every feature. Features are rejected when their complexity outweighs their value — the Decision Log records the rejects (e.g., inheritance hierarchies as the default OOP story, string-templating macros, implicit exceptions).

**The one-sentence rule** that resolves most micro-debates: *a beginner should encounter less complexity, not a less capable language.*

---

## 4. Target users

| User | Enters via | Needs first | Grows into |
|---|---|---|---|
| **The 10-year-old** | Scratch / Code.org | `say`, `ask`, `make`, `if`, `repeat` — one concept per lesson | Tiny games, quizzes, stories |
| **The teenager** | First real language | Functions, lists, structs — feeling of real software | File tools, chat bots, small servers |
| **The CS student** | University course | Static types, algorithmic data structures, complexity analysis | Compilers, databases as coursework |
| **The self-taught adult** | Career change | Web/API programming with safety and good errors | Professional backend work |
| **The professional** | Go/Rust/Java | Fast builds, one deployable binary, no GC pauses to tune | Services, tooling, cross-platform apps |
| **The systems programmer** | C/C++/Zig | Predictable memory, manual control, FFI, SIMD | Engines, runtimes, kernels, embedded |

The design must serve the *edges* of this table, not just the middle: the 10-year-old shapes the student layer; the kernel hacker shapes the expert layer. The middle is easy; the edges are the design problem.

---

## 5. Feature matrix

The complete capability map with layer assignments. Every feature listed here has a defined syntax in [§7](#7-proposed-syntax) or a defined plan in the roadmap ([docs/10-roadmap.md](10-roadmap.md)); no capability is unspecified. Sub-sections in parentheses point to the section that specifies the feature.

### 5.1 Beginner core — `student`

| Feature | Syntax (§7) | Status |
|---|---|---|
| Output | `say "…"`, `say x` | M0 |
| Input | `make answer equal to ask "…"` | M0 |
| Text↔number conversion | `number from answer` (`can fail`), `text from 42` — [§7.1](#71-output-and-input--student) | M0 |
| Variables (immutable by default) | `make x equal to 15` | M0 |
| Mutable variables | `make changing score equal to 0`, `set score to 5` | M0 |
| Arithmetic | `+ - * /`, `divided by`, `divided evenly by`, `remainder of a and b` | M0 |
| Comparisons | `is greater than`, `is less than`, `is equal to`, `is not equal to`, `is at least`, `is at most` | M0 |
| Boolean logic | `and`, `or`, `not` | M0 |
| Conditionals | `if … / otherwise if … / otherwise` | M0 |
| Counting loops | `repeat 10 times using i` | M0 |
| While loops | `repeat while …` | M0 |
| For-each loops | `repeat for each item in things` | M0 |
| Loop control | `stop`, `next` | M0 |
| Lists | `make things equal to a list of "a", "b", "c"` | M0 |
| Maps | `make ages equal to a map from "ana" to 11, "bo" to 12` | M0 |
| Functions | `function greet` + `takes` / `returns` clauses, `gives back …` | M0 |
| Strings & interpolation | `text` type, `"hello {name}"` | M0 |
| First-class numbers | `number` (i64-backed) with `decimal` promotion on true division | M0 |
| Basic tests | `test "name"` + `check that …` | M0 |
| Modules | `use math for square root` | M0 |
| Errors as values (basic) | `can fail`, `fail with`, `attempt … if it fails then …` | M0 (basic) / M1 (full) |
| Options as values (basic) | `first of` returns an option; `say` prints the value or the word `nothing` (D-34) | M0 (basic) / M1 (full) |

### 5.2 Intermediate — `intermediate`

| Feature | Status |
|---|---|
| Optional values (`nothing` / option types) — [§8.5](#85-option-and-result-types--intermediate) | M1 |
| Result/error values (full) — [§8.5](#85-option-and-result-types--intermediate) | M1 |
| Structs (plain data types) — [§7.11](#711-structs--student) | M1 |
| Enums & pattern matching — [§7.12](#712-enums-and-sum-types--intermediate) | M1 |
| Classes, methods, constructors — [§10](#10-oop-model) | M1–M2 |
| Interfaces (protocols) — [§10.6](#106-interfaces--intermediate) | M2 |
| Generics with constraints — [§12](#12-generics) | M2 |
| Type aliases, tuples, function types — [§8](#8-type-system) | M1–M2 |
| Iterators & lazy sequences — [§11.3](#113-iterators--intermediate) | M2 |
| Closures & higher-order functions — [§11.1](#111-closures-and-higher-order-functions--intermediate) | M1–M2 |
| Files & filesystem, serialization (JSON), CLI args — stdlib [§19](#19-standard-library) | M1–M3 |
| Package manager & registry — [§20](#20-package-manager) | M2–M3 |
| Formatter, doc generator — tooling [§26](#26-tooling-strategy) | M2–M4 |

### 5.3 Advanced — `advanced`

| Feature | Status |
|---|---|
| Tasks (structured concurrency), channels — [§14](#14-concurrency-model) | M2 |
| Async via `can wait` capability — [§15](#15-async-model) | M2 |
| C FFI (call out / be called) — [§17](#17-ffi-design) | M3 |
| Shared memory & mutexes, `shared` types — [§14.4](#144-shared-memory-and-race-freedom--advanced) | M2–M3 |
| Compile-time evaluation — [§16](#16-metaprogramming) | M3 |
| Property-based testing & fuzzing — tooling [§26](#26-tooling-strategy) | M3–M4 |
| Cross-compilation — [§23.4](#234-targets-and-cross-compilation) | M3 |
| Profiler & debugger — tooling [§26](#26-tooling-strategy) | M3–M4 |
| Reflection (compile-time; runtime metadata opt-in) — [§16.4](#164-reflection--advanced) | M3 |

### 5.4 Expert — `expert`

| Feature | Status |
|---|---|
| Ownership & borrowing (zero-cost sharing) — [§9.6](#96-ownership-and-borrowing--expert) | M3–M4 |
| `unsafe` blocks, raw pointers, manual allocation — [§18](#18-unsafe--low-level-design) | M3–M4 |
| Custom allocators & arenas — [§9.8](#98-allocators-and-arenas--expert) | M4 |
| Fixed-size integer types, packed structs, alignment — [§18.3](#183-layout-control-packed-structs-alignment-fixed-size-integers--expert) | M4 |
| SIMD vectors & CPU intrinsics — [§18.5](#185-simd-and-intrinsics--expert--elite) | M4–M5 |
| Atomics & memory ordering — [§14.5](#145-atomics-and-lock-free-programming--expert) | M4 |
| ABI control & calling conventions — [§18.4](#184-abi-and-calling-conventions--expert) | M5 |
| Inline assembly & linker control — [§18.6](#186-inline-assembly-and-linker-control--expert) | M5–M6 |
| Kernel/bare-metal targets (freestanding) — [§23.4](#234-targets-and-cross-compilation) | M6 |
| Compiler plugins / new IR passes — [§16.5](#165-compiler-plugins--expertdeferred) | M6+ (deferred) |

Milestone definitions are in [§33](#33-mvp-proposal) and [docs/10-roadmap.md](10-roadmap.md).

---

## 6. Syntax philosophy

### 6.1 Words over punctuation — but a *formal* grammar

Lagom prefers words when words are as clear as punctuation. This is not natural-language programming. The grammar is a small, deterministic, formally specified CFG (sketch in [§7.15](#715-formal-grammar-sketch)); the compiler needs exactly one parse for every program, and it needs no intelligence to find it. Words are chosen so that the *parse the compiler finds* is the *meaning a human reads*.

**Why words over symbols?**

- **Educational transfer:** research on novice programmers consistently shows that symbol-heavy syntax is a significant early barrier — students confuse `=` with mathematical equality, misread `>` as "greater-or-equal", and struggle with parentheses balance. Words like `is greater than` name the concept they denote, so learning Lagom *is* learning the concept, not learning an encoding of it.
- **Readability at distance:** `if age is greater than 13 and score is at least 50` survives projection screens, low vision, and dyslexia better than `if age > 13 && score >= 50`.
- **Scratch continuity:** Scratch blocks are labeled with words ("greater than", "and", "repeat 10"). The transition from Scratch to Lagom is a translation of *blocks into lines*, not of *blocks into punctuation*.

**Why not full natural language?** Ambiguity. If `make the box full` is valid, the grammar needs world knowledge to disambiguate. Lagom instead defines ~120 reserved words and one compositional grammar. The student never writes something the compiler misreads; the expert can always predict the parse. The illusion of natural language comes from *deliberately chosen vocabulary*, not from parsing freedom.

### 6.2 The canonical example

```lagom
make age equal to 15

if age is greater than 13
    say "You can join!"
otherwise
    say "Sorry, come back later."
```

### 6.3 Layered vocabulary rule (the anti-bloat mechanism)

New features must express themselves through *existing* clause words whenever possible — the capability-clause system ([§7.8](#78-functions-and-capability-clauses--all-layers)) is the template: `takes`/`returns` for functions extended to `can fail` and `can wait` rather than inventing `async function` / `throws` / `Result<_, _>` as separate mechanisms. Any proposal that adds a new *statement shape* must show why an existing shape cannot express it. This is the single strongest defense against syntax creep.

### 6.4 Rejected syntax philosophies

| Alternative | Why rejected |
|---|---|
| Symbolic (C-family) | Fails goals 1–3 for the target audience; the entire premise of Lagom is the opposite. |
| Full natural language | Ambiguous; needs AI or unbounded lookahead; not deterministic; violates goal 3 and the brief's explicit constraint. |
| S-expression (Lisp-family) | Superb for metaprogramming, hostile to the target audience's first hour. |
| Python-like symbolic with significant whitespace | Great second language; the symbol barrier is exactly what Lagom removes first. |
| Scratch-like drag-and-drop *text* hybrid | Two syntaxes to maintain, two toolchains, and the text form ends up unparseable. Lagom's student layer *is* the bridge. |

---

## 7. Proposed syntax

The complete surface syntax, feature by feature. For each: student-readable form, advanced form, internal representation, equivalents in two established languages, parsing notes, and ambiguities. The formal grammar sketch is [§7.15](#715-formal-grammar-sketch).

### 7.0 Lexical rules — `student`

- **Source:** UTF-8. `.lagom` extension.
- **Blocks:** indentation (4 spaces per level in canonical form; the formatter enforces 4, the compiler accepts any consistent width ≥ 2). One blank line between top-level items is conventional.
- **Statements:** one per line. A statement continues onto the next line when the next line is *more indented* than the statement's line and the statement is syntactically incomplete (see 7.0.1). No semicolons, no line-continuation characters.
- **Comments:** `#` to end of line. Doc comments: `##` (attached to the following item).
- **Identifiers:** lowercase words, digits, underscore: `high_score`. Reserved words (~120) are listed in the grammar; user names may not collide with them.
- **Multi-word identifiers are the norm.** `total score`, `top player` — spaces are allowed *inside* a name when the words are not a grammar keyword sequence and the parse stays deterministic (rule 7.0.3). This is what makes `function calculate average` with `takes number of correct answers` read naturally.

#### 7.0.1 The continuation rule (the one subtle part of the lexer/parser boundary)

A line joins the previous statement iff (a) the previous statement is incomplete without it, and (b) it is indented deeper than the statement's start. Example:

```lagom
make message equal to "Hello, " plus name plus ", " plus
    "welcome to Lagom!"        # continues the previous statement (incomplete + deeper indent)
```

`plus` at end-of-line makes the expression incomplete → continuation. If an expression is *complete* at end-of-line, the statement ends. The parser needs no backtracking: completeness is a property of the token at the line end. This rule is formally defined in the grammar sketch and is the single most-tested parser invariant.

#### 7.0.2 Case rule

Keywords are always lowercase. User identifiers must start lowercase (`Student` as a type name is written `student` — type names are just names; the type context disambiguates, see 7.11). This avoids a second case-sensitivity cognitive rule.

#### 7.0.3 Multi-word-name disambiguation (the main ambiguity risk)

`make top score equal to 0` — is `top score` a name? Rule: greedy match against *known names in scope*, else parse `top` as a modifier word only if it is in the modifier set (it is not; `changing` is the only variable-declaration modifier), else `top score` is the name. Declarations *bind* the name, so later uses resolve against the bound set with the same greedy-longest-match. Ambiguity between a bound name and a keyword is impossible because keywords win lexically and names may not collide with keywords. Ambiguity between two *bound names* (`top` and `top score` both bound) is a compile error — the declaration is rejected, not the use. This keeps parsing deterministic with no backtracking. Cost: an expert cannot freely use the words `equal`, `to`, `is`, `and`… inside names — accepted; the vocabulary cost is small and the determinism payoff is total.

### 7.1 Output and input — `student`

```lagom
say "Hello, world!"
make answer equal to ask "What is your name?"
say "The answer is {answer}!"           # interpolation (7.7)
```

- Internally: `say` is a builtin function `say(…arguments: text) -> nothing` using variadic formatting; `ask` is `ask(prompt: text) -> text`.
- **Text↔number conversion is a student-layer API, not syntax** (R-10 of doc 11 — the calculator and guessing-game validation projects need it): `number from "42"` parses text into a number, `decimal from "3.5"` into a decimal, and `text from 42` formats back. Parsing is `can fail` — `"42x"` is a *teaching moment about input*, not a crash, and the failing form composes with the error model the student already knows:

```lagom
make answer equal to ask "Pick a number:"
attempt number from answer if it fails then
    say "That was not a number!"
otherwise
    say "You picked {result}."
```

- Conversion is deliberately *not* `as number` syntax (REJECT list, doc 11): `as` is reserved for the `attempt … as problem` binding, and parsing-visible failure is the honest shape for text input.
- Equivalents: Python `print()`/`input()`; C `printf`/`scanf` (sans format-string fragility — Lagom formats values directly, no format-specifier/argument mismatch class of bugs).
- Parsing: `say` and `ask` are ordinary functions from the implicit `standard` module — *not* parser keywords. Their readable call syntax is a *calling-convention* feature (7.9), not a special form.

### 7.2 Variables and assignment — `student`

```lagom
make age equal to 15                  # immutable binding
make changing score equal to 0        # mutable binding
set score to 10                       # assignment to an existing mutable
increase score by 5                   # score = score + 5
decrease score by 5                   # score = score - 5
```

- Internally: AST `Let { name, mutable, init }`, `Assign { target, value }`, `CompoundAssign { op, target, value }` (compound forms desugar in HIR).
- Equivalents: `const`/`let` (JS, Swift `let`/`var`), Python `x = 5` (no immutability), Rust `let` / `let mut` — Lagom's `make changing` is literally `let mut`, spelled out.
- **Immutability by default** is a deliberate teaching + safety choice (same conclusion Rust and Swift reached; Python's lack of it is a documented source of student bugs).
- Parsing: `make` opens a declaration; `changing` is the single modifier word; `equal to` binds name → initial value. Deterministic by rule 7.0.3.

### 7.3 Expressions and operators — `student`

The only symbolic operators are the four school-math operators:

```lagom
make total equal to a + b * c          # precedence: * / before + - (school rules)
make half equal to total divided by 2          # true division: 5 / 2 = 2.5, result decimal
make each equal to total divided evenly by 2   # floor division: 5 // 2 = 2, result number
make rest equal to remainder of total and 2    # modulo
```

Comparisons and logic are words:

```lagom
if age is greater than 13 …
if name is equal to "bo" …
if count is not equal to 0 …
if score is at least 50 …              # >=
if score is at most 50 …               # <=
if age is greater than 13 and not banned or admin …   # precedence: not > and > or (same as most languages)
```

- Why keep `+ - * /` symbolic? **Scratch precedent** (Scratch's arithmetic blocks show `+ − × ÷` as glyphs), universality, and the fact that arithmetic is the *first* symbolic notation every child learns in school. Words for arithmetic (`a plus b`) are legal *aliases* everywhere, but symbols are the canonical form — the reverse of comparisons, where words are canonical.
- `divided by` vs `/`: `/` is an alias for `divided by` (true division). `divided evenly by` is the floor-division word because its Python equivalent `//` has no school-math meaning a 10-year-old knows, while "divide evenly" is precisely how children describe it ("5 divided evenly by 2 is 2, with 1 left over" — hence `remainder of`). See Decision Log D-7.
- Precedence ladder (tightest to loosest): `not` → `* / divided by divided evenly by remainder of` → `+ - plus minus` → comparisons (`is … than/equal to`) → `and` → `or`. This exactly matches Python/C for the symbolic subset, so transfer goes *both* ways.
- Internally: expression AST is conventional binary/unary trees; word operators desugar to the same operator nodes as symbolic ones in HIR.
- Equivalents: Python (`and or not`, `//`, `/`, `%`); Rust (`&& || !`, `/`, `%`) — Lagom's mapping to both is 1:1 at the concept level.

### 7.4 Conditionals — `student`

```lagom
if age is greater than 13
    say "You can join!"
otherwise if age is greater than 5
    say "Kids area."
otherwise
    say "Too young."
```

- Keywords: `if`, `otherwise if`, `otherwise`. Internally: `If { cond, then, else }` chains — `otherwise if` is sugar for a nested else-if in AST (flat in HIR for codegen).
- Equivalents: Python `if/elif/else`; Java `if/else if/else`; Rust `if/else if/else` (expression-capable). **Lagom's `if` is also an expression** in intermediate+ code (7.14), preserving Rust/Swift's conditional-expression power while the student only ever sees it as a statement.
- Parsing: block by indentation; condition is everything after `if` to end-of-line (or continuation lines).

### 7.5 Loops — `student`

```lagom
repeat 10 times using i
    say i

repeat while score is less than 100
    increase score by 10

repeat for each item in things
    say item

repeat for each word, position in words     # index variant (like Python enumerate)
    say "{position}: {word}"

stop        # break
next        # continue
```

- One keyword (`repeat`) for all three loop shapes — the shape is determined by the continuation phrase (`N times` / `while` / `for each`). One concept ("do this many times / as long as / for each thing"), one keyword. `stop`/`next` mirror Scratch's "stop this loop" and "next iteration" blocks.
- Internally: `Loop::Count { n, binding }`, `Loop::While { cond }`, `Loop::For { pattern, iter }`; all three desugar to one `Loop` MIR form + iterator protocol (11.3).
- Equivalents: Python `for i in range(10)` / `while` / `for x in xs`; Rust `for i in 0..10` / `while` / `for x in xs`; C `for(;;)`. Concept transfer is exact.

### 7.6 Collections — `student`

```lagom
make things equal to a list of "apple", "pear", "plum"
make ages equal to a map from "ana" to 11, "bo" to 12
make pairing equal to a pair of 3 and 4            # fixed 2-tuple

say first of things                                # option (see 8.5)
say size of things
set things at 2 to "plum"                          # index assignment
say things at 0                                    # explicit indexing (0-based, taught explicitly)

make copy equal to a; set copy at 1 to 99          # copy then mutate one slot
```

- Internally: list = growable vector (heap, value-like — see §9); map = hash map; pair = 2-tuple. Literal forms are ordinary constructor calls desugared in HIR: `a list of …` → `List::from(…)`.
- `first of`, `size of`, and indexing (`things at 0`) are ordinary functions with readable call syntax (7.9). Indexing is a function, not a *syntax-level operator* — a deliberate choice: it keeps the operator table at four symbols, and it lets indexing fail as an error value (out-of-bounds → failure, 8.5) instead of a special panic path.
- 0-based indexing is taught explicitly from the first indexing lesson (a 1-based "sugar" mode was rejected — see Decision Log D-9 — because two indexing systems break transfer to every mainstream language).
- Equivalents: Python `list/dict/tuple`; Rust `Vec/HashMap/(a, b)`.
- `first of` returns an option; at the student layer, `say` of an option prints the value, or the word `nothing` when absent (the R-18 printing rule, 8.5) — the empty-list case is a *visible* lesson, not a silent blank. Basic option printing/promotion to the student layer: see 8.5 and the M0 gate (33.1).

### 7.7 Strings — `student`

```lagom
make greeting equal to "Hello, {name}!"          # interpolation, any expression inside {}
make long equal to "line one
line two"                                        # multi-line literal
make letter equal to greeting at 2                        # Unicode code point by position
make count equal to size of greeting             # counts characters (code points), not bytes
```

- Type name: `text`. UTF-8 in memory. Interpolation is desugared to `format` calls in HIR — no runtime string parsing. Comparisons are code-point order; concatenation via `+`/`plus`.
- Equivalents: Python f-strings; Kotlin string templates; C# interpolation. The `{expr}` form matches Python/JS template habits.

### 7.8 Functions and capability clauses — all layers

The single most important structural decision in the syntax. Functions are declared as a sequence of **clauses**, and the *same clause grammar* extends from student functions to expert capabilities:

```lagom
## Greets one person by name.
function greet
    takes text called name
    say "Hello, {name}!"

function calculate average
    takes a list of numbers called scores
    returns a number
    make total equal to 0
    repeat for each score in scores
        increase total by score
    gives back total divided evenly by size of scores
```

- Clause words, in order: `takes …` (one per parameter), `returns …` (optional; inferred when absent), `can fail …` (error capability, 13), `can wait` (async capability, 15 — orthogonal to `can fail`; both compose on one function). Body indented under the header.
- Calling: `greet "bo"` (positional) or `greet with name "bo"` (labeled — the same `with` shape as construction, 7.9).

**The beginner's signature from the project brief** — `takes number of correct answers` — is the same grammar: `takes number of correct answers` declares a parameter named `correct answers` of type `number` (type word first, `called` optional when the type word directly precedes the name; an `of` directly after a *complete atomic type* is filler — the type grammar consumes compound types like `a list of …` greedily first, so `takes number of correct answers` parses as type `number`, filler `of`, name `correct answers`):

```lagom
function calculate score
    takes number of correct answers
    takes number of total questions
    returns a number
    gives back correct answers divided by total questions
```

**The expert uses the identical clause grammar with more explicit types:**

```lagom
function merge
    takes a list of numbers called left      # precondition documented: left is sorted
    takes a list of numbers called right     # precondition documented: right is sorted
    returns a list of numbers
    …
```

- Internally: `FnSig { params: Vec<Param>, ret: Type, caps: CapabilitySet }` where `CapabilitySet ⊆ {fail(ErrorType?), wait}`. Capabilities are part of the type — a `can fail` function *is* a function returning a result type (13); a `can wait` function *is* an async function (15). There is no separate `async function` syntax, no separate `throws` syntax, no separate `Result<T, E>` wrapper at the call site.
- Equivalents: Rust `fn f(x: T) -> Result<U, E>` (+ `async fn`); Swift `func f(_ x: T) throws -> U` (+ `async`). Lagom compresses all four axes (params, return, errors, async) into one readable clause list.
- **Why signature-as-clauses instead of `function name(params)`?** (a) It reads like the brief's educational target. (b) It scales: capabilities attach as clauses without new syntax. (c) It is trivially parseable — clause order is fixed. (d) It gives labeled arguments for free. Cost: more vertical lines than `f(x, y)` — accepted (goal ranking explicitly allows longer source).
- Anonymous functions and closures: 11.1. Function types: 8.6. Recursion: unrestricted (with tail-call optimization in the MIR lowering, best-effort).
- Variadic parameters: `takes any number of numbers called values` (`advanced` — implemented as slice parameter; the production is in the grammar sketch, 7.15).
- Default parameter values: `takes number called speed with default of 10` (`intermediate`).
- **The filler-`of` mechanism is the grammar's one bounded backtrack point** (R-14 of doc 11): the type grammar consumes compound types (`a list of …`) greedily; if the following *name* production then fails, the parser re-reads the trailing `of …` as filler. One documented backtrack point, confined to `takes` clauses, pinned exhaustively by the ambiguity corpus (doc 02) — every combination of atomic/compound/generic types × filler/name boundaries is a corpus entry. The "no backtracking" claim in 7.0.1 therefore reads: *no backtracking except this one documented point, which is corpus-pinned.*

### 7.9 Readable call syntax — `student`

Function calls may flow rightward without parentheses when unambiguous:

```lagom
say first of things
make larger equal to bigger of a and b             # bigger(a, b) — the maximum
say 3 plus 4 and bigger of 10 and 20               # two arguments: 7 and bigger(10, 20)
```

- Rule: a call is `name [argument1] [preposition argument2] … [and argument3 …]` where **arguments bind at the additive level** (numbers, names, arithmetic — the R-4 rule of doc 11). This makes `and` inside a call *unambiguously* an argument separator: the sentence a human reads and the sentence the compiler parses are the same. `bigger of a and b or c` is a **parse error with a teaching diagnostic** ("did you mean `bigger of a and (b or c)`, or `(bigger of a and b) or c`?") — the ambiguity is caught at compile time, never silently misparsed. Comparisons and boolean operators inside arguments need parentheses (`bigger of (a or b) and c`) or the `where` suffix (7.9's `keep scores where it is at least 80`), which takes full expressions. The *innermost* open call greedily consumes each `and`-separated argument; the closed preposition set (`of`, `at`, `from`, `to`) introduces prepositional arguments. Labeled arguments use the construction word: `greet with name "bo" and punctuation "!"` (the same shape as struct construction, 7.11). A function-valued argument is introduced by `using`: `map scores using double`, with inline lambdas written `taking … giving back …` (`map scores using taking score giving back score plus 5`) or, for a single parameter, the implicit name `it` (`map scores using it plus 5`). A `where` clause (`keep scores where it is at least 80`) is sugar for `keep scores using taking it giving back it is at least 80` — one desugaring rule, teachable in one sentence.
- This is **syntax sugar over ordinary calls**: `bigger of a and b` ≡ `bigger(a, b)`. The parenthesized form is always legal and always means the same thing: `bigger(a and b)` is *not* valid; use `bigger(a, b)` or the flowing form. (Comma form and flowing form may be mixed: `bigger of a, b`.)
- Risk register: this was the second-highest ambiguity source (after multi-word names) until the R-4 fix made arguments additive-level; the remaining risk is the human who *expects* full-expression arguments. The diagnostic names both readings. Grammar sketch 7.15 encodes the rule; the parser test corpus (§36 of roadmap) includes every combination of nested flowing calls, and the R-4 divergence case (`bigger of a and b or c`) is a pinned corpus entry. If corpus evidence shows real ambiguity trouble at M0 review, the fallback is to require commas in all multi-argument flowing calls (`bigger of a, b`) — decision point tracked in 10-roadmap.

### 7.10 Types as annotations — `intermediate`

Types are always optional (inference, §8.1) and always spelled in the same *wordy* style as the rest of the language:

```lagom
make changing attempts equal to 0 of type number
make names equal to a list of "ana" of type a list of text
```

`of type` introduces an explicit annotation. In `takes`/`returns` clauses the type word appears directly (`takes number called x` = `takes x of type number`). Type names are: `number`, `decimal`, `text`, `boolean` (whose values are `true`/`false`), `a list of T`, `a map from K to V`, `a pair of A and B`, `a box of T` (heap cell, 13's recursive-type rule and 12.2), option types (`T?` or `a T or nothing`, 8.5), user-defined names. Expert layer adds fixed-size integers and fixed-size decimals (18.3).

### 7.11 Structs — `student`

```lagom
structure player
    has name of type text
    has score of type number

make p equal to a player with name "bo" and score 0
say name of p                     # field access, flowing form
increase score of p by 10
```

- Internally: product type, value semantics, inline/stack allocation by default (§9). `with name … and score …` is a labeled constructor expression; field order is irrelevant.
- Equivalents: Rust `struct` + `Player { name: "bo", score: 0 }`; C `struct`; Python `dataclass`.
- **Structs are the default data shape.** Classes (10) are introduced only when behavior-with-state is the point; the docs steer composition-first.

### 7.12 Enums and sum types — `intermediate`

```lagom
kind shape
    is a circle with radius of type number
    is a rectangle with width of type number and height of type number
    is a blank

function describe
    takes shape called s
    returns text
    match s
        when a circle with radius r
            gives back "round, {r}"
        when a rectangle with width w and height h
            gives back "boxy, {w} by {h}"
        when blank
            gives back "empty"
```

- `kind` declares an algebraic data type (sum type); `match` + `when` patterns destructure. Patterns: literal, binding, tuple/struct destructuring, `when nothing`/`when something with value v` for optionals. Exhaustiveness is enforced — a `match` that misses a case is a compile error with the missing case named.
- Internally: tagged union with discriminant; match lowers to switch + field loads. This is the Rust `enum`/`match`, ML/Haskell ADT — the single most valuable intermediate-level concept for later languages (Option/Result in Rust and Swift *are* enums).
- Equivalents: Rust `enum Shape { Circle { radius: f64 }, … }` + `match`; Swift `enum` + `switch` with associated values; OCaml/Haskell variants.
- `student` layer sees `kind` only in lesson form ("a shape is a circle, a rectangle, or blank") without pattern variables until `intermediate`.

### 7.13 Modules — `student` → `intermediate`

```lagom
use math for square root, floor           # import specific names
use drawing                               # import module namespace
use cards from "card-game/deck"           # package-qualified import (20)

square root of 16
use drawing for circle
say circle at 3 and 4
```

- One file = one module; directory = package (20). `use` is the only import form (no re-export gymnastics at first; `export` clause planned M3 if ecosystem demands it — tracked in unresolved questions). **There is no dot operator** (R-2 of doc 11): imported names are used plainly after `use … for …` (`use drawing for circle`, then `circle at 3 and 4`), and methods are ordinary calls with the receiver as first argument (10.2). The grammar has no `.` production.
- Cycles are compile errors with the cycle printed. Names are file-relative at student level and package-relative in packages.
- Equivalents: Python `from math import sqrt`; Rust `use math::{sqrt, floor};`; Go `import "math"`.

### 7.14 Everything-is-an-expression boundary

For transfer to Rust/Swift/functional languages, **most constructs are expressions**: `if` (when both branches present), `match`, blocks, `attempt`. Statements are: declarations, assignments, `repeat`, `stop`, `next`, `use`. `gives back` is the return form (expression), `fail with` the failure form (13). This uniformity is what lets Lagom grow functional idioms (11) without a second "expression language".

### 7.15 Formal grammar sketch

The grammar is deliberately small. This sketch is normative for shape (the full machine-readable grammar lives in the spec, doc 02, and is *generated into the parser tests*):

```ebnf
program     = { blank | toplevel } ;
toplevel    = use | function | structure | kind | class | interface | typealias
            | test | export | doccomment statement ;   (* M1–M2 declaration forms are in the sketch
                                                          from day one so the generated parser tests
                                                          cover the whole language, not just M0 *)

block       = INDENT { statement } DEDENT ;          (* indentation-scanner generated, no tabs/spaces mixing *)

statement   = say | ask | make | set | increase | decrease
            | if | repeat | match | stop | next | use | checkthat
            | giveback | failwith | attempt
            | taskstmt | unsafe | region | usingscope
            | assignment | exprstmt ;                (* exprstmt = a call used for effect: `bump c` *)

make        = "make" ["changing"] name "equal to" ( expr | funclit ) ;   (* funclit = block lambda (11.1) *)
funclit     = "a function" ["taking" name { "and" name }] block ;   (* the block may open with
                                                        takes/returns/can clauses (11.1: `twice`) *)
set         = "set" target "to" expr ;
increase    = ("increase"|"decrease") target "by" expr ;
target      = name { prep additive } ;               (* set things at 2 to "plum"; field access via
                                                        the same closed prepositions (7.6, 7.11) *)
if          = "if" expr block { "otherwise if" expr block } [ "otherwise" block ] ;
repeat      = "repeat" (count | whilec | foreach) block ;
count       = number "times" ["using" name] ;
whilec      = "while" expr ;
foreach     = "for each" pattern "in" expr ;

function    = "function" name { clause } block ;
clause      = takes | returns | canfail | canwait ;
takes       = "takes" [articles] [parammode] (type | "any number of" type) ["called"] name
              ["with default of" expr] ;
parammode   = "owned" | "borrowed" ;                 (* owning-region transfers only (9.6, expert) *)
            (* greedy-type-then-filler (7.8): `takes number of correct answers` = type `number`,
               filler `of`, name `correct answers`. The ONE bounded backtrack point in the grammar: *)
returns     = "returns" [articles] type ;            (* commit to the greedy type read; if the name
                                                        production then fails, re-read the trailing
                                                        `of …` as filler. Pinned by the corpus, doc 02. *)
canfail     = "can fail" [type] ;
canwait     = "can wait" ;

hasfield    = "has" name "of type" type ;            (* struct/class field clause (7.11, 10.2) *)
constr      = "construction" { takes | canfail | canwait } block ;   (* 10.3 — takes-clauses like a
                                                        function; no `returns` (a construction
                                                        returns the new object) *)
method      = "can" name [takes] block ;             (* method clause inside a class block (10.2):
                                                        methods are ordinary functions whose FIRST
                                                        parameter is the receiver (10.2, R-2) *)
class       = "class" name ["of" name] ["extends" name] ["does" name { "," name }] block ;
                                                      (* 10.2/10.5/12.2, M1–M2; `of some type` names
                                                         the type parameter (12.2) *)
interface   = "interface" name ["of" name] { interfacereq } ;   (* 10.6, M2 *)
interfacereq = "can" name [takes] [canfail] [canwait] ;   (* a requirement — no body (10.6) *)
typealias   = "a type called" name "is a" type ;     (* 8.4, M1 *)

giveback    = "gives back" expr ;
failwith    = "fail with" expr ;
attempt     = "attempt" orexpr [attempttail] ;       (* ONE production, used in statement AND
                                                          expression position (R-20.1, 13.1) *)
attempttail = "and pass the problem on"                (* propagation — ONE reserved phrase-token, so
                                                          the call grammar's greedy `and` can never
                                                          consume into it (7.15 note, R-11) *)
            | "if it fails" "then" block ["otherwise" block]
            | "as" name block ["otherwise" block] ;
            (* in the if-it-fails/otherwise form: error is bound to `problem`, success value to `result`.
             `and pass the problem on` is valid only when the attempt is the whole statement
             (parser-checked: nothing may follow the tail). *)

match       = "match" expr { "when" pattern block } [ "otherwise" block ] ;
pattern     = literal | name | "a" usertype [destructure] | "nothing" | "something with value" pattern | pair ;

test        = "test" textliteral block ;
checkthat   = "check that" comparison ;
doccomment  = "##" text ;
export      = "export" (function | structure) ;
use         = "use" name [ ("for" name { "," name }) | ("from" textliteral) ] ;   (* 7.13 *)

(* Concurrency / regions / resources — deliberately clause-free, one statement per form (14.2): *)
taskstmt    = "start" "a" "task" block { "start" "a" "task" block } ["wait for all tasks"] ;
            (* one statement per spawn — no chaining word; region exit joins implicitly.
               `wait for all tasks` is a single reserved phrase-token, same rule as above. *)
unsafe      = "unsafe" ["because" text] block ;      (* 18.1 — the because-justification is
                                                        compiler-enforced from M3 *)
region      = "within" ("owning" | "arena" name) block ;   (* memory-discipline regions (9.6, 9.8) *)
usingscope  = "using" expr block ;                   (* disposable-resource scope (10.4); statement-
                                                          initial `using` never collides with the call
                                                          suffix `using` (position-distinct) *)

expr        = orexpr ;
orexpr      = andexpr { "or" andexpr } ;
andexpr     = notexpr { "and" notexpr } ;
notexpr     = ["not"] comparison ;
comparison  = additive [ ("is" cmpword | cmpword) additive ] ;
cmpword     = "equal to" | "not equal to" | "greater than" | "less than"
            | "at least" | "at most" ;               (* `same as`/`different from` deleted (R-6):
                                                          one equality operator, two behaviors (8.5) *)
additive    = multiplicative { ("+"|"plus") multiplicative | ("-"|"minus") multiplicative } ;
multiplicative = unary { ("*"|"times") unary | ("/"|"divided by") unary
                          | "divided evenly by" unary | ("remainder of"|"modulo") unary } ;
unary       = "-" primary | primary ;
primary     = literal | name | call | "(" expr ")" | listlit | maplit | pairlit
            | interpolation | newexpr | attempt ;    (* bare attempt-as-expression, 13.1 *)

(* The call grammar (R-1): ONE production covers every call form in this document —
   `ask "What is your name?"` (bare positional), `divide 10 and 0` (positional + `and`-separated),
   `send "hello" to messages` (positional + prepositional), `bigger of a and b`,
   `square root of 16`, `things at 0`. Arguments are ADDITIVE-level (R-4): `and` inside a call is
   always an argument separator, so the human parse and the compiler parse are the same sentence. *)
call        = flowcall { "with" name additive } { "using" lambda } { "where" orexpr } ;
            (* each suffix at most once, in this fixed order *)
flowcall    = name [additive] { prep additive } { "and" additive } ;   (* closed preposition set *)
prep        = "of" | "at" | "from" | "to" ;          (* boolean arguments need parentheses or the
                                                          `where` suffix — the ambiguous form is a
                                                          compile error with a teaching diagnostic (7.9) *)
lambda      = comparison                             (* a bare name passes the function itself:
                                                        `using double`; a larger comparison is
                                                        evaluated per element with `it` — and any
                                                        names bound by preceding `with` suffixes —
                                                        in scope: `using it plus 5`,
                                                        `using start plus it` (11.2) *)
            | "taking" name { "and" name } "giving back" comparison ;  (* inline lambda — the body is
               one additive-or-comparison expression; boolean/multi-operator bodies use the block
               lambda `a function taking n …`, delimited by indentation (R-3, 11.1) *)

newexpr     = "a new" name { "with" name additive } ;   (* construction: a new counter with step 2 (10.3) *)
listlit     = "a list of" expr { "," expr } ;
maplit      = "a map from" primary "to" primary { "," primary "to" primary } ;
            (* keys are PRIMARY expressions (R-5): a flow-call key would collide with the `to`
               preposition — `home to work` must parse as the pair, never as a call `home(work)` *)
pairlit     = "a pair of" expr "and" expr ;

type        = "number" | "decimal" | "text" | "boolean"
            | type "?"                              (* optional — canonical compact spelling (8.5, R-18) *)
            | "a" type "or nothing"                 (* optional — word spelling; the same type *)
            | "a list of" type | "a map from" type "to" type | "a pair of" type "and" type
            | "a box of" type                       (* heap cell — the indirection primitive that makes
                                                          recursive types finitely representable (R-13) *)
            | usertype | fntype | gentype           (* fntype/gentype: doc 04 *)
            ;   (* the EXPERT layer extends this type grammar with the fixed-size integers,
                   fixed-size decimals, and C/pointer types of 17–18 — layer-gated, never legal in
                   student/intermediate code; doc 04 owns the extended production *)
```

Notes: the implicit combinator parameter `it` is a **reserved word** (11.1) — a user name can never collide with it, which is what makes the `"it" comparison` lambda form deterministic. The call grammar is **unified** (R-1 of doc 11) — one production covers every call form used in this document: `ask "What is your name?"` (bare positional), `greet "bo"`, `divide 10 and 0` (positional + `and`-separated), `download "a" to "a.file"` and `send "hello" to messages` (positional + prepositional), `bigger of a and b`, `square root of 16`, `things at 0`. Arguments bind at the **additive level** (R-4), so `and` inside a call is *always* an argument separator — the sentence a human reads and the sentence the compiler parses are the same; a boolean argument needs parentheses or the `where` suffix, and the ambiguous spelling is a compile error with a teaching diagnostic, never a silent misparse. Multi-word phrase-statements that could collide with the greedy `and` — `and pass the problem on`, `wait for all tasks` — are **single reserved phrase-tokens**, so no name can ever contain them (7.0.3) and no call can ever consume into them. `flowcall` prepositions are a **closed set** (`of`, `at`, `from`, `to`) — extending it requires a grammar change, not prose. `either` was deleted at v0.1 (it had no job; reserving unused words is how ambiguity creep starts — REJECT list, doc 11). The reserved-word list (~120) is frozen per release cycle and printed by `lagom words` (tooling, doc 09).

---

## 8. Type system

Full spec: [docs/04-types.md](04-types.md). This section states the architecture.

### 8.1 Inference-first, annotation-optional — `student` gets inference

Types are checked **statically**, but beginners rarely write them. `make age equal to 15` gives `age` type `number` by inference. Annotations (7.10) appear when teaching types explicitly or when inference is ambiguous. This mirrors Swift/Haskell's inference-first stance with far fewer inference features (no unification beyond local bindings at M0 — HM-style inference is an M2 upgrade path if needed; see Decision Log D-11).

### 8.2 Core types — `student`

| Type | Meaning | Backing |
|---|---|---|
| `number` | Default integer | **i64**, wrapping prohibited — overflow is a checked failure in debug, defined trap in release unless `unsafe` (18) |
| `decimal` | Floating point | f64, IEEE 754 |
| `text` | Unicode string | UTF-8, immutable, copy-on-write not needed (ARC'd, 9) |
| `boolean` | Logic | `true` / `false` |
| `a list of T` | Growable sequence | Vec-like |
| `a map from K to V` | Key→value | Hash map |
| `a pair of A and B` | 2-tuple | Inline |
| `nothing?` | Optional (8.5) | Tagged |

**The `number` decision (D-10).** Beginners get *one* number type. `number` is a 64-bit integer; `/` (`divided by`) on two `number`s promotes to `decimal` (5/2 = 2.5) — matching Scratch and school intuition where division makes fractions. Experts who need exact integer division write `divided evenly by`; experts who need floats annotate `decimal`; experts who need f32/i32/i8/packed go to the expert layer (18.3). This "one visible type, precise escape hatches" pattern repeats throughout the design. Overflow: a beginner's `9,000,000,000,000,000,000 + 1` must not silently wrap — checked failure teaches the boundary honestly; `unsafe`/`expert` layer can choose raw wrapping ops.

**Literal typing and mixed arithmetic (R-15 of doc 11).** Numeric literals are *polymorphic at inference*: a literal unifies with its context and defaults to `number` when unconstrained. This is what makes the canonical accumulator work:

```lagom
make total equal to 0                        # 0 is number or decimal — whatever the arithmetic needs
repeat for each score in scores              # scores: a list of decimal
    increase total by score                  # works: literal 0 unified with decimal
```

Mixed `number`/`decimal` arithmetic promotes to `decimal` — the *only* implicit numeric conversion (doc 04 owns the operator table). An *annotated* `number` accumulator receiving decimals is a compile error whose diagnostic teaches the annotation — the teachable case is an error, the common case just works.

### 8.3 Static checking with dynamic-feeling errors

Statically typed but the *errors* are runtime-shaped: a failed type match is not a "TypeError" at line 1 of a 500-line program — it is a precise diagnostic at the exact expression, with the fix (see the example in doc 09-tooling). No truthiness: `if name` (name is `text`) is a compile error, "a text is not a yes-or-no value; did you mean `size of name is greater than 0`?" — one of many diagnostics that teach the type system *through* the error.

### 8.4 User types, aliases, function types — `intermediate`

Structures (7.11), kinds (7.12), classes (10), interfaces (10.6) are user-defined types. Type aliases: `a type called score is a number`. Function types: `a function from number and number to number`; capabilities extend them (`a function from text to text that can fail`). Function types are ordinary types — functions are first-class values (11).

### 8.5 Option and result types — `intermediate`

```lagom
make maybe_name equal to first of things        # type: text?  ("maybe a text")
if maybe_name is nothing
    say "The list is empty."
otherwise
    say "First is {maybe_name}."               # narrowed inside otherwise
```

- **One concept, two canonical spellings** (R-18 of doc 11): the compact form `T?` (`text?`) and the word form `a T or nothing` (`a text or nothing`) are *the same type* — one desugaring rule. `nothing?` as a standalone annotation is deleted (it hides the inner type).
- Values: `nothing` and `something with value v`. Flow-sensitive narrowing: after the `is nothing` test, the `otherwise` branch sees a definite value — no `!`, no unwrap ceremony at student level.
- **Printing an option:** `say` and interpolation print the contained value, or the word `nothing` when absent — honest, Scratch-friendly, and it makes the empty-list `first of` case a visible lesson (7.6).
- The same mechanism is **the error story**: `can fail` functions return a result; `attempt … as problem` binds the error (13). Option and result are two spellings of one concept (tagged unions, 7.12) — students learn it once.
- Equivalents: Rust `Option<T>`/`Result<T, E>`; Swift `Optional<T>` / `throws`; Kotlin nullable types. **No null literal exists in the language** — the billion-dollar mistake is simply absent (Tony Hoare's own diagnosis; Lagom takes the same exit Kotlin, Rust, and Swift took).
- **Equality semantics (R-6 of doc 11): one operator, two behaviors.** `is equal to` is **structural** for value types (numbers, decimals, text, booleans, structs, lists, maps, pairs, kinds, options) and **identity** for classes — two distinct counter objects with equal fields are never `equal to`; the teaching diagnostic points at field comparison (`count of a is equal to count of b`) and at `does comparable` (10.8) for opt-in structural comparison of classes. `same as`/`different from` are deleted (decision D-30). The grammar's `cmpword` list reflects this.

### 8.6 What the type system deliberately does *not* have (yet)

- **No inheritance-based subtyping** as a type-system feature (10.5 explains; interfaces are the substitution mechanism).
- **No dependent types, no refinement types** in v1. Compile-time evaluation (16) covers most of the use cases at far lower complexity. Revisit per roadmap M6+; Decision Log D-19.
- **No union-of-arbitrary-types** (`string | int`) — `kind` types cover the need explicitly.
- **No typestate/effect polymorphism** — `can fail`/`can wait` are fixed capabilities, not an open effect system (D-20).

---

## 9. Memory management

Full spec: [docs/05-memory.md](05-memory.md). **Chosen model: hybrid — ownership semantics at the type level for expert zero-cost code + automatic reference counting (ARC) for shared, beginner-facing data.** The philosophy: *safe and simple by default; low-level control when explicitly requested.*

### 9.1 The two-tier answer

- **Tier S (student/intermediate — "make it and forget it"):** value semantics for structs/lists/maps at their allocation site; classes (10) are reference types managed by ARC. No student ever writes a free, a destructor call, or a lifetime. There is no manual `delete`. There are no crashes from memory bugs — not because a GC hides them, but because the compiler's rules make them impossible in safe code.
- **Tier E (expert — "own it explicitly"):** the same compiler enforces ownership/borrowing when the expert opts in per-region (`within owning` regions, 9.6), turning refcount traffic into zero. Below that, `unsafe` (18) exposes raw pointers and custom allocators. The tiers differ in *annotations and runtime cost*, never in *semantics*: code that compiles in Tier E behaves identically to Tier S.

### 9.2 Value semantics by default — `student`

Structs, lists, maps, and pairs are **values**: assignment copies (shallow for structs with value fields; lists/maps copy-on-write internally at M0–M1, real deep-copy tuning at M2 — tracked in doc 05). A beginner's mental model — "this variable *is* that list" — matches Scratch's value-block intuition and avoids the classic aliasing lesson before week three.

```lagom
make a equal to a list of 1, 2, 3
make b equal to a
set b at 1 to 99
say a at 1          # 2 — b was a copy; this is *taught*, with the aliasing lesson at intermediate
```

**The one honest exception (R-12 of doc 11): a struct with a class-typed field copies shallowly.** The struct itself is copied; the *reference* to the class object is copied with it — both structs then name the same object. This is Swift's real semantics, and Lagom specs it, teaches it, and lints it rather than hiding it: the compiler emits a one-time student-layer note the first time such a struct is copied ("both players now name the same address object — make address a structure if it should copy"), and the lint recommends struct-over-class for pure data (10.1 already steers there). Doc 05 owns the full copy/elide/refcount matrix, and the value/reference lesson (9.7, 11 of doc 11) is taught *at* the struct/class boundary.

### 9.3 Classes are ARC'd references — `student` → `advanced`

Class instances (10) are reference-counted (Swift-style ARC): construction increments, scope exit decrements, `before last reference disappears` defines deterministic cleanup (10.4). ARC gives students deterministic destruction (files close when the function exits — a *teachable moment*, unlike tracing GC) and gives experts predictable latency (no GC pauses — the reason Nim moved to ARC/ORC and the reason game/server programmers choose Swift-over-Java shapes). Overhead: refcount traffic on shared objects; the ownership tier removes it where experts need zero cost.

### 9.4 Cycle collector — `student` layer only

Reference cycles leak under pure ARC. Lagom's student layer includes a **cycle collector** (trial-deletion, as in Nim ORC) that reclaims unreachable cycles, running opportunistically (at allocation failure or explicit `collect cycles` for lessons). The expert can disable it per-region (`within owning` implies no collector) — the expert who manages lifetimes honestly never needs it. Tradeoff: a small collector runtime in every student binary (accepted; same tradeoff Nim ORC ships).

### 9.5 Escape analysis and stack allocation — `advanced`

The compiler performs escape analysis: class instances that provably stay within a function are stack-allocated with refcounting elided; ARC traffic on non-escaping shared values is removed (Swift implements exactly this). Lists/maps/structs already live inline. Result: most student programs allocate near-zero heap blocks despite the ARC runtime.

### 9.6 Ownership and borrowing — `expert`

Opt-in regions for zero-cost systems code:

```lagom
function process
    takes an owned buffer called input
    returns an owned buffer
    within owning
        …body: every value here has exactly one owner; refcount traffic is erased…
```

Ownership mode is declared **per region**: a function may declare `within owning` for its whole body (or any nested block). Parameter modes mark how arguments transfer: `an owned …` (the caller gives up the value — a move) and `a borrowed …` (the caller keeps it; the callee may use but not store it beyond the call).

Within `within owning` regions: every value has one owner; `takes owned`/`takes borrowed` parameter modes; moves are destructive; the borrow checker (the same rules as Rust's, minus lifetimes-as-syntax — borrows are lexically scoped) enforces no-aliasing for `changing` borrows. This is Rust's discipline **without Rust's syntax tax**: a beginner never sees it; an expert who wants C-speed sharing and mutation without refcounts writes it. Errors in these regions are the full what/where/why/fix diagnostics, teaching ownership *when the student arrives at it*.

**The borrow contract (R-7 of doc 11): borrows cannot escape.** A borrow never outlives the region or scope in which it is created — so a function cannot return a borrow of its parameters (`longest of a and b` returning `a borrowed text` is inexpressible by design, not by accident). This is the honest price of "no lifetime syntax," and the workaround ladder is documented, not hidden: **(1)** return an owned copy; **(2)** return an ARC'd class; **(3)** take a continuation — the callback style, which needs no lifetime at all (`longest of a and b using say it`, or with a block lambda). An escape-analysis-based checker that permits borrow-returning APIs is an INVESTIGATE item (M4, benchmarked against the continuation ladder before adoption). Interface *objects* (existentials) built from borrows are rejected — they escape by construction; a `borrowed` value satisfies an interface constraint for the call's duration only. The tiers-differ-in-annotations-never-in-semantics invariant (9.1) is preserved: these are capability restrictions, not semantic differences.

### 9.7 What beginners *learn* about memory anyway

Hiding machinery ≠ hiding concepts. The student curriculum teaches: values vs references (with the class/struct distinction), what "the program's memory" is, why a closed file must be closed, and at intermediate level, the difference between copying and sharing. The *mechanism* (refcounts, allocators) stays invisible; the *concept* (lifetime) is on the syllabus. That is the difference between "hiding complexity" and "hiding the subject".

### 9.8 Allocators and arenas — `expert`

```lagom
within arena my_allocator
    …allocations inside go to the arena; freed all at once at region exit…
```

Custom allocators are a library-level facility (`allocate`, `deallocate`, `allocator` types) usable only inside `unsafe` or `within owning` regions. Standard library containers take an optional allocator parameter (`advanced`). This is the Rust/Zig allocator model, chosen because it keeps the *beginner* API allocator-free while making the *expert* API total (every allocation goes through an allocator — there is no hidden global malloc).

---

## 10. OOP model

### 10.1 Composition first — the honest hierarchy

Lagom's OOP story is deliberately **composition-first**: structs (data), kinds (variants), functions (behavior), interfaces (substitution). Classes exist for stateful, identity-bearing objects and are taught *after* structs and interfaces. This ordering is a pedagogical claim (data-first beats class-first — the "objects early" trap of Java-era curricula) and an engineering claim (composition ages better than inheritance hierarchies — the industry-wide retreat from deep inheritance documented across Go, Rust, and modern Swift style).

### 10.2 Classes — `intermediate`

```lagom
class counter
    has count of type number
    can reset
        set count of myself to 0
    can bump
        increase count of myself by 1

make c equal to a new counter
bump c                       # method call: the receiver is the first argument
add 5 to c                   # a method with arguments — same grammar
say count of c            # 2
```

- **Methods are ordinary calls whose first argument is the receiver** (R-2 of doc 11): `bump c`, `add 5 to c` — no dot syntax anywhere (the grammar has no `.` production; see 7.13). Declared methods lower to functions with a leading receiver parameter; `can` is reserved for declarations (methods, interface requirements). Dynamic dispatch is unaffected — the receiver is simply the vtable-carrying argument. This is the Go/Lua shape, and the transfer sentence is honest: *methods are functions that take the object first* (true of Python's `self` too).
- `myself` is the receiver name *inside the method body* (Python's `self`, Rust's `self`, Swift's `self` — same concept, reserved word). Methods use `can` to declare (they are *capabilities of the object* — the same clause system as functions; `can` methods may also declare `can fail`/`can wait`).
- Internally: class instance = ARC'd heap object with a vtable pointer when dynamic; methods on a non-polymorphic class are statically dispatched until a subclass/interface forces a vtable (devirtualization by default, 25).
- Equivalents: Swift class + methods; Python class; C++ class (with ARC instead of manual new/delete).

### 10.3 Constructors and initialization — `intermediate`

```lagom
class counter
    has count of type number
    has step of type number
    construction
        takes number called starting step
        set count of myself to 0
        set step of myself to starting step
```

`construction` clauses are named constructors with `takes`-clause parameters (`a new counter with starting step 2` — construction arguments use the ordinary call-suffix shapes); the compiler enforces full field initialization before any method runs (the Swift rule, which kills the un-initialized-field bug class *at compile time*). Multiple constructors are multiple named clauses — no overloading-by-arity ambiguity (D-14).

### 10.4 Destructors / finalization — `intermediate` (concept) → `advanced` (mechanism)

```lagom
class document
    before last reference disappears
        say "document is being released"
```

Runs deterministically when ARC drops the last reference (same guarantee as Swift `deinit`, C++ RAII). For resources needing prompt release, the standard idiom is the **use-block** (RAII spelled readably):

```lagom
using open file at "data.txt"
    …file is open here; closed at region exit even on failure…
```

Two region words, two jobs: **`using`** binds a *disposable resource* for a scope (RAII — closing files, releasing locks); **`within`** sets a *memory-discipline region* (`owning`, `arena`). They compose but never substitute for each other.

No finalizers-as-safety-net (the Java lesson): cleanup is deterministic or explicit; the GC-style "eventually" path does not exist. **Finalizers cannot fail** (R-20.3 of doc 11): `before last reference disappears` may not call `can fail` functions un-attempted — a failing cleanup is a bug class, not a feature, and the checker enforces it; this is also the rule that keeps the cycle collector's re-entrancy story sound (doc 05 owns it).

Backend timing note (documented resolution): the interpreter implements exact last-drop timing (the finalizer fires the moment the last reference drops, even mid-program). Native compiled values are not ref-counted (the M0 value model), so the native backend runs each class's finalizer once, at a deterministic program-end sweep in creation order. The two agree byte-for-byte for every scope-held-object program — the idiom all examples use — and tests pin that parity.

### 10.5 Inheritance: allowed, single, discouraged — `intermediate` → `advanced`

```lagom
class animal
    can speak
        say "…"

class dog extends animal
    can speak          # overrides
        say "Woof"
```

(`animal.speak` remains callable as `speak of myself` inside the override — calling up is a plain call on the receiver, not new syntax.)

Single inheritance only; fields private-by-default to subclasses too (no fragile-base-field exposure); `extends` is the only inheritance word. The docs and linter actively recommend interfaces + composition for anything but genuine is-a-with-shared-state. **Why allow it at all?** Because students meet inheritance in every mainstream OO curriculum and should learn its *real* shape (including its costs) in Lagom — and because GUI/widget frameworks (a legitimate intermediate use) are naturally hierarchical. **Why not make it the default story?** Composition-first age better; see 10.1.

Implementation resolutions (documented, tested in the M2 suite):

- **Bare field reads inside method bodies.** §10.5's own examples read fields bare (`side times side` in `can area`), so a single-word name that is otherwise unbound inside a method body (and a plain write target like `increase side by 1`) reads/writes the receiver's field of that name — own or inherited. The explicit `side of myself` spelling stays and means the same thing; `myself` itself also reads. Outside a method body nothing changes: fields are reachable only through a receiver, keeping privacy honest for all non-method code.
- **Inherited fields are readable by the subclass's own methods, not by outside code.** The field table of a subclass is the base's fields prefixed to its own (so positional field access stays sound); privacy ("private-by-default to subclasses too") is enforced against callers, not against the subclass implementing the is-a relationship — the same access Swift grants inside a class's own methods.
- **Construction clauses inherit.** A construction clause declared on a base class serves subclass sites: `a new dog with starting count 5` dispatches to the nearest ancestor clause, whose receiver argument is the `dog` instance being built (the clause body's `set … of myself` writes the subclass's inherited field). The constructed value's static type is the *named* class at the site, never the declaring class. A subclass may declare its own additional clauses; site matching stays by argument names (D-14) across the whole chain.
- **Calling up.** `speak of myself` inside an override resolves to the nearest ancestor implementation of that method — never the override itself (no accidental infinite recursion). Calling up to an inherited (non-overridden) method is an ordinary static-dispatch call; a subclass receiver is accepted wherever an ancestor receiver is the parameter type.

### 10.6 Interfaces — `intermediate`

```lagom
interface drawable
    can draw

class circle does drawable
    can draw
        …
```

Interfaces are the substitution mechanism (no field inheritance, no method conflicts): any type `does` any number of interfaces; dispatch is static when the concrete type is known, vtable when not (25). Interfaces + generics (12) interact through constraints (`a function that takes anything that does drawable`), which is also the bridge to expert traits (12.4). Equivalents: Java interfaces (default methods included via `can` bodies), Rust traits (object-safe subset), Swift protocols.

Implementation resolutions (documented, tested in the M2 suite):

- **Defaults count toward conformance.** A requirement with a body checks in-waiting under the interface's name; every conforming class that lacks its own method of that name gets the default copied in under the *class's* method name with the receiver re-typed to the class — the default body is checked once, against the interface, and may read the class's fields (own or inherited) through the usual 10.5 rules. A class's own method always wins over a default.
- **Colliding defaults are a compile error, not a pick.** If two conformed interfaces (or one, twice) both provide a default for the same requirement, neither is more specific: the checker rejects the class (E0382) and asks for the class's own method, which replaces every default. This is what "no method conflicts" means operationally — conflicts are either resolved by the class or rejected, never silently decided.

### 10.7 Static members, class-level behavior — `intermediate`

No `static` keyword: class-level behavior lives in module-level functions in the same file as the class (construction is spelled `a new counter with step 2`, 10.3; "static constants" are module constants). Rationale: `static` mutable state is shared mutable state — the exact thing the concurrency model (14) makes painful, so the language does not hand beginners a tool that the rest of the language discourages. Rejected alternative: `shared` members — deferred to M6 if a real need emerges (tracked in doc 10-roadmap open questions).

### 10.8 Operator overloading — `intermediate`

Allowed for arithmetic/comparison on user types via interface implementations (`does comparable`, `does addable`), never for arbitrary symbols, never for `=`-shaped semantics (no overloading `set … to`). Rationale: keeps the four-symbol operator table honest while allowing `vector + vector` (the pedagogically valuable case).

---

## 11. Functional programming

### 11.1 Closures and higher-order functions — `intermediate`

```lagom
make twice equal to a function
    takes a function from number to number called f
    returns a function from number to number
    gives back f of f of 3        # f(f(3)) — nested flowing calls are legal when unambiguous
```

Lambdas:

```lagom
make double equal to a function taking n
    gives back n times 2

make doubled equal to map things using double
```

(The inline form is `map things using taking n giving back n times 2`; for a single parameter, `map things using it times 2`.)

`a function taking n` is the one-line lambda (params without types → inferred); `using f` is the readable way to pass a function argument to a flowing call. Closures capture by reference with ownership rules per §9 (capturing in a `can wait` closure follows the send rules, 14.4). Equivalents: Rust closures (`|n| n * 2`), Swift closures, Python lambdas (which are weaker — single expression — Lagom's lambda is a full function).

### 11.2 Map/filter/reduce and friends — `intermediate`

```lagom
make scores equal to a list of 85, 92, 78
make raised equal to map scores using it plus 5
make passing equal to keep scores where it is at least 80
make total equal to combine scores with start 0 using start plus it
```

`map`/`keep` (filter)/`combine` (reduce) are standard-library functions over the iterator protocol — not syntax. `map scores using it plus 5` reads: map over `scores`, using each element (named `it`), producing `it plus 5`. `with start 0` is a labeled argument (7.9). These compose lazily (11.3) so `map … then keep …` costs one pass — and multi-stage pipelines read as plain nested calls (the composition depth is a teaching topic, and the builder style below is the idiomatic alternative):

```lagom
make passing equal to keep scores where it is at least 80
make raised equal to map passing using it plus 5
```

(An `applied to` chaining word was considered and rejected at v0.1 — Decision Log D-29: it adds a third call syntax before the need is proven; open question 00 §31.13.)

### 11.3 Iterators — `intermediate`

Any type `does iterable` can be the source of `repeat for each` and the combinators. Iterator = a type with `can give next returning a text?`-style optional protocol (actually `a value or done`); combinators are lazy wrappers; loops lower to the protocol (§7.5 MIR note). This is Rust's `Iterator` design (the best-in-class version) with student words.

### 11.4 What Lagom adopts from FP — and what it leaves out

Adopts: first-class functions, closures, immutability-by-default, ADTs + pattern matching, expression orientation, lazy iterators. Leaves out (v1): full HM inference (D-11), currying/partial application as syntax, monadic do-notation, tail-call *guarantees* as a language contract (MIR does TCO best-effort; a `recursive` contract keyword is an unresolved question), lens/arrow abstractions. Rationale: each excluded feature has a poor transfer story to mainstream languages or a large implementation cost with a small student payoff — all revisit points are logged in doc 10-roadmap.

---

## 12. Generics

### 12.1 Implicit generics — `intermediate`

```lagom
function first item
    takes a list of anything called items
    returns anything         # same type as the list's item type
    gives back items at 0
```

`anything` (called `T` elsewhere) is inferred from use — the beginner writes a generic function *without knowing generics exist*. This is the Swift-style opaque inference form; the checker unifies `anything` per call site (and per-body occurrence, co-consistently), and monomorphization happens in MIR (22).

### 12.2 Explicit generics — `intermediate`

```lagom
function first item
    takes a list of some type called items
    returns some type
    gives back items at 0
```

`some type` names the type parameter explicitly. Containers: `a list of some type`, `a box of some type` (user generic classes use the same word). In v0.1, **type arguments are always inferred at call sites**; when inference cannot decide, the programmer annotates a binding (`of type`) or an intermediate variable instead — an explicit type-argument syntax is deferred until the M2 generics work proves it necessary (tracked with doc 04's open questions).

### 12.3 Constraints — `intermediate` → `advanced`

```lagom
function biggest item
    takes a list of some type that does comparable called items
    returns some type
    …
```

Constraints name interfaces (10.6): `that does comparable`, `that does addable`, `that does printable`. Static dispatch per monomorphization (zero-cost by construction); dynamic dispatch via interface objects (`an anything that does drawable`, the existential form) when the expert wants erasure.

Implementation resolutions (documented, tested in the M2 suite):

- **The constraint suffix applies to type parameters, and the checker checks it per call site.** `takes anything that does comparable called x` and `takes some type that does comparable called x` are the same parameter — one constraint, both spellings (12.1's two forms of one concept). The parameter's static type inside the body is still the type parameter (12.1); the constraint is a promise about the argument, checked when the function is called: every argument at every call site must itself satisfy the interface (its concrete class declares `does <interface>`, it is a constrained value of the same or wider constraint, or it is a built-in type whose operator implementation satisfies the interface — see the resolution below). A list element constraint (`a list of some type that does comparable called items`) constrains the element type, not the list. The constraint also flows through an unconstrained `anything`/`some type` *parameter* that receives it as an argument (the generic forwarder case): passing a constrained value onward keeps checking at the next call site. There is no user-written satisfaction witness — a class satisfies an interface exactly when it claims `does` (10.6), so the checker has one source of truth.
- **Built-in types satisfy comparison/combination interfaces by their frozen operator behavior.** §12.3's own example (`biggest item` over a list of numbers) is only type-correct if built-ins can satisfy `comparable`, and 10.8 overloads the arithmetic/comparison operators for user types *via these same interfaces* — so the correspondence is two-directional by construction. Documented table: `comparable` is satisfied by number and decimal (plus any class `does comparable`) — text is deliberately excluded, because the ordering operators are number/decimal-only by frozen rule and text's order was explicitly rejected (S-61); `addable` is satisfied by number, decimal, and text (`plus` is defined on all three); a class satisfies any interface it `does`. Any other built-in type fails any constraint. This keeps `a list of some type that does comparable` usable on the stdlib-shaped examples the docs pin, without inventing a witness protocol.
- **Method calls on a constrained parameter dispatch through the interface (the 10.6 vtable), both backends identically.** Inside a constrained parameter's body, `<method> of <param>` (and the labeled-suffix forms) resolve to the interface's requirement method with the receiver kept as the parameter — the one method the constraint guarantees. At runtime both backends pick the receiver's dynamic class's implementation (a class's own method, an inherited one, or a copied default — 10.6's resolution order), which is §10.6's "vtable when the concrete type is not known"; backends share one class→method table built by the checker, so interpreter and native agree by construction. A method call on an *un*constrained `anything` parameter stays a compile-time error (nothing is known about the value); the constraint is exactly what makes the call legal.

### 12.4 Expert traits = interfaces + this machinery

The expert layer does not add a second mechanism ("traits") — interfaces + constraints + existentials + operator-does (10.8) *are* the trait system, Rust-shaped at the limit. One mechanism, four levels of exposure.

---

## 13. Error handling

### 13.1 Errors are values, failures are capabilities — all layers

```lagom
function divide
    takes number called top
    takes number called bottom
    returns a number
    can fail
    if bottom is equal to 0
        fail with "cannot divide by zero"
    gives back top divided by bottom

attempt divide 10 and 0 if it fails then
    say problem
otherwise
    say result          # the successful value is bound to `result` here

function read config
    …
    attempt open file at path and pass the problem on        # the readable `?` — `intermediate`
```

- A function that can fail declares `can fail` (its type includes it, 7.8). `fail with` produces an error value (a `text` at student level; a rich error type at intermediate+, 13.4). The caller *must* handle it: `attempt … if it fails …` (student), `attempt … as problem …` (bind the error under a chosen name), or `attempt … and pass the problem on` (propagate — the `?`-operator's readable form, `intermediate`). Forgetting to handle a `can fail` call is a **compile error** (the Rust/Swift rule, which is the whole point).
- **`attempt` is an expression** (R-20.1 of doc 11): the bare form evaluates to the success value and makes the function `can fail`; in the `if it fails/otherwise` form it evaluates to the branch's value, so nesting is legal — `(attempt divide 10 and 0) plus 1` — with a lint steering deeply nested attempts toward named intermediate bindings (the same house style as nested combinators). `and pass the problem on` is the statement-shaped propagation shortcut.
- **The two fixed binding names:** in the `if it fails … otherwise` form, the failure branch binds the error to `problem` and the success branch binds the value to `result` — two fixed names, the Scratch `answer`-block precedent, chosen so the beginner's first error handling needs zero extra syntax. The `as` form binds a custom name when clarity demands it. Shadowing a user's own `result`/`problem` inside these branches is allowed with a student-layer lint (doc 03 owns the exact rule).
- **Why not exceptions?** (a) Hidden control flow: any line can jump — the hardest thing for beginners to trace and for experts to audit. (b) Transfer: Rust, Go, Swift, Zig, C all use value/explicit error flow; Lagom's mapping to all five is 1:1. (c) Determinism: error propagation is visible in the signature, so "what can this line do?" is answered by reading one line. Exceptions' single advantage (unobtrusive happy path) Lagom buys with the `attempt`-flowing syntax instead.
- Panics (unrecoverable bugs: assertion failures, overflow traps, OOM) exist separately — they are crashes, not errors, and are not catchable (Go's distinction). A beginner's unhandled failure stops the program with the teaching diagnostic (see doc 09-tooling for the exact format).

### 13.2 The student progression

1. **Student:** `attempt X if it fails then …` — one shape, always works, never silently swallowed.
2. **Intermediate:** `as problem` binding, custom error kinds (a `kind` for errors), `and pass the problem on`.
3. **Advanced:** error unions across module boundaries, error hierarchies via kinds, `when`-matching on error values.
4. **Expert:** no new mechanism — errors are already values; allocation-free error construction in owning regions is the only expert topic (doc 05).

### 13.3 Guarantees

- No silent failures: every `can fail` call site is visibly `attempt`-wrapped (compiler-enforced).
- No error-as-control-flow-for-loops; `stop`/`next` own that job.
- Errors carry: message, position, kind, and (advanced) a chain of causes.

### 13.4 Error values at scale — `advanced`

```lagom
kind file problem
    is missing with path of type text
    is denied with path of type text
    is too large with path of type text and limit of type number
```

Error kinds are ordinary kinds (7.12) — one concept again: students learn tagged unions for data, options for absence, results for failure, and discover at the advanced layer that all three were the same machinery.

---

## 14. Concurrency model

### 14.1 The layered answer

| Layer | What they see |
|---|---|
| `student` | Nothing. Programs are sequential; no locks, no threads, no vocabulary. |
| `intermediate` | Timers in game/UI frameworks (hidden task spawn). |
| `advanced` | Tasks, channels, async (`can wait`), structured cancellation, `shared` types + mutexes. |
| `expert` | Atomics, memory orderings, lock-free structures, `unsafe` threading. |

### 14.2 Structured tasks — `advanced`

```lagom
start a task
    download "a" to "a.file"
start a task
    download "b" to "b.file"
wait for all tasks        # optional early join; the region also joins implicitly at scope exit
```

Each spawn is its own `start a task` statement — the form repeats (no `start another task` variant to hardcode the second one); `wait for all tasks` is a single reserved phrase-token (7.15). `start a task` spawns on a work-stealing thread pool; the *region* waits for all spawned tasks at its exit (structured concurrency — the Python trio / Kotlin coroutine-shape guarantee): no orphaned tasks, no detached-thread leaks, cancellation propagates inward. A single `start a task` in a function is implicitly awaited before return unless `in the background` is written (which requires a scope that outlives it — enforced). **Supervision (doc 06 decision):** a failed child fails the region unless the task declares `keep going` — fail-fast by default, matching structured-concurrency precedent.

### 14.3 Channels — `advanced`

```lagom
make messages equal to a channel of text
start a task
    send "hello" to messages
say receive from messages
```

Typed channels (`a channel of text`), blocking and `can wait` variants, closing semantics, `repeat for each message in messages` integration (channels are iterable; explicit close — the loop does not close the channel, doc 06). **Send moves (R-8 of doc 11):** a send transfers ownership — the sender loses access, checker-enforced, and the receiver owns the value. This makes the race-freedom story structural rather than disciplinary: shared mutable state through a channel must be a `shareable` class with `shared`/`guard`ed fields (14.4), never an accident of aliasing. `shareable` classes may be sent as shared references — the Rust Send/Sync split, spelled in student words ("sending gives it away"). Channels + tasks = the Go/Clojure composition, chosen over actor systems as the *default* story because channels compose with structured tasks; actors remain a library pattern (an actor is a task owning state with a channel inbox — stdlib provides it at M3).

**v0.1 resolutions (M2, documented per the underspecification rule):** the spec above fixes the *shape* (clause-free statements, region join, fail-fast supervision) but leaves four points open, resolved here before implementation:

- **Region = function body, drain points are explicit in MIR.** The region is the enclosing function body (the script counts). "The region waits at its exit" is lowered as one `wait for all tasks` instruction injected at the function's end (after its own statements, at the same level as the spawns) — so nesting behaves lexically: an inner `wait for all tasks` joins only tasks spawned *before it in the same function*, and a task's own body is a function with its own (empty at spawn-time) region. There is no second implicit drain inside control-flow constructs.
- **Ordering.** Each spawned task begins running when the spawning function reaches its next join point (an explicit `wait for all tasks` or the region exit) — v0.1 tasks are *not* preemptive; there is no interleaving mid-statement and no observable race inside one backend run. At the join, started tasks run to completion in spawn order (deterministic — §4's replay contract covers concurrent programs too), then the join returns. The demo transcripts' "task output, then `done`" is exactly this order.
- **What a started task returns.** Nothing. `start a task` is a statement with no value (a task handle would be an M3+ surface; the architecture assigns only spawn/join/channels to M2). The body's own `gives back` value is discarded; its `fail with` is the supervision signal below.
- **Error propagation (supervision).** `fail with` (or a trap) in a task body is stored, not raised mid-join: the join runs *all* already-started tasks first (so their side effects complete — structured teardown, not abort), then re-raises the first failure (spawn order) at the join site as an ordinary failure — catchable by an `attempt` wrapping the spawn/join, otherwise unhandled at the caller exactly like a failing call. **`keep going`** on a spawn declares that its failure must not fail the region: the join consumes that task's failure silently (doc 06's supervision decision). A `fail with` inside a task body is legal without the body declaring a function-level `can fail` — the body IS the capability boundary; the supervision rule replaces propagation (a task has no caller to propagate to).
- **Channels, v0.1 shape.** `a channel of T` constructs a typed FIFO channel value (`Channel`); `send <v> to <ch>` is a statement-ish builtin (queues a deep-copied v — values, not aliases, cross the boundary per §9/R-8's teaching form) and `receive from <ch>` is a builtin expression that dequeues: on an empty channel it waits for the region's remaining tasks to finish and then fails (`receive from an empty channel`) unless a value arrives — v0.1's wait is the join barrier, not a general scheduler park; `repeat for each message in messages` drains a channel by iterating its queued values (channels are iterable, 14.3). Channels are values (copy shares the same queue, the ARC'd shape).

### 14.4 Shared memory and race freedom — `advanced`

Safe Lagom has **no data races** — by send/share semantics (the Rust "Send/Sync" rule, spelled `sendable`/`shareable`):

- Values crossing a task boundary via channel/spawn must be `sendable` (owning values, or ARC'd classes marked `shareable`).
- Class instances shared across tasks must be `shareable`, which requires their fields to be protected: `shared` fields accessed only through a built-in mutex (`guard`), enforced by the type system.

```lagom
class tally
    has shared count of type number guarded by a lock
    can add one
        increase count of myself by 1      # the guard is held for the whole method body
```

**Lock scope = the method body (R-9 of doc 11).** On any class with `shared` fields, entering a method acquires the guard(s) for the fields it touches, held for the *entire* method; direct field access from outside methods is a compile error. A per-statement lock would admit the classic lost-update race (`make current equal to count of myself` … `set count of myself to current plus 1` as two separate acquisitions) inside the very model that promises no races — so the *method* is the unit of atomicity. Coarse but sound, trivially explainable ("while a tally method runs, nobody else can touch its count"), and it is exactly Java's `synchronized`-method model, which students later meet verbatim. Experts who need finer granularity use explicit locks or atomics (14.5) inside `unsafe`.

The borrow-free beginner path: tasks + channels (message passing). The intermediate path: `shared`+`guard`. The expert path: 14.5. Race conditions are a compile-time category error, not a runtime mystery — the same decision Rust made, with a syntax that starts at message passing.

### 14.5 Atomics and lock-free programming — `expert`

`atomic number` types with explicit orderings (`load with relaxed ordering`, `compare and exchange with acquire release ordering`) inside `unsafe` (18). The expert gets C11/Rust-class primitives; the language gets to keep "safe code has no races" honest by fencing them behind `unsafe`.

---

## 15. Async model

### 15.1 Async is a capability, not a ceremony — `advanced`

```lagom
function download
    takes text called address
    returns text
    can fail
    can wait
    …
attempt download "example.com" if it fails then
    say problem
otherwise
    say result
```

`can wait` marks the function as asynchronous; calling it inside another `can wait` function composes directly; awaiting happens at capability boundaries and the `attempt …` form (13) makes the handling point visible in words. There is no `async`/`await` keyword pair because the capability clause (7.8) already carries the information — the same clause grammar from student functions.

### 15.2 Runtime design

- MIR lowers `can wait` functions to **state machines** (Rust/Swift model) over a work-stealing scheduler (same pool as tasks, 14.2) — no green-thread runtime in v1 (D-17). Colors are real but *contained*: `can wait` functions may only call `can wait`/sync functions; sync functions may not block on async results (deadlock-by-color is a compile error where provable, a diagnostic otherwise).
- Cancellation: structured (14.2); every await point is a cancellation point; `stop waiting` cancels a task region.
- IO: stdlib network/file APIs are `can wait` variants of sync APIs — one naming scheme (`fetch` / `fetch and wait`), not two parallel universes.

### 15.3 Why not implicit-everything async (Go-style)?

Go's model is superb and was considered seriously (D-17): goroutine-style green threads with blocking IO would delete the `can wait` capability entirely. Rejected for v1 because (a) the state-machine model keeps FFI and embedded targets honest (no runtime requirement), (b) Rust/JS/Swift/Python learners meet the colored model in every mainstream language, and (c) green threads remain an *implementation* possibility behind the same syntax — if a future stdlib/runtime adds them, `can wait` clauses become no-ops and code is unchanged. The design keeps the door open rather than betting the language on either runtime.

---

## 16. Metaprogramming

### 16.1 The ladder

| Layer | Mechanism |
|---|---|
| `student` | (none — and no vocabulary for it) |
| `intermediate` | Generics (12) — type-level abstraction without metaprogramming vocabulary |
| `advanced` | **Compile-time evaluation** (`at compile time`) |
| `expert` | Compile-time reflection, compile-time code generation via types-as-programs |
| `expert`/deferred | Compiler plugins (16.5) |

### 16.2 Compile-time evaluation — `advanced`

```lagom
at compile time
    make table equal to a list of …computed values…
    function lookup …
```

`at compile time` regions execute at compile time with the full safe language (the Zig comptime insight: **the metaprogramming language is the language** — no second macro language, no template syntax). Compile-time functions build lookup tables, unroll loops, and validate embedded data (parse a regex at compile time, fail the build if invalid). **Staged in two tiers** (R-19 of doc 11 — MIR is monomorphized, so it cannot host type-generating programs): **value-level comptime at M3** runs over monomorphized MIR exactly as described here (tables, validation, unrolling); **type-level comptime at M4+** — a function returning a type is a type-level program — runs over a pre-monomorphization IR (HIR) with its own evaluation budget and caching story, designed only after value-level comptime has real users. Cost model per tier: compile-time interpreter with budgets that prevent pathological build times.

### 16.3 Why not macros? — Decision Log D-18

Hygienic procedural macros (Rust-style) were the leading alternative and are **deferred, leaning rejected**: comptime covers code generation with one mechanism and no token-tree API to learn; macros that *rewrite syntax* are the single largest readability hazard in every macro-heavy language (the reader must run a mental compiler). If a real need emerges (derive-like boilerplate), the first response is compiler-provided *derive clauses* (`can be compared automatically`) as builtin capabilities — a fixed, readable, greppable mechanism — with procedural macros as the M6+ escape hatch only if derives prove insufficient.

### 16.4 Reflection — `advanced`

Compile-time reflection is free via comptime (iterate a type's fields at compile time — this powers serialization/ORM-style libraries without runtime metadata). **Runtime** reflection ships as opt-in metadata per type (`can be examined at runtime`) — never by default (binary size + the security surface, 28); JSON/serialization stdlib uses comptime by default and works without metadata.

### 16.5 Compiler plugins — `expert`/deferred

User-written compiler passes (custom IR transforms, new backends) are an M6+ investigation gated on IR stability (22). Explicitly *not* in v1: it couples the ecosystem to compiler internals that must first settle.

---

## 17. FFI design

### 17.1 Principle: the language is never the wall

The brief's escape-hatch ladder (stdlib → packages → FFI → OS APIs → unsafe → intrinsics → asm) starts working at FFI. Lagom's FFI targets the **C ABI** — the universal substrate (same decision as Rust, Go, Swift, Zig, Python's ctypes) — and treats *everything else* (C++ classes, COM, JNI-style VMs) as library-level bridging over it.

### 17.2 Calling C — `advanced`

```lagom
from the C library "libsqlite3"
    function sqlite3 libversion
        returns a C string

function library version
    returns text
    can fail
    unsafe because calling a C library that needs raw string handling
    make raw equal to sqlite3 libversion
    gives back text from C string raw        # the wrapper converts; C calls return values, not errors
```

**C calls return values, period — no magic capability inference** (R-20.4 of doc 11). A C function never implicitly becomes `can fail`; error conversion is *explicit in the wrapper*: the Lagom wrapper declares `can fail`, inspects the C return code, and constructs a Lagom error with `fail with` (or returns the converted value). The caller sees only the readable Lagom API; the unsafe C boundary and the error translation both live in one audited place.

- C declarations are written in Lagom syntax (no separate header language, no bindgen *required* — a `lagom bind` tool that reads C headers and emits Lagom declarations ships at M3 for ergonomics).
- Type bridge: C `int/long long/double/char*/void*/fn ptr` map to Lagom `C number`/`C decimal`/`C string`/`C pointer`/`C function` types — distinct from safe types, usable only in `unsafe`-adjacent contexts (18.2 defines the exact rules). Strings cross as UTF-8 with explicit ownership marks (`borrowed` vs `owned`).
- Structs: `C structure` declarations with explicit layout (18.3) match C layouts bit-for-bit.
- Linking: `Lagom.toml` declares `link with "sqlite3"`, static or dynamic; the build system invokes the platform linker. `lagom add` can fetch prebuilt bindings from the registry (20) — with the security caveats of 28.

### 17.3 Being called from C — `advanced`

`export` marks a Lagom function/structure to emit with C ABI:

```lagom
export function lagom add
    takes a C number called a
    takes a C number called b
    returns a C number
    gives back a plus b
```

This enables Lagom-built static/shared libraries callable from C, Python (ctypes), Swift, etc. This is also the embedding story (a Lagom runtime library with a C API, the Lua/SQLite model).

### 17.4 OS APIs — `advanced` → `expert`

POSIX/Win32 headers are just C libraries (17.2); the stdlib wraps the common ones readably (files, processes, sockets). Graphics/audio: stdlib provides window/input/audio *hooks* (M4) via SDL-shaped interfaces; the full GPU story is package-level (Vulkan/Metal bindings over FFI).

---

## 18. Unsafe / low-level design

### 18.1 `unsafe` is a region, not a mode

```lagom
unsafe
    make p equal to a raw pointer to byte with size 64
    store 65 at p
    say load byte from p
```

Inside `unsafe`: raw pointers (`a raw pointer to T`), manual allocation (`allocate`, `deallocate`, `free all at`), pointer arithmetic, casts between pointer types, `reinterpret`, atomic primitives (14.5), intrinsics (18.5), inline asm (18.6). `unsafe` does not disable type checking of safe code — it *admits* unsafe values and operations; safe code outside the region may hold the results but the unsafety is contained at the boundary (the Rust rule).

**Every `unsafe` region must carry a `because` comment** — a one-line justification (`unsafe because calling C function that needs a raw buffer`) — enforced by the compiler as a lint-error at M3+. This is the teaching hook: unsafe is not forbidden, it is *accountable*.

### 18.2 The safety contract

Unsafe code may break the memory-safety guarantees *inside* the region, but the **language-level invariants remain the expert's contract to the rest of the program**: an `unsafe` block that hands a dangling pointer to safe code is a bug in the unsafe block, not in the safe caller. The standard library's unsafe parts are audited to uphold total safety; user unsafe code is the expert's explicit responsibility. Debug builds insert poison values, guard pages where available, and pointer-use checks after every unsafe region (the "unsafe sanitizer") to convert latent unsafe bugs into loud crashes during development.

### 18.3 Layout control, packed structs, alignment, fixed-size integers — `expert`

```lagom
C structure packet header
    has magic of type unsigned 16 bit number
    has length of type unsigned 16 bit number
    is packed to 1 byte
```

Fixed-size integer types (`unsigned 8 bit number` … `signed 64 bit number`) and **fixed-size decimals** (`a 32 bit decimal`, `a 64 bit decimal` — the latter an alias of `decimal`; R-21 of doc 11), explicit endianness for IO types, `is packed to N bytes`, `is aligned to N bytes` on C structures — the complete hardware-layout kit (needed for FFI struct matching — C `float`/`double` distinctions — network protocols, and memory-mapped IO). These types are *invalid* in safe student code (the checker rejects them outside `unsafe`/owning regions with a diagnostic explaining why) — the layer system enforced by the compiler, not the docs.

### 18.4 ABI and calling conventions — `expert`

`export`/`from the C library` blocks accept explicit calling convention (`with C calling convention`, `with system calling convention`), and expert declarations may pin struct return strategies. RISC-V/x86-64/ARM64 differences are the compiler's job; the user-level surface is two words. Bare function pointers with convention annotations close the C-callback loop (17.3).

### 18.5 SIMD and intrinsics — `expert` → `elite`

Two mechanisms, mirroring the field's best practice:

1. **Portable vectors:** fixed-size vector types (`4 of a 32 bit decimal packed` — a 128-bit lane type; the R-21 element type) with element-wise ops; the compiler lowers to NEON/SSE/AVX/RVV as the target allows, and *auto-vectorizes* plain loops regardless (25).
2. **Intrinsics:** `cpu intrinsic "pause"`-style named intrinsics per target ISA, valid only in `unsafe` (a registry of intrinsics ships with the compiler, generated from target ISA docs).

No embedded-DSL matrix library in the core language; that is stdlib/package territory (Mojo's lesson: DSLs belong above a clean vector base, not in the grammar).

### 18.6 Inline assembly and linker control — `expert`

Inline asm: `run assembly on x86-64 …` blocks with register/operand templates (GCC-shaped, the most-transferrable form), `unsafe`-gated. Linker control: linker sections (`place this in section ".init"`) on declarations, custom linker scripts per target in `Lagom.toml` — the last mile for kernels/bootloaders (M6, with the freestanding target, 23.4).

### 18.7 Memory-mapped IO and embedded — `elite`

`volatile`-qualified pointers (`a volatile pointer to unsigned 32 bit number at address 1073741824`), register-block C structures, no-std-style freestanding targets (23.4), allocator-free programs (the ARC runtime is *optional*: a `no runtime` build mode where only owning/unsafe code compiles — the embedded/kernel configuration, M6).

---

## 19. Standard library

### 19.1 Architecture: a small kernel, batteries included, stdlib is *just a package*

The stdlib is technically a package (`standard`) that every program implicitly `use`s. It has two tiers:

- **Kernel modules** (always available, no feature flags): `standard` (say/ask/format), `math`, `text`, `lists`, `maps`, `files`, `time`, `random`, `errors`, `testing`.
- **Batteries** (still stdlib, still zero-config, but heavier): `network`, `http`, `json`, `csv`, `compression`, `cryptography`, `database` (a SQLite wrapper), `processes`, `environment`, `logs`, `regular expressions`, `graphics` (window/input/audio hooks, M4).

**Why batteries in stdlib?** The brief's core audience cannot manage dependencies. Python/Go's batteries-included model is the validated answer for learners; Rust's thin-stdlib model is the validated answer for ecosystems — Lagom splits the difference: batteries included *and* every battery is versioned, swappable, and implemented as a plain package (so the ecosystem can outgrow the stdlib without a language release — the Go stdlib's rigidity, avoided).

### 19.2 The stdlib is written in Lagom

From M1 onward, everything above the OS-syscall boundary is Lagom (the `C` boundary layer is hand-written per platform). This is the self-hosting rehearsal (29): the stdlib *is* the first serious Lagom program.

### 19.3 What stays out of the stdlib — Decision Log D-16

GUI toolkits (package: a Lagom-native toolkit is an ecosystem flagship project), web *frameworks* (package: http is in, frameworks out — Python's stdlib lesson), machine learning (package), regex *engines* beyond one good one (one, not five — Python's lesson), YAML (package: the spec is a trap — JSON/TOML only in stdlib).

---

## 20. Package manager

### 20.1 Concepts

- **Manifest:** `Lagom.toml` — name, version, Lagom version requirement, dependencies, build settings, capability declarations (28).
- **Lockfile:** `Lagom.lock` — exact resolved versions + hashes, committed for reproducible builds.
- **Packages:** git repositories (any host) + a central index (`lagom.land`, an API over git refs — the Cargo/crates.io shape) at M3. Package = one versioned, semver-2.0-checked unit.
- **Versions:** strict semantic versioning enforced at publish; the resolver prefers compatible (caret) ranges; MSRV-style `requires lagom >= 0.4`.
- **Build scripts:** `build.lagom` — a Lagom program run at compile time with restricted capabilities (FS access in package dir, no network — the same comptime interpreter, 16.2; no shell-script native layer, the D-18 logic again).
- **Native dependencies:** declared (`link with …`, 17.2); `lagom build` shells to the platform C toolchain — vcpkg/pkg-config integration is package-level tooling, not core.

### 20.2 Commands

`lagom new / add / remove / build / run / test / doc / publish / login`. Workspaces (multi-package repos) at M3. Offline mode: a global package cache (the Cargo model); `lagom build --offline` fails loudly rather than silently re-resolving.

### 20.3 Registry security — see 28.4

Signed publishes, content-addressed tarballs, audit `lagom audit` against advisory feeds, mandatory 2FA at M3, organizational private registries (token-authenticated, same protocol).

---

## 21. Compiler architecture

Full spec: [docs/08-compiler.md](08-compiler.md). Implementation language: **Rust** (D-21: memory safety for a compiler-as-long-lived-infrastructure, best-in-class LLVM bindings, incremental-compilation ecosystem (Salsa), and the strongest talent pool for the *exact* skill set — Rust compiler contributors are the natural contributor base; Zig was the runner-up for self-hosting affinity, C++ rejected for the memory-bug tax on volunteer code).

### 21.1 Pipeline

```text
source.lagom
  → lagom_lexer      (hand-written, SIMD-friendly, ~1 GB/s target)
  → lagom_parser     (recursive descent + indentation scanner; error-recovery with region commits)
  → lagom_ast        (arena-allocated, lossless-with-comments available for formatter/LSP)
  → lagom_sema       (name resolution, type check, capability check, exhaustiveness, borrow/region check)
  → lagom_hir        (desugared: clauses → signatures, flowcalls → calls, interpolation → format calls)
  → lagom_mir        (SSA-ish, monomorphized, ARC/borrow insertion, closure lowering, async state machines)
  → lagom_lir        (backend-neutral: explicit control flow, explicit types, calling conventions)
  → backend          (Cranelift | LLVM | Wasm | interpreter)
  → native object / wasm module
  → linker           (platform linker; mold/lld when present)
```

### 21.2 Backend pairing (approved decision)

- **Cranelift** for debug/`compile quickly` — compilation speed is the product for students; Cranelift is the production-proven fast compiler (Wasmtime's default backend), pure-Rust, with a Wasm path for free.
- **LLVM** for `compile with maximum optimization` — the performance ceiling (vectorization, PGO/LTO integration, the whole backend literature).
- Both consume `lagom_lir`. **No performance claim about either backend is made in this document**: M0's exit criteria require benchmarking Cranelift vs LLVM on Lagom's own benchmark suite before the numbers go in the docs (the Bytecode Alliance's own measurements — Cranelift-generated code within ~14% of LLVM on Wasmtime's workload, at a large compile-speed win — are *context*, not Lagom's claim; other workloads vary widely). This is Decision D-8, evidence-driven by design.

### 21.3 The interpreter backend

A tree-walking/MIR interpreter ships with the compiler from M0 — it powers the REPL (`lagom play`), inline unit tests, and comptime evaluation (16.2). One evaluator, three uses.

### 21.4 Diagnostics pipeline

Errors are a structured type (code, spans, labels, helps, notes) rendered by a renderer with three verbosity modes (`student`/`normal`/`expert`) — see doc 09-tooling for the format spec and the concepts-glossary that maps each error to the lesson that teaches it.

### 21.5 Compiler library, not just binary

Every stage is a Rust library crate with a stable-ish internal API; the LSP, the formatter, the package manager, and the test runner are all *clients* of the same crates. This is the rustc-architecture lesson applied from day one (and it is what makes the LSP correct-by-construction rather than a re-implementation).

---

## 22. IR design

### 22.1 HIR (desugared, still high-level)

All sugar eliminated: clauses → typed signatures, flowcalls → ordinary calls, `make`/`set` → bindings, string interpolation → `format` calls, `if`-expressions explicit, operator words → operator nodes. Region/layer information recorded here (a student layer check rejects expert-only syntax *here*, with a teaching diagnostic).

### 22.2 MIR (semantic core)

SSA-construction form with explicit: types (monomorphic after monomorphization), capabilities (`fail`/`wait`), memory operations (explicit alloc/load/store/arc-increment/decrement), coroutine frames for `can wait`, closure environments, move/borrow markers for owning regions, and loop/branch structure. Passes: constant folding, escape analysis (9.5), ARC elimination (Swift-class algorithm), monomorphization, closure capture lowering, async state-machine construction, drop elaboration, inlining (tiered), and the optimization ladder gated by build mode (24).

### 22.3 LIR (backend-neutral machine shape)

Explicit control-flow graph, explicit machine-ish types, calling conventions resolved, vtables laid out, relocations declared. Backends consume LIR only — adding a backend is implementing one crate against one interface (this is the multi-backend bet, 23).

### 22.4 IR invariants (the implementation discipline)

Every IR has a verifier; `lagom build --verify-ir` runs all of them. Debug-mode MIR inserts checks (overflow, bounds, null-ARC) that release mode removes — the debug/release distinction is *checks*, never *semantics*.

---

## 23. Backend strategy

### 23.1 The pairing, restated

Cranelift (fast, Rust, Wasm-native) + LLVM (ceiling) over one LIR. A hand-rolled x86-64 backend is the *fallback* if Cranelift maintenance ever becomes a blocker (documented contingency, not a plan-of-record — D-8).

### 23.2 Why not C-transpilation?

Evaluated seriously (D-8): fastest to ship, free portability, free C interop. Rejected as the *primary* path because (a) diagnostics and debug info degrade (the compiler becomes a second-order citizen behind the C compiler), (b) the performance ceiling is the C compiler's interpretation of generated C — historically fine but unownable, and (c) the comptime/ARC/async machinery needs IR-level control that C source cannot express cleanly. Kept as a *bootstrap convenience* only (a C-emitting mode for weird targets, non-goal for M0–M4).

### 23.3 Why not GCC/GCC-style multi-frontend?

GCC's runtime, plugin-ABI instability, and C++ codebase make it the wrong substrate for a Rust-implemented compiler; LLVM already covers the "mature optimizing backend" role. Revisited only if LLVM's license/governance ever becomes an issue (unlikely; documented).

### 23.4 Targets and cross-compilation

| Priority | Target | Milestone |
|---|---|---|
| 1 | x86-64 Linux | M0 |
| 1 | x86-64 macOS (aarch64 too) | M0–M1 |
| 1 | x86-64 + ARM64 Windows | M1–M2 |
| 2 | WebAssembly (via Cranelift) | M2–M3 |
| 2 | ARM64 Linux | M2 |
| 3 | RISC-V 64 | M4+ |
| 3 | 32-bit ARM / embedded | M5+ |
| 4 | Freestanding/kernel (no runtime) | M6 |

Cross-compilation is a first-class flag from M1 (`lagom build for mac on linux`), backed by the Zig-adopted approach of shipping sysroot-bundled libc headers or requiring the target SDK. Design-for-all rule: every IR pass must be target-agnostic; target-specific lowering lives only in backends — that single discipline is what makes the target list achievable.

### 23.5 Linking

Platform linkers by default; `mold`/`lld` auto-detected for speed (linking dominates small-build wall time — measurable, and the cheapest big win for the compile-speed goal).

---

## 24. Compilation-speed strategy

### 24.1 The bar

- Student program (≤ 200 lines): **< 150 ms** to native binary on a laptop (debug, Cranelift) — *faster than saving the file*; the REPL-grade experience.
- Stdlib rebuild: < 10 s clean, < 1 s incremental.
- Large project (100k+ lines): parallel per-module pipelines; incremental rebuild touches only dirty modules (Salsa-style query provenance).

### 24.2 Mechanisms

- **Query-based incremental compilation** (Salsa model): every compiler artifact is a query result keyed by inputs; edit-driven invalidation recomputes only the dirty closure. Designed in from M0 (retrofitting incrementality is the classic fatal mistake — the rustc lesson).
- **Parallel by construction:** module-level parallelism in parsing/sema (per-module checkers share an immutable global context), intra-module parallelism in MIR/backend.
- **No serialization cliff:** on-disk incremental cache is the query database itself (Salsa-style), not a second representation to keep in sync.
- **Fast linking:** mold/lld detection (23.5); shared-stdlib-dylib mode for dev (`compile quickly` links against a prebuilt libstd dylib, the D-style "std as dylib" trick, cutting hello-world link time dramatically).
- **Tiered optimization:** `compile quickly` = Cranelift + no MIR opt passes beyond correctness ones; default build = + inlining-lite, ARC elim; `compile with maximum optimization` = full LLVM pipeline + LTO + PGO hooks. The compiler *never* spends the last 2% unless asked (brief §10).
- **Determinism:** same inputs → same outputs, always (reproducible builds, 28.5).

### 24.3 Compile-speed regression is a release blocker

CI tracks compile-time benchmarks (self-hosting the stdlib is the first real benchmark, 37); a regression > 10% blocks release. Speed is a feature with an owner, not a hope.

---

## 25. Runtime-performance strategy

### 25.1 The claim discipline

**No unmeasured performance claims.** The M0 exit criteria include a benchmark suite (toy-compiler, JSON parse, matrix multiply, string processing, channel ping-pong, allocator churn) run against C, Rust, Go, and — when available — the backend pairing; results are published with methodology and machine specs, and every release re-runs them (37). The design *goal* is "within striking distance of Rust/C on compute-bound code, decisively faster than Python/JS, competitive with Go on concurrent services" — stated as a target to be verified, not a fact.

### 25.2 Where the speed comes from

- **Native code, AOT** — no VM, no JIT latency (JIT is an *option* the expert may add via FFI/plugin later, not the runtime model).
- **Static dispatch by default:** calls dispatch statically; vtables appear only when a value's concrete type is erased (interface objects, 12.3); devirtualization runs before inlining (21.2/22.2).
- **Monomorphized generics** (22.2) — zero abstraction cost; the tradeoff (binary size, compile time) is accepted and mitigated by 24's tiering.
- **Value semantics + inline allocation** (9.2/9.5) — cache-friendly by default; structs nest inline; lists are dense vectors.
- **ARC with escape analysis** (9.3–9.5) — most refcount traffic eliminated; deterministic destruction enables RAII-level IO performance (no GC-write barriers, no pause-related tail latency).
- **Auto-vectorization + portable vectors** (18.5) — plain loops vectorize in the LLVM backend; the Cranelift debug path does not (and does not need to — debug is for debugging).
- **Zero-cost abstractions where the design allows:** iterators, closures (non-capturing → static), `match` (jump tables), optionals (niche-optimized where possible), errors (no unwinding tables — failures are ordinary returns).
- **PGO/LTO** available in `compile with maximum optimization` (23.1/24.2).

### 25.3 What we deliberately do not chase in v1

Profile-guided *inline caching*, speculative JIT, software-managed NUMA, and auto-parallelization beyond vectorization — all logged as future investigations (doc 10-roadmap), none load-bearing for the performance story.

---

## 26. Tooling strategy

Full spec: [docs/09-tooling.md](09-tooling.md). One principle drives all of it: **the tooling teaches.** The compiler is the first teacher; the tools are the rest of the faculty.

### 26.1 The `lagom` CLI (one binary)

`lagom run|build|test|check|fmt|doc|play|new|add|words|explain|profile` — the Go single-binary model; `lagom play` is the REPL (with incremental eval, teaching mode showing inferred types after each line); `lagom explain E0003` prints the full teaching write-up for any error code (docs-in-the-compiler, so explanations ship with the version that emits them).

### 26.2 Teaching diagnostics (the differentiator)

Three verbosity modes; `student` mode is default for new projects:

```text
ERROR on line 3

You are comparing text with a number.

    age is greater than 13
    ^^^

You created age as text:

    make age equal to "15"

If age is supposed to be a number, write:

    make age equal to 15

Learn more: run `lagom explain E0003` — "Types describe what values are".
```

`expert` mode is terse (`E0003: mismatched types text vs number at 3:5-3:8`). The mode is per-project (`Lagom.toml`) and per-invocation. Diagnostic content is spec'd, reviewed like API, and tested like code (every diagnostic has a snapshot test + a `lagom explain` entry — doc 09).

### 26.3 The rest of the faculty

- **Formatter** (`lagom fmt`): canonical 4-space, opinionated, non-configurable beyond indentation width (the gofmt lesson: zero-config formatting is what makes uniform code cultures).
- **LSP**: hover shows inferred types *in student words*; autocomplete ranked by layer (student project → no `unsafe` completions); inline errors use the same diagnostics pipeline (21.4).
- **Debugger**: DAP server over LLVM debug info (Cranelift emits full DWARF — it was built for Wasmtime debugging); stepping, scopes, and a *teaching pane* (what changed in this step).
- **Profiler**: perf/DTrace integration + a Lagom-flavored sampling profiler with task-aware views (per-task timelines, 14); allocation profiling (ARC counts) at M4.
- **Test runner**: `lagom test` runs `test` blocks (28 of the brief → doc 09), with property-based testing (`try many random … check that …`) and fuzzing (`lagom fuzz`) at M3.
- **Debugging, as a product of LOM (26.5)**: the dev/release boundary is a *build-mode* fact, not a flag — dev builds instrument (events + provenance + the failure report) and are also fully DAP-debuggable; release builds contain zero LOM code and are bit-identical to non-LOM builds. `lagom run --trace` (interpreter) is the teaching/replay mode. The 90/10 objective is an *engineering target* (the LOM test is its acceptance criterion), never an unconditional guarantee — budgets bound what is recorded, and release behavior is unchanged by design.
- **Benchmarks**: `lagom bench` (criterion-shaped, with compile-time benchmarks too, 24.3).
- **Documentation**: `lagom doc` renders doc-comments (`##`) into a static site with *executable examples* (doctest-style — every example in every doc compiles and runs, the Rust lesson).

### 26.4 Editor support

VS Code extension at M2 (LSP + syntax + snippets); the grammar ships a tree-sitter + textmate definition from M0 so highlighters exist before the LSP does. Educational deployments (Code.org-shaped) get a hosted playground (Wasm backend, 23.4) at M4 — no install is the K-12 on-ramp.

### 26.5 The Lagom Observability Model (LOM) — the 90/10 debugging contract

The brief's goal 5 makes debugging an *explanation* process: the system answers what happened, where, why, what the program was doing, which values caused the failure, where those values came from, how to fix it, and what concept was missed — without the programmer scattering `say` statements. Compile-time diagnostics (26.2) are half of that; **LOM is the runtime half, adopted as a named subsystem with a testable definition** (R-22 of doc 11):

> **LOM test:** any unhandled failure in a dev build must produce a report that names the failing value's origin — without the programmer having added any code.

**Principle: observability is a build mode, not a runtime cost you opt out of. Development builds explain; release builds are silent — bit-identical to non-LOM builds (a tested invariant, like the reproducible-builds rule).**

1. **Dev builds (default) — structured events with provenance.** The MIR instrumentation pass (the pass that already owns every load/store/ARC op, 22.2) emits, *only in dev mode*: function entry/exit with argument snapshots, task spawn/join/cancel events, failure events carrying the full error value, and **value provenance** — every binding records where its value came from (which expression, which earlier binding). Storage is a bounded in-memory ring (fixed budget, oldest evicted — no unbounded recording, ever).
2. **The failure report (the payoff).** On unhandled failure, the dev runtime prints, from the ring: the call chain in source words, each frame's argument values, and the provenance chain of the failing values — *"bottom was 0, bound at line 12 (`make bottom equal to size of names`); names became empty at line 9 after the `stop`."* Answers 1–7 of the goal from one artifact the student never built. Answer 8 (the concept) comes from the existing `lagom explain` pipeline: the failure's error kind links to its lesson.
3. **Interpreter mode — full tracing and replay.** `lagom play` and `lagom run --trace` run on the interpreter backend, where per-step value history is free — the teaching mode, and the deterministic-replay story for M0–M2 (re-run the recorded event ring; strict evaluation makes replay deterministic).

**Explicit non-goals:** no always-on production tracing; no value recording in release; no instrumentation-induced heisenbugs. **Ownership:** doc 08 owns the instrumentation pass, ring format, and the release-identity invariant; doc 09 owns the failure-report format (designed and snapshot-tested like diagnostics); doc 06 owns task events; doc 10 schedules M0 (failure report v1 on the interpreter), M2 (MIR instrumentation), M3 (provenance in native dev builds), M4 (replay tooling).

---

## 27. Educational progression

### 27.1 The bridge table (brief §20, made concrete)

| Lagom | Python | JavaScript/TS | Java | C | Rust | Go | Swift |
|---|---|---|---|---|---|---|---|
| `make x equal to 5` | `x = 5` | `let x = 5` | `var x = 5;` | `int x = 5;` | `let x = 5;` | `x := 5` | `let x = 5` |
| `if a is greater than b` | `if a > b:` | `if (a > b)` | `if (a > b)` | `if (a > b)` | `if a > b` | `if a > b` | `if a > b` |
| `repeat 10 times using i` | `for i in range(10):` | `for (let i=0;i<10;i++)` | `for(int i=0;i<10;i++)` | `for(int i=0;i<10;i++)` | `for i in 0..10` | `for i := 0; i < 10; i++` | `for i in 0..<10` |
| `function add … returns a number` | `def add(...):` | `function add(...)` | `int add(...)` | `int add(...)` | `fn add(...) -> i64` | `func add(...) int` | `func add(...) -> Int` |
| `gives back` | `return` | `return` | `return` | `return` | `return` | `return` | `return` |
| `can fail` + `attempt` | try/except | try/catch | try/catch | errno/retval | `Result` + `?` | error values | `throws` + `try` |
| `can wait` | async/await | async/await | CompletableFuture | (none) | `async`/`.await` | goroutines | `async`/`await` |
| `kind` + `match` | (match 3.10) | (switch) | sealed+switch | (switch) | `enum`+`match` | (type switch) | `enum`+`switch` |
| class + `can` | class | class | class | (struct+fn ptr) | impl | (struct+methods) | class |
| interface | ABC/Protocol | (duck/interface) | interface | (vtables by hand) | trait | interface | protocol |
| `attempt … and pass the problem on` | `raise` | `throw` | `throw` | (retval) | `?` | `return err` | `try` |
| list / map | list / dict | Array / Map | ArrayList / HashMap | arrays/by hand | Vec / HashMap | slice / map | Array / Dictionary |

Every row is the *same concept with different clothes* — the explicit goal (brief §20): the student's reaction to any mainstream language should be "I know this idea; new spelling."

### 27.2 What Lagom deliberately teaches (and how the syntax carries it)

- **Types are descriptions, not paperwork** — inference-first, but `of type` is taught in week N when "what values may live here" becomes useful.
- **Immutability as a value** — `make` vs `make changing` is the first "mutable vs immutable" lesson students meet *before* they can hurt themselves with it.
- **Explicit error paths** — `can fail` signatures make "this can go wrong" a *visible, teachable* property.
- **Ownership intuition before ownership syntax** — value-vs-reference lessons (9.7) prepare the `within owning` layer years later.
- **One mechanism, many costumes** — option/result/kind/match are one tagged-union idea (13.4); generics-as-inference (12.1) precede generics-as-annotation (12.2).

### 27.3 Curriculum shape (doc 09 elaborates)

Scratch → Lagom week 1 (`say`, `ask`, `make`, `if`) → loops/lists week 2–3 → functions week 4–6 → structs/kinds/tests weeks 7–10 → [traditional CS-1] → classes/interfaces/generics → [CS-2/algorithms] → concurrency/files/networking → [systems elective] → ownership/unsafe/FFI → [capstone: compiler, database, or engine].

### 27.4 Guardrail (brief §19, restated as a design test)

Every syntax feature must pass: *"Does this hide a concept, or only the punctuation for it?"* Punctuation-hiding is good (`is greater than`); concept-hiding is a defect (no auto-truthiness, no implicit conversions beyond number→decimal, no operator-magic for control flow).

---

## 28. Security model

### 28.1 The secure-defaults principle

Beginner-friendly defaults are security defaults: memory safety (9), no null (8.5), no data races (14.4), no arbitrary implicit execution (no macros, D-18), capability-declared side effects (28.3), immutable-by-default (7.2). The language is designed so that the *naive* program is the *safe* program.

### 28.2 Sandboxing and capabilities — `advanced`

Build-mode capability declarations in `Lagom.toml`:

```toml
[capabilities]
allow-files = ["data/"]
allow-network = ["api.example.com:443"]
allow-processes = false
```

At M2 these are *soft warnings* (Deno's early model); at M4 with the Wasm target they become *hard enforcement* (Wasm's linear memory + import-binding model is the enforcement substrate — the same reasoning Wasmtime applies). Native binaries enforce what the OS can enforce (no in-process sandbox short of full seccomp plumbing — documented honestly as out of scope for native mode in v1).

### 28.3 Unsafe boundary security

`unsafe` requires `because` (18.1); packages containing `unsafe` must declare it in their manifest (`contains unsafe = true`) — visible in `lagom add` output and the registry UI. This is the npm-left-pad-style *visibility* defense applied to memory risk.

### 28.4 Supply chain

Content-addressed package tarballs; signed publishes (ed25519); provenance records (which commit built which artifact); `lagom audit` against an advisory database; lockfiles committed by default; install scripts are *impossible* (build scripts run in the restricted comptime sandbox, 20.1 — no package postinstall arbitrary-execution, the npm lesson); dependency count warnings for student projects.

### 28.5 Reproducible builds

Bit-for-bit reproducible release builds from lockfiles (deterministic codegen, vendored toolchains, `SOURCE_DATE_EPOCH` support) — verifiable by third parties, the supply-chain endgame.

### 28.6 Crypto API

`cryptography` stdlib module: AEAD (AES-GCM/ChaCha20-Poly1305), hashing (SHA-2/3, BLAKE3), KDFs (Argon2id), signing (Ed25519), TLS via OS trust stores. Design rule: *no footgun primitives exported at the student layer* (no ECB, no unauthenticated AES, no MD5/SHA1); crypto-adjacent APIs fail closed.

---

## 29. Self-hosting strategy

### 29.1 The plan (brief §24, made concrete)

1. **M0–M4 — Bootstrap compiler in Rust.** The Rust compiler is the reference implementation; its test corpus (§36 of roadmap) is the *specification in executable form*.
2. **M2+ — stdlib in Lagom** (19.2): the language grows up on its own libraries.
3. **M5 — `lagomc` written in Lagom.** Staged: (a) Lagom compiler compiles *itself* using the Rust compiler's LIR backend initially if needed; (b) differential testing: both compilers run over the entire corpus daily, outputs must match (same binaries, byte-for-byte, in release mode); (c) the Rust compiler is demoted to *cross-check*; (d) the Rust compiler is retired to a museum branch.
4. **M5+ — the compiler is the largest Lagom program,** making every compiler improvement dogfood the language at expert level.

### 29.2 Why self-host (and when not to)

Self-hosting forces the language to be genuinely capable (the compiler is a real systems program: arena allocators, hash maps, FFI, concurrency — all exercised), and it is the ultimate marketing proof. The cost (two compilers during transition) is bounded by the differential-testing harness. The Rust bootstrap is *never* allowed to define semantics that Lagom cannot express — that is the corpus rule: **the test corpus is the spec; both compilers must satisfy it.**

---

## 30. Comparison against existing languages

Status claims below are current as of September 2026 (sources: language project pages and announcements — see the notes in the README's reference list).

### 30.1 The landscape table

| Language | What it does well | What it does poorly | What Lagom takes | What Lagom rejects |
|---|---|---|---|---|
| **C** | Simplicity of runtime; portability; the ABI substrate; teaching "what the machine does" | Memory safety; string handling; undefined behavior; modern feature vacuum | C ABI as FFI substrate (17); the "no hidden runtime" expert mode (18.7) | Manual memory as the default; UB as a lifestyle |
| **C++** | Zero-cost abstraction range; RAII; template metaprogramming power | Complexity without ceiling; safety; compile times; beginner experience | RAII-shaped deterministic destruction (10.4); the "abstraction cost is zero" bar (25.2) | The whole complexity model; multiple inheritance; SFINAE-class metaprogramming |
| **Rust** | Ownership as a safety proof; traits; ADTs; the best diagnostics in the industry; cargo | Ownership tax on beginners; borrow-checker timing in the learning curve; compile times | Ownership/borrow rules for the expert tier (9.6); traits-shape interfaces (10.6/12.4); diagnostics-as-teaching (26.2); cargo-shaped tooling (20) | Making ownership the *entry* price; macro syntax culture |
| **Go** | Compilation speed; deployment simplicity; goroutines+channels; single-binary UX; batteries stdlib | Type system ceiling; error verbosity; no immutability story | Single-binary CLI (26.1); channels+structured tasks (14); batteries-included stdlib (19.1); compile-speed-as-release-blocker (24.3) | Interfaces-with-implicit-satisfaction (Lagom's are opt-in `does`); goroutine-only concurrency *as the only story* |
| **Swift** | ARC with ownership options; optionals; protocol-oriented design; inference-first types; playgrounds | Toolchain weight; ABI churn history; Apple-platform gravity | ARC + escape analysis (9.3–9.5); optionals without null (8.5); protocol shapes (10.6); playground culture (26.4) | Platform lock-in; @objc-era baggage |
| **Java** | JVM maturity; tooling; library ecosystem; "write once" | Verbosity; primitives-vs-objects split; null; GC latency for systems use | Interface culture; strong tooling expectations (26) | Class-first pedagogy (10.1); checked exceptions (they are the *one* thing worse than both errors-as-values and unchecked exceptions for teaching); null |
| **Kotlin** | Null safety migration pragmatics; DSL culture; Java interop | JVM-bound; DSL power hurts readability | Nullable-types-without-null (8.5) | Extension-function sprawl; DSL-as-default |
| **Python** | The beginner on-ramp of record; readability culture; batteries | Dynamic typing costs at scale; GIL; packaging; deployment | Batteries (19.1); readability as *the* goal (6); the on-ramp position itself (27) | Dynamic typing (Lagom is static — inference gives Python's feel without Python's 2 a.m. TypeErrors); significant-whitespace *expressions* (Lagom's indentation is blocks-only) |
| **TypeScript** | Gradual typing as migration; ecosystem; editor experience | The JS floor (any, type holes); build-chain complexity | Inference-first statics (8.1); editor-first tooling (26.3) | Gradual typing (soundness holes); a transpile target as the story |
| **Zig** | comptime as THE metaprogramming idea; no hidden control flow; cross-compilation; honest unsafe | Pre-1.0 instability (still pre-1.0 in 2026); ecosystem size; async void | comptime-shaped metaprogramming (16.2); no-hidden-control-flow (13.1); cross-compile posture (23.4) | Manual memory as the *only* memory story (Lagom's student tier exists precisely for this gap); the "no hidden allocations" absolutism (Lagom hides allocations from students by design) |
| **Nim** | ARC/ORC memory model (the direct proof of Lagom's 9.3–9.4 design); Python-ish readability; metaprogramming range | Macro-heavy culture (readability tax); ecosystem size; GC-mode history confusion | ARC + trial-deletion cycle collector (9.4); destructors+move semantics (9.3) | Whitespace-significant *expressions*; macro culture; identifier-style insensitivity |
| **D** | Range of paradigms; betterC mode (the no-runtime precedent, 18.7); C++ interop attempts | Two-standard-library history; complexity accumulation | `betterC`-shaped `no runtime` mode (18.7) | The complexity accumulation itself |
| **Julia** | Type system ergonomics for numerics; multiple dispatch; JIT warm code | JIT cold-start; general-systems story; deployment | Inference-feel with static backing (8.1) | JIT-first runtime (25.1); multiple dispatch (Lagom: interfaces, 12.3) |
| **Odin** | Systems simplicity; explicit context/allocator threading (the allocator-parameter precedent, 9.8); Orthographic readability | Ecosystem; tooling maturity | Allocator-explicit expert APIs (9.8) | No-GC-only story (same as Zig's entry) |
| **Carbon** | C++-interop ambition; generics design rigor; governance transparency | Still pre-0.1 (no production language as of 2026); C++-first focus | The *discipline* of writing down alternatives (this Decision Log borrows Carbon's process) | The C++-interoperability mission itself (Lagom targets C ABI only, 17.1) |
| **Hare** | Radical simplicity; small-toolchain philosophy; C-adjacent honesty | Feature ceiling (no generics for years); ecosystem | The simplicity discipline as a *feature* (3) | The feature ceiling (Lagom's entire point is not having one) |
| **Jai** | Compile-time execution as the core loop (games); compile-time-first design; compile speed | Closed beta (still closed in 2026); single-vendor risk | comptime (16.2 — Jai and Zig independently validated it); compile-speed culture (24) | Closed development (Lagom is open-source from M0) |
| **Mojo** | Python-ecosystem bridge; performance goals; SIMD story (1.0 released and open-sourced Aug 2026) | Systems-general story still forming; ML-tilt; ownership story evolving | The SIMD-above-a-clean-base shape (18.5); performance-target discipline | Being ML-first (Lagom is learner-first); proprietary history |
| **OCaml/Haskell** | ADTs+pattern matching as *normal*; inference; type-driven design | Adoption barriers; effects/laziness debates | ADTs (7.12); expression orientation (7.14) | Laziness (Haskell); the learning cliff as identity |
| **Elixir** | Concurrency story as joy (OTP); error culture ("let it crash" shaped) | BEAM-bound performance ceiling; static-typing retrofit | Channels-as-default concurrency feel (14.3); *supervision-shaped* task regions (14.2, structure not supervision — noted) | Dynamic typing; the actor-only model as default (14.2 note) |

### 30.2 The synthesis

Every language above solved *some* of Lagom's problem and paid a named price for the rest. Lagom's thesis: the price each paid was **syntactic or pedagogical**, not semantic — ownership need not cost readability (Rust's rules, Lagom's words); statics need not cost approachability (inference, 8.1); safety need not cost control (layers, §0); compile speed need not cost performance (backend pairing, 21.2). Whether that thesis survives contact with a decade of users is exactly what the roadmap's validation ladder (docs/10-roadmap.md) is built to find out.

---

## 31. Major unresolved questions

Tracked in doc 10-roadmap.md with decision dates; summarized here:

1. ~~`either` keyword~~ — **resolved:** deleted at v0.1 (it had no job; reserving unused words is how ambiguity creep starts — doc 11 REJECT list).
2. **Flow-call fallback:** if the M0 corpus shows ambiguity trouble, does `bigger of a, b` become required for multi-arg flowing calls? (M0 review)
3. **Type inference depth:** local-only (M0) vs HM-style (M2)? Ship whichever the corpus proves sufficient; HM is the fallback, not the default bet. (D-11)
4. **Tail-call contract:** best-effort TCO (M0) vs `recursive` keyword guarantee? (M2)
5. **`export` module re-export semantics** for packages (M3, ecosystem-driven).
6. **Trademark/legal screening** for the name Lagom before 1.0 (M4).
7. **String type at the expert layer:** is `text` an ARC'd immutable, or does the owning tier get a byte-buffer string? (M3–M4, memory-model spec doc 05)
8. **Integer overflow in release:** trap, wrap, or UB-with-poison? Debug is checked either way; release default is the question. **Recommendation (D-35):** trap in release too, with explicit wrapping ops at the expert layer — decide with the M1 benchmark datum (doc 03-semantics).
9. **Package registry governance** (foundation vs single steward) before public launch (M3).
10. **Windows ARM64 stdlib scope** at M2 (syscall layer breadth).
11. **Derived capabilities menu** (`can be compared automatically` etc.): exact set at M2 (16.3).
12. **Whether `match` on `decimal` equality is allowed** (floating-point equality teaching hazard) — currently rejected; revisit if numerics users object. (M2)
13. **A method-chaining word** (`applied to`) for readable multi-stage data pipelines — rejected at v0.1 (D-29); revisit only if nested-combinator readability proves poor in real student code. (M3+)

---

## 32. Major risks

| # | Risk | Severity | Mitigation |
|---|---|---|---|
| R1 | **Grammar ambiguity creep** — word-based syntax invites "just make this sentence work" pressure; each exception multiplies parse complexity | High | Closed preposition set (7.15); capability-clause uniformity (6.3); every proposed syntax change must be expressible in the EBNF *and* survive the ambiguity corpus; a formal grammar freeze per release |
| R2 | **Compiler scope explosion** — two backends + incrementality + LSP + REPL + comptime is a multi-year build | High | M0 scope discipline (33); library-first architecture (21.5) lets work parallelize; Cranelift defers codegen cost; interpreter keeps tests fast before backends land |
| R3 | **The ARC story has real costs** — refcount traffic on hot shared paths; cycle collector complexity | Medium | Escape analysis (9.5); owning tier (9.6) as the documented escape; benchmark suite owns the numbers (25.1); Nim ORC proves the model is shippable |
| R4 | **Ecosystem cold start** — a language without libraries is a hobby | High | Batteries-included stdlib (19.1); C FFI early (M3) so "the language is never the wall"; playground for zero-install onboarding (26.4); curriculum partnerships before 1.0 |
| R5 | **Pedagogical validation never happens** — the language is plausible but no classroom ever adopts it | High | The validation ladder (docs/10-roadmap.md §real-world) with real user testing at every milestone from M0; teacher advisory group; the diagnostics UX is a *product*, with research, not an afterthought |
| R6 | **Two-backend drift** — Cranelift and LLVM disagree on semantics | Medium | One LIR; both backends verified against the same corpus (29.1 differential model applied to backends from M2) |
| R7 | **Self-hosting stall** — the M5 rewrite never completes, leaving a Rust-forever compiler | Medium | Differential harness makes partial self-hosting safe (29.1); stdlib-in-Lagom (M2) is the intermediate win that de-risks it |
| R8 | **Security of the package ecosystem** — the npm crime wave repeats | High | 28.4 design-from-day-one: no install scripts, signed publishes, audit feed; capability declarations (28.3) |
| R9 | **Legal/name collisions** | Low | Screening task (31.6); the name is uncommon enough that the namespace looks clean |
| R10 | **"Worse Python for beginners, worse Rust for experts" — the middle-language trap** | High | The only true defense is the validation ladder: expert projects (compiler, DB) must actually be *buildable* (M3–M5 gates), and student projects must actually be *joyful* (M0–M1 gates). If either fails, the design iterates rather than ships |

---

## 33. MVP proposal

### 33.1 M0 — "the language exists and is real" (the gate to everything else)

**Definition of done — a student can, on day one:**

1. Install one binary (`lagom`) on Linux/macOS (Windows M1).
2. `lagom new quiz && lagom run` — sub-second build, native binary.
3. Write, in pure student layer: variables (both kinds), the four arithmetic operators + division words, all comparison words, `and/or/not`, `if/otherwise`, all three loops + `stop`/`next`, lists, maps, pairs, **structs with `with`-construction and field access (7.11 — the student layer owns them)**, strings with interpolation, functions with `takes`/`returns` clauses, `say`/`ask`, `number from answer` text-parsing (D-39), **`random from 1 to N` (the kernel `random` module's teaching API — the guessing game needs it)**, basic `can fail` + `attempt`, `test` + `check that`, `use math for …`.
4. Have every mistake answered by a student-mode diagnostic that passes the five-part test (what/where/why/fix/concept).
5. Ship it: `lagom build` → a binary they can send a friend.

**Explicitly not in M0** (and what M0 must *not* break to allow them later): classes, interfaces, generics beyond `anything`-inference, async/tasks, FFI, unsafe, comptime, packages beyond local modules. **Architecture slots reserved:** capability sets in signatures (7.8), MIR ARC insertion points (22.2), LIR backend interface (22.3), layer tags through HIR (22.1), query-based incrementality (24.2) — each slot has a written contract in the relevant spec doc.

**M0 exit criteria:** the four validation projects (calculator, guessing game, quiz, text adventure) are written, *by non-authors*, in the student layer, with task-completion feedback captured; **the failure report ships at v1 on the interpreter backend (LOM tier 2, 26.5) and passes the LOM test on the validation projects' seeded failures**; compile-time budget met (24.1); benchmark-vs-LLVM baseline recorded (D-8); corpus green (lexer/parser/sema/codegen/runtime). **M0 is the milestone the consistency pass validated as fully expressible (doc 12): every required program derives from the frozen grammar with no M1+ features.**

### 33.2 The MVP discipline (brief §33, restated)

No shortcuts that close doors: every M0 simplification must be a *missing feature*, not a *wrong shape*. The four shape commitments that M0 makes permanently: capability-clause functions (7.8), values-not-exceptions (13), layers-as-checks-not-dialects (§0), one-IR-many-backends (22.3).

---

## 34. Decision log

Every major decision: what was chosen, what was considered, why, and the accepted tradeoff.

| # | Decision | Chosen | Alternatives considered | Why | Tradeoff accepted |
|---|---|---|---|---|---|
| D-1 | Name | `Lagom` (user-chosen) | Braid, Onwards, Sprout | Owner's choice; meaning fits philosophy | Uncommon-but-real word; trademark screening queued (31.6) |
| D-2 | Overall structure | One language, layered exposure (§0) | Separate kid/expert languages; gradual typing | Two languages double the toolchain and the teaching debt; gradual typing has soundness holes (TS lesson) | Expert surface must not leak into student UX — enforced by checker, not docs |
| D-3 | Syntax family | Word-based, formally specified | Symbolic; natural language; s-expr | Brief's core premise; determinism is non-negotiable (6.1) | Longer source (allowed by brief); English-centrism (accepted, with localized diagnostics later — not localized keywords) |
| D-4 | Blocks | Indentation | Braces; end-keywords | Visual scope = the concept; Scratch/Python continuity | Copy-paste and tooling must respect indentation (formatter+LSP own this) |
| D-5 | Immutability default | `make` immutable, `make changing` mutable | Mutable default (Python/JS) | Safety + the transfer lesson (Rust/Swift agree); bugs from accidental mutation are classic | Slightly more typing for mutable vars (accepted: `changing` is one word) |
| D-6 | Comparisons/logic as words | `is greater than` etc. | Symbols | Brief's example; teach-the-concept principle (6.1) | More verbose for experts — but symbols *remain as aliases* (7.3), so nobody is forced |
| D-7 | Division | `divided by` = true (promotes to decimal), `divided evenly by` = floor; `remainder of` | `/` = floor (C-style); `/` = true only for floats; separate `//` | School intuition + Scratch; silent floor-division of integers is a classic beginner trap; explicit words teach the two *concepts* distinctly | `divided evenly by` is long (accepted: explicitness is the point); division type-promotion needs teaching (a semantic spec item, doc 03) |
| D-8 | Backends | Cranelift (debug) + LLVM (max) over one LIR; no perf claims until benchmarked on Lagom's suite | LLVM-only; Cranelift-only; C-transpile; custom from day one (21.2, 23.2) | Compile speed AND ceiling; the evidence-driven requirement avoids unowned numbers | Two backends to verify (R6); Cranelift dependency (contingency: hand-rolled x86-64) |
| D-9 | Zero-based indexing | 0-based only | 1-based sugar mode | Transfer to every mainstream language; two systems teach confusion | The classic off-by-one lesson arrives early — taught explicitly, not dodged |
| D-10 | `number` = i64 with decimal promotion on true division | One visible numeric type | i32; f64-only (JS-style); int/float split from day one | One type for beginners; overflow honesty; precision escape hatches at intermediate (8.2) | i64≠bignum (accepted, documented); promotion rules need spec care (doc 03) |
| D-11 | Inference | Local inference M0; HM-style only if corpus demands | Full HM from day one | Local inference covers 95% of student code with far less machinery; HM is a *fallback*, not a bet (31.3) | Occasional annotation needed earlier than HM would require |
| D-12 | Errors | Values + `can fail` capability; no exceptions | Exceptions; Go-style naked error values; Result-types-as-syntax | Hidden control flow is anti-pedagogical and anti-audit; Go's version is *too* unobtrusive (missed checks); Rust's is right-shaped but ill-spelled for beginners (13.1) | Slightly wordier call sites; no stack-trace-style propagation (a deliberate loss — chains are explicit values) |
| D-13 | Memory | Hybrid ownership + ARC + cycle collector + `unsafe` (9) | Tracing GC; ARC-only; ownership-only; pure manual | Swift/Nim proved ARC; Rust proved ownership; only the hybrid serves both edges of §4 | Two memory disciplines to implement and document (R3); the collector exists at student tier (a small runtime cost) |
| D-14 | Overloading | No arity/type overloading; named constructions (10.3) | Classic overloading | Overloading interacts terribly with inference and word-names; named constructors are clearer | Migration friction for `init`-style APIs (accepted) |
| D-15 | Inheritance | Single, allowed, discouraged; interfaces for substitution | Multiple inheritance; no inheritance at all | Students meet it everywhere; single-only avoids diamond costs; composition-first docs steer practice (10.5) | Some OO designs need delegation ceremony (accepted) |
| D-16 | Stdlib scope | Batteries included, implemented as plain packages, thin kernel (19) | Thin stdlib (Rust); monolithic stdlib (Go) | Learners can't manage deps; plain-package implementation avoids Go's rigidity | Larger maintenance surface per release |
| D-17 | Async | Capability clauses + state machines over a work-stealing pool (15) | Green threads (Go-style) | FFI/embedded honesty; mainstream transfer; door stays open to green threads behind the same syntax | Async color exists (contained by the clause system); scheduler complexity |
| D-18 | Metaprogramming | comptime only; no macros; derives as builtin capabilities | Procedural macros; templates; nothing-but-generics | One mechanism; readability hazard of syntax-rewriting macros (16.3) | Some boilerplate needs compiler cooperation (derives) rather than user macros |
| D-19 | Type system ceiling | ADTs+generics+interfaces; no dependent/refinement types in v1 | Dependent types; refinement types; typestate | Comptime covers most uses at far lower cost (8.6); revisitable later | Some invariants remain runtime-checked |
| D-20 | Effects | Fixed capabilities (`fail`, `wait`), no open effect system | Full effect polymorphism | Open effects are a research frontier, not a v1 dependency; fixed caps are teachable and transferable | Some library designs (e.g., generic IO contexts) need workarounds |
| D-21 | Implementation language | Rust | Go; Zig; C++ | Safety-for-compiler-infra, LLVM bindings, Salsa, talent pool (21) | Contributors must know Rust (mitigated by good module docs) |
| D-22 | Compiler architecture | Library-first, query-based incremental from M0 (21.5, 24.2) | Monolithic binary; retrofit incrementality later | Parallelizable work; LSP/formatter correctness-by-construction; the rustc lesson learned *before* the mistake | More upfront design; internal APIs churn pre-1.0 |
| D-23 | Ownership exposure | Opt-in regions, expert layer (9.6) | Ownership everywhere (Rust); ownership nowhere | Both edges of §4 served by one semantics | Experts must learn region discipline; ARC tier needed anyway |
| D-24 | Concurrency default | Tasks+channels first; shared+mutex second; atomics last (14) | Shared-memory first; actors-only | Message-passing is teachable and race-free; the ladder ends at full power | Some shared-state designs are chattier in messages (accepted) |
| D-25 | Data-race prevention | `sendable`/`shareable` + `shared`/`guard` fields (14.4) | Runtime race detection only | Compile-time prevention is the Rust-proven route; runtime detection complements (debug sanitizer) | Some legible programs need restructuring to satisfy the checker |
| D-26 | Packages | Git+index, semver, lockfiles, no install scripts, signed publishes (20, 28.4) | Central-only; scripts-in-packages | Supply-chain design before the crime wave, not after | Slower publish UX than npm-style (accepted deliberately) |
| D-27 | Self-hosting | M5, differential-tested, corpus-as-spec (29) | Self-host at M1 (grand, early); never self-host | Forces real capability exactly when the language is ready; bounded cost | Two compilers during transition (R7) |
| D-28 | Validation | Real projects at every milestone, by non-authors (33, doc 10) | Benchmarks-only; demos-by-authors | The middle-language trap (R10) is only detectable with real users | Slower milestone cadence (user testing takes calendar time) |
| D-29 | Data-pipeline chaining | No chaining word at v0.1; pipelines are nested combinator calls or intermediate variables | `applied to` chaining word; Unix-pipe operator | A third call syntax before the need is proven violates the anti-bloat rule (6.3); intermediate variables are readable and teachable | Deeply nested combinators can be harder to read — revisit via 31.13 with real student code |
| D-30 | Equality semantics | One operator (`is equal to`): structural for values, identity for classes; `same as`/`different from` deleted | Four operators with a structural/identity split; `equal to`=structural + `same as`=identity | Four spellings for two meanings is accidental complexity in the most-used operator; Rust/Swift/Java all converged on this shape (review R-6) | Structural comparison of classes is opt-in via `does comparable` (accepted) |
| D-31 | Channel send | Send **moves** ownership; `shareable` classes may be sent as shared references | Send shares ARC references (Go-style) | Race freedom must be structural, not disciplinary — shared mutable state through a channel must be explicit (review R-8) | Shared-state-via-channel designs must mark classes `shareable` with `shared`/`guard` fields (accepted; 14.4 is that path) |
| D-32 | Observability | The Lagom Observability Model (26.5): dev-only structured events with value provenance + the failure report; release builds bit-identical to non-LOM builds | Print-debugging only; always-on tracing | The 90/10 debugging goal needs a runtime half with a testable definition, not an implementation afterthought (review R-22) | MIR instrumentation pass complexity (accepted); dev-build overhead (bounded ring, dev-only) |
| D-33 | Comptime staging | Value-level comptime at M3 (over monomorphized MIR); type-level comptime at M4+ (over pre-monomorphization IR) | Type-level comptime at M3; comptime interpreter over MIR for everything | MIR is monomorphized and ARC-inserted — it cannot host type-generating programs; interleaving evaluation with type checking is the hardest machinery and waits for evidence of need (review R-19) | Type-generated APIs wait one milestone (accepted; derive clauses cover common boilerplate) |
| D-34 | Option spelling | `T?` and `a T or nothing` are one type with two canonical costumes; printing an option shows the value or the word `nothing`; standalone `nothing?` deleted | `T?` only; word form only; `nothing?` as the annotation | Two *declared* spellings match the design's two-costumes pattern; the printing rule makes absence a visible lesson (review R-18) | Two spellings to teach as one concept (accepted, stated) |
| D-35 | Release overflow | Recommendation: trap in release too (like debug), with explicit wrapping ops at the expert layer | Wrap in release (Rust); UB-with-poison | "Overflow honesty" must not be build-mode-dependent — semantics may differ between debug and release in checks, never in results (review R-17) | A branch on arithmetic in hot loops; escapable at the expert layer; confirmed by the M1 benchmark before freezing (31.8) |
| D-36 | Method invocation | Methods are ordinary calls with the receiver as first argument (`bump c`); no dot syntax; `can` reserved for declarations | `.` member access; `can name of target` call spelling | No post-fix operator keeps the four-symbol operator table and kills chain-syntax pressure; the Go/Lua shape gives the honest transfer sentence "methods take the object first" (review R-2) | `bump c` reads less object-oriented than `c.bump` (accepted) |
| D-37 | Call grammar | Unified flow-call production with **additive-level arguments**, `with`/`using`/`where` suffixes, bounded lambda bodies; boolean args need parentheses or `where` | Full-expression arguments with greedy `and` | The sentence a human reads and the sentence the compiler parses must be the same — divergence is the failure mode the syntax philosophy exists to prevent (review R-1/R-4) | Comparisons/booleans inside flowing arguments need parens or the `where` suffix (accepted; the ambiguous spelling is a teaching diagnostic, not a misparse) |
| D-38 | Shared-state locking | Lock scope = the whole method body on classes with `shared` fields; outside-method field access is a compile error | Lock per field access; explicit lock blocks as the intermediate default | Per-statement locks admit the lost-update race inside the model that promises none; the method is the unit of atomicity — Java's `synchronized`-method model (review R-9) | Coarser locks may serialize more than necessary (accepted; experts have atomics and explicit locks) |
| D-39 | Text↔number | Student-layer API: `number from text` / `decimal from text` (`can fail`) and `text from number`; no `as`-conversion syntax | `as number` conversion syntax; silent coercion | The calculator/guessing-game M0 projects need it; failing parsing composes with the error model the student already knows; `as` stays reserved for the `attempt … as problem` binding (review R-10) | Input handling is explicit from day one (accepted — it previews the error model) |

---

*Next documents: [01-philosophy](01-philosophy.md) · [02-syntax](02-syntax.md) · [03-semantics](03-semantics.md) · [04-types](04-types.md) · [05-memory](05-memory.md) · [06-concurrency](06-concurrency.md) · [07-stdlib](07-stdlib.md) · [08-compiler](08-compiler.md) · [09-tooling](09-tooling.md) · [10-roadmap](10-roadmap.md)*