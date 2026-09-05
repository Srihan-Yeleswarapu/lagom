# Phase 7 — Standard Library

*Status: stub — the stdlib architecture is set in [§19 of 00-architecture.md](00-architecture.md#19-standard-library); this document will hold the complete standard-library specification.*
*Next review: at M1 planning (kernel modules land M0–M1).*

## Scope of this document (when filled in)

1. **Module catalog** — one chapter per module, with the full API in student words: `standard` (say/ask/format), `math`, `text`, `lists`, `maps`, `files`, `time`, `random`, `errors`, `testing` (kernel tier); `network`, `http`, `json`, `csv`, `compression`, `cryptography`, `database`, `processes`, `environment`, `logs`, `regular expressions`, `graphics` (batteries tier) ([§19.1 of 00](00-architecture.md#191-architecture-a-small-kernel-batteries-included-stdlib-is-just-a-package)).
2. **API design rules** — naming conventions (verbs, no abbreviations), error behavior per module (`can fail` signatures everywhere failure is possible), capability layer per function (nothing unsafe exposed above the expert layer), async pairing rule (`fetch` / `fetch and wait`, [§15.2 of 00](00-architecture.md#152-runtime-design)), and allocator-parameter policy for containers ([§9.8 of 00](00-architecture.md#98-allocators-and-arenas--expert)).
3. **The stdlib is Lagom** — the self-hosting rehearsal plan ([§19.2 of 00](00-architecture.md#192-the-stdlib-is-written-in-lagom)): module-by-module migration order, the C boundary layer (`C`-prefixed hand-written platform bindings), and the "stdlib as first big Lagom program" validation loop.
4. **Strings deep-dive** — Unicode handling commitments (UTF-8 storage, code-point indexing semantics and its O(n) cost honesty, grapheme-cluster APIs at the intermediate layer, no byte/code-point conflation).
5. **Numerics** — `math` module contents (the school-math set + full expert set), decimal formatting, the `random` module's teaching API (`a random number from 1 to 6`) vs expert API (seeded, algorithm-specified, cryptographically secure variant in `cryptography`).
6. **What stays out and why** — the D-16 exclusion list with per-item rationale ([§19.3 of 00](00-architecture.md#193-what-stays-out-of-the-stdlib--decision-log-d-16)): GUI toolkits, web frameworks, ML, YAML, multiple regex engines.
7. **Versioning policy** — semver for stdlib modules, deprecation policy, and the "stdlib is a plain package" upgrade path ([§19.1 of 00](00-architecture.md#191-architecture-a-small-kernel-batteries-included-stdlib-is-just-a-package)).

## Decisions inherited from 00 (binding here)

- D-16 (batteries included, plain-package implementation), D-12 (`can fail` across all fallible APIs), the crypto fail-closed rule ([§28.6 of 00](00-architecture.md#286-crypto-api)).

## Open questions

- Does `http` ship client-only at M2 with the server at M3, or both at M2? (Propose: client M2, server M3 — the server wants the concurrency spec settled first.)
- `graphics` hooks: SDL-shaped C bindings vs a hand-rolled minimal window/input layer? (Propose: bindings first, native later; decide at M3.)
- Is `csv` in the batteries tier or a package? (Propose: batteries — it is the most-requested classroom format; decide at M1.)

## Cross-references

- [06-concurrency](06-concurrency.md) (async API pairing) · [05-memory](05-memory.md) (allocators) · [09-tooling](09-tooling.md) (doc generation of stdlib docs)
