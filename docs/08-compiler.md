# Phase 8 — Compiler Architecture

*Status: stub — the compiler architecture is set in [§21–§25 of 00-architecture.md](00-architecture.md#21-compiler-architecture); this document will hold the complete compiler specification.*
*Next review: at M0 planning.*

## Scope of this document (when filled in)

1. **Pipeline specification** — stage-by-stage contracts ([§21.1 of 00](00-architecture.md#211-pipeline)): lexer (hand-written, SIMD-friendly), parser (recursive descent + indentation scanner, error recovery with region commits), AST (arena allocation, lossless mode for formatter/LSP), sema (name resolution, type check, capability check, exhaustiveness, region check), HIR, MIR, LIR, backends.
2. **IR specifications** — HIR/MIR/LIR data structures, invariants, and verifiers ([§22 of 00](00-architecture.md#22-ir-design)); the debug-vs-release check-insertion policy ([§22.4](00-architecture.md#224-ir-invariants-the-implementation-discipline)); **the HIR span-retention contract — every desugared node (interpolation, flow calls, capability clauses, `where`-sugar) keeps its source span** (rich diagnostics depend on it, doc 11 §9); monomorphization policy and cost control; ARC/drop/escape-analysis/closure/async-state-machine pass order and contracts.
3. **The LOM instrumentation pass (D-32, R-22)** — the pass that emits dev-build structured events with value provenance (function entry/exit snapshots, task events, failure events, per-binding provenance) into a bounded ring; owns the **release-identity invariant: release builds are bit-identical to non-LOM builds** (a tested invariant, like reproducible builds) — [00 §26.5](00-architecture.md#265-the-lagom-observability-model-lom--the-9010-debugging-contract).
4. **Backend specifications** — the Cranelift and LLVM backend contracts over LIR ([§23 of 00](00-architecture.md#23-backend-strategy)); the interpreter backend ([§21.3](00-architecture.md#213-the-interpreter-backend)); the Wasm path; the **evidence-driven benchmark requirement** (D-8): Cranelift-vs-LLVM on Lagom's suite is measured before any performance claim is documented — the Bytecode Alliance's Wasmtime-measured context (Cranelift ≈14% slower code than LLVM on *their* workload, at a large compile-time win) is cited as context only, never as Lagom's claim.
5. **Incremental compilation** — the Salsa-style query design ([§24.2 of 00](00-architecture.md#242-mechanisms)), the on-disk cache format, invalidation rules, and the determinism guarantees. **Monomorphization × incrementality policy (doc 11 §9):** per-instantiation query caching (the rustc approach), with a shared-generic fallback (compiled once, dynamic dispatch) for cold instantiations to bound binary size — an M2 decision with a benchmark. The M2 decision is recorded below.
6. **Compile-time evaluation design (D-33, R-19)** — the two-tier staging: value-level comptime runs over monomorphized MIR at M3; type-level comptime runs over a pre-monomorphization IR (HIR) at M4+, each with its own evaluation budget and caching story — MIR cannot host type-generating programs.
7. **Linking and targets** — platform linker strategy, mold/lld detection, cross-compilation mechanics ([§23.4](00-architecture.md#234-targets-and-cross-compilation)), target-specific lowering containment rule ("IR passes are target-agnostic; only backends see targets").
8. **Compile-speed budget** — the numbers from [§24.1 of 00](00-architecture.md#241-the-bar) as CI-enforced budgets; the regression-blocking policy ([§24.3](00-architecture.md#243-compile-speed-regression-is-a-release-blocker)); benchmark methodology.
9. **Compiler-as-library** — the crate layout (`lagom_lexer` … `lagom_lir`, plus `lagom_driver`, `lagom_diagnostics`), the internal-API stability policy pre-1.0, and the client list (LSP, formatter, package manager, test runner — all library clients, [§21.5 of 00](00-architecture.md#215-compiler-library-not-just-binary)).
10. **Testing strategy** — the full test pyramid: unit (lexer/parser/sema snapshots), corpus (valid + invalid programs, the spec-in-executable-form rule from [§29.1 of 00](00-architecture.md#291-the-plan-brief-24-made-concrete)), differential backend testing from M2, fuzzing the parser/sema, compile-time benchmarks, and the self-host differential harness (M5).

## The M2 decision: monomorphization × incrementality (recorded 2026-10)

Implemented reality first: a generic function (`takes anything ...` / `takes
some type ...`) compiles to **one body**, executed over the runtime's tagged
value representation — exactly the "shared generic fallback (compiled once,
dynamic)" arm of doc 11 §9. Nothing is cloned per instantiation, and there is
no incremental-compilation machinery yet to interact with.

**The datum** (`scripts/measure_monomorph.py`, native backend in release,
2,000,000-iteration hot loop × 2 calls, best of 5 builds/runs, macOS arm64,
Apple clang 21):

- **Per-call cost of the shared body is unmeasurable** for the reachable M2
  generic surface: a hot loop calling a generic identity
  (`takes anything called x; gives back x`) runs at **24.5 ns/call**; the
  same loop through a hand-specialized `takes number` body runs at
  **24.4 ns/call**. The two are identical within noise.
- **Instantiation churn costs nothing**: an N-function program's compile
  wall time and binary size are flat and identical between the generic and
  the hand-typed variant (N = 16/64/240: ~0.09 s and ~620–650 KiB in both
  shapes; growth tracks source size, not instantiation).
- The measurement cannot yet see arithmetic *inside* a generic body: sema
  refuses operators on `anything` (`plus` needs a numeric type), so the
  reachable surface is pass-through, combinators, and builtins that already
  dispatch through tagged values. When that surface widens, this script is
  the instrument that re-measures it.

**The decision (binding):**

1. **The shared arm is the M2/M3 default**: one compiled body per generic
   function, no per-instantiation cloning. The measured cost of reaching for
   it is zero on every axis the M2 surface can express.
2. **Per-instantiation specialization** (monomorphize MIR per instantiation)
   becomes the upgrade only when **both** hold: (a) the surface can express
   the work (operators inside generic bodies), and (b) this benchmark shows
   a hot-loop regression above **2%** — the same revisit threshold D-35 set
   for check-insertion. Until then, specialization would buy compile time
   and binary size at zero measured benefit.
3. **Incrementality interacts later, not now**: per-instantiation *query
   caching* enters when incremental compilation itself lands; this
   benchmark's churn axis (compile wall time and binary size against N)
   is the regression instrument for that landing too.

**Environment finding (not a decision datum):** on the M2 dev machine,
natively linking a Lagom object with **≥ 254 top-level functions** crashes
Apple's ld (Trace/BPT trap; reproduced in dev and release; Apple clang
21.0.0, darwin27.2). The Lagom objects themselves are clean across the
boundary — object size and defined-symbol counts grow linearly through it
(verified at N = 250…258). Not reproduced elsewhere; tracked for
investigation with the linker-crash snapshot it emits. The benchmark's churn
sweep therefore stops at 240 by default (see the script's note).

## Decisions inherited from 00 (binding here)

- D-8 (backend pairing, evidence-driven claims), D-21 (Rust), D-22 (library-first, query-based incrementality from M0), the one-LIR-many-backends rule ([§22.3 of 00](00-architecture.md#223-lir-backend-neutral-machine-shape)).
- **From the design review (doc 11), applied 2026-09:** D-32 (the LOM instrumentation pass + release-identity invariant), D-33 (comptime two-tier design), the HIR span-retention contract, and the monomorphization×incrementality policy note.

## Open questions

- The Apple-ld crash on ≥ 254 top-level-function Lagom objects (recorded in "The M2 decision" above): reproduce outside this machine, study the linker-crash snapshot, and decide whether it is a Lagom object-layout matter or purely toolchain.
- Cranelift version pinning vs tracking main — propose: track stable releases, pin in CI matrix; decide at M0.
- Does the Wasm backend target wasip1 or wasip2? (Propose: wasip2; decide at M2 with the toolchain.)
- PGO pipeline ownership: LLVM's PGO vs a Lagom-flavored instrumentation mode — decide at M4.

## Cross-references

- [09-tooling](09-tooling.md) (compiler-library clients) · [05-memory](05-memory.md) (ARC passes) · [06-concurrency](06-concurrency.md) (async lowering) · [10-roadmap](10-roadmap.md) (milestone mapping)
