# Phase 12 — Post-Fix Consistency Pass

*Status: complete. The final consistency pass over the changed areas before the architecture freeze, per the agreed sequence: architecture → adversarial review (doc 11) → fix verified defects → **re-review changed areas (this document)** → architecture freeze → M0.*
*Method: every finding below was verified against the actual document text; every applied change is classified into one of the four buckets agreed for this pass — **(1) required contradiction fix, (2) required implementation clarification, (3) pure wording/documentation improvement, (4) new design decision — DO NOT APPLY**. No change was applied without a bucket; no bucket-4 item exists in this pass.*

---

## 1. Checklist results

### Grammar — PASS (after bucket-2 completions, listed in §2)

| Check | Result |
|---|---|
| EBNF derives every documented example | **Pass.** The unified call production (`flowcall = name [additive] { prep additive } { "and" additive }`) derives `ask "What is your name?"`, `greet "bo"`, `divide 10 and 0`, `send "hello" to messages`, `download "a" to "a.file"`, `bigger of a and b`, `things at 0`. Arguments at additive level (R-4). |
| `attempt` has exactly one production | **Pass.** One `attempt = "attempt" orexpr [attempttail]` used in statement *and* expression position; tails are `and pass the problem on` / `if it fails then …` / `as name …`; the propagation tail is statement-only (parser-checked). |
| No reintroduction of deleted vocabulary | **Pass.** `same as`, `different from`, `either`, dot syntax, `can-name-of`, `start another task`, `second of`, standalone `nothing?`, `float numbers`: zero occurrences as language (prose mentions and one comment "(same as most languages)" excluded; verified by scan). |
| `and` ambiguity rejected, not misparsed | **Pass.** Additive-level arguments make `bigger of a and b or c` a compile error with the two-reading diagnostic; phrase-tokens (`and pass the problem on`, `wait for all tasks`) are unnameable, so no call can consume into them. |
| Lambda/`where` consistency | **Pass** after the bucket-2 fixes: the `using`-lambda is a *comparison* (so `using it plus 5` and `using start plus it` — with the labeled `with` name `start` in scope — both derive); `taking … giving back …` bodies are comparisons; block lambdas are `a function [taking …] block`, attached in `make` (`make twice equal to a function …`) and usable as an inline `where`-body. `keep scores where it is at least 80` derives from `call = flowcall { "using" lambda } { "where" orexpr }` and is declared sugar for the `using taking …` form. |

### Semantics — PASS

| Check | Result |
|---|---|
| `number from answer` makes M0 programs expressible | **Pass** — calculator, guessing game, quiz, text adventure all derive (see §4; the guessing-game `random from 1 to N` dependency was caught and closed in this pass). |
| Equality consistent across operators/collections/copy/interfaces | **Pass** after one bucket-2 addition: the D-30 rule now states it applies *recursively through containers* — a list/map/pair of classes compares elements by identity, so container equality never deep-compares a class (doc 03 §4). Copy interacts via the R-12 shallow-copy note; interfaces via opt-in `does comparable`. |
| Send-as-move consistent with ownership | **Pass.** D-31 in §14.3/doc 06; `shareable` classes are the explicit exception; composes with `sendable`/`shareable` marks (14.4) and the ownership tier (moves are the owning region's transfer, channels the ARC tier's). |
| Lock scope consistently method-wide | **Pass.** D-38 rule + example in §14.4 and doc 06 §4; the tally example's comment now says "the guard is held for the whole method body." |
| Non-escaping borrows: no escaping example | **Pass** after one bucket-2 fix: the R-7 workaround example used `longest of a and b into some function` — **`into` is not in the closed preposition set** (a real bug this pass caught). Replaced with the grammatical continuation form (`longest of a and b using say it`, or a block lambda). |

### Memory — PASS

