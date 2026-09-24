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

## License

Lagom is **source-available**, not open-source — see [LICENSE.md](LICENSE.md).

- **You may:** view the source, use the compiler and tools to build and run your own Lagom programs (your `.lagom` files are entirely yours), and keep private backups.
- **You may not, without written permission:** modify the compiler or its tooling, redistribute it (source or binary, including public forks with changes), or present it as your own work.

Copyright © 2026 Srihan Yeleswarapu. Permission requests: srihan.yeleswarapu@gmail.com.

Docs 01–10 are phase stubs by design: they scope what each specification must contain, bind the decisions already made in 00, and record open questions with decision dates. They fill in as each phase matures.

## Installing

Install from the [**latest release**](https://github.com/Srihan-Yeleswarapu/lagom/releases/latest): Windows grabs the zip below; macOS and Linux grab one file — **`install.sh`** — from the same page's **Assets** and run it. The release page gives you the exact steps for each OS — verify, unpack, run the day-one flow, and (optionally) add it to PATH. Everything is built and machine-verified by CI before the release publishes.

| File | For |
|---|---|
| `lagom-Windows.zip` | Windows 10/11 (x64) |
| `lagom-Linux.tar.gz` | Linux (x64) |
| `lagom-macOS.tar.gz` | macOS (Apple Silicon) |

On macOS and Linux, this is the whole install:

1. On the release page, scroll down to the **Assets** file list (just below the release notes) and click **`install.sh`** — it downloads to your **Downloads** folder.
2. Open the **Terminal** app: on macOS press **⌘ + Space**, type `Terminal`, press Enter (on Linux: **Ctrl + Alt + T**).
3. Type `sh ` (with a space after it), **don't press Enter yet**, then drag `install.sh` from Downloads into the Terminal window — its path appears by itself — and press Enter.
4. You should see `==> sha256 OK`, then `==> installed to ~/.lagom (lagom …)`.
5. Close Terminal, open it again, and try `lagom version`.

No clone, no Rust, no account needed — the script picks the right package for your CPU, verifies its checksum, installs to `~/.lagom`, and adds one PATH line to your shell config, idempotently. `sh install.sh --uninstall` reverses it; `sh install.sh --version vX.Y.Z` pins a release; `--dir` chooses another folder.

### From source (any OS)

```
cargo install --path crates/lagom_cli
```

This builds the compiler from this checkout and puts `lagom` on your PATH (removal: `cargo uninstall lagom`).

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
| `lagom license` | print the license terms (the Lagom License) |
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

## Status

**M2: intelligence and tooling (current).** Everything from M1, plus:

- **Type aliases (8.4):** `a type called score is a number` — transparent names, order-free, with cycle detection; listed with `lagom words --types`.
- **Causal diagnostics:** every fix comes with why it solves the problem (`This fixes it because …`), provenance labels quote where a bad value was made, and ambiguous intent says so instead of guessing. Intentionally-broken programs are regression-tested to reject misleading fixes.
- **`lagom play`:** the REPL — incremental evaluation on the interpreter, teaching mode showing each new binding with its type, `#replay` re-running the accepted history deterministically.
- **`lagom doc`:** `##` doc comments become markdown pages; `## >>>` examples are real statements checked against `## =` expected output with `--check`.
- **`lagom add`/`remove`:** a package workflow — vendored sources in `packages/`, recorded in `Lagom.toml`, pinned in `Lagom.lock`, merged into every build.
- **`lagom profile`:** exact per-function call counts from the LOM event ring (dev runs only) plus the compile-time breakdown.
- **Richer trace/replay:** `lagom run --trace` now prints the step/value timeline of the run; seeded runs replay to byte-identical timelines.

M1 remains the baseline: kinds and `match` with patterns, the full option/result model, closures with `map`/`keep`/`combine`, files, and JSON — verified to behave identically on the interpreter and the native Cranelift backend.

M0 details: Rust bootstrap; lexer → parser → sema → HIR → MIR → LIR → Cranelift, scoped in [00 §33](docs/00-architecture.md#33-mvp-proposal) and [doc 10](docs/10-roadmap.md).

## Reference notes

External status claims in the docs (Carbon pre-0.1, Mojo 1.0 open-sourced August 2026, Zig pre-1.0, Jai closed beta) were checked against project pages and coverage in September 2026; the Cranelift-vs-LLVM comparison cited in [00 §21.2](docs/00-architecture.md#212-backend-pairing-approved-decision) comes from the Bytecode Alliance's Wasmtime measurements and is used as *context only* — Lagom's own numbers must come from Lagom's own benchmarks (decision D-8).
