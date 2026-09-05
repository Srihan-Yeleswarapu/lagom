# Phase 6 — Concurrency Model

*Status: stub — the concurrency and async architecture is set in [§14 and §15 of 00-architecture.md](00-architecture.md#14-concurrency-model); this document will hold the complete concurrency specification.*
*Next review: at M2 planning (concurrency lands M2–M3).*

## Scope of this document (when filled in)

1. **The concurrency ladder** — per-layer exposure contract ([§14.1 of 00](00-architecture.md#141-the-layered-answer)): student sees nothing; intermediate sees framework timers; advanced sees tasks/channels/`can wait`/`shared`; expert sees atomics/lock-free/unsafe threading.
2. **Structured tasks** — spawn syntax, region-join semantics, cancellation propagation (every await point is a cancellation point), `in the background` scoping rules, the supervision question (does a failed child fail the region? propose: yes by default, `keep going` per-task override — Elixir's supervision idea, structured), and task-local state ([§14.2 of 00](00-architecture.md#142-structured-tasks--advanced)).
3. **Channels** — type parameters, buffering modes (bounded/unbounded), closing semantics, `repeat for each` integration, `can wait` variants, select-like composition (propose: `wait for the first of …`), backpressure teaching story ([§14.3](00-architecture.md#143-channels--advanced)).
4. **`can wait` state machines** — the MIR lowering contract ([§22.2 of 00](00-architecture.md#22-ir-design)), the color rules (what may call what; deadlock-by-color diagnostics), cancellation tokens, and the stdlib sync/async API pairing rule ([§15.2 of 00](00-architecture.md#152-runtime-design)).
5. **Shared memory and race freedom** — `sendable`/`shareable` semantics (the Send/Sync port), `shared` fields + `guard` locking rules, the compile diagnostics for race-class errors, and the debug-mode race sanitizer ([§14.4 of 00](00-architecture.md#144-shared-memory-and-race-freedom--advanced)).
6. **Atomics and memory ordering** — the expert API (`load with relaxed ordering` etc.), the ordering ladder, and the fence between safe and unsafe concurrency ([§14.5 of 00](00-architecture.md#145-atomics-and-lock-free-programming--expert)).
7. **Scheduler contract** — work-stealing pool sizing, fair scheduling guarantees (propose: fairness best-effort, documented, no hard contract at v1), blocking-call policy (what happens when a `can wait` task blocks a pool thread — detection + diagnostic), and embedded/no-runtime behavior (tasks disabled, document the fallback).

## Decisions inherited from 00 (binding here)

- D-17 (capability-clause async, state machines, no green threads in v1), D-24 (tasks+channels first), D-25 (type-level race prevention).

## Open questions

- Supervision semantics for task regions (fail-fast vs `keep going`) — decide at M2 with the structured-concurrency spec.
- Timer/timeout primitives: builtin (`wait for 2 seconds`?) or stdlib-only? (Propose: stdlib; decide at M2.)
- Does `repeat for each` over a channel close the channel at loop exit? (Propose: no — explicit close; decide at M2.)

## Cross-references

- [05-memory](05-memory.md) (`sendable`/`shareable` ↔ ownership) · [07-stdlib](07-stdlib.md) (network/file async APIs) · [08-compiler](08-compiler.md) (state-machine lowering)