| Check | Result |
|---|---|
| Shallow-copy rule vs value semantics | **Pass.** §9.2 states the one honest exception, the teaching note, and the lint; doc 05 owns the copy/elide matrix; no contradiction remains with "value semantics by default." |
| ARC/ownership/borrows/regions/unsafe layered correctly | **Pass.** Tier boundaries unchanged; the two-ownership-systems guard (doc 05 §13) preserved; `owned`/`borrowed` parameter modes now appear in the grammar's `takes` production (bucket 2 — §9.6's example was underivable). |
| No beginner-facing syntax exposes expert memory concepts | **Pass.** Student-layer purity scan across all 47 lagom blocks: zero occurrences of `within owning`/`within arena`/`unsafe`/`atomic`/`C structure`/`raw pointer` in student-tagged sections. |

### Errors + observability — PASS

| Check | Result |
|---|---|
| `attempt`/`can fail`/finalizers/async failures/propagation agree | **Pass.** One attempt production; `can fail` ⊥ `can wait` orthogonality stated explicitly (§7.8); finalizers-cannot-fail (R-20.3) in §10.4/doc 05; C-wrapper error construction (R-20.4) in §17.2/doc 07. |
| LOM reflected in compiler/runtime architecture, not just prose | **Pass.** §26.5 names the MIR instrumentation pass as the owner (22.2 already owns all loads/stores/ARC ops); doc 08 owns the pass + ring format + release-identity invariant; doc 09 owns the failure-report format (snapshot-tested); doc 06 owns task events; doc 10 schedules M0→M4; **the M0 exit criterion now includes passing the LOM test on the validation projects' seeded failures.** |
| Dev/release boundary defined | **Pass** after one bucket-2 addition: §26.3 now states the boundary as a *build-mode fact* (dev = instrumented + DAP-debuggable; release = zero LOM code, bit-identical; `--trace` = teaching/replay mode). |
| 90/10 remains an engineering target, not a guarantee | **Pass.** §26.3 says exactly that: the LOM test is the acceptance criterion; budgets bound recording; release behavior is unchanged by design. |

### M0 expressibility — PASS (after the struct alignment, bucket 1)

All eleven items from the checklist are expressible in the student layer with no M1+ features. **One contradiction existed and was fixed:** the user's list includes *structs*, but the milestone tables put structs at M1 while §7.11 tags them `student`. Resolution: the **spec does not change** — §7.11 already owns structs as student-layer; the *milestone tables* (33.1 item 3, the doc-10 M0/M1 rows) were aligned to the spec (structs at M0). Spec wins over roadmap; that is the "00 is authoritative" rule operating.

Also verified: `repeat for each … in …` takes a *pattern* (`foreach = "for each" pattern "in" expr`), which covers "for each score in scores" — no destructure-everywhere ambiguity is required for M0. The fourth validation project (text adventure) needs nothing beyond the gate list.

### Cross-document consistency — PASS

- **00 authoritative; docs 01–10 carry no contradictory syntax/semantics** — verified by re-read; every doc's "inherited decisions" section now names its D-30…D-39 subset with a dated note.
- **Doc 11 accurately describes current state** — its status block now says the approved §18 subset is applied (R-1…R-23 list) and the remaining items await the freeze; issue cards unchanged (they describe the pre-fix state, by design — ⚠ marks quote the *then*-current defects).
- **Decision log D-1…D-39 matches the architecture** — all 39 rows exist; every D-reference in all docs resolves (verified by scan); no row contradicts the text it is cited from.

---

## 2. Applied changes, classified

### Bucket 1 — Required contradiction fixes (2)

1. **Struct milestone alignment** (§33.1 item 3; doc 10 M0/M1 rows): the checklist requires structs at M0; the spec already says student; the tables said M1. Fixed the tables to match the spec.
2. **`into` is not a preposition** (§9.6 R-7 ladder): the workaround example used a word outside the closed preposition set — the review's own example violated the review's own grammar. Replaced with the `using`-continuation form.

### Bucket 2 — Required implementation clarifications (7, all grammar-completeness for already-documented examples)

