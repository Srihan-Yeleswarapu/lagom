# 15 — Driver and CLI structure (M0)

Recorded after the M0 packaging pass, so later work builds with the grain
instead of re-litigating it. The module doc-comments carry each module's
invariants in full; this file is the map — where the boundaries are and who
owns what — not a restatement of the code.

## The three driver modules

`lagom_driver` is the orchestration layer between the `lagom` CLI and the
compiler crates. It is split by concern, one owner each:

| Module | Concern | Owns |
|---|---|---|
| `pipeline` | the shared front end | source → parse → check → HIR → MIR → verify; `Frontend` (with the facts back ends need: `has_entry`, `test_count`), `FrontendError`, diagnostic rendering |
| `native` | the native back end | object code, the runtime-rlib search order (`LAGOM_RT_LIB` → bundled `<exe dir>/lib/lagom/` → workspace target dir), the rustc link step, the scratch `NativeBuild` dir's lifecycle |
| `interp` | the interpreter back end | `run_interpreted`, `run_tests_interpreted` over the shared front end |

`lib.rs` is the public face only: it re-exports the surface the CLI and the
integration tests use (`frontend`, `render_diagnostics`, `Frontend`,
`FrontendError`, `needs_input`, `build_native`, `compile_object`,
`CompiledObject`, `link`, `NativeBuild`, `run_interpreted`,
`run_tests_interpreted`). The split is internal structure, not API churn —
callers did not change.

## The CLI modules

`lagom_cli` follows the same one-concern-per-module rule; `main.rs` is the
face only (dispatch and the exit-code mapping). Each command lives in its
own module; shared plumbing lives in `args`:

| Module | Concern |
|---|---|
| `args` | `CliError` (the one failure type every command returns), the source-file convention, flag reading, stdin capture, failure rendering |
| `run` | `lagom run` / `lagom build` — the native commands that share the build plumbing, plus the `--trace` interpreter path |
| `test`, `check` | `lagom test` / `lagom check`, thin over driver calls |
| `fmt_cmd`, `explain_cmd`, `new` | `lagom fmt` / `lagom explain` / `lagom new` |
| `fmt`, `words`, `explain`, `template` | the formatter engine, the reserved-word listing, the error-code write-ups, and the `new` project template — data/engines behind the commands |

The naming convention: a `_cmd` suffix marks a command module whose plain
name is the library code it drives (`fmt_cmd` vs `fmt`, `explain_cmd` vs
`explain`). Each module's doc-comment is the contract; `main.rs`'s is the
dispatch table.

## Data flow

One direction: source text flows into `pipeline`, whose `Frontend` output
flows into exactly one backend (`native` or `interp`). Backends never
re-parse, re-check, or call each other. The CLI reads files and renders
results; it holds no compilation state. It asks the driver for the facts it
needs to speak honestly (`needs_input`, `Frontend::has_entry`/`test_count`)
rather than re-deriving policy from source text itself.

## State ownership

- The `FrontendError::Tool` variant carries environment/toolchain failures
  (missing runtime, rustc, unwritable output); `Internal` stays reserved for
  compiler bugs. The CLI maps them to different messages and the same exit 1.
- `NativeBuild` owns its scratch directory's lifecycle (removed on `Drop`).
- Runtime discovery is one policy in `native`: `runtime_rlib` dispatches,
  `bundled_rlib` and `workspace_rlib` each own their freshness rule (a
  bundled copy must be newer than every runtime source; a workspace copy is
  rebuilt when stale).
- Whether a program needs stdin is `interp`'s policy (`needs_input`, a
  documented source-text heuristic); whether it has a script body to run is
  `pipeline`'s fact (`Frontend::has_entry`), computed from the MIR — both
  answered by the driver, never guessed in the CLI.

## Boundaries that later passes should keep

- New backends join as sibling modules of `native`/`interp`, consuming
  `pipeline::Frontend` — not by growing `pipeline`.
- Diagnostic rendering stays in `pipeline`; the CLI never formats compiler
  output itself.
- The runtime search order lives only in `native`; packaging changes
  (static-linking the runtime, a `lagom-rt` shared library) land there.
- New commands land as a sibling module of `run`/`test`/… with the shared
  bits in `args`; `main.rs` stays dispatch-only.