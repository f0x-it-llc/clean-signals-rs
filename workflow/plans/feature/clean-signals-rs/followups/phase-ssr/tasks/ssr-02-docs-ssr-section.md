## Task: ssr-02 — Document the SSR architecture

**Agent:** doc_maintainer

**Objective**: Add the SSR story to the core docs and downstream template, grounded in the merged team-demo-ssr example.

**Depends on**: ssr-01 (merged)

**Complexity:** medium

### Scope

**Files Modified (Write):**
- `docs/ARCHITECTURE.md`: an "SSR applications" subsection — same layers/controllers/use cases as CSR; what changes: cfg-gated composition roots (server main vs hydrate entry), repository transport (server functions with one ServerFnError→AppFailure conversion site), initial-load strategy (SSR-shell: server renders loading state, client fires the load post-hydration; effects don't run under ssr, so no hydration mismatch). Note the deliberate non-goal: first-paint server data via a Resource bridge is out of scope until an app needs it (YAGNI). Reference both examples by path.
- `templates/AGENTS.md`: extend the DI/composition-root rules for SSR apps (two roots, cfg-gated; server functions live in the data layer as transport; controllers/pages must stay render-mode-agnostic — no ssr cfg in presentation beyond the standard wasm-gated initial-load spawn).
- `docs/DEVELOPMENT.md`: the team-demo-ssr build/verify commands (both feature-target combos) if not already accurate.

**Files Read:** merged `examples/team-demo-ssr/**` + ssr-01's completion summary, `~/.claude/skills/doc-standards/schemas.md`.

### Acceptance Criteria

1. Docs describe the REAL merged example (verify claims against code, not the task file).
2. Size caps respected; content boundaries respected; zero Dart references.
3. AGENTS.md rules keep presentation render-mode-agnostic as a MUST.
