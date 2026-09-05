# Phase 3 — Semantic Specification

*Status: stub — semantics are asserted throughout [00-architecture.md](00-architecture.md); this document will define precisely what every construct means.*
*Next review: at M0 planning.*

## Scope of this document (when filled in)

1. **Per-construct semantics** — one chapter per construct ([§7 of 00](00-architecture.md#7-proposed-syntax)): evaluation order, value vs reference behavior, error behavior, edge cases (overflow, empty inputs, empty text), and the reference IR lowering (HIR → MIR) for each.
2. **Evaluation model** — strict evaluation; expression/statement boundary ([§7.14](00-architecture.md#714-everything-is-an-expression-boundary)); short-circuit `and`/`or`; argument evaluation order (left-to-right, specified); the exact `say`/`ask`/formatting behavior.
3. **Numeric semantics** — the `number` (i64) ↔ `decimal` (f64) promotion rules for every operator, incl. `divided by` promotion (D-7); overflow behavior per build mode (debug = checked failure; release = TBD, open question [00 §31.8](00-architecture.md#31-major-unresolved-questions)); decimal equality in `match` (currently rejected, 00 §31.12); rounding and formatting rules for `say`.
4. **Equality semantics** — structural equality for values, identity for classes (spec the exact rule); text comparison (code-point order); the `is equal to`/`is the same as` distinction (structural vs identity).
5. **Scope and name resolution** — lexical scoping rules, shadowing rules (allowed? propose: allowed, with a student-layer lint), module name spaces, the greedy-multi-word-name algorithm formalized ([§7.0.3 of 00](00-architecture.md#703-multi-word-name-disambiguation-the-main-ambiguity-risk)).
6. **Lifetime semantics of values** — when copies happen (value semantics, [§9.2 of 00](00-architecture.md#92-value-semantics-by-default--student)); when ARC traffic happens (classes, [§9.3](00-architecture.md#93-classes-are-arcd-references--student--advanced)); move semantics in owning regions ([§9.6](00-architecture.md#96-ownership-and-borrowing--expert)).
7. **Failure semantics** — `can fail` propagation rules, `attempt` forms ([§13 of 00](00-architecture.md#13-error-handling)), panics vs failures (Go's distinction), unhandled-failure runtime behavior and its teaching diagnostic.
8. **Undefined behavior policy** — safe Lagom has none, full stop; the list of operations that are traps (not UB) in safe code; the unsafe contract ([§18.2 of 00](00-architecture.md#182-the-safety-contract)).

## Decisions inherited from 00 (binding here)

- D-7 (division semantics), D-10 (`number` = i64, promotion on true division), D-12 (errors as values), D-9 (0-based indexing), the strict-evaluation and no-truthiness rules ([§8.3](00-architecture.md#83-static-checking-with-dynamic-feeling-errors)).

## Open questions

- Release-mode overflow: trap vs wrap vs poison (00 §31.8) — decide at M1 with benchmark input.
- Are `and`/`or` results operands (Python-style) or strictly `boolean`? (Propose: strictly boolean — transfer to C-family `&&`/`||` short-circuit *behavior* is preserved, result type is cleaner; decide at M0.)
- Exact fractional-text formatting for `decimal` in `say` (shortest-roundtrip vs fixed) — spec at M1.

## Cross-references

- [02-syntax](02-syntax.md) · [04-types](04-types.md) · [05-memory](05-memory.md)
