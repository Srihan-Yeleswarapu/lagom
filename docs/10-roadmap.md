# Phase 10 — Implementation Roadmap

*Status: stub — milestones are set in [§33 of 00-architecture.md](00-architecture.md#33-mvp-proposal); this document will hold the detailed, dated roadmap with per-milestone gates.*
*Next review: at M0 kickoff.*

## Scope of this document (when filled in)

1. **Milestone definitions with gates** — for each milestone: scope, entry/exit criteria, validation projects, benchmark requirements, and the decisions that must be made before it starts (the brief's §35 methodology: define → syntax → semantics → AST → types → compiler → implement → test → benchmark → document → interaction-check, per feature).

### The milestone ladder (from 00 §33 and the feature matrix)

| Milestone | Theme | Ships | Gate (exit criteria) |
|---|---|---|---|
| **M0** | The language is real | Core student layer → **native binaries** (Cranelift); lexer→LIR→Cranelift; interpreter; diagnostics (student mode); `lagom run/build/test/fmt`; tests; modules; basic errors | The four validation projects (calculator, guessing game, quiz, text adventure) completed *by non-authors*; compile budget met; **Cranelift-vs-LLVM baseline benchmarked on Lagom's suite** (D-8); corpus green |
| **M1** | Data and safety | Structs, kinds+match, option/result full, closures+combinators, files, JSON, Windows target, `test` expansion, student diagnostics v2 | A file-organizer and CSV-parser project pass validation; sema corpus at 100% spec coverage |
| **M2** | Abstraction | Classes, interfaces, generics (constraints), iterators, package manager (git+index), tasks+channels+`can wait`, Wasm (wasip2), VS Code extension | A small HTTP client+server project passes validation; differential backend testing live; a real package published end-to-end |
| **M3** | Reach | C FFI + bindgen tool, owning regions (ownership tier), shared+guard, comptime, cross-compilation matrix, debugger, profiler, property testing+fuzzing, audit | A "call libsqlite3 through FFI" project and a concurrent web server pass validation; unsafe `because` lint enforced |
| **M4** | Polish | Full tooling suite (doc gen, playground, allocation profiler), stdlib batteries completion, RISC-V, allocator/arena APIs, expert integer/layout types | A GUI-free database-in-Lagom feasibility spike; classroom pilot with teacher feedback loop |
| **M5** | Self-host | `lagomc` in Lagom, differential compiler harness, no-runtime mode hardening, ABI/calling-convention controls, inline asm | Self-hosted compiler bootstraps and passes the full corpus byte-identical in release mode (29.1); a JIT-free "compiler-in-Lagom" capstone |
| **M6** | Systems ceiling | Kernel/freestanding target, linker sections/scripts, SIMD+intrinsics completion, compiler-plugin investigation, elite validation projects | An OS-kernel-component or embedded-firmware project runs on hardware; SIMD library benchmarked against C |

2. **The validation ladder** — the real-world project list (brief §30) mapped to milestones, each with the missing-feature feedback loop: findings feed the spec via the decision-log process ([§34 of 00](00-architecture.md#34-decision-log)).
3. **Per-milestone benchmark requirements** — the suite (JSON parse, matrix multiply, string processing, channel ping-pong, allocator churn, toy compiler), comparison targets (C, C++, Rust, Go; Java/Swift where available), methodology, and publication rule ([§25.1 of 00](00-architecture.md#251-the-claim-discipline)).
4. **Team scaling plan** — which milestones are single-maintainer-feasible vs which need contributors; contributor onboarding docs; the module-ownership map over the crate layout (doc 08).
5. **Risk register maintenance** — the live version of the risk table ([§32 of 00](00-architecture.md#32-major-risks)) with review cadence per milestone.
6. **Release engineering** — versioning of the compiler, the language-version freeze policy, the deprecation process, and the 1.0 criteria (stability promise, corpus freeze, governance answer for the registry — 00 §31.9).

## Decisions inherited from 00 (binding here)

- D-8 (no performance claims before benchmarks), D-27 (self-hosting at M5, differential-tested), D-28 (validation with real users at every milestone), M0 scope per [§33.1 of 00](00-architecture.md#331-m0--the-language-exists-and-is-real-the-gate-to-everything-else).

## Open questions

- M0 platform order: Linux+macOS first with Windows at M1 — confirm or flip based on pilot-classroom OS mix (decide at M0 kickoff).
- Which validation project exercises the package manager first (M2): a real published teaching library or a multi-package workspace? (Propose: teaching library.)
- Governance body formation timing: before registry public launch (M3) per 00 §31.9 — confirm the legal vehicle.

## Cross-references

- [00-architecture §33](00-architecture.md#33-mvp-proposal) · all phase docs — this document sequences their completion.
