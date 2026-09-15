# Phase 6 — Concurrency Model

*Status: stub — the concurrency and async architecture is set in [§14 and §15 of 00-architecture.md](00-architecture.md#14-concurrency-model); this document will hold the complete concurrency specification.*
*Next review: at M2 planning (concurrency lands M2–M3).*

## Scope of this document (when filled in)

1. **The concurrency ladder** — per-layer exposure contract ([§14.1 of 00](00-architecture.md#141-the-layered-answer)): student sees nothing; intermediate sees framework timers; advanced sees tasks/channels/`can wait`/`shared`; expert sees atomics/lock-free/unsafe threading.
2. **Structured tasks** — spawn syntax (`start a task`, repeated per spawn; the phrase-token `wait for all tasks`), region-join semantics, cancellation propagation (every await point is a cancellation point), `in the background` scoping rules, **supervision (resolved: fail-fast by default — a failed child fails the region unless the task declares `keep going`)**, and task-local state (**rejected for v1** — module constants + channel state cover it; a `task local` feature is shared-mutable-state-shaped) ([§14.2 of 00](00-architecture.md#142-structured-tasks--advanced)).
3. **Channels** — type parameters, buffering modes (bounded/unbounded), closing semantics (**explicit close — `repeat for each` does not close the channel**), `repeat for each` integration, `can wait` variants, select-like composition (propose: `wait for the first of …`), backpressure teaching story ([§14.3 of 00](00-architecture.md#143-channels--advanced)). **Send = move (D-31, R-8):** a send transfers ownership — the sender loses access, checker-enforced; `shareable` classes may be sent as shared references only (the Send/Sync split). Channel operations (`send`/`receive`) are cancellation points like awaits.
4. **Shared memory and race freedom (R-9 fix, D-38)** — `sendable`/`shareable` semantics (the Send/Sync port), **lock scope = the whole method body** on classes with `shared` fields (the method is the unit of atomicity; outside-method field access is a compile error — the lost-update hazard is closed by construction), the compile diagnostics for race-class errors, and the debug-mode race sanitizer ([§14.4 of 00](00-architecture.md#144-shared-memory-and-race-freedom--advanced)).
5. **`can wait` state machines** — the MIR lowering contract ([§22.2 of 00](00-architecture.md#22-ir-design)), the color rules (what may call what; deadlock-by-color diagnostics), cancellation tokens, and the stdlib sync/async API pairing rule ([§15.2 of 00](00-architecture.md#152-runtime-design)).
6. **Ownership × async (doc 05 contract)** — an owning function that is also `can wait` holds owned values across await points; moves across awaits are checker-visible; spec the rule here with doc 05.
7. **Atomics and memory ordering** — the expert API (`load with relaxed ordering` etc.), the ordering ladder, and the fence between safe and unsafe concurrency ([§14.5 of 00](00-architecture.md#145-atomics-and-lock-free-programming--expert)).
8. **Scheduler contract** — work-stealing pool sizing, fair scheduling guarantees (propose: fairness best-effort, documented, no hard contract at v1), blocking-call policy (detection + diagnostic when a `can wait` task blocks a pool thread; **the stdlib marks every blocking API so the checker can warn; C calls are blocking by definition — a `can wait` function calling C must be flagged unless the wrapper provides an async variant**), and embedded/no-runtime behavior (tasks disabled, document the fallback).
9. **Shutdown semantics** — region-join at `main`'s exit gives quiescence; unclosed channels with waiting senders at shutdown are a diagnostic, not a hang.
10. **Task events for LOM (D-32)** — spawn/join/cancel events feed the observability model ([00 §26.5](00-architecture.md#265-the-lagom-observability-model-lom--the-9010-debugging-contract)); doc 09 owns the failure-report format.

## Decisions inherited from 00 (binding here)

- D-17 (capability-clause async, state machines, no green threads in v1), D-24 (tasks+channels first), D-25 (type-level race prevention).
- **From the design review (doc 11), applied 2026-09:** D-31 (**send = move**), D-38 (**lock scope = method body**), fail-fast supervision default, channel operations as cancellation points, the blocking-FFI checker rule, shutdown semantics, and the task-local-state rejection.

## Open questions

- ~~Supervision semantics for task regions~~ — **resolved:** fail-fast by default, `keep going` per-task override (structured-concurrency precedent; spec here).
- Timer/timeout primitives: builtin (`wait for 2 seconds`?) or stdlib-only? (Propose: stdlib; decide at M2.)
- ~~Does `repeat for each` over a channel close the channel at loop exit?~~ — **resolved:** no — explicit close.

## Cross-references

- [05-memory](05-memory.md) (`sendable`/`shareable` ↔ ownership) · [07-stdlib](07-stdlib.md) (network/file async APIs) · [08-compiler](08-compiler.md) (state-machine lowering)
