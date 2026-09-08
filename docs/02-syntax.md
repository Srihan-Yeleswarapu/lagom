# Phase 2 — Syntax Specification

*Status: stub — the complete syntax is drafted in [§7 of 00-architecture.md](00-architecture.md#7-proposed-syntax); this document will hold the normative, machine-readable specification.*
*Next review: at M0 planning.*

## Scope of this document (when filled in)

1. **Normative grammar** — the full EBNF (growing from [§7.15 of 00](00-architecture.md#715-formal-grammar-sketch)) in machine-readable form, *generated into the parser test corpus* so the grammar and the tests cannot drift.
2. **Lexical specification** — exact token rules, the indentation scanner's state machine, the continuation rule (7.0.1) formalized, the reserved-word list (`lagom words` output is generated from this document).
3. **Per-feature syntax chapters** — for every feature in [§5 of 00](00-architecture.md#5-feature-matrix): student-readable syntax, advanced syntax, internal representation (AST node), equivalents in ≥2 established languages, parsing considerations, type-system considerations, compiler implications, potential ambiguities, alternative designs, and the comparison that justified the choice (the brief's §21 checklist, one chapter per feature).
4. **Ambiguity corpus** — the curated set of programs that pin down every ambiguity decision (multi-word names, flow calls, continuation lines); each entry states the *one* intended parse and why. **Required entries (doc 11):** the R-4 divergence case (`bigger of a and b or c` — pinned as a compile error with the two-reading diagnostic); every `takes` combination of atomic/compound/generic types × filler-`of`/name boundaries (the grammar's one documented backtrack point); map-literal keys (`home to work` is a pair, never a call); every flow-call shape (bare positional, `and`-separated, prepositional, suffixed); lambda-body extents (`it`/`taking … giving back …` vs the block form); and every phrase-token boundary (`and pass the problem on`, `wait for all tasks` — no name may contain them).
5. **Grammar evolution policy** — the freeze-per-release rule, the deprecation process, the vocabulary-approval process (links to doc 01's open questions).

## Decisions inherited from 00 (binding here)

- D-3, D-4 (indentation blocks), D-6 (words canonical, symbols aliased), D-7 (division words), D-9 (0-based), D-14 (no overloading), plus the closed-preposition-set and capability-clause rules ([§6.3](00-architecture.md#63-layered-vocabulary-rule-the-anti-bloat-mechanism), [§7.9](00-architecture.md#79-readable-call-syntax--student), [§7.15](00-architecture.md#715-formal-grammar-sketch)).
- **From the design review (doc 11), applied 2026-09:** D-30 (equality — one operator, `same as`/`different from` deleted), D-34 (option spelling canon), D-36 (methods = ordinary calls, no dot syntax), D-37 (unified call grammar with additive-level arguments, `with`/`using`/`where` suffixes, bounded lambda bodies), D-39 (text↔number as `can fail` API). The §7.15 sketch is now complete through the M1–M2 declaration forms (`class`, `interface`, `typealias`, method/construction clauses, `attempt` tails, task/unsafe/region statements) so parser tests generated from it cover the whole language.

## Open questions (with decision dates)

- Flow-call fallback `bigger of a, b` — decide at M0 review from corpus evidence (00 §31.2).
- ~~`either` keyword~~ — **resolved:** deleted at v0.1 (REJECT list, doc 11); the reserved-word list shrinks accordingly.
- Multi-line string literal indentation semantics (leading-whitespace stripping rule) — spec at M0.
- `match` guard syntax (`when … when also …`?) — spec at M1 with pattern design.
- Reserved-word list update for the applied fixes: `same as`, `different from`, `either` out; `it`, `where`, phrase-tokens (`and pass the problem on`, `wait for all tasks`) in — fold into the v0.1 freeze.

## Cross-references

- [00-architecture §7](00-architecture.md#7-proposed-syntax) · [03-semantics](03-semantics.md) · [09-tooling](09-tooling.md) (formatter = canonical syntax)
