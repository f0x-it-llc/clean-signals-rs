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