1. **`target` production** — `set`/`increase` targets are `name { prep additive }`, making `set things at 2 to "plum"` and `increase score of p by 10` derivable (§7.6/§7.11 examples existed).
2. **Block lambda (`funclit`)** — `make twice equal to a function …` (§11.1's flagship) now derives: `make` accepts `a function [taking …] block`, whose block may open with takes/returns clauses; also usable as an inline `where` body.
3. **`using`-lambda expression form** — the lambda production's first alternative is a *comparison* with `it` (and preceding `with`-bound names) in scope, so `using it plus 5` and the §11.2 `combine scores with start 0 using start plus it` both derive as written.
4. **`takes` production** — gains `[parammode]` (`owned`/`borrowed`, expert) and `[called]`, so §9.6's `takes an owned buffer called input` and the brief's `takes number of correct answers` both derive. One production, no new syntax.
5. **`use` production** — the three §7.13 forms (`use math for …`, `use drawing`, `use cards from "…"`) are now explicit; previously the sketch referenced `use` without defining it.
6. **`unsafe` production** — carries the optional `because` justification (18.1's compiler-enforced lint).
7. **Class/interface heads** — `class name ["of" name]` (12.2's generic `class box of some type` was underivable), `interface name ["of" name] { interfacereq }` (10.6's requirements), and **`constr` rewritten to clause form** (`construction { takes | canfail | canwait } block`) to match §10.3's example — the earlier `construction with step of type number` shape contradicted the `takes`-clause rule (bucket 1 in spirit; the example was also repaired to use clauses).

### Bucket 3 — Wording/documentation improvements (3)

1. **Expert type-grammar note** on the `type` production: the expert extensions (fixed-size ints/decimals, C/pointer types) are layer-gated and owned by doc 04 — stated where a reader looks for them.
2. **§26.3 debugging bullet** — the LOM dev/release boundary and the "engineering target, not guarantee" sentence (quoted in §1 above).
3. **§7.8 clause-order sentence** — `can fail` and `can wait` are orthogonal and compose on one function (was implicit; now stated where the clauses are listed).

### Bucket 4 — New design decisions proposed

**None.** Zero items. The pass deliberately produced no new features, no new vocabulary, no new mechanisms; every applied change makes existing documented syntax derivable or states an implicit rule.

---

## 3. What this pass did NOT re-litigate

Per the pass's charter: no decision from D-1…D-39 was reopened; no doc-11 KEEP/INVESTIGATE item was re-argued (the INVESTIGATE items keep their M1–M4 dates); no syntax outside the changed areas was redesigned. The two REJECT-list items that surfaced (`into`, the old `constr` shape) were *review-introduced or pre-fix* artifacts, not live proposals.

---

## 4. The frozen-state claim

With this pass applied, the architecture satisfies the brief's freeze conditions:

1. The §7.15 sketch derives every code example in the documents (47 blocks audited; zero banned vocabulary; zero non-grammatical connectors).
2. The semantics documents (03/04/05/06) name every rule the compiler will need for M0–M2, with owners.
3. The decision log (D-1…D-39) is the single source of truth for every non-obvious choice, each with alternatives and accepted tradeoffs.
4. The M0 gate is fully expressible in the student layer, and the M0 exit criteria include the LOM acceptance test.
5. Doc 11 documents *why* each fix was made; doc 12 (this document) classifies every post-review change; the git history holds the before-state.

**Verdict: the architecture is frozen for M0.** The next prompt switches modes: implement the compiler against the frozen specification (crate layout, lexer/parser generated test corpus from §7.15, the four validation projects as the acceptance suite). No further design passes are scheduled before M0; any design change discovered during implementation goes through the decision-log process, not ad-hoc edits.

---

## 5. Verification (this pass)

1. Internal + cross-file links: pass (anchor-slug verified, all 12 docs).
2. Code fences: balanced in all 12 documents.
3. Grammar-vs-examples: all lagom blocks derive from the revised sketch (the audit's two "hits" are a prose comment and filename string literals — verified non-code).
4. Keywords: no snippet uses vocabulary outside the sketch + the documented expert extensions.
5. Layer tags: every feature section tagged; student-layer purity scan clean.
6. Decision-log consistency: D-1…D-39 all defined; all references resolve; no contradictions found.
7. Cross-document contradictions: none found after the bucket-1 fixes.
8. Doc 11 status updated to reflect the applied state; this document added to the README table.
