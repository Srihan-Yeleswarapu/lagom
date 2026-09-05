# Phase 2 — Syntax Specification

*Status: stub — the complete syntax is drafted in [§7 of 00-architecture.md](00-architecture.md#7-proposed-syntax); this document will hold the normative, machine-readable specification.*
*Next review: at M0 planning.*

## Scope of this document (when filled in)

1. **Normative grammar** — the full EBNF (growing from [§7.15 of 00](00-architecture.md#715-formal-grammar-sketch)) in machine-readable form, *generated into the parser test corpus* so the grammar and the tests cannot drift.
2. **Lexical specification** — exact token rules, the indentation scanner's state machine, the continuation rule (7.0.1) formalized, the reserved-word list (`lagom words` output is generated from this document).
3. **Per-feature syntax chapters** — for every feature in [§5 of 00](00-architecture.md#5-feature-matrix): student-readable syntax, advanced syntax, internal representation (AST node), equivalents in ≥2 established languages, parsing considerations, type-system considerations, compiler implications, potential ambiguities, alternative designs, and the comparison that justified the choice (the brief's §21 checklist, one chapter per feature).
4. **Ambiguity corpus** — the curated set of programs that pin down every ambiguity decision (multi-word names, flow calls, continuation lines); each entry states the *one* intended parse and why.
5. **Grammar evolution policy** — the freeze-per-release rule, the deprecation process, the vocabulary-approval process (links to doc 01's open questions).

## Decisions inherited from 00 (binding here)

- D-3, D-4 (indentation blocks), D-6 (words canonical, symbols aliased), D-7 (division words), D-9 (0-based), D-14 (no overloading), plus the closed-preposition-set and capability-clause rules ([§6.3](00-architecture.md#63-layered-vocabulary-rule-the-anti-bloat-mechanism), [§7.9](00-architecture.md#79-readable-call-syntax--student), [§7.15](00-architecture.md#715-formal-grammar-sketch)).

## Open questions (with decision dates)

- Flow-call fallback `bigger of a, b` — decide at M0 review from corpus evidence (00 §31.2).
- `either` keyword: keep reserved or delete — v0.1 spec freeze (00 §31.1).
- Multi-line string literal indentation semantics (leading-whitespace stripping rule) — spec at M0.
- `match` guard syntax (`when … when also …`?) — spec at M1 with pattern design.

## Cross-references

- [00-architecture §7](00-architecture.md#7-proposed-syntax) · [03-semantics](03-semantics.md) · [09-tooling](09-tooling.md) (formatter = canonical syntax)
