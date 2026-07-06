## Task: cln-02 — Docs/template Dart scrub + AGENTS.md accuracy fixes

**Agent:** doc_maintainer

**Objective**: Remove every Dart/Flutter lineage reference from docs/ and templates/ (standalone Rust lib), reframe comparison sections as self-contained design rationale, and fix the two known accuracy gaps in the docs.

**Depends on**: None (parallel with cln-01; coordinate via cln-01's task file, not code)

**Complexity:** medium

### Scope

**Files Modified (Write):** `docs/ARCHITECTURE.md`, `docs/CODE_STANDARDS.md`, `docs/DEVELOPMENT.md`, `templates/AGENTS.md`

**Files Read:** `~/.claude/skills/doc-standards/schemas.md`, `workflow/plans/feature/clean-signals-rs/followups/phase-cleanup/tasks/cln-01-code-deferred-and-scrub.md` (the async_trait re-export + other conventions landing in parallel — document THAT convention, not the old one)

### Details

1. **Dart scrub** — `grep -rni 'dart|flutter|AsyncDataReloading' docs/ templates/` → zero. Specifics found in inventory: ARCHITECTURE.md line 5 (intro "is a Rust port of the Dart clean_signals framework" → describe what the lib IS: a clean-architecture framework for Leptos apps), line 17 (example description), § "Design Deltas vs. the Dart Original" → retitle/rewrite as "Design Rationale" with each delta stated as a standalone decision + why (generic failure type → exhaustive matching; std Result → no custom result type; callback-registry failure events → no channel dependency; drop-based stream cancellation); line 102 stream-subscription comparison. CODE_STANDARDS.md line 76 "Dart-record-equivalent" → just "a small named struct, not a tuple". Also purge any `research/dart-test-spec.md` / SPIKE_NOTES cross-references that leak pipeline-internal paths into user-facing docs — docs must stand alone (describing the workflow/ dir in DEVELOPMENT.md's repo-layout section is fine; citing research files as doc justification is not).
2. **AGENTS.md controller-layer wording fix** (deferred item): the rule "may depend on `reactive_graph` signal types, never `leptos::prelude::*`" contradicts minimal apps whose only path to signal types IS leptos::prelude (they don't declare reactive_graph). Reword to match the reference implementation: controllers import signal types via NAMED imports (from `leptos::prelude` or `reactive_graph` — whichever the crate already depends on), never wildcard `leptos::prelude::*`, and never view/DOM items.
3. **async_trait convention update**: CODE_STANDARDS.md's cfg_attr pattern section + AGENTS.md's corresponding rule should show the `clean_signals::async_trait` re-export form (landing in cln-01) so downstream apps don't add their own async-trait dependency.
4. Keep every doc within its existing size cap; targeted edits, no restructuring beyond the retitled section.

### Acceptance Criteria

1. Zero case-insensitive Dart/Flutter/AsyncDataReloading matches in docs/ + templates/.
2. Docs stand alone (no reliance on workflow/research files for meaning).
3. AGENTS.md controller rule matches team-demo reality; async_trait convention shows the re-export.
4. doc-standards content boundaries respected; size caps respected.

---

## Completion Summary

**Status:** Done
**Branch:** worktree-wf_357712b9-098-2

### Files Modified

| File | Changes |
|------|---------|
| `docs/ARCHITECTURE.md` | Overview no longer frames the crate as a "Rust port of the Dart clean_signals framework" — describes it as a clean-architecture framework for Leptos apps. `examples/team-demo` row drops the "ported from the Dart example" framing. "Design Deltas vs. the Dart Original" retitled "Design Rationale" and rewritten as six standalone decision+why bullets (exhaustive failure typing, std `Result`, composition over inheritance, `Memo` over `Effect`, channel-free `FailureSink`, drop-based `watch` cancellation) with the `research/SPIKE_NOTES.md` citation removed. |
| `docs/CODE_STANDARDS.md` | "Dart-record-equivalent" wording replaced with plain "a small named struct, not a tuple". `cfg_attr` async_trait pattern section now shows the `clean_signals::async_trait` re-export form instead of raw `async_trait::async_trait`, with a note that it exists so downstream crates don't pin their own `async-trait` dependency. |
| `docs/DEVELOPMENT.md` | Dropped the `research/SPIKE_NOTES.md` Q5 citation from the `watch`/`spawn_local` test recipe intro (recipe itself unchanged); repo-layout mention of the `research/` subdirectory under Workflow locations retained as-is (directory description, not doc justification). |
| `templates/AGENTS.md` | Controller rule reworded: controllers import signal types via named imports from whichever of `leptos::prelude`/`reactive_graph` the crate already depends on, never a wildcard `leptos::prelude::*`, never render/DOM items — replacing the prior "may depend on `reactive_graph` signal types, never `leptos::prelude::*`" wording that contradicted minimal apps with no direct `reactive_graph` dependency. Added a Domain-rules bullet showing the `clean_signals::async_trait` cfg_attr pair for async use cases, cross-referencing `docs/CODE_STANDARDS.md` instead of duplicating rationale. |

### Content Boundary Compliance

- All updates within correct document boundaries: YES
- Cross-contamination detected and fixed: N/A
- No content duplicated across docs / into CLAUDE.md: YES (no `CLAUDE.md` exists in this repo; AGENTS.md async_trait note cross-references CODE_STANDARDS.md rather than restating the rationale)

### Size Discipline

| Doc | Lines before | Lines after | Under cap? |
|-----|-------------|------------|-----------|
| `docs/ARCHITECTURE.md` | 119 | 122 | YES |
| `docs/CODE_STANDARDS.md` | 122 | 124 | YES |
| `docs/DEVELOPMENT.md` | 135 | 134 | YES |
| `templates/AGENTS.md` | 187 | 194 | YES |

All four docs stayed well under their respective targets/hard caps; net line growth was minimal (retitled section replaced 1:1 at similar length, plus two small necessary additions in AGENTS.md for the async_trait convention and reworded controller rule).

### Notable Decisions/Tradeoffs

1. **AGENTS.md async_trait addition is new, not just reworded**: the task file asked for the re-export form to show up in "AGENTS.md's corresponding rule," but no async_trait rule previously existed there — added one concise bullet cross-referencing CODE_STANDARDS.md for rationale rather than duplicating the "why," to stay net-neutral on size.
2. **cln-01 (code-side async_trait re-export) had not landed in this worktree** at time of writing — docs were updated to describe the target convention (`clean_signals::async_trait` re-export) per the task's explicit instruction to "document THAT convention, not the old one," since the two tasks land in parallel.
