# Cleanup round — deferred items + standalone-lib scrub

User directive (2026-07-06): clean up the deferred items; remove ALL Dart/clean_signals-Dart references from the library's descriptions and comments — this is a standalone Rust lib (YAGNI: no lineage references). Deferred list source: parent TASKS.md § Deferred Items.

## Tasks

| # | Task | Status | Complexity | Depends On | Files (Write) |
|---|------|--------|------------|------------|---------------|
| 1 | [cln-01-code-deferred-and-scrub](tasks/cln-01-code-deferred-and-scrub.md) | Not Started | high | - | `crates/**`, `examples/**` |
| 2 | [cln-02-docs-scrub](tasks/cln-02-docs-scrub.md) | Not Started | medium (Agent: doc_maintainer) | - | `docs/*.md`, `templates/AGENTS.md` |

## File Overlap Analysis

| Task | Files Modified (Write) | Files Read |
|------|----------------------|------------|
| cln-01 | crates/clean-signals/src/*.rs, crates/clean-signals-leptos/src/*.rs, examples/team-demo/** (incl. its Cargo.toml — dep removal), crates/clean-signals/src/lib.rs | parent TASKS.md Deferred Items |
| cln-02 | docs/ARCHITECTURE.md, docs/CODE_STANDARDS.md, docs/DEVELOPMENT.md, templates/AGENTS.md | cln-01 task file (async_trait re-export convention) |

### Overlap Matrix

| Task Pair | Shared Write Files | Isolation Strategy |
|-----------|-------------------|-------------------|
| cln-01 + cln-02 | None | Parallel (worktree) |

## Explicitly ACCEPTED (not implemented — YAGNI / future)

- `use_load_once` framework helper: new API surface, not needed (example shows the gated pattern).
- loom / multi-thread stress test for dispose/watch: future hardening pass.
- AsyncView clone-per-read redesign: inherent to `Signal<AsyncState<T>>` + owned-children design; document the cost instead.
- run/run_into failure-emission TOCTOU for cross-thread app-scoped controllers: document as narrow accepted limitation instead of lock-coordinating the hot path.

## Success Criteria

- `grep -rni 'dart|flutter|AsyncDataReloading' crates/ examples/ docs/ templates/` → zero hits.
- Deferred items either implemented (with tests where behavioral) or listed under ACCEPTED above.
- Full DoD green incl. `cargo fmt --all -- --check`.
