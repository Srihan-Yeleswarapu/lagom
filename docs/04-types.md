# Phase 4 — Type System

*Status: stub — the type-system architecture is set in [§8 and §12 of 00-architecture.md](00-architecture.md#8-type-system); this document will hold the complete type-system specification.*
*Next review: at M0 planning.*

## Scope of this document (when filled in)

1. **Type grammar** — full type-expression syntax including generics (`some type`), capabilities in function types (`a function from text to text that can fail`), **option spellings (`T?` = `a T or nothing`, one type — D-34)**, **`a box of T` (heap cell — the indirection primitive that makes recursive types finitely representable, R-13)**, and the expert fixed-size integers **and fixed-size decimals (`a 32 bit decimal`, `a 64 bit decimal` — R-21)** ([§18.3 of 00](00-architecture.md#183-layout-control-packed-structs-alignment-fixed-size-integers--expert)).
2. **Recursive-type rule (R-13)** — recursion is legal through any type with a fixed-size handle representation (lists, maps, text, pairs of such, classes, `a box of T`); direct self-embedding in structs/kinds is a compile error whose diagnostic suggests the box/list indirection; mutually recursive types follow the same rule. The tree example (`has children of type a list of node`) must type-check.
3. **Type rules per construct** — assignability, subtyping rules (structural for interfaces, nominal for user types), variance rules for generic containers (invariant by default; the covariance audit for read-only views), the `anything` unification algorithm ([§12.1 of 00](00-architecture.md#121-implicit-generics--intermediate)).
4. **Inference specification** — the M0 local-inference rules (D-11) written precisely **including literal unification (polymorphic numeric literals, D-37's obligation — the accumulator pattern must infer)**; the annotation-requirements table (where inference refuses); the HM-style upgrade path if the corpus demands it ([00 §31.3](00-architecture.md#31-major-unresolved-questions)).
5. **Option and result integration** — option layout and niche optimization; the two canonical spellings as one type (D-34); flow-sensitive narrowing rules ([§8.5 of 00](00-architecture.md#85-option-and-result-types--intermediate)); the exact interaction with `attempt` ([§13](00-architecture.md#13-error-handling)).
6. **Interfaces and generics** — interface satisfaction rules (including operator interfaces, [§10.8](00-architecture.md#108-operator-overloading--intermediate)); constraint checking; static vs existential dispatch ([§12.3](00-architecture.md#123-constraints--intermediate--advanced)); monomorphization policy and its compile-time cost control (specialization limits, shared generic fallbacks).
7. **Ownership × types (doc 05 contracts, stated here too)** — `borrowed` values satisfy interface constraints for the call's duration; existential interface objects built from borrows are rejected (they escape — R-7's contract).
8. **Exhaustiveness and patterns** — the pattern grammar and type-checking rules; exhaustiveness algorithm; unreachable-case diagnostics.
9. **Type aliases and equality** — structural vs nominal identity per kind of type; alias transparency; the equality operator's two behaviors (D-30: structural for values, identity for classes).
10. **The expert integer lattice** — the full fixed-size integer type set, conversion rules, and the checker's enforcement of "expert-only types stay in expert regions" ([§0 of 00](00-architecture.md#0-how-to-read-this-document-layers)).
11. **Error-kind variance (R-20.2)** — v1 rule: the caller's declared error type must *be* the callee's (no hierarchy); chains carry causes; kind-hierarchies for errors are an M3 question (doc 03 open question).
12. **Conversion ladder (R-10/D-39)** — implicit: `number→decimal` only (mixed arithmetic promotes); explicit APIs: `number from text` / `decimal from text` (`can fail`), `text from number`; **no `as`-conversion syntax** (`as` is reserved for `attempt … as problem`).

## Decisions inherited from 00 (binding here)

- D-10 (one visible `number` type), D-11 (inference strategy), D-15 (interfaces as the substitution mechanism), D-19 (no dependent/refinement types in v1), D-14 (no overloading — interfaces + named constructions instead).
- **From the design review (doc 11), applied 2026-09:** D-30 (equality semantics), D-34 (option spelling canon), D-37 (literal unification obligation), D-39 (conversion ladder, no `as`-conversions); the recursive-type rule and `a box of T` (R-13); fixed-size decimals (R-21); error-kind exact-type matching in v1 (R-20.2).

## Open questions

- Variance defaults for `a map from K to V` in student code — propose: invariant, with diagnostics that explain why in student words; decide at M2.
- Do interfaces support associated types (`does container of some type`)? Propose: defer to M3; the generics doc owns the design; links to trait-object soundness rules.
- Numeric conversion ladder — **resolved:** implicit `number→decimal` only; parsing is `can fail` API (`number from text`, D-39); no `as`-conversion syntax (doc 02/03 own the tables).
- Error-kind hierarchies for `can fail a file problem`-style declarations — v1 = exact-type matching; design at M3 (owned jointly with doc 03; R-20.2).

## Cross-references

- [03-semantics](03-semantics.md) · [08-compiler](08-compiler.md) (monomorphization passes) · [02-syntax](02-syntax.md) (type grammar)
