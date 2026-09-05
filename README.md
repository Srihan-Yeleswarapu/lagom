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
| [11 — Design review](docs/11-design-review.md) | Adversarial architecture review: 24 issue cards, stress tests, contradiction matrix, KEEP/MODIFY/INVESTIGATE/REJECT verdicts |

Docs 01–10 are phase stubs by design: they scope what each specification must contain, bind the decisions already made in 00, and record open questions with decision dates. They fill in as each phase matures.

## Status

**Design phase.** No compiler exists yet, by intention — the design (brief §32/§33) comes first. Next step: M0 planning for the Rust-bootstrap compiler (lexer → parser → sema → HIR → MIR → LIR → Cranelift), scoped in [00 §33](docs/00-architecture.md#33-mvp-proposal) and [doc 10](docs/10-roadmap.md).

## Reference notes

External status claims in the docs (Carbon pre-0.1, Mojo 1.0 open-sourced August 2026, Zig pre-1.0, Jai closed beta) were checked against project pages and coverage in September 2026; the Cranelift-vs-LLVM comparison cited in [00 §21.2](docs/00-architecture.md#212-backend-pairing-approved-decision) comes from the Bytecode Alliance's Wasmtime measurements and is used as *context only* — Lagom's own numbers must come from Lagom's own benchmarks (decision D-8).
