# Phase 9 — Tooling

*Status: stub — the tooling strategy is set in [§26 of 00-architecture.md](00-architecture.md#26-tooling-strategy); this document will hold the complete tooling specification.*
*Next review: at M0 planning (CLI + diagnostics land M0).*

## Scope of this document (when filled in)

1. **The `lagom` CLI** — full command reference (`run`, `build`, `test`, `check`, `fmt`, `doc`, `play`, `new`, `add`, `words`, `explain`, `profile`), output contracts, exit codes, and the single-binary distribution model ([§26.1 of 00](00-architecture.md#261-the-lagom-cli-one-binary)).
2. **Teaching diagnostics specification** — the diagnostic data model (code, spans, labels, helps, notes, concept links), the three verbosity modes (`student`/`normal`/`expert`) with the exact student-mode five-part format (what / where / why / fix / concept — [§26.2 of 00](00-architecture.md#262-teaching-diagnostics-the-differentiator)), the diagnostic review process (content reviewed like API, snapshot-tested like code), and the `lagom explain` content model with its concepts glossary.
3. **The LOM failure report (D-32, R-22)** — the runtime half of the teaching system ([00 §26.5](00-architecture.md#265-the-lagom-observability-model-lom--the-9010-debugging-contract)): the report format (call chain in source words, per-frame argument values, the provenance chain of the failing values, the concept link via `lagom explain`) designed and snapshot-tested exactly like diagnostics; **the LOM test is the acceptance criterion — any unhandled failure in a dev build produces a report naming the failing value's origin, with zero programmer-added code**; ships at v1 on the interpreter backend (M0 gate item).
4. **Formatter** — the canonical style (4-space, zero-config beyond indentation width, gofmt lesson), the lossless-AST round-trip contract, and idempotency guarantees ([§26.3 of 00](00-architecture.md#263-the-rest-of-the-faculty)).
4. **LSP specification** — hover in student words (inferred types), layer-aware completions (student projects get no `unsafe` completions), diagnostics reuse (21.4), goto-def/find-refs/rename over multi-word names ([§7.0.3 of 00](00-architecture.md#703-multi-word-name-disambiguation-the-main-ambiguity-risk)), and incremental-sync strategy.
5. **Debugger** — DAP server over Cranelift/LLVM debug info, the teaching pane (per-step change summaries), breakpoint/stepping semantics with async tasks and channels ([§26.3 of 00](00-architecture.md#263-the-rest-of-the-faculty), doc 06).
6. **Profiler** — sampling profiler with per-task views (doc 06), allocation/ARC profiling at M4, perf/DTrace integration, `lagom bench` methodology and compile-time benchmarks ([§24.3 of 00](00-architecture.md#243-compile-speed-regression-is-a-release-blocker)).
7. **Testing tools** — `lagom test` (unit), property-based testing (`try many random … check that …`), `lagom fuzz` (M3), doctest-style executable documentation examples ([§26.3 of 00](00-architecture.md#263-the-rest-of-the-faculty)).
8. **Package tooling UX** — `lagom new/add/publish/audit` flows, the `contains unsafe` visibility rule ([§28.3 of 00](00-architecture.md#283-unsafe-boundary-security)), and private-registry auth.
9. **Editors and playground** — the M0 grammar assets (tree-sitter + textmate), the M2 VS Code extension, and the M4 hosted Wasm playground for zero-install K-12 onboarding ([§26.4 of 00](00-architecture.md#264-editor-support)).
10. **REPL (`lagom play`)** — incremental evaluation over the interpreter backend, teaching mode (inferred types shown after each line), and history/session semantics.

## Decision D-40: the executable-example contract (`## >>>` / `## =`)

*M2, applied 2026-09. 00 §26.6 mandates only "every example in every doc compiles and runs"; this pins what "runs" means. No `>>>` examples existed in any doc before M2, so nothing frozen is redefined — this is the first written contract for the form.*

- **`## >>> <statement>`** starts one example and quotes its first statement; **consecutive `## >>>` lines are one example** (a multi-statement session, like a REPL — later statements see earlier bindings). Any other `##` line ends the example.
- The example runs with the **whole file in scope** (the Rust doctest rule): the file's functions, structures, kinds, and aliases are visible to it.
- **`## = <text>`** pins what the example *produces*, one line per `=`. Pins are checked when present (the Rust doctest rule: an unpinned example must still compile and run); a present pin is a hard contract. The produced value is:
  1. the `say` output the example printed (one `=` per printed line), or — when the example printed nothing —
  2. the **value of the example's last binding** (`make` is the example's teaching point; rendering is `Value::format`, the S-9 one-formatting-place rule — the same words the play echo prints).
- The evaluator is **the play session engine** (`lagom_driver::Session`) — the same incremental evaluation, teaching echo, and seeded determinism as `lagom play`. One evaluator, no second semantics.
- **Unsupported forms fail loudly**: a `=` after an example that produced nothing (no `say`, no value-producing statement) is a *check error naming the example and its file line* — never a false pass. Compile errors and wrong pinned values render as the student diagnostic, with the expected/actual diff.
- Rendering in the doc site: the `>>>` lines show as the example; `=` lines show as its output.

Rationale for the last-statement rule: the teaching docs' natural example is `make x equal to double 2` with `## = 4` — a binding, not a print. Requiring `say` for every example would force noise into every doc; borrowing the binding echo (already specified for the REPL in 26.4's teaching mode) keeps `=` meaning "what the student sees". When both `say` output and a final value exist, the `say` lines win — they are what the run visibly printed.

## Decisions inherited from 00 (binding here)

- D-22 (all tools are compiler-library clients), D-28 (diagnostics and tooling validated with real users from M0), the layers-as-checks rule applied to editor completions ([§0 of 00](00-architecture.md#0-how-to-read-this-document-layers)).
- **From the design review (doc 11), applied 2026-09:** D-32 — the failure-report format is a designed, snapshot-tested artifact (LOM tier 2) and an M0 gate item; error-code examples unify on the `E0003`-style form (`E1024`-style examples are stale).

## Open questions

- Default verbosity mode for *existing* projects as users mature — auto-escalation heuristic or explicit setting? (Propose: explicit; decide at M1 with user data.)
- ~~Does the debugger's teaching pane ship at M2 (with basic DAP) or M3? (Propose: M3; decide at M2 planning.)~~ **Decided (2026-10, M2 planning): M3.** The M2 gate did not name the debugger — its three criteria (HTTP project, differential testing, packaged library) are met without it — and the honest cost is large: the teaching pane needs the per-step change summaries, breakpoint/stepping semantics over tasks and channels ([doc 06](06-concurrency.md)), and a DAP server whose stepping can be differential-tested exactly like the backends. Landing a half-paned basic DAP inside M2 would be an editor artifact nobody validated with students (D-28's rule). M3's tooling scope owns the full design.
- Playground hosting governance (cost, privacy for student code) — decide before public playground launch (M3–M4).

## M2 explainer ledger (2026-10)

The "no diagnostic the compiler can produce goes unexplained" property is now a test, not an intention: `every_emitted_code_has_a_page` sweeps every compiler crate's source for `"E####"` literals and asserts each resolves to a `lagom explain` page ([`crates/lagom_cli/src/explain.rs`](../crates/lagom_cli/src/explain.rs)). Indicator probes (the `E9998`/`E9999` renderer fixtures) live under `crates/*/tests/`, which the sweep excludes. What M2 added to the ledger:

- **11.3's four protocol codes** explain what the walk is doing: E0395 (the built-in `iterable` name is reserved — the claim carries the protocol, not a redeclaration), E0396 (the polls ride compiler-lowered loop code, so a `can fail` `give next` would leak a failure no student line wrote), E0397 (a walk ends only when a poll returns `nothing` — the option annotation is what makes the loop honest), E0398 (class walks own no positions, so the comma-index form is refused with the list counting rule named).
- **Stale pages are drift too:** E0338's page had predated the class walk — it now names lists, maps, and `does iterable` classes, and points a count-intent reader at `repeat N times`.
- **Context suggestions:** unknown names offer the closest real binding by Damerau–Levenshtein single-edit distance (`did you mean \`score\`?`), with containment + length bars so short names never suggest noise (sema's `single_edit_would_touch`, `dam_levenshtein`). The bar's teaching rule: a suggestion must survive a rename of every other name — otherwise it guesses instead of teaching (G-naming).

## Cross-references

- [08-compiler](08-compiler.md) (diagnostics pipeline, library architecture) · [02-syntax](02-syntax.md) (formatter = canonical syntax) · [10-roadmap](10-roadmap.md) (tooling milestones)
