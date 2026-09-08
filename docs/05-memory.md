# Phase 5 — Memory Model

*Status: stub — the memory architecture is set in [§9 and §18 of 00-architecture.md](00-architecture.md#9-memory-management); this document will hold the complete memory-model specification.*
*Next review: at M0 planning.*

## Scope of this document (when filled in)

1. **The two-tier contract** — exact semantics of Tier S (student: value semantics + ARC + cycle collector) and Tier E (expert: owning regions + borrow rules + unsafe) ([§9 of 00](00-architecture.md#9-memory-management)); the guarantee that tiers change *annotations and runtime cost*, never *program results*.
2. **Value-semantics rules** — copy-on-assignment rules per type (structs, lists, maps, pairs, text); the M0 copy-on-write implementation for lists/maps vs the M2 final design; the aliasing lesson's placement in the curriculum ([§9.2](00-architecture.md#92-value-semantics-by-default--student), [§9.7](00-architecture.md#97-what-beginners-learn-about-memory-anyway)).
3. **The copy/elide matrix (R-12)** — the full composition table (struct-in-class, class-in-struct, class-in-class, list-of-classes; copy/elide/refcount per cell) with the spec'd shallow-copy rule for structs with class fields: the struct is copied, the reference is copied with it, both structs name the same object; the one-time student-layer note and the struct-over-class lint ([§9.2 of 00](00-architecture.md#92-value-semantics-by-default--student)).
4. **Capture rules per type kind (R-24 §6.9)** — what a closure captures for values (the COW handle — mutable closure state is shared through it) vs classes (refcount +1); currently only a pointer in [§11.1 of 00](00-architecture.md#111-closures-and-higher-order-functions--intermediate) — this document owns the exact rule.
5. **FFI ownership conventions (R-20.4)** — the two C-allocation exits as named conventions (Lagom frees vs C frees), the `owned`/`borrowed` marks on `C string`, and the RAII `using`-type pattern whose `before last reference disappears` calls the C deallocator.
6. **ARC specification** — refcount layout, increment/decrement insertion points (MIR pass, [§22.2 of 00](00-architecture.md#22-ir-design)), weak references (needed? propose: `a weak reference to some class`, advanced layer, M3), `before last reference disappears` execution guarantees and re-entrancy rules ([§10.4](00-architecture.md#104-destructors--finalization--intermediate-concept--advanced-mechanism)) — including the **finalizers-cannot-fail rule (R-20.3)** and the cycle-finalizer honesty note (below).
7. **Cycle collector specification** — trial-deletion algorithm parameters, trigger points (allocation pressure + explicit `collect cycles`), pause-time bounds, the interaction with `before last reference disappears` (finalizers must be cycle-safe — spec the rule), and the per-region disable switch ([§9.4 of 00](00-architecture.md#94-cycle-collector--student-layer-only)). **The honesty note (doc 11 §6.5):** scope-exit decrements are deterministic; *cycle reclamation* is not — a finalizer in a cycle runs at collection time, not scope time. Docs must not promise both unconditionally.
8. **Escape analysis and stack promotion** — the analysis's precision contract (what it must prove), the fallback behavior when analysis fails, and the observable differences (if any) between promoted and non-promoted code — propose: none (semantics-preserving by construction).
9. **Owning regions and borrow checking** — `owned`/`borrowed` parameter modes, lexical-borrow rules (no NLL at v1), move semantics, the error diagnostics for borrow violations (teaching-mode examples), and the region syntax grammar ([§9.6 of 00](00-architecture.md#96-ownership-and-borrowing--expert)). **Owns the non-escaping-borrow contract (R-7):** borrows never outlive their creating region; borrow-returning APIs are inexpressible by design; the workaround ladder (owned copy / ARC'd class / continuation) is documented; the escape-analysis investigation is the M4 INVESTIGATE item.
10. **Allocators and arenas** — the allocator interface (`allocate`, `deallocate`, `allocator` types), region syntax (`within arena …`; the `using` word is reserved for disposable-resource scopes, 00 §10.4), stdlib container allocator parameters ([§9.8 of 00](00-architecture.md#98-allocators-and-arenas--expert)), and the no-runtime embedded mode ([§18.7](00-architecture.md#187-memory-mapped-io-and-embedded--elite)).
11. **Unsafe memory operations** — the full raw-pointer API, alignment/packing rules ([§18.3](00-architecture.md#183-layout-control-packed-structs-alignment-fixed-size-integers--expert)), the unsafe-sanitizer behavior in debug builds ([§18.2](00-architecture.md#182-the-safety-contract)), and the `because` lint.
12. **The no-runtime verification item (doc 11 §6.6)** — verify the whole kernel stdlib compiles under `no runtime` (lists/maps/text are COW/ARC'd — a no-runtime stdlib needs owning-tier replacements); the real M6 work item, currently invisible in the roadmap.
13. **Two-ownership-systems guard (doc 11 §6.17)** — an ARC'd class inside an owning region keeps its refcount (owning erases *traffic*, not the mechanism); the same object is never simultaneously in two discipline regimes. State as a normative sentence ([§9.1 of 00](00-architecture.md#91-the-two-tier-answer)).
14. **Deliberate non-issue** — foreign exceptions do not exist (C has none; errors are values); recorded so the question is not re-opened.

## Decisions inherited from 00 (binding here)

- D-13 (hybrid ownership + ARC + cycle collector), D-23 (ownership opt-in by regions), the no-hidden-finalizers rule ([§10.4](00-architecture.md#104-destructors--finalization--intermediate-concept--advanced-mechanism)), D-16's thin-kernel/batteries split as it applies to allocator-aware containers.
- **From the design review (doc 11), applied 2026-09:** the non-escaping-borrow contract (R-7), the shallow-copy-with-lint rule and copy/elide matrix (R-12), finalizers-cannot-fail (R-20.3), the capture rules per type kind, the FFI ownership conventions, the no-runtime kernel-stdlib verification item, and the two-ownership-systems guard.

## Open questions

- Is `text` ARC'd-immutable at every tier, or does the owning tier get a byte-buffer string type? (00 §31.7 — decide at M3–M4 with the ownership spec.)
- Cycle-collector default trigger policy for long-running student programs (propose: allocation-pressure threshold + explicit call; decide at M2 with real usage data).
- Weak references: student-visible ("a weak reference") or expert-only? (Propose: advanced layer, M3; decide when the ARC spec is written.)

## Cross-references

- [03-semantics](03-semantics.md) (copy/move/failure semantics) · [08-compiler](08-compiler.md) (ARC/drop/escape passes) · [06-concurrency](06-concurrency.md) (`sendable`/`shareable` interplay)
