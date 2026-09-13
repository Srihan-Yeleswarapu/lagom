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

## Getting the command

**From a checkout** (requires a Rust toolchain — the compiler's implementation detail, not yours):

```
cargo install --path crates/lagom_cli
```

This installs `lagom` onto your PATH. It works from any directory.

**From a release artifact** (once the CI release story lands): download the artifact for your platform from a release workflow run, unpack it, and put its `lagom` binary on your PATH.

## First program

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
| `lagom run --trace` | run on the teaching interpreter — live probes and a failure report when things go wrong |
| `lagom build [file] [-o out]` | compile to a native executable |
| `lagom test [file]` | run this file's `test` blocks |
| `lagom check [file]` | parse and check; show diagnostics |
| `lagom fmt [file] [--write]` | canonically format (print, or rewrite with `--write`) |
| `lagom explain <code>` | the teaching write-up for an error code |
| `lagom words` | the reserved words |

Everything works from any directory on a single `.lagom` file — no project manifest needed.

## Status

**M1: data and safety.** Everything from M0 — `lagom run`, `build`, `test`, `check`, `fmt`, `new`, `explain`, `words` — plus the M1 feature set: kinds and `match` with patterns (including pair destructuring), the full option/result model (`text?`, `a T or nothing`, `when something with value`, `attempt … if it fails then … otherwise …`), closures (`taking … giving back`, `it`, `where`) with `map`/`keep`/`combine`, file reading and writing, and JSON — each verified to behave identically on the interpreter (`--trace`) and the native Cranelift backend. Validation programs ([file organizer](validation/file_organizer.lagom), [CSV parser](validation/csv_parser.lagom)) run on both backends, and every diagnostic on the M1 surface has a `lagom explain` write-up.

M0 details: Rust bootstrap; lexer → parser → sema → HIR → MIR → LIR → Cranelift, scoped in [00 §33](docs/00-architecture.md#33-mvp-proposal) and [doc 10](docs/10-roadmap.md). `doc`, `play`, `add`, and `profile` are M2+ (doc 09/10).

## Reference notes

External status claims in the docs (Carbon pre-0.1, Mojo 1.0 open-sourced August 2026, Zig pre-1.0, Jai closed beta) were checked against project pages and coverage in September 2026; the Cranelift-vs-LLVM comparison cited in [00 §21.2](docs/00-architecture.md#212-backend-pairing-approved-decision) comes from the Bytecode Alliance's Wasmtime measurements and is used as *context only* — Lagom's own numbers must come from Lagom's own benchmarks (decision D-8).
