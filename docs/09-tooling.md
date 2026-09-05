# Phase 9 — Tooling

*Status: stub — the tooling strategy is set in [§26 of 00-architecture.md](00-architecture.md#26-tooling-strategy); this document will hold the complete tooling specification.*
*Next review: at M0 planning (CLI + diagnostics land M0).*

## Scope of this document (when filled in)

1. **The `lagom` CLI** — full command reference (`run`, `build`, `test`, `check`, `fmt`, `doc`, `play`, `new`, `add`, `words`, `explain`, `profile`), output contracts, exit codes, and the single-binary distribution model ([§26.1 of 00](00-architecture.md#261-the-lagom-cli-one-binary)).
2. **Teaching diagnostics specification** — the diagnostic data model (code, spans, labels, helps, notes, concept links), the three verbosity modes (`student`/`normal`/`expert`) with the exact student-mode five-part format (what / where / why / fix / concept — [§26.2 of 00](00-architecture.md#262-teaching-diagnostics-the-differentiator)), the diagnostic review process (content reviewed like API, snapshot-tested like code), and the `lagom explain` content model with its concepts glossary.
3. **Formatter** — the canonical style (4-space, zero-config beyond indentation width, gofmt lesson), the lossless-AST round-trip contract, and idempotency guarantees ([§26.3 of 00](00-architecture.md#263-the-rest-of-the-faculty)).
4. **LSP specification** — hover in student words (inferred types), layer-aware completions (student projects get no `unsafe` completions), diagnostics reuse (21.4), goto-def/find-refs/rename over multi-word names ([§7.0.3 of 00](00-architecture.md#703-multi-word-name-disambiguation-the-main-ambiguity-risk)), and incremental-sync strategy.
5. **Debugger** — DAP server over Cranelift/LLVM debug info, the teaching pane (per-step change summaries), breakpoint/stepping semantics with async tasks and channels ([§26.3 of 00](00-architecture.md#263-the-rest-of-the-faculty), doc 06).
6. **Profiler** — sampling profiler with per-task views (doc 06), allocation/ARC profiling at M4, perf/DTrace integration, `lagom bench` methodology and compile-time benchmarks ([§24.3 of 00](00-architecture.md#243-compile-speed-regression-is-a-release-blocker)).
7. **Testing tools** — `lagom test` (unit), property-based testing (`try many random … check that …`), `lagom fuzz` (M3), doctest-style executable documentation examples ([§26.3 of 00](00-architecture.md#263-the-rest-of-the-faculty)).
8. **Package tooling UX** — `lagom new/add/publish/audit` flows, the `contains unsafe` visibility rule ([§28.3 of 00](00-architecture.md#283-unsafe-boundary-security)), and private-registry auth.
9. **Editors and playground** — the M0 grammar assets (tree-sitter + textmate), the M2 VS Code extension, and the M4 hosted Wasm playground for zero-install K-12 onboarding ([§26.4 of 00](00-architecture.md#264-editor-support)).
10. **REPL (`lagom play`)** — incremental evaluation over the interpreter backend, teaching mode (inferred types shown after each line), and history/session semantics.

## Decisions inherited from 00 (binding here)

- D-22 (all tools are compiler-library clients), D-28 (diagnostics and tooling validated with real users from M0), the layers-as-checks rule applied to editor completions ([§0 of 00](00-architecture.md#0-how-to-read-this-document-layers)).

## Open questions

- Default verbosity mode for *existing* projects as users mature — auto-escalation heuristic or explicit setting? (Propose: explicit; decide at M1 with user data.)
- Does the debugger's teaching pane ship at M2 (with basic DAP) or M3? (Propose: M3; decide at M2 planning.)
- Playground hosting governance (cost, privacy for student code) — decide before public playground launch (M3–M4).

## Cross-references

- [08-compiler](08-compiler.md) (diagnostics pipeline, library architecture) · [02-syntax](02-syntax.md) (formatter = canonical syntax) · [10-roadmap](10-roadmap.md) (tooling milestones)
