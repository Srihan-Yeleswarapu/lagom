# Lagom

> **Simple syntax, not a simple language.**

Lagom (Swedish for *"just enough"*) is a general-purpose, statically typed, natively compiled programming language designed to grow with its programmer — from a 10-year-old's first

```lagom
say "Hello, world!"
```

through

```lagom
make age equal to 15

if age is greater than 13
    say "You can join!"
```

all the way to systems programming: compilers, databases, game engines, and operating-system components — **in the same language, without ever leaving it.**

The surface syntax is word-based and reads like structured English, but it is a small, deterministic, formally specified grammar — no natural-language parsing, no AI in the compiler. Every readable spelling hides punctuation, never a concept.

## Design highlights

- **One language, layered exposure** — `student` → `intermediate` → `advanced` → `expert` layers over a single semantics; beginners encounter less complexity, not a less capable language.
- **Immutable by default** — `make x equal to 15` / `make changing x equal to 0`.
- **Capability-clause functions** — `takes` / `returns` / `can fail` / `can wait`: one grammar covers plain, error-returning, and async functions.
- **Errors are values, not exceptions** — `attempt … if it fails then …`; unhandled failure is a compile error.
- **Hybrid memory model** — value semantics + ARC for students, opt-in ownership/borrowing for experts, `unsafe` for raw control.
- **Fast compiles and fast code** — Cranelift for instant debug builds, LLVM for maximum optimization, one IR for both; performance claims are earned by benchmark, never asserted.
- **Compiler as teacher** — three diagnostic verbosity modes; every student-mode error explains what, where, why, how to fix, and what concept to learn.

## Documentation

| Document | Contents |
|---|---|
| [00 — Architecture & Design Proposal](docs/00-architecture.md) | The complete proposal: vision, principles, feature matrix, full syntax draft, type system, memory, OOP, generics, errors, concurrency, async, metaprogramming, FFI, unsafe, stdlib, packages, compiler/IR/backends, tooling, education, security, self-hosting, language comparison, risks, MVP, and the decision log |
| [01 — Philosophy](docs/01-philosophy.md) | Phase 1 spec (stub) |
| [02 — Syntax](docs/02-syntax.md) | Phase 2 spec (stub) |
| [03 — Semantics](docs/03-semantics.md) | Phase 3 spec (stub) |
| [04 — Types](docs/04-types.md) | Phase 4 spec (stub) |
| [05 — Memory](docs/05-memory.md) | Phase 5 spec (stub) |
| [06 — Concurrency](docs/06-concurrency.md) | Phase 6 spec (stub) |
| [07 — Standard library](docs/07-stdlib.md) | Phase 7 spec (stub) |
| [08 — Compiler](docs/08-compiler.md) | Phase 8 spec (stub) |
| [09 — Tooling](docs/09-tooling.md) | Phase 9 spec (stub) |
| [10 — Roadmap](docs/10-roadmap.md) | Phase 10 spec (stub) — milestones M0–M6 with gates |
| [11 — Design review](docs/11-design-review.md) | Adversarial architecture review: 24 issue cards, stress tests, contradiction matrix, KEEP/MODIFY/INVESTIGATE/REJECT verdicts — **approved fixes applied** (grammar unified with additive-level args, method calls without dots, equality/send/locking semantics, text↔number, comptime staging, the Lagom Observability Model; decisions D-30…D-39) |
| [12 — Consistency pass](docs/12-consistency-pass.md) | Post-fix re-review of the changed areas: checklist results, every applied change classified (contradiction fix / clarification / wording), zero new design decisions — **architecture frozen for M0** |

Docs 01–10 are phase stubs by design: they scope what each specification must contain, bind the decisions already made in 00, and record open questions with decision dates. They fill in as each phase matures.

## Installing

### macOS & Linux — one command

From a clone of this repo:

```
sh install.sh
```

(or straight from the browser, once the repo is public:
`curl -fsSL https://raw.githubusercontent.com/Srihan-Yeleswarapu/lagom/main/install.sh | sh`)

The script picks the package for your OS **and CPU architecture**, verifies its sha256, sanity-checks the binary, installs to `~/.lagom`, and adds one PATH line to your shell config — idempotently, so re-running is safe. Options:

```
sh install.sh --version v0.1.1   # pin a release instead of the latest
sh install.sh --dir ~/.lagom     # install somewhere specific (default: ~/.lagom)
sh install.sh --uninstall        # remove everything again (folder + PATH line)
```

