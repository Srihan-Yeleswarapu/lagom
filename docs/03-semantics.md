# Phase 3 — Semantic Specification

*Status: stub — semantics are asserted throughout [00-architecture.md](00-architecture.md); this document will define precisely what every construct means.*
*Next review: at M0 planning.*

## Scope of this document (when filled in)

1. **Per-construct semantics** — one chapter per construct ([§7 of 00](00-architecture.md#7-proposed-syntax)): evaluation order, value vs reference behavior, error behavior, edge cases (overflow, empty inputs, empty text), and the reference IR lowering (HIR → MIR) for each.
2. **Evaluation model** — strict evaluation; expression/statement boundary ([§7.14](00-architecture.md#714-everything-is-an-expression-boundary)); short-circuit `and`/`or`; argument evaluation order (left-to-right, specified); the exact `say`/`ask`/formatting behavior.
3. **Numeric semantics** — the `number` (i64) ↔ `decimal` (f64) promotion rules for every operator, incl. `divided by` promotion (D-7); **literal typing** (polymorphic literals unify by context, default `number`) and the mixed-arithmetic promotion table (D-37's obligations; doc 04 owns the table); overflow behavior per build mode (debug = checked failure; release = D-35's recommendation is trap, decided with the M1 benchmark — [00 §31.8](00-architecture.md#31-major-unresolved-questions)); **negative division:** floor division with remainder taking the divisor's sign (Python pair), preserving the law `(a divided evenly by b) times b plus (a remainder of b) is equal to a`; decimal equality in `match` (currently rejected, 00 §31.12) plus the student-layer decimal-comparison lint (doc 11 §5); rounding and formatting rules for `say`.
4. **Equality semantics** — **one operator, two behaviors (D-30):** `is equal to` is structural for value types and identity for classes; **the rule applies recursively through containers — a list/map/pair of classes compares elements by identity, so container equality never deep-compares a class**; text comparison is code-point order; structural comparison of classes is opt-in via `does comparable`; the teaching diagnostic for the class case points at field comparison and `does comparable`.
5. **Scope and name resolution** — lexical scoping rules, shadowing rules (allowed? propose: allowed, with a student-layer lint), module name spaces, the greedy-multi-word-name algorithm formalized ([§7.0.3 of 00](00-architecture.md#703-multi-word-name-disambiguation-the-main-ambiguity-risk)).
6. **Lifetime semantics of values** — when copies happen (value semantics, [§9.2 of 00](00-architecture.md#92-value-semantics-by-default--student)); when ARC traffic happens (classes, [§9.3](00-architecture.md#93-classes-are-arcd-references--student--advanced)); move semantics in owning regions ([§9.6](00-architecture.md#96-ownership-and-borrowing--expert)).
7. **Failure semantics** — `can fail` propagation rules, `attempt` forms ([§13 of 00](00-architecture.md#13-error-handling)); **`attempt` is an expression** (the bare form evaluates to the success value; the `if it fails/otherwise` form evaluates to the branch's value — R-20.1); **exact-type error matching in v1** (a caller's declared error type must *be* the callee's; kind-hierarchies are an M3 doc-04 question — R-20.2); **finalizers cannot fail** (R-20.3); **C calls never implicitly `can fail`** — wrappers construct errors explicitly (R-20.4, D-39 of the FFI pattern); panics vs failures (Go's distinction); unhandled-failure runtime behavior and its teaching diagnostic — **the LOM failure report ([00 §26.5](00-architecture.md#265-the-lagom-observability-model-lom--the-9010-debugging-contract)) is the D-32 contract for dev builds**.
8. **Undefined behavior policy** — safe Lagom has none, full stop; the list of operations that are traps (not UB) in safe code; the unsafe contract ([§18.2 of 00](00-architecture.md#182-the-safety-contract)).

## Decisions inherited from 00 (binding here)

- D-7 (division semantics), D-10 (`number` = i64, promotion on true division), D-12 (errors as values), D-9 (0-based indexing), the strict-evaluation and no-truthiness rules ([§8.3](00-architecture.md#83-static-checking-with-dynamic-feeling-errors)).
- **From the design review (doc 11), applied 2026-09:** D-30 (equality semantics), D-33 (comptime staging — value-level over MIR at M3, type-level over HIR at M4+), D-34 (option printing rule), D-35 (release-overflow recommendation: trap, decided with the M1 datum), D-37's literal-typing/promotion obligations, and the four error corners (attempt-as-expression, exact-type errors, finalizers-cannot-fail, FFI wrapper error construction).

## Open questions

- Release-mode overflow: trap vs wrap — **recommendation locked (D-35: trap)**; the M1 numeric-suite benchmark decides whether the cost is acceptable or the recommendation is revisited with data (00 §31.8).
- Are `and`/`or` results operands (Python-style) or strictly `boolean`? (Propose: strictly boolean — transfer to C-family `&&`/`||` short-circuit *behavior* is preserved, result type is cleaner; decide at M0.)
- Exact fractional-text formatting for `decimal` in `say` (shortest-roundtrip vs fixed) — spec at M1.
- Error-kind hierarchies for `can fail a file problem`-style declarations (v1 = exact type only) — design at M3 with doc 04 (R-20.2).

## Cross-references

- [02-syntax](02-syntax.md) · [04-types](04-types.md) · [05-memory](05-memory.md)
