# Phase 10 — Implementation Roadmap

*Status: stub — milestones are set in [§33 of 00-architecture.md](00-architecture.md#33-mvp-proposal); this document will hold the detailed, dated roadmap with per-milestone gates.*
*Next review: at M0 kickoff.*

## Scope of this document (when filled in)

1. **Milestone definitions with gates** — for each milestone: scope, entry/exit criteria, validation projects, benchmark requirements, and the decisions that must be made before it starts (the brief's §35 methodology: define → syntax → semantics → AST → types → compiler → implement → test → benchmark → document → interaction-check, per feature).

### The milestone ladder (from 00 §33 and the feature matrix)

| Milestone | Theme | Ships | Gate (exit criteria) |
|---|---|---|---|
| **M0** | The language is real | Core student layer → **native binaries** (Cranelift); lexer→LIR→Cranelift; interpreter; diagnostics (student mode); `lagom run/build/test/fmt`; tests; modules; basic errors; **structs (7.11)**; **text↔number conversion (`number from answer`, D-39)**; **LOM failure report v1 on the interpreter (D-32)** | The four validation projects (calculator, guessing game, quiz, text adventure) completed *by non-authors*; compile budget met; **Cranelift-vs-LLVM baseline benchmarked on Lagom's suite** (D-8); corpus green |
| **M1** | Data and safety | Kinds+match, option/result full, closures+combinators, files, JSON, Windows target, `test` expansion, student diagnostics v2; **the release-overflow benchmark datum (trap-vs-wrap, D-35 — decides 00 §31.8)** | A file-organizer and CSV-parser project pass validation; sema corpus at 100% spec coverage |
| **M2** | Abstraction | Classes, interfaces, generics (constraints), iterators, package manager (git+index), tasks+channels+`can wait`, Wasm (wasip2), VS Code extension; **monomorphization×incrementality policy decided with a benchmark (doc 08 §5)**; **MIR LOM instrumentation live (D-32)** | A small HTTP client+server project passes validation; differential backend testing live; a real package published end-to-end |
| **M3** | Reach | C FFI + bindgen tool, owning regions (ownership tier), shared+guard, **comptime scoped to value-level (D-33)**, cross-compilation matrix, debugger, profiler, property testing+fuzzing, audit; **LOM provenance in native dev builds (D-32)** | A "call libsqlite3 through FFI" project and a concurrent web server pass validation; unsafe `because` lint enforced |
| **M4** | Polish | Full tooling suite (doc gen, playground, allocation profiler, **LOM replay tooling, D-32**), stdlib batteries completion, RISC-V, allocator/arena APIs, expert integer/layout types; **the borrow-escape INVESTIGATE item (R-7 follow-up: escape-analysis checker benchmarked against the continuation ladder)**; **type-level comptime investigation (R-19 follow-up)** | A GUI-free database-in-Lagom feasibility spike; classroom pilot with teacher feedback loop |
| **M5** | Self-host | `lagomc` in Lagom, differential compiler harness, no-runtime mode hardening, ABI/calling-convention controls, inline asm | Self-hosted compiler bootstraps and passes the full corpus byte-identical in release mode (29.1); a JIT-free "compiler-in-Lagom" capstone |
| **M6** | Systems ceiling | Kernel/freestanding target, linker sections/scripts, SIMD+intrinsics completion, compiler-plugin investigation, elite validation projects; **the no-runtime kernel-stdlib verification (doc 05 §12 — owning-tier replacements for lists/maps/text)** | An OS-kernel-component or embedded-firmware project runs on hardware; SIMD library benchmarked against C |

2. **The validation ladder** — the real-world project list (brief §30) mapped to milestones, each with the missing-feature feedback loop: findings feed the spec via the decision-log process ([§34 of 00](00-architecture.md#34-decision-log)).
3. **Per-milestone benchmark requirements** — the suite (JSON parse, matrix multiply, string processing, channel ping-pong, allocator churn, toy compiler), comparison targets (C, C++, Rust, Go; Java/Swift where available), methodology, and publication rule ([§25.1 of 00](00-architecture.md#251-the-claim-discipline)).
4. **Team scaling plan** — which milestones are single-maintainer-feasible vs which need contributors; contributor onboarding docs; the module-ownership map over the crate layout (doc 08).
5. **Risk register maintenance** — the live version of the risk table ([§32 of 00](00-architecture.md#32-major-risks)) with review cadence per milestone.
6. **Release engineering** — versioning of the compiler, the language-version freeze policy, the deprecation process, and the 1.0 criteria (stability promise, corpus freeze, governance answer for the registry — 00 §31.9).

## Decisions inherited from 00 (binding here)

- D-8 (no performance claims before benchmarks), D-27 (self-hosting at M5, differential-tested), D-28 (validation with real users at every milestone), M0 scope per [§33.1 of 00](00-architecture.md#331-m0--the-language-exists-and-is-real-the-gate-to-everything-else).
- **From the design review (doc 11), applied 2026-09:** M0 gate gains text↔number conversion (D-39) and LOM failure-report v1 (D-32); M1 gains the D-35 benchmark datum; M2 gains the monomorphization-policy decision; M3 comptime is value-level only (D-33); M4 gains the R-7 and R-19 investigations; M6 gains the no-runtime stdlib verification.

## Open questions

- M0 platform order: Linux+macOS first with Windows at M1 — confirm or flip based on pilot-classroom OS mix (decide at M0 kickoff).
- ~~Which validation project exercises the package manager first (M2): a real published teaching library or a multi-package workspace?~~ **Decided (2026-10): the teaching library.** The flow is pinned end-to-end by `crates/lagom_cli/tests/packages_git.rs` — a library published as a git repository, `lagom add`'d by a consumer, its resolved commit hash pinned in `Lagom.lock`, vendored into `.lagom/`, and rebuilt from a fresh checkout at the same pin. A real registry still ships with M3 tooling; `git+<url>` (and the `owner/repo` shorthand) is the M2 transport.
- Governance body formation timing: before registry public launch (M3) per 00 §31.9 — confirm the legal vehicle.

## Gate records (as met)

### M2 — all three gate criteria met (2026-10)

- **"A small HTTP client+server project passes validation."** [validation/http_server.lagom](../validation/http_server.lagom) and [validation/http_client.lagom](../validation/http_client.lagom) exchange a real HTTP/1.1 request and response over the M2 networking surface (`open listener`/`open connection`/`expect connection`/`put`/`take`/`close`/`port`, all failure-checked through `attempt`). Pinned end-to-end by `crates/lagom_driver/tests/net_validation.rs`: the programs run as real processes and runs in **all four backend pairings** (native↔native, native↔interpreted, interpreted↔native, interpreted↔interpreted), with server-side readiness races absorbed by a bounded harness retry, never by the programs. Networking fails as errors (13.1/S-10) on both backends, with mirrored registries and messages (`lagom_interp::net_op` ↔ `lagom_rt::rt_net_op`). Enabler shipped first: the `\r` string escape (HTTP headers end CRLF — a student is not told to "use the real carriage return"), and the formatter re-emits it in its escape form per the lossless contract.
- **"Differential backend testing live."** `crates/lagom_driver/tests/differential.rs` runs every validation project through the interpreter and the native backend, dev and release modes, same stdin/seed/sandbox, asserting identical ordered output and exit code. The two HTTP programs are deliberately excluded from its corpus sweep — they need a coordinated peer, which is `net_validation.rs`'s job.
- **"A real package published end-to-end."** Pinned by `crates/lagom_cli/tests/packages_git.rs` (see the resolved open question above).

Remaining M2 ships, honestly open: none as of 2026-10 — the editor story's backlog closed with the **debugger decision taken at M2 planning**: teaching-pane DAP moves to M3 (rationale in [doc 09's open questions](09-tooling.md), now resolved). The VS Code extension ships (`editors/vscode/`) with the grammar, snippets, layer-aware completions, and `lagom lsp` (hover types, goto-def over multi-word names via sema's `resolved_uses`, published diagnostics, end-to-end stdio tested); **rename and find-references** completed the name story (the callee head of a split run, `set`/`increase`/`decrease` targets, declaration clicks, quoted refusals); and **incremental text sync** shipped (the server advertises `change: 2`, applies ranged edits in batch order with one reanalysis per batch; the client sends the minimal prefix/suffix splice — end-to-end tested: a one-range fix clears the published errors). **Native Wasm (wasip2)** is decided (D-41 in [doc 00](00-architecture.md)'s decision log): the browser surface is the interpreter shipped to wasm32 — it has been from the start (README's API note) — and a wasm-emitting backend is the M5+ INVESTIGATE item, behind a benchmarked needs case. The abstraction core (classes, interfaces, constrained generics, tasks+channels+`can wait`), the package manager, the monomorphization decision (doc 08), and MIR LOM instrumentation (D-32) are all landed. The 11.3 iteration protocol (`does iterable` / `give next`) closed the last honest gap in the abstraction core's semantics: the walk polls through the 12.3 vtable on both backends, and `validation/iterator.lagom` pins it in the differential corpus (exact-output agreement, interpreter ↔ native dev ↔ native release).

## Cross-references

- [00-architecture §33](00-architecture.md#33-mvp-proposal) · all phase docs — this document sequences their completion.
