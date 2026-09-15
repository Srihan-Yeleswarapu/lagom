# Lagom — Architecture Adversarial Review & Design Stress Test

*Status: complete. Review of [00-architecture.md](00-architecture.md) v0.1.0-draft and phase stubs 01–10, performed before any implementation begins.*
*Update (2026-09, post-review): the approved subset of §18 is **applied** — R-1, R-2, R-3, R-4, R-5, R-6, R-7, R-8, R-9, R-10, R-11, R-12, R-20, R-21, R-22, R-23 across 00 and docs 01–10, recorded as decisions D-30…D-39 in [§34 of 00](00-architecture.md#34-decision-log). The remaining §18 items and the INVESTIGATE list await the re-review/freeze step.*
*Method: every criticism below was tested against the actual document text and the §7.15 grammar sketch. Examples marked **⚠** are quotations from the current documents that are *not derivable* from the grammar — they are cited as defects, not proposed syntax. Nothing here proposes AI parsing or natural-language freedom; every proposed fix is a small deterministic grammar or specification change.*

Issue IDs (`R-1` … `R-24`) are referenced by the [verdict](#17-final-verdict) and the [recommended revisions](#18-recommended-architecture-revision).

---

## Table of contents

1. [Executive summary](#1-executive-summary)
2. [Issue cards](#2-issue-cards)
3. [Syntax stress tests](#3-syntax-stress-tests)
4. [English-ambiguity audit](#4-english-ambiguity-audit)
5. [Type-system stress test](#5-type-system-stress-test)
6. [Memory-model stress test](#6-memory-model-stress-test)
7. [Concurrency stress test](#7-concurrency-stress-test)
8. [Error-handling stress test](#8-error-handling-stress-test)
9. [Compiler-architecture stress test](#9-compiler-architecture-stress-test)
10. [The 90/10 debugging goal → the Lagom Observability Model](#10-the-9010-debugging-goal--the-lagom-observability-model)
11. [Educational-progression audit](#11-educational-progression-audit)
12. ["Can Lagom actually build this?"](#12-can-lagom-actually-build-this)
13. [Accidental-complexity review](#13-accidental-complexity-review)
14. [Missing-capability classification](#14-missing-capability-classification)
15. [Decision-log audit (D-1 … D-29)](#15-decision-log-audit-d-1--d-29)
16. [Contradiction matrix](#16-contradiction-matrix)
17. [Final verdict](#17-final-verdict)
18. [Recommended architecture revision](#18-recommended-architecture-revision)
19. [Verification report](#19-verification-report)

---

## 1. Executive summary

The architecture is conceptually strong: the layer system, capability clauses, errors-as-values, the evidence-driven backend rule (D-8), and the corpus-as-spec self-hosting plan all survive scrutiny and should not be reopened. The design is **not yet implementation-ready**, for one dominant reason and eight secondary ones:

**The grammar sketch and the document's own examples disagree.** Multiple flagship examples — including the *day-one* `ask "What is your name?"` — cannot be derived from the EBNF in [§7.15 of 00](00-architecture.md#715-formal-grammar-sketch). The prose in §7.9 is broader (and correct) but the grammar is what the parser is generated from. This is fixable with one production change, but it means the grammar is not yet normative. (R-1)

The other load-bearing findings, in descending severity:

| # | Finding | Sev. | One-line resolution |
|---|---|---|---|
| R-1 | EBNF ≠ examples: positional calls (`ask "…"`, `divide 10 and 0`), two-argument flow calls (`send "hello" to messages`) not derivable | **Critical** | Extend the flow-call production to `name [arg] {prep arg} {"and" arg}` |
| R-2 | Method invocation has no defined syntax (`can bump of c` ⚠); `.` appears in two examples despite "no punctuation" | High | Methods are ordinary calls with the receiver as first argument (`bump c`, `add 5 to c`); delete `.` |
| R-3 | Combinator call syntax (`keep scores where …` ⚠, `combine scores with start 0 using …` ⚠) not derivable; inline-lambda body extent undefined | High | Add `where`-clause and labeled-`with`+`using` suffixes to the call grammar; bound lambda bodies |
| R-4 | Greedy-`and` argument rule produces human/parser divergence (`bigger of a and b or c`) | High | Flow-call arguments bind at the *additive* level; comparisons/boolean `and` inside args need parentheses |
| R-5 | Map literals with name keys collide with the `to` flow-call preposition | High | Map keys are primary expressions |
| R-6 | Equality is over-provisioned: four operators, structural-vs-identity for classes unspecified; `same as`/`different from` redundant | High | `is equal to` = structural for values, identity for classes; reject `same as`/`different from` |
| R-7 | "No lifetime syntax" claim is only coherent if borrows cannot escape — the `longest of a and b` problem makes expert APIs second-class | High | Document non-escaping borrows as the v1 contract; analysis-based escape as a benchmarked follow-up |
| R-8 | Channel-send semantics (move vs share) unspecified — the race-freedom story is incomplete without it | High | Send = move by default; `shareable` classes are the explicit exception |
| R-9 | `shared` field "lock held automatically for the field access" ⚠ is unsound for compound read-modify-write sequences | High | Lock scope = the whole method body on classes with `shared` fields |
| R-10 | Student-layer text→number conversion does not exist anywhere — the M0 calculator validation project cannot be written | High | Specify `number from text`-style parsing (can fail) at the student layer |
| R-11 | Grammar sketch omits all M1–M2 declaration forms: `class`, `interface`, type aliases, generic declarations, `and pass the problem on` | High | Add the missing productions now (they gate parser-test generation) |
| R-12 | Structs with class fields copy *shallowly* — "value semantics by default" silently leaks aliasing to students | Med-High | Spec shallow copy + the teaching moment + a lint |
| R-13 | Recursive data types (trees!) unspecified | Med-High | Allowed through collection indirection; direct structural cycles rejected |
| R-14 | The filler-`of` rule in `takes number of correct answers` contradicts the "no backtracking" claim | Medium | Spec the greedy-type-then-filler algorithm explicitly |
| R-15 | Literal typing and mixed `number`/`decimal` arithmetic unspecified — the day-one accumulator pattern breaks | Medium | Literals unify by context; only implicit conversion is number→decimal |
| R-16 | Negative division/remainder semantics unspecified | Medium | Floor division + remainder takes the divisor's sign (Python pair) |
| R-17 | Release-mode overflow undecided (00 §31.8) | Medium | Recommend trap in release too; verify the cost with the M1 benchmark |
| R-18 | Option type has three spellings (`nothing?`, `text?`, `a value or done`); printing an optional is unspecified | Medium | Canonical `T?`; word form `a T or nothing`; define printing |
| R-19 | "Compile-time interpreter over the MIR" is technically wrong for type-generating comptime | Medium | Stage: value-level comptime (M3), type-level comptime (M4+) |
| R-20 | `attempt` statement-vs-expression, error-kind variance, finalizer errors, C-call capabilities all unspecified | Medium | Spec each (small rules, one per doc 03/04) |
| R-21 | No fixed-size floats: FFI (`C float`), SIMD (`4 float numbers packed` ⚠ implies a type that is never defined), graphics | Medium | Add `a 32 bit decimal` / `a 64 bit decimal` to the expert lattice |
| R-22 | The 90/10 debugging goal is asserted but not architected | Medium | Adopt the **Lagom Observability Model** (§10 below) as a named subsystem |
| R-23 | Assorted small example/production gaps (`second of` ⚠, variadic production, `boolean` wording, `start another task`) | Low | Sweep fix, listed in §18 |
| R-24 | Features examined and found *sound* — recorded so they are not re-litigated | Info | Keep (see §17 KEEP list) |

Nothing found requires abandoning any of the four permanent shape commitments ([§33.2 of 00](00-architecture.md#33-mvp-proposal)): capability clauses, errors-as-values, layers-as-checks, one-IR-many-backends.

---

## 2. Issue cards

Each card: issue, severity, affected area, why it is a problem, concrete example, who it affects, possible solutions, recommended solution, tradeoffs, and whether the architecture should change.

---

### R-1 — The grammar sketch and the examples disagree (positional calls, two-argument flow calls)

**Severity: Critical.**
**Affected area:** [§7.15 of 00](00-architecture.md#715-formal-grammar-sketch), [§7.9](00-architecture.md#79-readable-call-syntax--student), [§7.1](00-architecture.md#71-output-and-input--student), [§13.1](00-architecture.md#131-errors-are-values-failures-are-capabilities--all-layers), [§14.2](00-architecture.md#142-structured-tasks--advanced), [§14.3](00-architecture.md#143-channels--advanced), [§15.1](00-architecture.md#151-async-is-a-capability-not-a-ceremony--advanced).

**Why it is a problem.** The EBNF is declared "normative for shape" and is to be *generated into the parser tests* (doc 02). But the following quotations are not derivable from it:

- `make answer equal to ask "What is your name?"` — `flowcall = name ("of"|"at"|"from"|"to") expr` requires a preposition; `ask "…"` has none. **The first-day example is ungrammatical.**
- `greet "bo"` ([§7.8](00-architecture.md#78-functions-and-capability-clauses--all-layers)) — same defect.
- `attempt divide 10 and 0 if it fails then` ([§13.1](00-architecture.md#131-errors-are-values-failures-are-capabilities--all-layers)) — `divide 10` has no preposition; `10 and 0` cannot attach.
- `download "a" to "a.file"` ([§14.2](00-architecture.md#142-structured-tasks--advanced)) and `send "hello" to messages` ([§14.3](00-architecture.md#143-channels--advanced)) — two-argument positional-then-prepositional calls; no production covers them.

§7.9's *prose* ("a call is `name argument1 [and argument2 …]`") actually describes the correct, broader grammar — the EBNF is the artifact that is wrong. If M0 generated the parser from the current sketch, every one of these would fail.

**Who it affects:** compiler/tooling developers (immediately); every user (the examples are the spec's face).

**Possible solutions.**
1. Fix the examples to match the EBNF (`ask of "…"`, `attempt divide of 10 and 0`) — rejected: it damages the readability thesis at its most visible points.
2. Extend the EBNF to match §7.9's prose, with the ambiguity guard from R-4.

**Recommended solution.** Option 2 — one production:

```ebnf
flowcall    = name [additive] { prep additive } { "and" additive } ;
prep        = "of" | "at" | "from" | "to" ;
```

This derives `ask "…"`, `greet "bo"`, `divide 10 and 0`, `download "a" to "a.file"`, `send "hello" to messages`, `receive from messages`, `bigger of a and b`, `square root of 16`, `things at 0` — every call form the documents use — while keeping the closed preposition set and the greedy-`and` rule from §7.9.

**Tradeoffs.** `name [additive]` without a preposition means `if count is greater than 0` must never parse as a call — guaranteed, because `is` is a reserved comparison word and args are additive-level (R-4). Ambiguity surface grows slightly; it is bounded by the closed-preposition rule and pinned by the ambiguity corpus (doc 02).

**Should the architecture change? Yes — the EBNF, before any parser work.**

---

### R-2 — Method invocation has no defined syntax; `.` appears despite "no punctuation"

**Severity: High.**
**Affected area:** [§10.2](00-architecture.md#102-classes--intermediate), [§10.7](00-architecture.md#107-static-members-class-level-behavior--intermediate), [§7.13](00-architecture.md#713-modules--student--intermediate), the call grammar.

**Why it is a problem.** The class example calls methods with `can bump of c` ⚠ — but `can` is the *method-declaration* keyword, and no production allows a `can`-statement. The grammar's statement list has no method-call form at all. Worse, two examples use dot syntax (`drawing.circle at 3 and 4` ⚠ in §7.13, `counter.new` ⚠ in §10.7) — but the grammar has no `.` production anywhere, and the entire syntax philosophy ([§6.1 of 00](00-architecture.md#61-words-over-punctuation--but-a-formal-grammar)) is "little to no punctuation when words can express the same concept clearly." A silent dot operator would also collide with the future question of method chains.

The examples also never show a method *with arguments* being called — because there is no way to write one.

**Who it affects:** student and intermediate (every class user); compiler developers (grammar).

**Possible solutions.**
1. Formalize `can name of target` as method-call syntax — rejected: overloads `can` (declaration vs invocation) and still has no argument story (`can add 5 of c`?).
2. Introduce `.` narrowly for member/method access — rejected: contradicts the thesis, introduces the only post-fix operator, and invites chaining syntax pressure.
3. **Methods are ordinary functions whose first parameter is the receiver** — `bump c`, `add 5 to c` — fully derivable from the R-1 grammar (`name [additive] { prep additive }`), no new syntax, and the receiver reads in its natural position.

**Recommended solution.** Option 3. Declared methods lower to functions with a leading receiver parameter; `can` is reserved for declarations (including interface requirements). Field access stays `name of obj` (as in §7.11). `drawing.circle` becomes `use drawing for circle` (the form §7.13 already defines); `counter.new` becomes the `construction` call (`a new counter with step 2`, §10.3, which already works).

```lagom
make c equal to a new counter
bump c                       # method call: receiver as ordinary argument
add 5 to c                   # method with arguments — same grammar
say count of c               # field access: unchanged
```

**Tradeoffs.** `bump c` reads slightly less "object-oriented" than `c.bump`; but it is exactly how Go and Lua express the same idea, it keeps the operator table at four symbols, and transfer is clean ("methods are functions that take the object first" — a *concept*, honestly taught, per the guardrail in [§27.4 of 00](00-architecture.md#274-guardrail-brief-19-restated-as-a-design-test)). Dynamic dispatch is unaffected (the receiver is just the vtable-carrying argument).

**Should the architecture change? Yes — replace the §10.2/§10.7/§7.13 examples and add one sentence to §10.2.**

---

### R-3 — Combinator call syntax and inline-lambda extent are undefined

**Severity: High.**
**Affected area:** [§11.1](00-architecture.md#111-closures-and-higher-order-functions--intermediate), [§11.2](00-architecture.md#112-mapfilterreduce-and-friends--intermediate), the call grammar.

**Why it is a problem.** The flagship FP examples are not derivable:

- `keep scores where it is at least 80` ⚠ — `where` is in no production and is not in the closed preposition set.
- `combine scores with start 0 using start plus it` ⚠ — no production combines a flow call with a labeled `with` argument *and* a `using` argument.
- The inline lambda `map scores using taking score giving back score plus 5` ⚠ has no defined body extent: `giving back score plus 5` — where does the lambda end? At `5`? At the next `and`? At end of statement? In `map xs using taking x giving back x plus 1 and ys using …` the `and` is的三-way ambiguous (argument separator / boolean / lambda-body operator).

**Who it affects:** intermediate (every user of combinators — the intended idiomatic style); compiler developers.

**Possible solutions.**
1. Drop `where` (require `keep scores using it is at least 80`) — rejected: `where` is the most readable spelling of the most common combinator and costs one reserved word used in exactly one position.
2. Define the full call suffix grammar with fixed order and bounded lambda bodies.

**Recommended solution.** Option 2. Extend the call grammar (composing with R-1):

```ebnf
call        = flowcall { "with" name additive } { "using" lambda } { "where" orexpr } ;
lambda      = name                                  (* named function: using double *)
            | "it" additive                         (* implicit single param: using it plus 5 *)
            | "taking" name { "and" name } "giving back" additive ;   (* inline lambda *)
```

Rules that make it deterministic: (a) suffix order is fixed (`with` → `using` → `where`), each at most once; (b) **a lambda body is a single additive-or-comparison expression** — `and`/`or`/nested lambdas inside an inline lambda body require the block form (`a function taking n` … body), which terminates by indentation. This bounds the body extent syntactically with no lookahead tricks.

`keep scores where it is at least 80` is *sugar for* `keep scores using taking it giving back it is at least 80` — one desugaring rule, teachable in one sentence.

**Tradeoffs.** Inline lambdas cannot contain boolean operators without the block form — acceptable: multi-operator bodies are exactly where the block form is more readable anyway (and this is the same trade Python makes with its expression-lambda).

**Should the architecture change? Yes — grammar + a §11.2 note.**

---

### R-4 — Greedy-`and` arguments create human/parser divergence

**Severity: High.**
**Affected area:** [§7.9](00-architecture.md#79-readable-call-syntax--student), expression grammar.

**Why it is a problem.** §7.9 says the innermost open call greedily consumes each `and`. Then:

```lagom
say bigger of a and b or c
```

By the stated rule, `bigger` (the innermost open call) consumes `and b`, and then `or c` extends the *argument* → `bigger(a, b or c)`. But every human reading the sentence parses "bigger of a and b, or c" → `(bigger(a, b)) or c`. The parse a human finds and the parse the compiler finds diverge — precisely the failure mode [§6.1 of 00](00-architecture.md#61-words-over-punctuation--but-a-formal-grammar) exists to prevent. The divergence compounds with `and`'s three existing jobs (boolean, argument separator, construction separator in `a player with name "bo" and score 0`).

**Who it affects:** every layer; the highest-volume ambiguity in the language.

**Possible solutions.**
1. Keep full-expression arguments, document the greedy rule loudly — rejected: "the compiler is right, you are wrong" is the opposite of the teaching-diagnostics philosophy.
2. **Bind flow-call arguments at the additive level** (numbers, names, `+ - * /`, prefixes) so `and` *inside* an argument list is unambiguously a separator, and `or`/boolean-`and` in argument position require parentheses.

**Recommended solution.** Option 2, encoded in the R-1 production (`{ "and" additive }`). `bigger of a and b or c` becomes a **parse error** with a teaching diagnostic ("did you mean `bigger of a and (b or c)`, or `(bigger of a and b) or c`?") — the ambiguity is caught at compile time, not misparsed. Note `say 3 plus 4 and bigger of 10 and 20` (§7.9) still parses correctly: both args are additive.

**Tradeoffs.** Boolean arguments to flow calls need parentheses (`keep scores using (it is at least 80)` — or better, the `where` form from R-3, which takes full expressions and is the intended spelling anyway). Experts lose nothing: the parenthesized form remains always-legal.

**Should the architecture change? Yes — this is the smallest deterministic grammar change that removes the divergence.**

---

### R-5 — Map literals collide with the `to` preposition

**Severity: High.**
**Affected area:** [§7.6](00-architecture.md#76-collections--student), grammar `maplit`.

**Why it is a problem.** `maplit = "a map from" expr "to" expr { "," expr "to" expr }` — but `to` is *also* a flow-call preposition (R-1 confirms: `download "a" to "a.file"`). For literal keys the parse survives (`"ana"` cannot start a flow call), but for name keys:

```lagom
make routes equal to a map from home to work, backup to school
```

`home to work` parses as a flow call `home(work)` — the map literal never finds its separator. The grammar sketch does not address this; it is a real parse ambiguity, not a hypothetical.

**Who it affects:** student (maps with variable keys are ordinary); compiler developers.

**Possible solutions.**
1. Require parens around non-literal keys — ugly, and students will not understand why.
2. **Restrict map keys (and the second `to`-operand) to primary expressions** — names, literals, `(...)`, constructors. Flow-call keys are meaningless anyway (a map key is a *value*; a flow call is a computation — a student never needs one as a key).

**Recommended solution.** Option 2:

```ebnf
maplit      = "a map from" primary "to" primary { "," primary "to" primary } ;
```

**Tradeoffs.** A computed key needs parentheses — rare, and the diagnostic writes itself.

**Should the architecture change? Yes — one production line.**

---

### R-6 — Equality: four operators, and the class case is unspecified

**Severity: High.**
**Affected area:** grammar `cmpword` (`same as`, `different from`), [doc 03 §4](03-semantics.md), [§8.2/§8.5 of 00](00-architecture.md#82-core-types--student).

**Why it is a problem.** The grammar defines `is equal to`, `is not equal to`, `is same as`, `is different from`, and doc 03 says these differ as *structural vs identity* — but nowhere is the mapping stated, and the default for classes is the dangerous case. `make c2 equal to a new counter` … `if c is equal to c2` — two distinct ARC'd objects with equal fields: true or false? If structural: students learn that two "different" objects are "equal," which breaks the identity intuition they need for exactly the stateful-class concept classes exist for (§10.1), and it makes `before last reference disappears` reasoning treacherous. If identity: then `same as` is redundant with `equal to`. Four spellings for two meanings is accidental complexity in the most-used operator in the language.

**Who it affects:** student and intermediate (every comparison); doc 03/04 authors.

**Possible solutions.**
1. Keep four operators; spec `equal to` = structural, `same as` = identity — rejected: four words for two concepts, and the student must learn which is which forever.
2. **Two behaviors, one operator:** `is equal to` is structural for value types (numbers, text, structs, lists, maps, kinds, options) and **identity for classes**; drop `same as`/`different from` from v1 entirely. Structural comparison of classes is opt-in by implementing `does comparable` (§10.8) — the same shape Rust/Swift/Java all converged on.

**Recommended solution.** Option 2. The teaching diagnostic for the surprising case already fits the house style: comparing two distinct class instances with equal fields → "Two counter objects are never `equal to` unless they are the same object. Their *fields* are equal — compare those: `count of a is equal to count of b`. To compare by fields, make the counter `does comparable`."

**Tradeoffs.** Value-vs-reference semantics (R-12) now also touches equality — that is honest: it is the same lesson, taught once, at the same time.

**Should the architecture change? Yes — remove two cmpwords, add the class rule to §8.5 and doc 03.**

---

### R-7 — Non-escaping borrows: the "no lifetime syntax" claim has a hidden cost

**Severity: High (expert tier).**
**Affected area:** [§9.6 of 00](00-architecture.md#96-ownership-and-borrowing--expert), doc 05.

**Why it is a problem.** §9.6 claims "the same rules as Rust's, minus lifetimes-as-syntax — borrows are lexically scoped, no NLL problem." But the *entire reason* Rust needs lifetime syntax is borrows that escape the lexical scope. The canonical case:

```lagom
function longest
    takes a borrowed text called left
    takes a borrowed text called right
    returns a borrowed text        # ⚠ cannot be expressed with lexical borrows:
    …                              #    the result outlives neither, but which one
                                   #    it *is* is a runtime property
```

With strictly lexical borrows, a function **cannot return a borrow of its parameters** — which makes `longest`, `max of list`, borrowed-slice sorting, and every zero-copy view API inexpressible in the owning tier. The expert tier would then be *less* capable than the ARC tier it exists to replace — the exact "worse Rust for experts" trap (risk R10 of 00).

**Who it affects:** expert; the credibility of the whole hybrid-memory story.

**Possible solutions.**
1. Ship lexical-only borrows, documented as a restriction — safe but second-class; every view API falls back to copying or ARC.
2. Infer lifetime-style constraints with no surface syntax (Polonius-style location-sensitive analysis) — the honest eventual answer, but a research-grade checker; not a v1 dependency.
3. **v1: non-escaping borrows (option 1), explicitly documented as the contract, with a benchmarked investigation of escape-analysis-based borrow return as a roadmap item.** Workarounds in v1: return an owned copy, or return an ARC'd class, or take a callback (`longest of a and b into some function` — the continuation style, which needs no lifetime at all).

**Recommended solution.** Option 3. The claim "no NLL problem" must be replaced by the honest statement: *borrows cannot escape the region they are created in; APIs that would return borrows take continuations or return owned values.* This is a real, documented limitation — not a design failure — and it matches what Swift's exclusive-access model and early Rust did.

**Tradeoffs.** Some zero-copy idioms wait for the M4+ investigation; the alternative (shipping inferred lifetimes in v1) is the single largest schedule risk available and contradicts the M0 scope discipline.

**Should the architecture change? Yes — §9.6 and doc 05 must state the restriction and the workaround ladder.**

---

### R-8 — Channel-send semantics are unspecified (move or share?)

**Severity: High.**
**Affected area:** [§14.3](00-architecture.md#143-channels--advanced), [§14.4](00-architecture.md#144-shared-memory-and-race-freedom--advanced), doc 06.

**Why it is a problem.** `send "hello" to messages` — and `a channel of text` generalizes to `a channel of some type`. When the payload is a class instance, what does the receiver get? If a shared ARC reference: two tasks hold the same mutable object, and the race-freedom guarantee ([§14.4 of 00](00-architecture.md#144-shared-memory-and-race-freedom--advanced)) now depends entirely on every field being `shared`+`guard`ed — the compile-time guarantee quietly degrades to a runtime discipline. If a move: the guarantee is structural. Go chose shared pointers (and got data races); Rust chose moves (and got compile-time freedom). Lagom's own D-25 says compile-time prevention — but the doc never states send = move, and `shareable` classes are described only as *also possible*.

**Who it affects:** advanced; the soundness story of the concurrency layer.

**Possible solutions.**
1. Send shares (ARC) unless the type is immutable — the Go-shaped trap.
2. **Send moves.** A send transfers ownership; the sender loses access (checker-enforced). `shareable` classes may instead be sent as shared references *only if* all their fields are `shared`+`guard`ed or immutable — the Rust Send/Sync split, already adopted in §14.4.

**Recommended solution.** Option 2. It makes the student-facing story one sentence ("sending gives it away"), composes with the ownership tier (a channel is the canonical move boundary), and keeps "safe code has no data races" *true* rather than *approximately true*.

**Tradeoffs.** Shared-state-through-channel designs must mark classes `shareable` and use `shared` fields — that is the documented intermediate path (§14.4) anyway.

**Should the architecture change? Yes — one paragraph in §14.3, owned by doc 06.**

---

### R-9 — `shared`-field auto-locking is unsound as written

**Severity: High.**
**Affected area:** [§14.4 of 00](00-architecture.md#144-shared-memory-and-race-freedom--advanced).

**Why it is a problem.** The example says the lock is "held automatically for the field access":

```lagom
class tally
    has shared count of type number guarded by a lock
    can add one
        increase count of myself by 1      # lock held automatically for the field access
```

One statement is safe. But the ordinary two-statement read-modify-write:

```lagom
make current equal to count of myself      # lock acquired and released here
set count of myself to current plus 1      # lock acquired and released here — a race window
```

is two separate lock acquisitions — a textbook lost-update race *inside the model that promises none*. Field-granularity auto-locking cannot be made sound for compound sequences without transaction machinery.

**Who it affects:** intermediate/advanced (the documented shared-state path); the safety claim itself.

**Possible solutions.**
1. Lock per statement — unsound, above.
2. **Lock per method body:** on any class with `shared` fields, entering a method acquires the guard(s) for the fields it touches, held for the whole body; direct field access from *outside* methods is a compile error. Coarse but sound, and trivially explainable ("while a tally method runs, nobody else can touch its count").
3. Explicit `using the lock of count of myself` blocks — expert-only, composable, but makes the *intermediate* path the unsound-by-default one.

**Recommended solution.** Option 2 for the intermediate layer (methods = the unit of atomicity), with option 3 as the expert escape for fine-grained locking inside `unsafe`-adjacent code. This mirrors Java's `synchronized` methods — a model students later meet verbatim.

**Tradeoffs.** Coarse locks can serialize more than necessary — acceptable at this layer; experts have atomics (§14.5) and explicit locks.

**Should the architecture change? Yes — §14.4's rule and example must change.**

---

### R-10 — The student layer cannot convert text to a number (M0 calculator is unbuildable)

**Severity: High (M0-blocking).**
**Affected area:** [§7.1](00-architecture.md#71-output-and-input--student) (`ask` returns text), doc 07 (module catalog), the M0 gate ([§33.1 of 00](00-architecture.md#331-m0--the-language-exists-and-is-real-the-gate-to-everything-else)).

**Why it is a problem.** `ask` returns `text`. The number-guessing game and the calculator — **two of the four M0 validation projects** — need text→number conversion. No conversion function, no `as`-conversion syntax, and no stdlib module section names one. The very first "real" program a student writes hits a wall the architecture never designed a door for. This is exactly the brief's §23 anti-goal ("the language should almost never be the reason a student cannot build something") occurring in the hello-world-adjacent tier.

**Who it affects:** student (day one); the M0 gate itself.

**Possible solutions.**
1. `as number` conversion syntax — rejected for now: `as` is reserved for the `attempt … as problem` binding; overloading it weakens both.
2. **A student-layer parsing API in the `standard` module:** `number from "42"` (can fail — "42x" is a teaching moment about input, not a crash), plus `decimal from "3.5"`, and the inverse `text from 42` / formatting via interpolation. The failing form composes with the error system the student already knows:

```lagom
make answer equal to ask "Pick a number:"
attempt number from answer if it fails then
    say "That was not a number!"
otherwise
    say "You picked {result}."
```

**Recommended solution.** Option 2, specced in doc 07's `standard` module and added to the M0 gate list in §33.1.

**Tradeoffs.** A `can fail` API means the student's first input-handling is explicit — that is a feature, not a cost (it previews the whole error model).

**Should the architecture change? Yes — add to §7.1, doc 07, and the M0 definition-of-done.**

---

### R-11 — The grammar sketch omits every M1–M2 declaration form

**Severity: High.**
**Affected area:** [§7.15 of 00](00-architecture.md#715-formal-grammar-sketch).

**Why it is a problem.** `toplevel = use | function | structure | kind | test | export | doccomment statement`. Missing, despite being specified in prose elsewhere: `class` (§10.2), `interface` (§10.6), type aliases (§8.4's `a type called score is a number` ⚠ — not derivable), generic declarations (§12.2's `some type` containers — `class box of some type` is never shown in any form), the `and pass the problem on` propagation form ([§13.1 of 00](00-architecture.md#131-errors-are-values-failures-are-capabilities--all-layers) — the `?`-equivalent, likely the most-used error form), and the task statement (only a comment). The M2 note at the bottom of the sketch covers only `taskstmt`. Since doc 02 generates parser tests *from the grammar*, these omissions guarantee drift between the prose spec and the tests.

**Who it affects:** compiler developers; the spec's credibility.

**Possible solutions.**
1. Leave them out until M1–M2 — rejected: the whole point of generating tests from the grammar is that the grammar is complete first; retrofitting declaration forms is how special cases breed.
2. Add all of them to the sketch now, marked with their milestone.

**Recommended solution.** Option 2. Proposed productions (each is a mechanical transcription of the prose):

```ebnf
toplevel    = use | function | structure | kind | class | interface
            | typealias | test | export | doccomment statement ;

class       = "class" name ["of" name]                (* generic: class box of some type *)
            [ "extends" name ] [ "does" name { "," name } ] block ;
interface   = "interface" name ["of" name] block ;
typealias   = "a type called" name "is a" type ;

propagate   = "and pass the problem on" ;             (* attempt suffix, §13.1 *)

taskstmt    = "start" "a" "task" block { "start" "a" "task" block } "wait for all tasks" ;
```

(`start another task` — see R-23 — is subsumed by repeating `start a task`; `wait for all tasks` is optional mid-region, and region exit joins implicitly, as §14.2 already states.)

**Tradeoffs.** None — this is completeness, not new design.

**Should the architecture change? Yes.**

---

### R-12 — Structs with class fields quietly break "value semantics by default"

**Severity: Medium-High.**
**Affected area:** [§9.2](00-architecture.md#92-value-semantics-by-default--student), [§7.11](00-architecture.md#711-structs--student), docs 03/05.

**Why it is a problem.** §9.2: "assignment copies (shallow for structs with value fields…)". What about a struct with a *class* field? Shallow copy of an ARC'd reference means:

```lagom
structure player
    has name of type text
    has home of type address        # address is a class

make p1 equal to a player with name "bo" and home …
make p2 equal to p1
set city of home of p2 to "Oslo"
say city of home of p1              # "Oslo" — the "copy" changed the original
```

The beginner's mental model from §9.2 ("this variable *is* that list") is silently false exactly when students start mixing structs and classes (M1–M2). The doc's own claim — "avoids the classic aliasing lesson before week three" — is undermined by the most natural combined example. This is Swift's real semantics (structs copy shallowly; class refs share), and Swift documents it loudly; Lagom currently doesn't.

**Who it affects:** student → intermediate (the value/reference transition — see §11 below).

**Possible solutions.**
1. Deep-copy structs with class fields — rejected: destroys object identity (two "copies" of a file handle), surprises experts, and is unimplementable for cyclic graphs.
2. Shallow copy + **spec it, teach it, lint it**: the compiler emits a student-layer note the first time a struct with a class-typed field is copied ("a player was copied — both players now name the same address object; make address a structure if it should copy").

**Recommended solution.** Option 2, with the lint recommending struct-over-class for pure data (which §10.1 already steers toward). Doc 03/05 own the exact rule; §9.2 gains one paragraph.

**Tradeoffs.** One more thing to teach — but it is *the* concept the curriculum already promises at intermediate (§9.7), now with a compiler guard-rail instead of a mystery.

**Should the architecture change? Yes — spec paragraph + lint; doc 05 owns the full copy/elide matrix.**

---

### R-13 — Recursive data types are unspecified (trees are core CS)

**Severity: Medium-High.**
**Affected area:** [§7.11](00-architecture.md#711-structs--student), [§7.12](00-architecture.md#712-enums-and-sum-types--intermediate), doc 04.

**Why it is a problem.** Nothing states whether this is legal:

```lagom
structure node
    has value of type number
    has children of type a list of node     # node contains a list of node — recursive
```

Trees are the bread of CS-1 and the validation ladder (brief §30 lists tree structures explicitly). The answer *is* derivable from the memory model — a list is a heap handle (COW/ARC'd), so `node` is finite: {i64, handle} — and indirect recursion through lists/maps/text/classes is fine, while *direct* structural recursion (`structure a has b of type b`) is an infinite type and must be rejected. But "derivable by the compiler engineer" is not "specified": the checker needs the rule, the diagnostic needs the wording, and the curriculum needs the guarantee. Kinds (§7.12) need the same rule for `kind tree is a leaf / is a branch with left of type tree…` — wait, that direct case must be *rejected* too (a kind variant holding itself inline is unbounded); the legal spelling goes through `a box of tree`-style indirection or a list.

**Who it affects:** CS student; intermediate; doc 04.

**Possible solutions.** Spec the rule: recursion is legal through any type with a fixed-size handle representation (lists, maps, text, pairs of such, classes, `a box of T`); direct self-embedding in structs/kinds is a compile error with a diagnostic suggesting the box/list indirection.

**Recommended solution.** As stated; add `a box of T` (a single-value heap cell, the `Box`/`ref cell` concept) to the type list — it is currently implied by §12.2's `a box of some type` mention but never defined, and it is the minimal indirection primitive the rule needs.

**Tradeoffs.** One new primitive type (`box`) — justified: it is *the* concept that makes recursive types and later expert-level indirection teachable.

**Should the architecture change? Yes — doc 04 owns it; §7.6/§8.2 gain `a box of T`.**

---

### R-14 — The filler-`of` rule contradicts the no-backtracking claim

**Severity: Medium.**
**Affected area:** [§7.8](00-architecture.md#78-functions-and-capability-clauses--all-layers), [§7.0.3](00-architecture.md#703-multi-word-name-disambiguation-the-main-ambiguity-risk).

**Why it is a problem.** `takes number of correct answers` is parsed by: "the type grammar consumes compound types like `a list of …` greedily first, so an `of` directly after a *complete atomic type* is filler." But whether the type grammar *can* consume the `of` depends on what follows it — for user-defined generic types (`takes a box of numbers called b`) the `of` is type grammar; for `takes number of correct answers` it is filler. Deciding requires either committed lookahead through the whole type expression or backtracking — while §7.0.1 claims "the parser needs no backtracking." The rule as stated is implementable (bounded trial-consumption with rollback = backtracking by another name), but the *claim* is false as written, and false claims in the determinism section are how ambiguity creep starts (risk R1 of 00).

**Who it affects:** compiler developers; spec credibility.

**Possible solutions.**
1. Keep the rule; rewrite the claim: name the mechanism honestly — "the type grammar consumes greedily; on failure of the *name* production the parser re-interprets the trailing `of …` as filler" — i.e., one bounded backtrack point, exhaustively covered by the ambiguity corpus.
2. Remove filler-`of` (`takes number called correct answers`) — rejected: it breaks the brief's target sentence.

**Recommended solution.** Option 1, with the ambiguity corpus gaining every `takes` combination of atomic/compound/generic types × filler/name boundaries.

**Tradeoffs.** One documented backtrack point in `takes` clauses only — tightly contained, and it buys the single most important sentence in the language's marketing (`takes number of correct answers`).

**Should the architecture change? Yes — a wording fix in §7.8/§7.0.1 plus corpus entries.**

---

### R-15 — Literal typing and mixed `number`/`decimal` arithmetic are unspecified

**Severity: Medium.**
**Affected area:** [§8.2](00-architecture.md#82-core-types--student), [§8.1](00-architecture.md#81-inference-first-annotation-optional--student-gets-inference), doc 03/04.

**Why it is a problem.** The averaging pattern — the single most-taught program shape — is:

```lagom
make total equal to 0
repeat for each score in scores
    increase total by score          # scores may be decimals
```

If `0` infers as `number` and `score` is `decimal`, `increase` is a type error — on day one, in the canonical example. The docs specify `number→decimal` promotion for *division* (D-7) and imply it for mixed arithmetic, but never specify **literal typing** (are literals polymorphic?) or the full operator table over mixed operands.

**Who it affects:** student (immediately and repeatedly); doc 03/04.

**Possible solutions.**
1. Literals are typed by inference context, defaulting to `number`; mixed `number`/`decimal` arithmetic promotes to `decimal` (the only implicit conversion, per doc 04's own proposal). An *annotated* `number` accumulator receiving decimals is a compile error that teaches the annotation.
2. Make every numeric literal `decimal` — rejected: then integer loops (`repeat 10 times`) and indices are decimals; worse.

**Recommended solution.** Option 1. This is also the cleanest defense of D-10 (see §5): the one visible type stays visible because the *common* mixed case just works, and the *teachable* case (annotated mismatch) is a diagnostic, not a silent promotion.

**Tradeoffs.** Type inference for literals is context-sensitive — standard machinery (every mainstream language does this); spec it in doc 04's inference section.

**Should the architecture change? Yes — spec paragraph in §8.2; doc 03/04 own the tables.**

---

### R-16 — Negative division and remainder semantics are unspecified

**Severity: Medium.**
**Affected area:** [§7.3](00-architecture.md#73-expressions-and-operators--student), doc 03.

**Why it is a problem.** `divided evenly by` is described as floor division; `remainder of` as modulo. For negatives — `-7 divided evenly by 2` — floor gives `-4`, truncation gives `-3`; school intuition is silent. Unspecified, this becomes the first implementation-driven divergence between backends (a real R6-of-00 hazard, since Cranelift and LLVM may differ on codegen of signed remainder).

**Who it affects:** intermediate/expert; backend consistency.

**Possible solutions.** (a) C-style truncation; (b) Python-style floor + remainder-with-divisor's-sign (`-7 // 2 = -4`, `-7 remainder of 2 = 1`).

**Recommended solution.** (b): floor+sign-of-divisor keeps the pair *consistent* (`(a divided evenly by b) times b plus (a remainder of b) is equal to a` always holds — a checkable law that makes a great lesson), matches the "divide evenly" phrasing, and matches Python — the language students most likely meet next.

**Tradeoffs.** C-programmers may expect truncation; the transfer table (§27.1) notes it. One spec paragraph in doc 03.

**Should the architecture change? Yes — doc 03, one rule + the invariant.**

---

### R-17 — Release-mode overflow is still undecided

**Severity: Medium.**
**Affected area:** [§8.2](00-architecture.md#82-core-types--student), open question 00 §31.8.

**Why it is a problem.** Debug checks overflow; release is TBD (trap/wrap/UB-with-poison). Whichever is chosen silently changes *program results between build modes* for the same source — the one place the "debug/release differ in checks, never semantics" invariant ([§22.4 of 00](00-architecture.md#224-ir-invariants-the-implementation-discipline)) would break.

**Who it affects:** everyone; the determinism story.

**Possible solutions.** (a) wrap in release (Rust's choice — fast, silent); (b) trap in release (Swift's choice for `+` — honest, tiny cost); (c) UB-with-poison (never — contradicts the no-UB policy, doc 03 §8).

**Recommended solution.** (b): trap in release by default, with explicit wrapping ops at the expert layer (`wrapping add`-style, `unsafe`-adjacent) — the cost on x86-64 is a predicted-taken `jo`, measurable in the M1 benchmark the open question already schedules. If the benchmark shows >2% on the numeric suite, revisit with data (the D-8 discipline applied to our own decision).

**Tradeoffs.** A branch on every arithmetic op in hot loops — the honest price of "overflow honesty" (D-10); escapable at the expert layer.

**Should the architecture change? Not yet — this records a recommendation and a decision date (M1), which is what 31.8 asks for.**

---

### R-18 — The option type has three spellings and an unspecified print behavior

**Severity: Medium.**
**Affected area:** [§7.10](00-architecture.md#710-types-as-annotations--intermediate) (`nothing?`), [§8.5](00-architecture.md#85-option-and-result-types--intermediate) (`text?`), [§11.3](00-architecture.md#113-iterators--intermediate) (`a value or done`), [§7.6](00-architecture.md#76-collections--student) (`say first of things`).

**Why it is a problem.** Three notations for one concept in the same document. `nothing?` reads as "optional nothing" — the *inner* type is invisible (is it `text?` or `number?`?); `T?` is the Swift/Kotlin-proven spelling; `a value or done` is the word-based form the syntax philosophy prefers. Also unspecified: what `say first of things` *prints* when the list is empty, and what `{first of things}` interpolates to — the student meets this in §7.6, the first collection lesson.

**Who it affects:** student/intermediate; doc 02's grammar freeze.

**Possible solutions.** (a) `T?` only — compact but the `?` is punctuation the philosophy would rather avoid; (b) word form only (`a text or nothing`) — verbose in signatures; (c) **both, one canonical:** `T?` is the canonical compact form; `a T or nothing` is the canonical word form; they are the same type (one desugaring rule). Define printing: `say`/interpolation of an optional prints the value, or the word `nothing` — honest, Scratch-friendly, and it makes the empty-list case a *visible* lesson instead of a silent blank.

**Recommended solution.** (c), plus the printing rule.

**Tradeoffs.** Two spellings — but they are *declared* as one concept with two costumes, which is the design's own pattern ([§27.2 of 00](00-architecture.md#272-what-lagom-deliberately-teaches-and-how-the-syntax-carries-it)).

**Should the architecture change? Yes — one paragraph in §8.5.**

---

### R-19 — Comptime's implementation story is wrong for type generation

**Severity: Medium.**
**Affected area:** [§16.2 of 00](00-architecture.md#162-compile-time-evaluation--advanced), doc 08.

**Why it is a problem.** §16.2: "Cost model: compile-time interpreter over the MIR." But MIR is *monomorphized and ARC-inserted* ([§22.2 of 00](00-architecture.md#222-mir-semantic-core)) — it exists *after* types are final. Comptime that "generates types (a function returning a type is a type-level program)" must run *before* monomorphization, over HIR or a pre-monomorphization IR with generics intact — the Zig architecture. As written, the doc promises the strongest feature on an IR stage that cannot host it.

**Who it affects:** compiler developers; the M3 comptime milestone.

**Possible solutions.** (a) One comptime over a pre-monomorphization IR; (b) **stage it**: M3 ships *value-level* comptime (lookup tables, validation, unrolling — runs fine over monomorphized MIR, exactly as §16.2 says); M4+ ships *type-level* comptime over HIR, with its own budget and caching story.

**Recommended solution.** (b). It matches the roadmap (comptime at M3), defers the hardest machinery (interleaving evaluation with type checking) behind real experience, and keeps every claim in §16.2 true by scoping it.

**Tradeoffs.** Type-generated APIs wait one milestone — acceptable; the derive-clause menu (16.3) covers the common boilerplate until then.

**Should the architecture change? Yes — §16.2 scoping note; doc 08 owns the two-tier design.**

---

### R-20 — Error-model corners: `attempt` expression-hood, error-kind variance, finalizer failures, C-call capabilities

**Severity: Medium.**
**Affected area:** [§13](00-architecture.md#13-error-handling), [§17.2](00-architecture.md#172-calling-c--advanced), docs 03/04.

**Why it is a problem.** Four small holes, one card because each has a small answer:
1. `attempt` is listed as a statement *and* §7.14 calls it an expression. Can a student write `make doubled equal to (attempt divide of 10 and 0) plus 1`? Unspecified. **Fix:** `attempt` is an expression; in the `if it fails/otherwise` form it evaluates to the branch's value; the bare-propagation form (`and pass the problem on`) is the statement-shaped shortcut. Doc 03 owns the evaluation rule.
2. Error *kinds* have no subtyping, so what does `can fail a file problem` mean for a caller declaring `can fail a text`? Variance/hierarchy rules are unspecified. **Fix:** v1 rule — the caller's declared error type must *be* the callee's (no hierarchy); error *chains* carry causes (§13.3 already has chains); kind-hierarchies for errors are an M3 doc-04 question.
3. Can `before last reference disappears` contain `can fail` calls ([§10.4 of 00](00-architecture.md#104-destructors--finalization--intermediate-concept--advanced-mechanism))? **Fix:** no — finalizers may not fail (a failing cleanup is a bug class, not a feature); attempts inside them must handle fully; the checker enforces it. This also answers the cycle-collector re-entrancy hazard (doc 05's open question) with the strongest available rule.
4. §17.2 wraps a C call in `attempt` — are C calls `can fail` by default? **Fix:** no; C functions return values, period. Errors crossing FFI are explicit: the wrapper declares `can fail` and *constructs* an error from the C return code — visible in the wrapper, invisible to the caller. The §17.2 example should be rewritten accordingly (it currently implies magic capability inference from C).

**Who it affects:** intermediate/advanced; FFI users.

**Should the architecture change? Yes — four spec paragraphs (doc 03 items 1, 7; doc 04; doc 07; §17.2 example).**

---

### R-21 — No fixed-size floats (FFI, SIMD, graphics all need one)

**Severity: Medium.**
**Affected area:** [§18.3](00-architecture.md#183-layout-control-packed-structs-alignment-fixed-size-integers--expert), [§18.5](00-architecture.md#185-simd-and-intrinsics--expert--elite), [§17.2](00-architecture.md#172-calling-c--advanced).

**Why it is a problem.** §18.3 defines fixed-size *integers* only, but §18.5's portable vectors use `4 float numbers packed` ⚠ — a type that is never defined — and C FFI requires `float`/`double` distinctions for every graphics/audio/numerics header. The expert numeric lattice is incomplete.

**Who it affects:** expert (FFI/SIMD); the M4 milestone.

**Recommended solution.** Add `a 32 bit decimal` and `a 64 bit decimal` (the latter an alias of `decimal`) to the expert lattice, same layer enforcement as the fixed-size integers. `4 float numbers packed` becomes `4 of a 32 bit decimal packed`.

**Tradeoffs.** None — pure completeness.

**Should the architecture change? Yes — §18.3, doc 04's type grammar.**

---

### R-22 — The 90/10 debugging goal is asserted, not architected

**Severity: Medium (high value).**
**Affected area:** goal 5 and [§26 of 00](00-architecture.md#26-tooling-strategy), docs 08/09.

**Why it is a problem.** The brief's §10 makes "debugging as explanation" a core goal, and the docs repeatedly promise teaching diagnostics — but nothing in the architecture *produces* the runtime half of the explanation (what was the program doing? which values caused it? where did those values come from?). The diagnostic pipeline (21.4) covers compile-time; the runtime story is a debugger-and-print-statements story, which is exactly what the 90/10 goal says should not be required.

**Who it affects:** student (the primary beneficiary); tooling developers.

**Recommended solution.** Adopt the **Lagom Observability Model (LOM)** as a named subsystem — full proposal in [§10](#10-the-9010-debugging-goal--the-lagom-observability-model) below. Summary: a structured runtime event stream with value provenance, *enabled by default in dev builds, compiled out in release*, plus interpreter-mode full tracing, and failure reports that print the provenance of the failing values. The architecture already contains every hook it needs (MIR owns all loads/stores/ARC ops; errors are structured values; the interpreter backend exists) — what is missing is the decision to make the event stream a *contract* rather than a debugging afterthought.

**Should the architecture change? Yes — new subsection in §26 (or its own doc-09 section), MIR hooks in doc 08, M0/M2 line items in doc 10.**

---

### R-23 — Small example and production gaps (sweep list)

**Severity: Low.** **Affected area:** scattered.

1. §9.2's canonical value-semantics example uses `second of b` ⚠ — `second` is defined nowhere (only `first of`). Fix the example (`make b equal to a; set b at 1 to 99` — derivable) or define `second of` in doc 07.
2. Variadic `takes any number of numbers called values` ([§7.8](00-architecture.md#78-functions-and-capability-clauses--all-layers)) has no production. Fix: `takes = "takes" [articles] (type | "any number of" type) name …`.
3. §7.10's type-name sentence is self-contradictory ("Type names are: … true/false (the boolean type's name is boolean…)"). Fix: the type is `boolean`; `true`/`false` are its values.
4. §14.2's `start another task` hardcodes the second spawn; the third is unspecified. Fix: `start a task` repeats (R-11's production).
5. §7.6 says `say first of things` at the *student* layer, but option types are listed M1 ([§5.2 of 00](00-architecture.md#52-intermediate--intermediate)). Either promote basic optionals (as returned by `first of`) to M0 — they fall out of the error model anyway — or move the example. Recommend the promotion with R-18's printing rule; it makes the empty-list lesson teachable in week two.
6. Doc 09's `lagom explain E1024` example vs `E0003` elsewhere — unify error-code examples.
7. `either` is reserved-but-unused (00 §31.1) — see REJECT list.

**Should the architecture change? Yes — mechanical sweep, listed in §18.**

---

### R-24 — Examined and found sound (recorded to prevent re-litigation)

**Severity: Info.** These were attacked and held:

- **The continuation rule (7.0.1)** — completeness-at-line-end is decidable from the last token; sound.
- **Layer system (§0)** — additive, checker-enforced; no student-layer example was found that requires advanced/expert features *after* the R-10 fix (the `first of` timing note in R-23.5 is the only borderline case).
- **Capability clauses (7.8)** — one mechanism covering four axes; the clause-order determinism claim holds.
- **Errors-as-values (13)** — the fixed `problem`/`result` bindings survive every stress case below once R-20's corners are specced.
- **Value semantics + COW (9.2)** — coherent *as an optimization* once doc 05 owns the copy/elide matrix (R-12's paragraph).
- **One-IR-many-backends + evidence discipline (D-8, 21.2, 22.3)** — the strongest part of the compiler design.
- **Corpus-as-spec self-hosting (29.1)** — sound; the differential harness answers the two-compiler risk.
- **Zero-based indexing (D-9), no-null (8.5), no-truthiness (8.3), no-overloading (D-14)** — all survive; the diagnostics for each are already the right shape.
- **Package security design-from-day-one (28.4)** — no install scripts + signed publishes + audit is the correct minimal set.

---

## 3. Syntax stress tests

Each program from the review brief, attempted *using only the current grammar*, with the reason for any failure. ("Clean" = expressible with grammar-defined forms. "Gap Rx" = blocked by the issue of that number.)

### 3.1 Beginner

| Program | Verdict | Notes |
|---|---|---|
| Hello world | Clean | `say "Hello, world!"` |
| Calculator | **Gap R-10** | No text→number conversion exists; `ask` returns text and arithmetic needs numbers |
| Number guessing game | **Gap R-10** | Same conversion wall; randomness itself is fine (`random` kernel module, doc 07) |
| Quiz | Clean | `ask`, `is equal to`, `if/otherwise`, score accumulation — all derivable |
| Simple list processing | Clean | List literals, `repeat for each`, `set … at … to`, `size of` |
| Simple text processing | Mostly clean | `greeting at 2`, `size of greeting`, interpolation all work; *splitting* text has no named API yet (doc 07's `text` module must list it — not a grammar gap) |

### 3.2 Intermediate

| Program | Verdict | Notes |
|---|---|---|
| Student management system | Clean | Structures + lists + maps + flowing field access |
| File parser | Clean (M1 features) | `files` module planned; `can fail`/`attempt` compose |
| Sorting algorithm | Clean | Index assignment (`set things at i to …`), nested loops, comparisons |
| Binary search | Clean | Same machinery |
| Tree structure | **Gap R-13** | Recursive types unspecified; needs the indirection rule + `a box of T` |
| Generic collection | **Gap R-11** | Generic *declaration* syntax (`class box of some type`) never defined |
| Interface-based program | Clean | `interface`/`does`/static dispatch — but see R-2 for method *call* syntax (`draw shape` not `can draw of shape`) |
| Error-producing function | Clean | `can fail` + `fail with` + `attempt … as problem` all grammatical; propagation form missing (R-11) |

### 3.3 Advanced

| Program | Verdict | Notes |
|---|---|---|
| Async HTTP client | Clean | `can fail` + `can wait` compose; `attempt` handling — the example call itself needs R-1's fix |
| Concurrent producer/consumer | **Gap R-1, R-8** | `send "hello" to messages` not derivable; send semantics (move/share) unspecified |
| Generic data structure | **Gap R-11, R-13** | As above |
| Custom allocator | Clean | `within arena` + allocator types (expert gate) |
| Ownership/borrowing example | Clean *within* the tier; **Gap R-7** at tier boundaries | Non-escaping-borrow restriction must be documented |
| FFI wrapper | Mostly clean | Declarations fine; §17.2's attempt-wrapped C call needs R-20.4's rewrite |
| Compile-time computation | Clean for values | Value-level comptime derivable; type-level comptime mis-staged (R-19) |
| SIMD-oriented algorithm | **Gap R-21** | The vector element type (`float numbers`) is undefined |

### 3.4 Expert

| Program | Verdict | Notes |
|---|---|---|
| Memory-mapped structure | Clean | Volatile pointers + packed C structures (18.7/18.3) |
| Low-level OS API call | Clean | C FFI + fixed-size integers |
| Custom memory allocator | Clean | Allocator interface + owning regions |
| Packed structure | Clean | `is packed to 1 byte` |
| Atomic data structure | Clean | `atomic number` + orderings inside `unsafe` |
| Lock-free algorithm | Clean | Same primitives; unsafe contract is coherent |
| SIMD operation | **Gap R-21** | Element type undefined; portable-vector ops otherwise specified |
| C ABI interface | Clean | `export` + conventions (18.4); example needs the R-20.4 capability rewrite |
| Inline assembly | Clean | `run assembly on x86-64 …` blocks, unsafe-gated |

**Summary:** 24 of 31 stress programs are expressible today; the 7 gaps cluster into five issues (R-1, R-8, R-10, R-11, R-21, R-13) — none of which require new *mechanisms*, only completions of specified ones.

---

## 4. English-ambiguity audit

The closed word set was audited word-by-word for human-vs-parser divergence. Determinism verdicts first, then the five genuine ambiguities (R-1/R-4/R-5/R-3 cover them; this section records the word-level reasoning).

| Word | Jobs today | Verdict |
|---|---|---|
| `and` | boolean · argument separator · construction separator | **Ambiguous in position** — R-4's additive-level rule makes each job position-unique; `bigger of a and b or c` divergence killed. Remaining cost: three jobs, documented, each syntactically distinct. |
| `or` | boolean only | Clean once R-4 holds (`or` never appears inside flow args). |
| `not` | unary boolean | Clean — fixed position in the ladder. |
| `is` | comparison introducer | Clean — `is` + cmpword is a closed pair; no other production starts with `is`. |
| `of` | flow-call prep · type grammar (`a list of`) · filler after atomic type (R-14) · field access (`name of p`) | Position-dependent but each usage is syntactically identifiable except the filler case (R-14's bounded backtrack). Field access vs zero/one-arg calls (`name of p` = call or field?) is resolved by *resolution order* — spec it in doc 03: if `name` is a known zero-arg function, the call wins; else field. Document that order. |
| `with` | labeled args · construction · `with default of` | Clean — all three are suffix positions on calls/clauses; no overlap. |
| `using` | function-valued arg · lambda introducer | Clean after R-3 (one suffix position). Was previously triple-overloaded with memory regions — already fixed (`within owning`/`within arena`). |
| `within` | owning/arena regions | Clean — statement-level only. |
| `for` | `repeat for each … in …` | Clean — fixed phrase. |
| `from` | flow-call prep · map literal opener (`a map from`) · `use cards from "…"` · function types (`a function from …`) | Context-separated (statement/expression/type); the map-literal case is R-5. |
| `to` | flow-call prep · map separator · function types (`… to number`) · `send … to` | **Ambiguous in map literals** — R-5. Elsewhere clean. |
| `by` | `increase … by` | Clean — fixed phrase. |
| `as` | `attempt … as name` | Clean — one job (recommended: do *not* add `as`-conversions, R-10). |
| `called` | parameter naming | Clean — clause position only. |

**The five genuine ambiguities and their smallest deterministic fixes:** (1) positional/two-arg calls missing → R-1's production; (2) greedy-`and` divergence → R-4's additive-level args; (3) map-key `to` collision → R-5's primary-only keys; (4) combinator/lambda extents → R-3's suffix grammar + bounded lambda bodies; (5) filler-`of` vs type grammar → R-14's documented single backtrack point. Every fix is a production-line change; none requires lookahead beyond one token of committed consumption, and none introduces AI parsing.

---

## 5. Type-system stress test

The brief's checklist, with verdicts: primitives ✓; nullable/optional ✓ (R-18 spelling); errors/results ✓ (R-20 corners); **recursive types ✗ (R-13)**; enums/sums ✓; generic types/functions ✓; generic constraints ✓; interfaces ✓; associated behavior — deferred by design (doc 04 open question; sound); function types ✓; closures ✓; recursive functions ✓ (TCO best-effort, 31.4 open); **mutually recursive types — implied by R-13's rule, must be stated**; type aliases ✓ (R-11 production); conversions — doc 04 open question, resolve as "number→decimal implicit only" (R-15); numeric promotion ✓ (D-7 + R-15/R-16); collection variance — invariant default proposed, sound; dynamic dispatch ✓ (interface objects); static dispatch ✓ (monomorphization); FFI types — integers ✓, **floats ✗ (R-21)**; platform-specific types — fixed-size integers ✓.

### The `number` = i64 / `decimal` = f64 decision (D-10) — verdict: **keep, with four spec obligations**

Attacks and outcomes:

1. **Division.** `5 divided by 2` → `2.5` (decimal). This is the *right* beginner default — Scratch and school math agree, and the C-style silent floor-division trap is documented pedagogy poison. The expert exit (`divided evenly by`) is explicit and well-named. **Holds.**
2. **The equality hazard.** Promotion makes float comparison *ubiquitous*: `make average equal to total divided by size of scores` then `if average is equal to 10` — works for 20/2=10.0, silently breaks for 0.1+0.2 cases. This is the real cost of D-7. **Obligation:** `is equal to` on `decimal` operands gets a student-layer lint ("comparing computed decimals is unreliable; compare within a small difference or compare the numbers before dividing") — *not* an error (31.12 already rejects `match` on decimal equality; the lint extends that honesty without blocking). Doc 03 owns it.
3. **Mixed arithmetic and literals.** Unspecified — the day-one accumulator breaks (R-15). **Obligation:** literal unification + promotion table.
4. **Overflow.** Checked-debug is right; release must trap too (R-17) or the honesty claim is build-mode-dependent.
5. **Interoperability/serialization.** i64/f64 map to C and JSON cleanly; the i64 range must be *taught* (the diagnostic for literal overflow writes itself: "9,000,000,000,000,000,000 is bigger than the biggest number Lagom's `number` can hold").
6. **Generic numeric functions.** `function biggest of two numbers` — fine; generic-over-numeric (`some type that does addable`) works via operator interfaces (§10.8). No numeric-tower polymorphism in v1 — correct scope.

**Why not the alternatives:** int/float split from day one recreates the `1/2 = 0` trap the design exists to kill; f64-only (JS-style) breaks counting, indexing, and `repeat` bounds; bignum-by-default costs an allocation per operation and breaks FFI honesty. D-10 with the four obligations is the best available point on the curve. **Remain locked; reopen only the R-17 release-overflow datum at M1.**

---

## 6. Memory-model stress test

The brief's question list, answered against the current text:

1. **What exactly is owned?** Structs/lists/maps/pairs/text are values with COW internals; classes are ARC'd references; owning-region values have a single owner. **The composition matrix (struct containing class, class containing struct, class containing class, list of classes) is unspecified** — R-12 covers the struct-contains-class case; doc 05 must own the full matrix (copy/elide/refcount per cell).
2. **What is refcounted?** Classes always; lists/maps via COW handles; `text` per §8.2. Consequence worth stating: *string-heavy and list-heavy expert code pays refcount traffic unless the owning tier is used* — this is why open question 31.7 (owning-tier byte-buffer strings) matters and should resolve to "yes, provide it."
3. **When does ARC happen?** Construction, capture, assignment of class references, scope exit. Specified at the right altitude (§9.3); exact insertion points are correctly an MIR-pass concern (doc 08).
4. **When can ARC be eliminated?** Escape analysis (§9.5) — the precision contract and the fallback are correctly deferred to doc 05, which must state: **elimination is observable only in performance, never in behavior** (the doc already promises this; keep it normative).
5. **How are cycles detected?** Trial-deletion collector (§9.4). **The interaction with deterministic destruction needs the honesty note:** scope-exit decrements are deterministic; *cycle reclamation* is not — a finalizer in a cycle runs at collection time, not scope time. Docs must not promise both unconditionally. (Fold into the doc-05 spec item that already exists.)
6. **What if the collector is unavailable?** `within owning` regions and the `no runtime` mode (18.7). Coherent — but doc 05 must verify the **whole kernel stdlib** compiles under `no runtime` (lists/maps/text are COW/ARC'd — a no-runtime stdlib needs owning-tier replacements; that is the real M6 work item, currently invisible in doc 10).
7. **Structs containing classes / classes containing structs?** Struct-in-class: the class owns the struct inline (ARC'd object with inline payload) — fine. Class-in-struct: R-12 (shallow copy + teach). Must be specced.
8. **Values crossing function boundaries:** copies (value semantics) — specified; with COW the copy is a handle-bump until write — doc 05's matrix.
9. **Closures capturing values:** capture by reference with ownership rules (§11.1) — for *value* types this means the COW handle is captured (mutable closure state is shared through the handle — matches student intuition); for classes, refcount +1. Spec the exact capture rule per type kind in doc 05 (currently only "ownership rules per §9" — a pointer, not a rule).
10. **Values sent between tasks:** R-8 (move by default).
11. **FFI-owned memory:** `borrowed` vs `owned` marks on `C string` (§17.2) — right shape; doc 05/07 must define the two C-allocation exits (Lagom frees vs C frees) as named conventions in the FFI wrapper API.
12. **C allocates, Lagom must free:** same mechanism — an `owned C string` the wrapper wraps in a RAII `using` type whose `before last reference disappears` calls the C deallocator. Specced enough at the architecture level; doc 07's `database`/`files` wrappers are the proof cases.
13. **Ownership × generics:** owning-region functions may be generic; borrows are non-escaping (R-7) — the interaction is then trivial (no lifetime variances). State it.
14. **Ownership × interfaces:** a `borrowed` value satisfying an interface constraint — allowed for the call's duration; interface *objects* (existentials) from borrows are rejected (they escape). One sentence for doc 05.
15. **Ownership × async:** an owning function that is also `can wait` — the state machine may hold owned values across await points; moves across awaits are checker-visible. Allowed; spec in doc 06.
16. **Ownership × foreign exceptions:** none exist (C has none; errors are values) — the question dissolves. Record that in doc 05 as a deliberate non-issue.
17. **Two ownership systems on one object?** The guard: an ARC'd class inside an owning region keeps its refcount (owning erases *traffic*, not the mechanism) — so the same object is never simultaneously in two discipline regimes; regions change bookkeeping, not ownership topology. This is the sentence §9.1 needs ("the tiers differ in annotations and runtime cost, never in semantics" — correct, and now demonstrable).

**The verdict on the meta-question — can the student model stay simple while the semantics stay systems-grade?** Yes, *with the R-12 teaching rule and the collector-honesty note*: the student model ("values copy, objects are shared and cleaned up when the last name goes away") is exactly true for acyclic scope-held data, which is what students write; the deviations (class-field aliasing, cycle finalizers) are each covered by one taught rule + one lint. That is the correct price.

---

## 7. Concurrency stress test

| Scenario | Verdict | Notes |
|---|---|---|
| Task spawning / completion | Clean | `start a task` + implicit region join; grammar fixed by R-11 |
| Task failure | **Unspecified** | Doc 06's supervision open question must resolve *fail-fast* (child failure fails the region unless `keep going`) — recommended default; matches structured-concurrency precedent |
| Task cancellation | Mostly clean | Every await = cancellation point; `stop waiting` for regions. **Blocked receives:** a task blocked on `receive from messages` must also be cancellable — spec that channel operations are cancellation points too (doc 06) |
| Channels | **Gap R-8** (send semantics) + buffering/close semantics are doc 06 items; `repeat for each` over channels: keep explicit close (doc 06's proposal is right) |
| Shared immutable data | Clean | Values are sendable by copy; ARC'd immutable classes shareable |
| Mutable shared data | **Gap R-9** (lock scope) — as written the model admits the lost-update race |
| Locks | Via `shared`+`guard` only; expert explicit locks inside unsafe — coherent after R-9 |
| Atomics | Clean | `unsafe`-gated, C11-shaped |
| Blocking operations | **Spec needed** | Doc 06's scheduler-contract item: blocking calls on pool threads get detection + diagnostic — keep, and add: the stdlib marks every blocking API, so the checker can warn on `can wait` functions calling them |
| Async operations | Clean | Capability model + state machines (D-17) |
| FFI calls | **Spec needed** | C calls are blocking by definition — a `can wait` function calling C must be flagged (it stalls a pool thread) unless the wrapper provides an async variant; one checker rule, doc 06/07 |
| Task-local state | Unspecified | Recommend: none in v1 (module constants + channel state cover it); a `task local` feature is rejected for now (shared-mutable-state-shaped) |
| Program shutdown | **Spec needed** | Region-join at `main`'s exit gives quiescence; unclosed channels with waiting senders at shutdown = diagnostic. One paragraph, doc 06 |

**Race conditions the type-level model does not yet prevent:** the R-9 lost-update window, and (post-R-8) none — with send=move + `shared`+`guard` methods + unsafe-gated atomics, the model is closed. The two specs above are the difference between "no data races" as a *theorem* and as a *slogan*.

---

## 8. Error-handling stress test

| Case | Verdict | Notes |
|---|---|---|
| Nested failures | Clean after R-20.1 | `attempt` as an expression composes; propagation stays visible in signatures |
| Multiple error types | **Spec needed** | R-20.2: exact-type matching in v1; hierarchies at M3 |
| Generic errors | Clean | `can fail` untyped = any error; constrain via kinds at intermediate |
| Async errors | Clean | `can fail` + `can wait` are orthogonal clauses; `attempt` is the single handling form |
| Task errors | Clean after doc-06 supervision default | Region failure surfaces as the region's `attempt` |
| FFI errors | Spec R-20.4 | Wrapper-constructed errors, no magic |
| Resource cleanup | Clean | `using` regions + deterministic ARC destruction compose with failure (region exit on failure — specified in §10.4) |
| Partial initialization | Clean | The Swift rule (§10.3) kills it at compile time |
| Errors in finalizers | Spec R-20.3 | Finalizers cannot fail |
| Ignored results | Mostly clean | Every `can fail` call must be `attempt`-wrapped (compiler-enforced — the key guarantee holds); a lint for discarded non-failure results is optional |
| Propagation through many functions | Clean | `and pass the problem on` (R-11 production) is the readable `?`; the chain is visible in every signature — the deliberate loss (no stack-trace-style propagation) is documented in D-12 and remains correct for teaching and auditing |

**Readability vs hidden control flow:** the model survives every case without exceptions. The one readability risk is *nested* `attempt` expressions (R-20.1) — mitigate by linting deeply nested attempts toward named intermediate bindings (the same advice the FP section gives for nested combinators — consistent house style).

---

## 9. Compiler-architecture stress test

The pipeline (lexer → parser → AST → sema → HIR → MIR → LIR → backends) was evaluated against each required capability:

| Capability | Supported by the design? | Notes |
|---|---|---|
| Type inference | Yes | Local at M0 (D-11); literal unification must be added (R-15) |
| Generics + monomorphization | Yes | MIR-stage monomorphization (22.2); **incrementality interaction** — see below |
| ARC insertion | Yes | MIR pass (22.2); elimination pass specified |
| Ownership checking | Yes | Sema region check (21.1) — R-7's contract must be stated |
| Async lowering | Yes | MIR state machines (15.2/22.2) |
| Structured concurrency | Yes | Region join lowers to scoped spawn/join in MIR |
| Compile-time evaluation | Partially | R-19: value-level over MIR (as written), type-level needs pre-mono IR — stage it |
| Closures | Yes | Capture lowering (22.2) |
| Optimization | Yes | Tiered ladder (24.2) |
| Incremental compilation | Yes | Salsa-style from M0 (24.2) — the right call; see monomorphization note |
| Debug information | Yes | Both backends emit DWARF |
| Rich diagnostics | Yes | Structured diagnostic type + spans through desugaring — **HIR must retain source spans for every desugared node** (interpolation, flow calls, clauses); add to doc 08's HIR contract |
| Cross-compilation | Yes | Target-agnostic IR discipline (23.4) |
| FFI | Yes | LIR calling conventions (22.3) |
| SIMD | Yes | Portable vectors + intrinsics via LLVM; Cranelift debug path exempt (documented) |
| Unsafe | Yes | Regions + sanitizer (18.2) |

**Missing passes/IRs identified:** none structural. Two interactions need explicit treatment in doc 08:

1. **Monomorphization × incremental compilation.** A new instantiation of a generic re-monoplizes that function — the classic recompile cascade. Doc 08 should state the policy: per-instantiation query caching (the rustc approach), with the option of a shared generic fallback (compiled once, dynamic dispatch) for cold instantiations to bound binary size — an M2 decision with a benchmark.
2. **Comptime × IR staging.** R-19's two-tier split.

**Verdict: the architecture stands.** The one-LIR/backends-clean interface (22.3) is what makes the Cranelift+LLVM+interpreter trio tractable, and the interpreter-from-M0 decision keeps the test loop fast until backends land.

---

## 10. The 90/10 debugging goal → the Lagom Observability Model

The goal: debugging is *explanation*, not investigation — the system answers what/where/why/what-was-the-program-doing/which-values/where-did-they-come-from/how-to-fix/what-concept — without scattering `say` statements, and **without taxing production builds**.

The current architecture enables the compile-time half (teaching diagnostics) but leaves the runtime half unspecified. The fix is not a tool — it is a **model**: a contract between the compiler, the runtime, and the tools, named so it can be specced, tested, and taught:

### The Lagom Observability Model (LOM)

**Principle: observability is a build mode, not a runtime cost you opt out of — development builds explain; release builds are silent.**

Three tiers, keyed off the existing build modes (24.2):

1. **Dev builds (default) — structured events with provenance.** The MIR instrumentation pass (already the owner of every load/store/ARC/move) emits, *only in dev mode*: function entry/exit with argument snapshots, task spawn/join/cancel events, failure events with the full error value, and — the differentiator — **value provenance**: every binding records *where its value came from* (which expression, which previous binding). Storage: a bounded in-memory ring (fixed budget, oldest evicted — no unbounded recording, ever). Zero release cost: the pass does not run.
2. **The failure report (the 90/10 payoff).** On unhandled failure or panic, the dev runtime prints, from the ring: the call chain in source words, each frame's argument values, and the **provenance chain of the failing values** — "bottom was 0, bound at line 12 (`make bottom equal to size of names`), names became empty at line 9 after the `stop`." That is answers 1–7 of the goal from *one* artifact the student never had to build. Concept-linking (answer 8) comes from the existing `lagom explain` pipeline: the failure's error kind links to its lesson.
3. **Interpreter mode — full tracing and replay.** `lagom play` and `lagom run --trace` run on the interpreter backend, where per-step value history is free. This is the teaching mode (and the deterministic-replay story for M0–M2: re-run the recorded event ring deterministically — the language's strict evaluation makes this straightforward).

**Explicit non-goals:** no always-on production tracing; no recording of every value in release; no heisenbugs-from-instrumentation (release binaries are bit-identical to non-LOM builds — testable, like the reproducible-builds rule).

**What each doc must add:** doc 08 — the instrumentation pass + ring format + "release builds contain zero LOM code" invariant; doc 09 — the failure-report format (designed like diagnostics, snapshot-tested); doc 06 — task events; doc 10 — M0: failure report v1 (interpreter); M2: MIR instrumentation; M3: provenance in native dev builds; M4: replay tooling.

**Why a named model matters:** it makes "rich debugging ↔ runtime performance" a *decided* contradiction (§16) instead of a perpetual negotiation, and it gives the 90/10 goal a testable definition: *any unhandled failure in a dev build must produce a report that names the failing value's origin without the programmer adding code.*

---

## 11. Educational-progression audit

Tracing the Scratch → systems-programmer ladder for jumps larger than one concept:

1. **`say`/`ask`/`make` → functions.** Smooth; capability clauses start as `takes`/`returns` vocabulary the student already reads.
2. **Values → references (structs → classes).** **The biggest jump — and R-12 makes it bigger than advertised.** The doc's ordering claim ("avoids the aliasing lesson before week three") is currently false for struct-with-class-field copies. With R-12's lint + the R-6 equality rule, the lesson arrives once, guarded, and in student words. Without them, the student meets three behaviors (structs copy, classes share, COW lists look-like-copy) with no compiler guidance — the exact "fundamentally different from mainstream" confusion the brief warns about. **Required: R-12, R-6, and a curriculum note that the value/reference lesson is taught *with* the struct/class boundary (which is where Python/JS students meet it too — transfer preserved).**
3. **Options/errors.** Smooth — one tagged-union concept, three costumes (§13.4); the R-18 printing rule prevents the "what did `say` just print?" moment.
4. **`anything` inference → explicit generics.** Smooth by design (12.1→12.2 is the same idea with a name).
5. **Sequentials → tasks/channels.** Smooth *for the message-passing path* (start/wait/send/receive is one new verb set). The jump to `shared`+`guard` is the second-biggest — R-9's method-body rule keeps it one rule ("methods lock their object") instead of a memory-model seminar.
6. **ARC intuition → owning regions.** The R-7 restriction actually *helps* pedagogy: "in this region you own everything outright; if you lend it, you get it back before the call returns" is a complete, honest rule. The escape-ladder (copy / ARC / continuation) is teachable.
7. **Lagom → other languages.** The bridge table (§27.1) holds after the R-1/R-2 fixes; the two deliberate divergences a student must be *told* about are method-call syntax (R-2: "methods take the object as their first argument" — true in Go/Lua/Python's `self` too) and `number`-promotion on division (true in Scratch and Python 3 — and the table already maps it). No divergence found that misleads a student about a *concept* — the guardrail (27.4) holds.

**One curriculum addition recommended:** a two-page "what the machine is doing" interlude at the intermediate boundary (values copy; objects are shared; a refcount is a tally of names) — §9.7 already promises this content; the review confirms it is load-bearing, not optional.

---

## 12. "Can Lagom actually build this?"

| Category | Status | Path / blocker |
|---|---|---|
| CLI applications | **Now (M0–M1)** | Args, files, processes in stdlib |
| Web servers | Planned M2–M3 | Tasks+channels+`can wait`+`http`; needs the R-8/R-9 specs to be sound |
| Databases | Planned M4–M5 | Owning regions, FFI, atomics, monomorphized generics — all specified; the R-7 borrow contract is the one to watch for zero-copy B-tree code |
| Game engines | Planned M4–M5 | Deterministic destruction (no GC pauses), SIMD (R-21), FFI to graphics APIs |
| Compilers / interpreters | **Now (dogfood) → M5** | Self-hosting plan is sound; comptime staging (R-19) fits |
| Operating-system components | M6 | Freestanding target, no-runtime mode, inline asm, linker control — specified; **blocker to verify:** the kernel stdlib subset under `no runtime` (see §6.6) |
| Embedded software | M6 | Same as above + allocator-free mode |
| Networking software | Planned M2–M3 | Channels/async/FFI |
| High-performance numerical software | Planned M4–M5 | Portable vectors + intrinsics + LLVM auto-vectorization; `decimal`=f64 is the right base; R-21 adds f32 |
| Distributed systems | Planned M3+ | Networking + comptime serialization; channels = the message-passing substrate |
| Graphics software | Escape hatch → M4 | FFI/Vulkan bindings; stdlib window/input hooks M4 |
| GUI applications | Package (post-1.0 flagship) | By design (D-16); hooks + FFI make it buildable |

**Architectural blockers: none absolute.** The two conditional ones — owning-region expressiveness (R-7) and the no-runtime stdlib subset — are both spec-and-verify items, not design dead ends. The claim discipline (D-8, 25.1) correctly prevents the docs from asserting more than this table.

---

## 13. Accidental-complexity review

Applying "if we removed this, what capability disappears?":

| Feature | If removed, what disappears? | Verdict |
|---|---|---|
| `same as` / `different from` cmpwords | Nothing — `equal to` + the R-6 rule covers both meanings | **Remove** (R-6) |
| `either` (reserved, unused) | Nothing | **Remove** (reserving words without jobs is how R1-of-00 creep starts) |
| Inline lambda `taking … giving back …` | Multi-line-free combinator pipelines | Keep — with R-3's bounded bodies |
| `it` implicit parameter | Half of all combinator calls' readability | Keep |
| The filler-`of` (`takes number of correct answers`) | The brief's signature sentence | Keep — R-14 contains its cost |
| Fixed `problem`/`result` bindings | The zero-syntax first error lesson | Keep |
| `within owning` + `within arena` as *two* region words | Zero-cost expert code / bulk allocation | Keep — one word (`within`) with a mode argument was the alternative; two words is clearer and grammar-cheaper |
| Capability clauses vs separate `throws`/`async` | The entire uniformity thesis | Keep |
| Trial-deletion cycle collector | Student-tier cycle reclamation | Keep — R3-of-00 already owns the cost |
| Comptime type-generation (unbounded) | Some API-generating ergonomics | **Stage** (R-19) — the value-level tier keeps most of the value |
| Four equality words | — | Two (R-6) |
| `no runtime` mode | Kernel/embedded story | Keep — it is the escape hatch that makes 12's last rows honest |

No feature failed the test outright; the removals are all *vocabulary*, which is exactly where accidental complexity should be paid down.

---

## 14. Missing-capability classification

| Capability | Class | Notes |
|---|---|---|
| Text→number conversion | **Required (M0)** | R-10 |
| Pair destructuring in `make` (`make x and y equal to …`) | **Required (M1)** | Patterns exist only in `match`; pairs are M0 but can only be *built*, not taken apart outside a match — a hole in the student layer |
| Recursive types + `a box of T` | **Required (M1)** | R-13 — trees |
| f32 / fixed-size decimals | **Required (M3–M4)** | R-21 — FFI/SIMD/graphics |
| Channel send semantics | **Required (M2)** | R-8 |
| Method-call syntax | **Required (M1–M2)** | R-2 |
| String split/join/trim/contains | **Required (M1)** | Stdlib `text` module — name them in doc 07 |
| Weak references | Useful (M3, advanced) | doc 05's proposal — caches and observers |
| Generators | Optional | Iterators + tasks cover the use cases; coroutines-for-iteration add a second async-shaped machinery — reject until proven |
| Custom operators | Unnecessary | D-log already rejects; the operator table stays four |
| Dependency injection | Unnecessary | Interfaces + constructors + module constants cover it; DI frameworks are a symptom, not a feature |
| JIT support | Optional/dangerous | Correctly positioned as FFI/plugin-era (25.1); keep out of the runtime model |
| Compile-time reflection | Required (M3, via comptime) | Already planned |
| Runtime reflection | Optional, opt-in | Already planned (16.4) — keep opt-in for size/security |
| Benchmarking/profiling | Required (tooling) | Planned — LOM (R-22) extends profiling with provenance |
| Property testing/fuzzing | Required (M3) | Planned |
| ABI/linker control | Required (M5–M6) | Planned (18.4/18.6) |
| Effect system | Dangerous complexity | D-20 holds — fixed capabilities only |
| Dependent/refinement types | Dangerous complexity (for v1) | D-19 holds |
| Macro system | Dangerous complexity | D-18 holds; derive-clauses are the right substitute |

---

## 15. Decision-log audit (D-1 … D-29)

| # | Decision | Verdict | Notes |
|---|---|---|---|
| D-1 | Name | Keep | R9-of-00 screening stands |
| D-2 | One language, layered exposure | Keep | No conflicts found; layer audit clean |
| D-3 | Word-based, formal grammar | Keep | R-1..R-5 are *completions* of this decision, not challenges to it |
| D-4 | Indentation blocks | Keep | Continuation rule sound (R-24) |
| D-5 | Immutable by default | Keep | |
| D-6 | Words for comparisons | Keep | R-6 trims two redundant words — in the decision's spirit |
| D-7 | Division words | Keep + refine | R-16 (negative semantics), R-15 (promotion table), decimal-equality lint (§5.2) |
| D-8 | Backend pairing, evidence-driven | Keep | Unanimously the most defensible decision in the doc |
| D-9 | 0-based indexing | Keep | |
| D-10 | number=i64, decimal promotion | Keep + obligations | §5 — four spec duties (R-15, R-16, R-17, equality lint) |
| D-11 | Local inference first | Keep | R-15 adds literal unification — still local |
| D-12 | Errors as values | Keep | R-11 adds the propagation production; R-20 the corners |
| D-13 | Hybrid ARC+ownership+collector | Keep | R-7 (borrow contract), R-12 (copy matrix), §6 honesty note |
| D-14 | No overloading | Keep | Named constructions survive all stress cases |
| D-15 | Single inheritance, discouraged | Keep | |
| D-16 | Batteries stdlib as packages | Keep | |
| D-17 | Capability async, state machines | Keep | Cancellation/blocking specs in §7 complete it |
| D-18 | comptime, no macros | Keep + stage | R-19 |
| D-19 | No dependent/refinement types | Keep | |
| D-20 | Fixed capabilities, no effect system | Keep | |
| D-21 | Rust implementation | Keep | |
| D-22 | Library-first, query-incremental | Keep | + monomorphization/incrementality policy note (§9) |
| D-23 | Ownership opt-in regions | Keep + contract | R-7 |
| D-24 | Tasks+channels first | Keep | R-8 completes it |
| D-25 | Compile-time race prevention | Keep + fix | R-9's unsoundness is a spec bug, not a design failure |
| D-26 | Package supply-chain design | Keep | |
| D-27 | Self-host at M5, differential | Keep | |
| D-28 | Real-user validation each milestone | Keep | |
| D-29 | No chaining word | Keep | Nested-combinator readability stays an M3+ question (31.13) |

**None reopened.** Nine refined with spec obligations; the refinement list is §18.

---

## 16. Contradiction matrix

| Decision A | Decision B | Potential conflict | Severity | Resolution |
|---|---|---|---|---|
| English-like syntax (D-3) | Formal grammar (§7.15) | **Examples not derivable from the EBNF** (R-1) | Critical | Fix the production to match the prose; corpus pins it |
| Formal grammar | "No backtracking" (7.0.1) | Filler-`of` needs bounded trial-consumption (R-14) | Medium | State the one backtrack point honestly |
| Type inference (D-11) | Diagnostics quality (goal 5) | Literal typing unspecified → wrong first error (R-15) | Medium | Literal unification + promotion table |
| ARC (9.3) | Ownership tier (9.6) | Borrow escape impossible without lifetimes (R-7) | High | Non-escaping contract + documented workaround ladder |
| ARC (9.3) | Deterministic destruction (10.4) | Cycle finalizers run late (§6.5) | Medium | Honesty note + finalizer rules (R-20.3) |
| Value semantics (9.2) | Classes are references (9.3) | Struct-with-class-field copies alias (R-12) | Med-High | Shallow-copy rule + lint + lesson placement |
| Ownership (9.6) | Concurrency (14.4) | Send semantics unspecified (R-8) | High | Send = move |
| Race freedom (D-25) | `shared`/`guard` auto-lock (14.4) | Field-granularity locks admit lost updates (R-9) | High | Lock scope = method body |
| Generics (12) | Interfaces (10.6) | Associated types deferred | Low | Already a doc-04 open question — sound |
| Async (15) | Errors (13) | `attempt` expression-vs-statement (R-20.1) | Medium | Attempt is an expression |
| Async (15) | Memory (9) | Captures across awaits | Low | MIR state machines own it (22.2); spec in doc 06 |
| FFI (17) | Memory safety (9) | C allocation ownership marks | Medium | Named conventions in doc 05/07 (§6.11–12) |
| FFI (17) | Capabilities (13) | C-call can-fail inference (R-20.4) | Medium | Wrappers construct errors explicitly |
| Unsafe (18) | Safe defaults (goal 8) | — | — | No conflict: `because` lint + manifest declaration hold (R-24) |
| Comptime (16.2) | Compilation speed (goal 6) | Unbounded type-generation builds (R-19) | Medium | Stage value/type tiers + budgets |
| Rich debugging (goal 5/§10) | Runtime performance (goal 7) | Instrumentation vs zero-cost release | High → **resolved** | LOM: dev-only instrumentation, release identical (R-22) |
| Cranelift (21.2) | LLVM (21.2) | Semantics drift between backends | Medium | One LIR + differential testing (R6-of-00) — stands |
| Incremental compilation (24.2) | Monomorphization (22.2) | Re-instantiation cascades | Medium | Per-instantiation query caching policy (§9) |
| Beginner simplicity (goal 2) | Expert capability (goal 9) | Owning-tier API expressiveness (R-7); no-runtime stdlib (§6.6) | High | Documented contracts + roadmap verifications — honest, not hidden |
| Grammar freeze (doc 02) | Vocabulary growth (doc 01) | — | — | Process conflict only; the approval process doc 01 proposes is adequate |

---

## 17. Final verdict

### KEEP

One language with layered exposure (D-2); the word-based *formal* grammar approach with the closed preposition set (D-3); indentation + the continuation rule (D-4); immutable-by-default (D-5); capability clauses as the one function syntax (7.8/D-12's mechanism); errors-as-values with fixed bindings (D-12); the hybrid memory model *shape* — values + ARC + collector + opt-in owning + unsafe (D-13); single discouraged inheritance and composition-first OOP (D-15/10.1); batteries-stdlib-as-packages (D-16); capability async over state machines (D-17); comptime-not-macros (D-18); no dependent types/effects/macros in v1 (D-19/D-20); Rust bootstrap (D-21); library-first query-incremental compiler (D-22); tasks+channels-first concurrency (D-24); compile-time race prevention *as a goal* (D-25 — with R-9's fix); supply-chain design (D-26); M5 differential self-hosting (D-27); real-user validation (D-28); no chaining word (D-29); Cranelift+LLVM over one LIR with the evidence discipline (D-8); zero-based indexing, no null, no truthiness, no overloading (D-9, 8.5, 8.3, D-14); the layer system as checker-enforced presentation (§0).

### MODIFY

The flow-call grammar — extend to the unified production (R-1); method invocation — ordinary calls with receiver-as-argument, delete `.` (R-2); the call suffix grammar + bounded lambda bodies (R-3); flow-call arguments at additive level (R-4); map keys as primaries (R-5); equality — two behaviors, one operator, drop `same as`/`different from` (R-6); the owning-region contract — state non-escaping borrows (R-7); channel send = move (R-8); lock scope = method body (R-9); student text→number API (R-10); complete the grammar sketch with all M1–M2 forms (R-11); shallow-copy-with-lint for class fields (R-12); recursive-type rule + `a box of T` (R-13); the no-backtracking claim (R-14); literal typing + promotion tables (R-15); negative division (R-16); release overflow recommendation (R-17); option spelling + printing (R-18); comptime staging (R-19); the four error corners (R-20); fixed-size decimals (R-21); the LOM adoption (R-22); the R-23 sweep.

### INVESTIGATE

1. **Borrow-escape analysis** (R-7 follow-up): can an escape-analysis-based checker permit parameter-borrow returns at acceptable complexity? Benchmark against the continuation workaround on real systems code (M4).
2. **Release-mode overflow cost** (R-17): measure trap-vs-wrap on the M1 numeric suite before freezing.
3. **Monomorphization/incrementality policy** (§9): per-instantiation caching vs shared-generic fallbacks, with binary-size and compile-time benchmarks (M2).
4. **Type-level comptime** (R-19): the HIR-interleaving evaluator's complexity, after value-level comptime has real users (M4).
5. **Flow-call `and` ergonomics** (31.2 of 00): keep the M0-review decision point — the corpus now has sharper cases (R-4's divergence) to test with.
6. **No-runtime stdlib subset** (§6.6): which kernel modules get owning-tier reimplementations for M6 (doc 05 + doc 10).
7. **`lagom explain` auto-escalation** (doc 09 open question): unchanged.

### REJECT

1. **`same as` / `different from`** as distinct equality operators (R-6).
2. **`either`** as a reserved-but-unused word (31.1 — delete at the v0.1 freeze).
3. **Dot syntax** (`drawing.circle`, `counter.new`) — remove; ordinary-call method invocation covers it (R-2).
4. **Field-granularity auto-locking** as specified (R-9) — replaced by method-body locking.
5. **`can name of target` method-call spelling** (R-2) — replaced.
6. **Type-level comptime at M3** (R-19) — value-level first.
7. **`as`-style conversion syntax** (R-10) — parsing stays a `can fail` API.

---

## 18. Recommended architecture revision

Exact changes, by document. (Issue IDs in parentheses.)

**`docs/00-architecture.md`**
1. §7.15: replace `flowcall` with the unified call grammar (R-1); add `where`/`using`/`with` suffixes and the lambda production (R-3); restrict `maplit` keys to primaries (R-5); add `class`, `interface`, `typealias`, `propagate`, and the corrected `taskstmt` productions (R-11); remove `same as`/`different from` from `cmpword` (R-6); add `a box of T` to the type grammar (R-13).
2. §7.9: state the additive-level argument rule and the parenthesization diagnostic (R-4).
3. §7.8: variadic production (R-23.2); honest description of the filler-`of` mechanism (R-14).
4. §7.1: add `number from`/`decimal from` parsing + the M0 note (R-10).
5. §7.6: fix the `second of` example (R-23.1); promote basic optionals to M0 with the printing rule (R-18, R-23.5).
6. §7.13: replace `drawing.circle` (R-2).
7. §8.2/§8.5: literal typing + promotion paragraph (R-15); option spelling canon + printing (R-18); equality semantics incl. class identity (R-6).
8. §9.2: shallow-copy-of-class-fields paragraph + lint (R-12); §9.6: non-escaping-borrow contract + workaround ladder (R-7).
9. §10.2/§10.7: method-call examples → ordinary calls; delete `counter.new` (R-2).
10. §13.1: add `and pass the problem on` example; §13: finalizers-cannot-fail rule (R-20).
11. §14.3/§14.4: send = move paragraph (R-8); lock-scope = method-body rule + fixed example (R-9).
12. §16.2: value-level vs type-level comptime staging (R-19).
13. §17.2: rewrite the FFI attempt example per R-20.4.
14. §18.3: add `a 32 bit decimal`/`a 64 bit decimal` (R-21); §18.5: fix the vector element type.
15. §26: add the Lagom Observability Model subsection (R-22, full proposal in [§10](#10-the-9010-debugging-goal--the-lagom-observability-model) of this review).
16. §31: update open questions (delete 31.1's `either` per REJECT; fold R-17's recommendation into 31.8 with an M1 date).
17. §34: append D-30 (equality semantics), D-31 (send = move), D-32 (LOM), D-33 (comptime staging), D-34 (option canon), D-35 (release overflow recommendation).

**`docs/01-philosophy.md`** — no changes required; the vocabulary-approval open question gains "deletions are also vocabulary decisions" (R-6, REJECT list).

**`docs/02-syntax.md`** — scope item 4 (ambiguity corpus) gains the R-1/R-4/R-5/R-14 case lists; the multi-line-string and match-guard open questions stand.

**`docs/03-semantics.md`** — numeric semantics: negative division + the division invariant (R-16); literal typing/promotion table (R-15); decimal-equality lint (§5.2); equality semantics chapter (R-6); overflow recommendation with M1 date (R-17); attempt-as-expression rule (R-20.1); capture rules pointer to doc 05 (§6.9).

**`docs/04-types.md`** — recursive-type rule + `a box of T` (R-13); error-kind variance (R-20.2); fixed-size decimals (R-21); `C` types in the type grammar; conversion ladder incl. text→number (R-10); literal unification in the inference spec (R-15).

**`docs/05-memory.md`** — the copy/elide matrix incl. struct-contains-class (R-12); non-escaping-borrow contract + INVESTIGATE item (R-7); finalizer/cycle rules (R-20.3, §6.5); capture rules per type kind (§6.9); FFI ownership conventions (§6.11–12); no-runtime kernel-stdlib verification item (§6.6); weak references stand.

**`docs/06-concurrency.md`** — send = move (R-8); method-body locking (R-9); fail-fast supervision default; channel operations as cancellation points; blocking-FFI checker rule; shutdown semantics; task-local-state rejection note (§7).

**`docs/07-stdlib.md`** — `standard` gains `number from`/`decimal from` (R-10); `text` module names split/join/trim/contains; FFI wrapper error-construction pattern (R-20.4).

**`docs/08-compiler.md`** — HIR span-retention contract; LOM instrumentation pass + release-identity invariant (R-22); comptime two-tier design (R-19); monomorphization/incrementality policy note (§9).

**`docs/09-tooling.md`** — failure-report format (LOM tier 2) as a designed artifact; unify error-code examples (R-23.6).

**`docs/10-roadmap.md`** — M0 gate gains the text→number requirement and the failure-report-v1 item (R-10, R-22); M1 gains the R-17 benchmark datum; M2 gains the monomorphization-policy decision; M3 comptime scoped to value-level (R-19); M6 gains the no-runtime stdlib verification (§6.6).

**`README.md`** — add doc 11 to the documentation table.

---

## 19. Verification report

1. **Internal links** — all `#…` anchors in this document resolve to headings present above (slug-verified).
2. **Cross-document links** — every link to 00/03/04/05/06/07/08/09/10 targets a heading that exists (verified against the heading inventory used by the prior verification pass, including the §7.0.3 and §18.3 slug corrections made there).
3. **Grammar-consistency of examples** — every Lagom snippet in this document is either derivable from the *revised* grammar proposed here, or is explicitly marked **⚠** as a quotation of a current defect.
4. **Keywords** — no snippet uses vocabulary outside the grammar sketch plus the productions this review proposes (flagged where it does).
5. **Layer tags** — the layer audit (R-24) found every user-facing feature tagged; the only untagged areas are compiler-internal sections (21–25), which have no user layer by nature.
6. **Student-layer purity** — one borderline case found (R-23.5, `first of` timing) with a recommended resolution; no student example requires advanced/expert features.
7. **Decision-log consistency** — §15 audits all 29 decisions; the revisions propose D-30…D-35 for the new commitments.
8. **Cross-document contradictions** — §16's matrix; the two High rows (R-8, R-9) have resolutions written above.
9. **Code-fence balance** — verified for all 12 documents *during this review*; the pass found and repaired **three stray/duplicated fence lines in 00-architecture.md** (the doubled opener at §9.6 and stray closers at §9.6/§10.4 — remnants of the earlier failed editing session) that were corrupting the rendering of §9.6–§9.8 and §10.4.
10. **What changed in the repo during this review:** (a) `docs/11-design-review.md` created (this document); (b) the three fence repairs in `docs/00-architecture.md` listed above — mechanical corrections, no content changed; (c) `README.md` documentation-table row for this review. **No other specification text was altered** — §18's revisions are recommendations awaiting approval, per the review's terms.

**Bottom line:** the architecture is *conceptually* ready to implement its student core (M0) once R-1, R-10, and the R-11 grammar completions land — all documentation-level changes. The intermediate/advanced tiers are ready after the R-6/R-8/R-9/R-12 specs. The expert tier ships honestly only with R-7's contract written down. Nothing found requires redesigning the architecture; everything found requires *completing* it.
