# Phase 8 — Compiler Architecture

*Status: stub — the compiler architecture is set in [§21–§25 of 00-architecture.md](00-architecture.md#21-compiler-architecture); this document will hold the complete compiler specification.*
*Next review: at M0 planning.*

## Scope of this document (when filled in)

1. **Pipeline specification** — stage-by-stage contracts ([§21.1 of 00](00-architecture.md#211-pipeline)): lexer (hand-written, SIMD-friendly), parser (recursive descent + indentation scanner, error recovery with region commits), AST (arena allocation, lossless mode for formatter/LSP), sema (name resolution, type check, capability check, exhaustiveness, region check), HIR, MIR, LIR, backends.
2. **IR specifications** — HIR/MIR/LIR data structures, invariants, and verifiers ([§22 of 00](00-architecture.md#22-ir-design)); the debug-vs-release check-insertion policy ([§22.4](00-architecture.md#224-ir-invariants-the-implementation-discipline)); **the HIR span-retention contract — every desugared node (interpolation, flow calls, capability clauses, `where`-sugar) keeps its source span** (rich diagnostics depend on it, doc 11 §9); monomorphization policy and cost control; ARC/drop/escape-analysis/closure/async-state-machine pass order and contracts.
3. **The LOM instrumentation pass (D-32, R-22)** — the pass that emits dev-build structured events with value provenance (function entry/exit snapshots, task events, failure events, per-binding provenance) into a bounded ring; owns the **release-identity invariant: release builds are bit-identical to non-LOM builds** (a tested invariant, like reproducible builds) — [00 §26.5](00-architecture.md#265-the-lagom-observability-model-lom--the-9010-debugging-contract).
4. **Backend specifications** — the Cranelift and LLVM backend contracts over LIR ([§23 of 00](00-architecture.md#23-backend-strategy)); the interpreter backend ([§21.3](00-architecture.md#213-the-interpreter-backend)); the Wasm path; the **evidence-driven benchmark requirement** (D-8): Cranelift-vs-LLVM on Lagom's suite is measured before any performance claim is documented — the Bytecode Alliance's Wasmtime-measured context (Cranelift ≈14% slower code than LLVM on *their* workload, at a large compile-time win) is cited as context only, never as Lagom's claim.
5. **Incremental compilation** — the Salsa-style query design ([§24.2 of 00](00-architecture.md#242-mechanisms)), the on-disk cache format, invalidation rules, and the determinism guarantees. **Monomorphization × incrementality policy (doc 11 §9):** per-instantiation query caching (the rustc approach), with a shared-generic fallback (compiled once, dynamic dispatch) for cold instantiations to bound binary size — an M2 decision with a benchmark.
6. **Compile-time evaluation design (D-33, R-19)** — the two-tier staging: value-level comptime runs over monomorphized MIR at M3; type-level comptime runs over a pre-monomorphization IR (HIR) at M4+, each with its own evaluation budget and caching story — MIR cannot host type-generating programs.
7. **Linking and targets** — platform linker strategy, mold/lld detection, cross-compilation mechanics ([§23.4](00-architecture.md#234-targets-and-cross-compilation)), target-specific lowering containment rule ("IR passes are target-agnostic; only backends see targets").
8. **Compile-speed budget** — the numbers from [§24.1 of 00](00-architecture.md#241-the-bar) as CI-enforced budgets; the regression-blocking policy ([§24.3](00-architecture.md#243-compile-speed-regression-is-a-release-blocker)); benchmark methodology.
9. **Compiler-as-library** — the crate layout (`lagom_lexer` … `lagom_lir`, plus `lagom_driver`, `lagom_diagnostics`), the internal-API stability policy pre-1.0, and the client list (LSP, formatter, package manager, test runner — all library clients, [§21.5 of 00](00-architecture.md#215-compiler-library-not-just-binary)).
10. **Testing strategy** — the full test pyramid: unit (lexer/parser/sema snapshots), corpus (valid + invalid programs, the spec-in-executable-form rule from [§29.1 of 00](00-architecture.md#291-the-plan-brief-24-made-concrete)), differential backend testing from M2, fuzzing the parser/sema, compile-time benchmarks, and the self-host differential harness (M5).

## Decisions inherited from 00 (binding here)

- D-8 (backend pairing, evidence-driven claims), D-21 (Rust), D-22 (library-first, query-based incrementality from M0), the one-LIR-many-backends rule ([§22.3 of 00](00-architecture.md#223-lir-backend-neutral-machine-shape)).
- **From the design review (doc 11), applied 2026-09:** D-32 (the LOM instrumentation pass + release-identity invariant), D-33 (comptime two-tier design), the HIR span-retention contract, and the monomorphization×incrementality policy note.

## Open questions

- Cranelift version pinning vs tracking main — propose: track stable releases, pin in CI matrix; decide at M0.
- Does the Wasm backend target wasip1 or wasip2? (Propose: wasip2; decide at M2 with the toolchain.)
- PGO pipeline ownership: LLVM's PGO vs a Lagom-flavored instrumentation mode — decide at M4.

## Cross-references

- [09-tooling](09-tooling.md) (compiler-library clients) · [05-memory](05-memory.md) (ARC passes) · [06-concurrency](06-concurrency.md) (async lowering) · [10-roadmap](10-roadmap.md) (milestone mapping)
