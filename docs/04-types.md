# Phase 4 — Type System

*Status: stub — the type-system architecture is set in [§8 and §12 of 00-architecture.md](00-architecture.md#8-type-system); this document will hold the complete type-system specification.*
*Next review: at M0 planning.*

## Scope of this document (when filled in)

1. **Type grammar** — full type-expression syntax including generics (`some type`), capabilities in function types (`a function from text to text that can fail`), and the expert fixed-size integers ([§18.3 of 00](00-architecture.md#183-layout-control-packed-structs-alignment-fixed-size-integers--expert)).
2. **Type rules per construct** — assignability, subtyping rules (structural for interfaces, nominal for user types), variance rules for generic containers (invariant by default; the covariance audit for read-only views), the `anything` unification algorithm ([§12.1 of 00](00-architecture.md#121-implicit-generics--intermediate)).
3. **Inference specification** — the M0 local-inference rules (D-11) written precisely; the annotation-requirements table (where inference refuses); the HM-style upgrade path if the corpus demands it ([00 §31.3](00-architecture.md#31-major-unresolved-questions)).
4. **Option and result integration** — `nothing?` layout and niche optimization; flow-sensitive narrowing rules ([§8.5 of 00](00-architecture.md#85-option-and-result-types--intermediate)); the exact interaction with `attempt` ([§13](00-architecture.md#13-error-handling)).
5. **Interfaces and generics** — interface satisfaction rules (including operator interfaces, [§10.8](00-architecture.md#108-operator-overloading--intermediate)); constraint checking; static vs existential dispatch ([§12.3](00-architecture.md#123-constraints--intermediate--advanced)); monomorphization policy and its compile-time cost control (specialization limits, shared generic fallbacks).
6. **Exhaustiveness and patterns** — the pattern grammar and type-checking rules; exhaustiveness algorithm; unreachable-case diagnostics.
7. **Type aliases and equality** — structural vs nominal identity per kind of type; alias transparency.
8. **The expert integer lattice** — the full fixed-size integer type set, conversion rules, and the checker's enforcement of "expert-only types stay in expert regions" ([§0 of 00](00-architecture.md#0-how-to-read-this-document-layers)).

## Decisions inherited from 00 (binding here)

- D-10 (one visible `number` type), D-11 (inference strategy), D-15 (interfaces as the substitution mechanism), D-19 (no dependent/refinement types in v1), D-14 (no overloading — interfaces + named constructions instead).

## Open questions

- Variance defaults for `a map from K to V` in student code — propose: invariant, with diagnostics that explain why in student words; decide at M2.
- Do interfaces support associated types (`does container of some type`)? Propose: defer to M3; the generics doc owns the design; links to trait-object soundness rules.
- Numeric conversion ladder: which conversions are implicit (propose: number→decimal only) vs explicit (`as decimal`? syntax TBD with doc 02).

## Cross-references

- [03-semantics](03-semantics.md) · [08-compiler](08-compiler.md) (monomorphization passes) · [02-syntax](02-syntax.md) (type grammar)
