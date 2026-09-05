# Phase 5 — Memory Model

*Status: stub — the memory architecture is set in [§9 and §18 of 00-architecture.md](00-architecture.md#9-memory-management); this document will hold the complete memory-model specification.*
*Next review: at M0 planning.*

## Scope of this document (when filled in)

1. **The two-tier contract** — exact semantics of Tier S (student: value semantics + ARC + cycle collector) and Tier E (expert: owning regions + borrow rules + unsafe) ([§9 of 00](00-architecture.md#9-memory-management)); the guarantee that tiers change *annotations and runtime cost*, never *program results*.
2. **Value-semantics rules** — copy-on-assignment rules per type (structs, lists, maps, pairs, text); the M0 copy-on-write implementation for lists/maps vs the M2 final design; the aliasing lesson's placement in the curriculum ([§9.2](00-architecture.md#92-value-semantics-by-default--student), [§9.7](00-architecture.md#97-what-beginners-learn-about-memory-anyway)).
3. **ARC specification** — refcount layout, increment/decrement insertion points (MIR pass, [§22.2 of 00](00-architecture.md#22-ir-design)), weak references (needed? propose: `a weak reference to some class`, advanced layer, M3), `before last reference disappears` execution guarantees and re-entrancy rules ([§10.4](00-architecture.md#104-destructors--finalization--intermediate-concept--advanced-mechanism)).
4. **Cycle collector specification** — trial-deletion algorithm parameters, trigger points (allocation pressure + explicit `collect cycles`), pause-time bounds, the interaction with `before last reference disappears` (finalizers must be cycle-safe — spec the rule), and the per-region disable switch ([§9.4 of 00](00-architecture.md#94-cycle-collector--student-layer-only)).
5. **Escape analysis and stack promotion** — the analysis's precision contract (what it must prove), the fallback behavior when analysis fails, and the observable differences (if any) between promoted and non-promoted code — propose: none (semantics-preserving by construction).
6. **Owning regions and borrow checking** — `owned`/`borrowed` parameter modes, lexical-borrow rules (no NLL at v1), move semantics, the error diagnostics for borrow violations (teaching-mode examples), and the region syntax grammar ([§9.6 of 00](00-architecture.md#96-ownership-and-borrowing--expert)).
7. **Allocators and arenas** — the allocator interface (`allocate`, `deallocate`, `allocator` types), region syntax (`within arena …`; the `using` word is reserved for disposable-resource scopes, 00 §10.4), stdlib container allocator parameters ([§9.8 of 00](00-architecture.md#98-allocators-and-arenas--expert)), and the no-runtime embedded mode ([§18.7](00-architecture.md#187-memory-mapped-io-and-embedded--elite)).
8. **Unsafe memory operations** — the full raw-pointer API, alignment/packing rules ([§18.3](00-architecture.md#183-layout-control-packed-structs-alignment-fixed-size-integers--expert)), the unsafe-sanitizer behavior in debug builds ([§18.2](00-architecture.md#182-the-safety-contract)), and the `because` lint.

## Decisions inherited from 00 (binding here)

- D-13 (hybrid ownership + ARC + cycle collector), D-23 (ownership opt-in by regions), the no-hidden-finalizers rule ([§10.4](00-architecture.md#104-destructors--finalization--intermediate-concept--advanced-mechanism)), D-16's thin-kernel/batteries split as it applies to allocator-aware containers.

## Open questions

- Is `text` ARC'd-immutable at every tier, or does the owning tier get a byte-buffer string type? (00 §31.7 — decide at M3–M4 with the ownership spec.)
- Cycle-collector default trigger policy for long-running student programs (propose: allocation-pressure threshold + explicit call; decide at M2 with real usage data).
- Weak references: student-visible ("a weak reference") or expert-only? (Propose: advanced layer, M3; decide when the ARC spec is written.)

## Cross-references

- [03-semantics](03-semantics.md) (copy/move/failure semantics) · [08-compiler](08-compiler.md) (ARC/drop/escape passes) · [06-concurrency](06-concurrency.md) (`sendable`/`shareable` interplay)