**Requirements:** none to run `check`, `fmt`, `play`, `test`, and `run --trace` (the interpreter needs no toolchain). The first native `lagom run`/`lagom build` rebuilds the bundled runtime once, which needs any Rust toolchain (`brew install rust`, or [rustup](https://rustup.rs)); after that one-time rebuild, no toolchain is needed. macOS builds are Apple Silicon; on an Intel Mac, build from source (below).

### Windows

1. Download `lagom-Windows.zip` (plus `sha256sums.txt`) from the [latest release](https://github.com/Srihan-Yeleswarapu/lagom/releases/latest) and verify: `sha256sum -c sha256sums.txt` (in the download folder).
2. Unpack somewhere stable, e.g. `C:\Users\you\lagom` (keep `lagom.exe` and its `lib` folder together):
   ```powershell
   Expand-Archive lagom-Windows.zip -DestinationPath ~\lagom
   ```
3. To type plain `lagom` from anywhere, add that folder to PATH **with the GUI** (Win → "edit environment variables for your account" → *Path* → Edit → New → paste → OK), then reopen the terminal. Skip guides that say `setx` — it can truncate your PATH.

If Windows flags the download: File Properties → **Unblock** → OK (normal for unsigned tools).

### From a checkout (any OS)

```
cargo install --path crates/lagom_cli
```

This builds the compiler and puts `lagom` on your PATH (removal: `cargo uninstall lagom`).

### After installing — day one

```
lagom new hello
cd hello
lagom run
```

`lagom new` writes a starter `main.lagom` that asks your name and greets you. From there:

| Command | What it does |
|---|---|
| `lagom run` | compile and run `main.lagom` (or `lagom run file.lagom`) |
| `lagom run --release` | optimized build |
| `lagom run --trace` | run on the teaching interpreter — live probes, a failure report when things go wrong, and a step/value timeline of the run |
| `lagom build [file] [-o out]` | compile to a native executable |
| `lagom test [file]` | run this file's `test` blocks |
| `lagom check [file]` | parse and check; show diagnostics |
| `lagom fmt [file] [--write]` | canonically format (print, or rewrite with `--write`) |
| `lagom explain <code>` | the teaching write-up for an error code |
| `lagom words` | the reserved words |
| `lagom play` | the REPL — evaluate line by line, teaching mode shows each new binding with its type |
| `lagom doc [file]` | render `##` doc comments to markdown; `--check` verifies `>>>` examples |
| `lagom add <name> <path>` | vendor a package into `packages/` and record it in `Lagom.toml` |
| `lagom uninstall [--rc] [--yes]` | remove an installed copy of the compiler — plan first, delete with `--yes`; `--rc` also cleans the PATH line from shell rc files |
| `lagom profile [file]` | run with per-function call counts and the compile-time breakdown |

Everything works from any directory on a single `.lagom` file — no project manifest needed. In a folder with a `Lagom.toml`, `run`/`build`/`test` also compile every vendored package in `packages/`, and `lagom add`/`remove` manage them.

## Uninstalling

| Installed how | Uninstall with |
|---|---|
| `sh install.sh` (macOS/Linux) | `sh install.sh --uninstall` — or `lagom uninstall --rc --yes` |
| the release zip (any OS) | `lagom uninstall --yes` (add `--rc` on macOS/Linux to clean the PATH line); on Windows the running exe cannot delete itself, so the command removes everything else and names that one file to delete afterwards |
| `cargo install --path crates/lagom_cli` | `cargo uninstall lagom` |

Both uninstall paths are safe: they refuse folders holding `Cargo.toml` (source checkouts) and cargo build caches, and they touch nothing outside the install folder and the shell rc files. Open a new terminal afterwards.

## Browser playground

A static, client-side tutorial runs Lagom in the browser: a Web Worker loads the compiler as WebAssembly, so no code leaves the page. It runs the interpreter (not the native Cranelift backend), so programs behave as they do under `lagom run`.

Pages: `/` is the course outline, `/lesson/<id>` is a lesson (e.g. `/lesson/basics.ask`) with its examples and exercises, and `/practice` is a standalone playground. Lesson content is static JSON under `web/content/`; each lesson is one file named after its id. Progress and saved code live in this browser's `localStorage` and can be exported and imported from the outline. Clearing browser data loses them.

Prerequisites, one time:

```sh
rustup target add wasm32-unknown-unknown
cargo install wasm-bindgen-cli --version 0.2.129   # must match crates/lagom_web/Cargo.toml
```

Build and serve:

```sh
scripts/build-web.sh                 # writes web/pkg/lagom_web.js + lagom_web_bg.wasm
node scripts/smoke-web.mjs           # optional: runs real programs through the wasm build
python3 scripts/serve-web.py 8765
```

Then open http://localhost:8765. Use `scripts/serve-web.py`, not `python3 -m http.server`: interactive `ask` needs `SharedArrayBuffer`, which needs the cross-origin isolation headers (`COOP`/`COEP`) that the script sends. The script also serves `.wasm` as `application/wasm`.

When a program reaches `ask`, it pauses and the page shows the prompt. Type an answer and press Send, or press **No more input** to end input (the `ask` then fails with "end of input"). **Stop** terminates the worker, so a runaway or paused program can be cancelled; the page then starts a fresh worker.

**API.** `crates/lagom_web` exports two functions, both returning JSON strings:

- `check(source)` → `{ ok, diagnostics }`. Each diagnostic has `code`, `severity`, `message`, `line`, `column`, `endLine`, `endColumn`, `rendered` (the teaching text with the source line and caret), `explanation`, `fix`, `concept`, and `notes`.
- `run(source, stdin)` → `{ status, output, message, report, diagnostics }`, where `status` is `completed`, `failed`, `panicked`, or `compile-error`. Answers come from a fixed `stdin` string, one line per `ask`. Used by the smoke test and non-interactive callers.
- `run_interactive(source)` → the same JSON. Each `ask` calls the page-provided `lagomAskLine(prompt, transcript)`, which blocks the worker until the user answers, so the program can depend on earlier answers.

This is not `compile(source) → { bytes, diagnostics }`. The module emits no bytecode or Wasm, because the native Cranelift backend can't target `wasm32-unknown-unknown`, so the browser runs the interpreter.

**Ask model: pause and resume.** The worker blocks on `Atomics.wait` inside `ask`. The page writes the answer into a `SharedArrayBuffer` and calls `Atomics.notify`. The interpreter is recursive, so this blocking approach avoids rewriting it as a resumable state machine. An `ask` with no answer left fails with "end of input".

Files: `crates/lagom_web` (the wasm-bindgen API), `web/src/` (the tutorial, playground, editor and progress modules), `web/content/` (course and lesson data), `web/runner.worker.js`, `web/vendor/editor.bundle.js` (CodeMirror, built with `npm run build:editor`), `scripts/build-web.sh`. Web tests: `npm run test:web`.

## Status

**M2: intelligence and tooling (current).** Everything from M1, plus:

- **Type aliases (8.4):** `a type called score is a number` — transparent names, order-free, with cycle detection; listed with `lagom words --types`.
- **Causal diagnostics:** every fix comes with why it solves the problem (`This fixes it because …`), provenance labels quote where a bad value was made, and ambiguous intent says so instead of guessing. Intentionally-broken programs are regression-tested to reject misleading fixes.
- **`lagom play`:** the REPL — incremental evaluation on the interpreter, teaching mode showing each new binding with its type, `#replay` re-running the accepted history deterministically.
- **`lagom doc`:** `##` doc comments become markdown pages; `## >>>` examples are real statements checked against `## =` expected output with `--check`.
- **`lagom add`/`remove`:** a package workflow — a real package published end-to-end at M2 (10-roadmap M2 gate): dependencies are recorded in `Lagom.toml` and vendored into every build, so `lagom run`/`build`/`test` are single-binary and build offline.
    - Path source: `lagom add economics ../../economics` vendors that directory's `.lagom` files in place.
    - Git source (real publishing, real reproducibility): `lagom add cards git+https://github.com/owner/repo` — or the `owner/repo` shorthand — clones into `.lagom/cards/`, records the manifest entry, and pins the **resolved commit hash** in `Lagom.lock` (`cards = "git+…@<sha>"`). A build without the vendored copy refetches it and checks out the pinned hash when HEAD has moved on, so a fresh checkout reproduces the build byte-for-byte; re-adding is the upgrade story. `remove` also drops the vendored clone.
    - Registry version (M3 tooling fetches): `lagom add mathutils 0.1.0` is recorded now, and a build reports a missing pack honestly instead of silently skipping it. `.lagom/` is machine-local (like `target/`) and must not be committed.
- **`lagom profile`:** exact per-function call counts from the LOM event ring (dev runs only) plus the compile-time breakdown.
- **Richer trace/replay:** `lagom run --trace` now prints the step/value timeline of the run; seeded runs replay to byte-identical timelines.
- **The abstraction core (M2):** classes (`class` with `has` fields, `can` methods, and a constructor form), single inheritance (`extends`), interfaces (`does`, with copied defaults), constrained generics (`takes anything that does scored called thing` — the call-site proves the constraint, 12.3), structured tasks and channels (`start a task` / `wait for all tasks` / `send x to c` / `receive from c` / `can wait`), and 11.3's iteration protocol: a class that `does iterable` and declares `can give next` returning `a <T> or nothing` is walked by `repeat for each x in obj` — the same `repeat for each` the student already writes for lists, now polling through the backend-shared 12.3 vtable. Demo: [validation/iterator.lagom](validation/iterator.lagom).
- **Differential backend testing is live (the M2 gate item, 10-roadmap):** `crates/lagom_driver/tests/differential.rs` runs every validation project — the four M0 programs, both M1 projects, the D-35 numeric suite, and the M2 `does iterable` walk (nine programs total) — through the interpreter and the native backend in **both dev and release modes**, with the same scripted stdin, the same seed (`LAGOM_SEED`/`Host::with_seed`), and per-program sandboxed directories, and asserts the same ordered output and the same exit code on all three (newlines are normalized, because prompt placement is a projection detail pinned per-feature elsewhere). A backend that observes different semantics cannot pass the suite.
- **Networking (M2):** real sockets and HTTP over the same word surface — `open listener at 8080`, `open connection at 8080 to "localhost"`, `expect connection from my listener`, `send "GET / HTTP/1.1\n\n" over c`, `take text from c`, `close c`, `port of l` — with failure handling through the existing `attempt … if it fails then` shape, so a refused connection is reported, never crashed on. The cross-backend suite gains `net_validation.rs`: an HTTP server and client program that talk to each other as real processes/runs in all four backend pairings (native↔native, native↔interpreted, interpreted↔native, interpreted↔interpreted), with server-side readiness races absorbed by a bounded client retry in the harness — never by the programs. Demo: [validation/http_server.lagom](validation/http_server.lagom) + [validation/http_client.lagom](validation/http_client.lagom).
- **Editor support (M2):** a VS Code extension ships under [`editors/vscode/`](editors/vscode/), driven by the language server: `lagom lsp` is a stdio JSON-RPC server publishing the teaching diagnostics as Problems, answering **hover** with the inferred type of the name under the cursor (sema's span-keyed side table) and **go-to-definition** — the 7.0.3 headline case, a multi-word name run jumps to the `make` that bound it (`resolved_uses`), with a call→callee fallback. The extension layers on: a TextMate grammar whose phrase sets are test-pinned byte-equal to the web tokenizer's lists (extended with M2's net built-ins), the beginner snippets, layer-aware completions (the student surface only — nothing the student layer can't say), and a dependency-free JSON-RPC client that degrades to `lagom check --json` diagnostics if the server cannot start.**Rename and find-references** complete the 7.0.3 story: rename rewrites the callee words of a split run without touching the argument that shares it, refuses punctuation with the rule quoted, covers `set`/`increase`/`decrease` targets (the mutability path records the same edge), and jumps from declaration clicks. Sync is incremental end-to-end: the server advertises `change: 2` and applies ranged edits in batch order (one reanalysis per batch), the client sends the minimal prefix/suffix splice for each keystroke burst, and a one-range fix is test-proven to clear the published errors. Server contract end-to-end-tested over real stdio ([`crates/lagom_cli/tests/lsp_server.rs`](crates/lagom_cli/tests/lsp_server.rs)); session spot-check via `python3 scripts/lsp-smoke.py`. The debugger — doc 09's last open tooling question — was decided at M2 planning for M3 with its rationale recorded there: the M2 gate never named it, and the teaching pane needs doc 06's task/channel stepping semantics validated picture-first. The native Wasm story is decided as D-41 in [00-architecture](docs/00-architecture.md)'s decision log: the browser surface is the interpreter shipped to wasm32 (as it has always been — see the API note above), a wasm-emitting backend is the M5+ INVESTIGATE item.
- **Monomorphization decision made with a benchmark (M2 gate item, doc 08 §"The M2 decision"):** generics compile to one shared body over tagged values — and the M2-datum benchmark ([`scripts/measure_monomorph.py`](scripts/measure_monomorph.py)) shows the erased shared body costs nothing the M2 surface can express: a generic identity loop runs at **24.5 ns/call vs 24.4** hand-specialized, and compile time/binary size stay flat and identical as instantiations grow (N = 16/64/240). Hand-specialization (per-instantiation cloning) is scheduled to wait for a surface that can do operator work inside generic bodies and a measured hot-loop regression above 2% (the D-35 threshold). Also recorded there: an environment finding — natively linking objects with ≥ 254 top-level functions crashes Apple's ld on the dev machine (object sizes/symbols stay linear; investigated separately).

M1 remains the baseline: kinds and `match` with patterns, the full option/result model, closures with `map`/`keep`/`combine`, files, and JSON — verified to behave identically on the interpreter and the native Cranelift backend.

M0 details: Rust bootstrap; lexer → parser → sema → HIR → MIR → LIR → Cranelift, scoped in [00 §33](docs/00-architecture.md#33-mvp-proposal) and [doc 10](docs/10-roadmap.md).

## Reference notes

External status claims in the docs (Carbon pre-0.1, Mojo 1.0 open-sourced August 2026, Zig pre-1.0, Jai closed beta) were checked against project pages and coverage in September 2026; the Cranelift-vs-LLVM comparison cited in [00 §21.2](docs/00-architecture.md#212-backend-pairing-approved-decision) comes from the Bytecode Alliance's Wasmtime measurements and is used as *context only* — Lagom's own numbers must come from Lagom's own benchmarks (decision D-8).
