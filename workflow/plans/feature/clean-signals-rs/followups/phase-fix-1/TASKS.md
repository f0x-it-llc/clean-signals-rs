# Followup fix round 1 — Task Index

Fixes for review round 0's five confirmed Majors (F1–F5). Diagnoses + verified fix approaches: [DIAGNOSES.md](DIAGNOSES.md). Parent review: `workflow/reviews/clean-signals-rs/REVIEW.md`.

## Tasks

| # | Task | Status | Complexity | Depends On | Findings | Files (Write) |
|---|------|--------|------------|------------|----------|---------------|
| 1 | [fix-01-controller-hardening](tasks/fix-01-controller-hardening.md) | Not Started | high | - | F2, F3, F5 | `crates/clean-signals/src/controller.rs` |
| 2 | [fix-02-deterministic-tests](tasks/fix-02-deterministic-tests.md) | Not Started | medium | fix-01 | F1 | `crates/clean-signals/src/use_case.rs`, `crates/clean-signals/src/controller.rs` (tests only) |
| 3 | [fix-03-example-liveness-gate](tasks/fix-03-example-liveness-gate.md) | Not Started | low | - | F4 | `examples/team-demo/src/features/team/presentation/pages.rs`, `.../components.rs` |

## File Overlap Analysis

| Task | Files Modified (Write) | Files Read |
|------|----------------------|------------|
| fix-01 | `crates/clean-signals/src/controller.rs` | DIAGNOSES.md, docs/CODE_STANDARDS.md |
| fix-02 | `crates/clean-signals/src/use_case.rs`, `crates/clean-signals/src/controller.rs` | DIAGNOSES.md |
| fix-03 | `examples/team-demo/src/features/team/presentation/{pages,components}.rs` | DIAGNOSES.md, crates/clean-signals-leptos/src/hooks.rs |

### Overlap Matrix

| Task Pair | Shared Write Files | Isolation Strategy |
|-----------|-------------------|-------------------|
| fix-01 + fix-02 | `controller.rs` | Sequential (same branch, 01 then 02) |
| fix-01 + fix-03 | None | Parallel (worktree for 03) |
| fix-02 + fix-03 | None | Parallel |

## Deferred (explicitly NOT this round — from investigation/minors)

- Official `use_load_once`/scoped-spawn helper in clean-signals-leptos (new API surface; enhancement, not a fix).
- `async_trait` re-export from core; AGENTS.md reactive_graph wording; `cargo fmt` sweep; `AsyncView` clone-per-read; `RetryPolicy::new(0,_)` clamp test; `dispose()` not cancelling in-flight `run`.

## Success Criteria

- All five findings' fix approaches from DIAGNOSES.md implemented with the pinned new tests (registry-pruning tests, watch/dispose race test, panicking-listener test, zero synchronization sleeps remaining in controller tests).
- Full DoD from docs/DEVELOPMENT.md green (build, test, clippy -D warnings, wasm checks).
