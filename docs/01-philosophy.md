# Phase 1 — Language Philosophy

*Status: stub — inherits decisions from [00-architecture.md](00-architecture.md), to be expanded into the full philosophy specification.*
*Next review: at M0 planning.*

## Scope of this document (when filled in)

1. **Goals and non-goals** — the ranked goal list with full rationale and evidence, including the goal-ranking experiment history (what the project expects to gain and lose from each ranking).
2. **Design principles** — each principle from [§3 of 00](00-architecture.md#3-design-principles) expanded into: statement, motivation, concrete examples of applying it, concrete examples of *declining* to apply it, and the test that decides disputes between principles (goal 1 outranks goal 2 outranks goal 3; performance claims always defer to measurement).
3. **Target users** — persona studies with real observation notes (not invented personas), including the 10-year-old, the CS student, the professional, and the systems programmer; per-persona "first 30 minutes" walkthroughs.
4. **Educational philosophy** — the transfer-first doctrine ([§27 of 00](00-architecture.md#27-educational-progression)); the "hide punctuation, not concepts" guardrail; the concept-inventory (the full list of concepts Lagom commits to teaching, mapped to syntax and to lessons); position on direct instruction vs discovery in tooling/docs.
5. **Performance philosophy** — the measurement discipline ([§25 of 00](00-architecture.md#25-runtime-performance-strategy)); the claim-ownership rule (no unmeasured claims, D-8); the tiered optimization stance; what "fast enough" means per persona.
6. **Philosophy of power** — the layered-capability model ([§0 of 00](00-architecture.md#0-how-to-read-this-document-layers)): "a beginner should encounter less complexity, not a less capable language"; progressive disclosure as a language property, not a UI property.

## Decisions inherited from 00 (binding here)

- D-2 (one language, layered exposure), D-3 (word-based, formally specified), D-5 (immutable by default), D-6 (words for comparisons/logic), D-28 (validation with real users at every milestone).

## Open questions

- How much vocabulary growth is acceptable per release? (Propose: a frozen ~120-word core + a documented approval process; owner decision needed.)
- Should localized *diagnostics* precede localized documentation? (Propose: diagnostics first, no localized keywords ever — D-3.)
- What is the project's stance on "playground-first" vs "install-first" onboarding for schools? (Ties to tooling doc 09; decide at M2.)

## Cross-references

- [00-architecture](00-architecture.md) · [09-tooling](09-tooling.md) (educational tooling) · [10-roadmap](10-roadmap.md) (validation ladder)
