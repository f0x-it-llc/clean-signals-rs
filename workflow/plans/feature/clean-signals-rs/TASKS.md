# clean-signals-rs — Task Index

## Overview

Build the clean-signals Rust workspace: reactive_graph-based core, Leptos integration crate, example app, docs/AGENTS template, cupline retrofit validation. API contract and locked design in [PLAN.md](PLAN.md).

**Total Tasks:** 10 (+ Wave-0 scaffold done inline by conductor)

## Task Dependency Graph

```
Wave 0: scaffold (conductor inline, initial commit)
Wave 1: 01-spike-reactive-graph        02-failure-trait
Wave 2: 03-use-case   04-retry-time   05-activity-async-state     (all depend on 02; 05 reads 01's notes)
Wave 3: 06-controller                                              (depends on 01,02,03,04,05)
Wave 4: 07-leptos-integration   08-docs-and-agents-template        (depend on 06)
Wave 5: 09-example-app          10-cupline-retrofit                (depend on 07; 10 also on 06)
```

## Tasks

| # | Task | Status | Complexity | Depends On | Modules |
|---|------|--------|------------|------------|---------|
| 1 | [01-spike-reactive-graph](tasks/01-spike-reactive-graph.md) | ✅ Done | high | - | `crates/clean-signals/tests/spike_reactive_graph.rs`, `research/SPIKE_NOTES.md` |
| 2 | [02-failure-trait](tasks/02-failure-trait.md) | ✅ Done | medium | - | `crates/clean-signals/src/failure.rs` |
| 3 | [03-use-case](tasks/03-use-case.md) | ✅ Done | medium | 02 | `crates/clean-signals/src/use_case.rs` |
| 4 | [04-retry-time](tasks/04-retry-time.md) | ✅ Done | medium | 02 | `crates/clean-signals/src/retry.rs`, `src/time.rs` |
| 5 | [05-activity-async-state](tasks/05-activity-async-state.md) | ✅ Done | medium | 02 (reads 01 notes) | `crates/clean-signals/src/activity.rs`, `src/async_state.rs` |
| 6 | [06-controller](tasks/06-controller.md) | ✅ Done | high | 01,02,03,04,05 | `crates/clean-signals/src/controller.rs`, `src/lib.rs` |
| 7 | [07-leptos-integration](tasks/07-leptos-integration.md) | ✅ Done | high | 06 | `crates/clean-signals-leptos/src/*` |
| 8 | [08-docs-and-agents-template](tasks/08-docs-and-agents-template.md) | ✅ Done | medium (Agent: doc_maintainer) | 06 | `docs/*.md`, `templates/AGENTS.md` |
| 9 | [09-example-app](tasks/09-example-app.md) | ✅ Done | medium | 07 | `examples/team-demo/src/*` |
| 10 | [10-cupline-retrofit](tasks/10-cupline-retrofit.md) | ⏸ Deferred (user, 2026-07-06) | high | 06,07 | **cupline repo** — NOT part of this pipeline; user will do this later |

## File Overlap Analysis

Wave 0 pre-creates every module file and pre-declares ALL dependencies in every Cargo.toml, so no task edits a Cargo.toml or `lib.rs` module list (exception: task 06 owns `src/lib.rs` re-export polish, alone in its wave).

| Task | Files Modified (Write) | Files Read (Dependencies) |
|------|----------------------|--------------------------|
| 01 | `crates/clean-signals/tests/spike_reactive_graph.rs`, `workflow/plans/feature/clean-signals-rs/research/SPIKE_NOTES.md` | PLAN.md, research/RESEARCH.md |
| 02 | `crates/clean-signals/src/failure.rs` | PLAN.md |
| 03 | `crates/clean-signals/src/use_case.rs` | PLAN.md, src/failure.rs |
| 04 | `crates/clean-signals/src/retry.rs`, `crates/clean-signals/src/time.rs` | PLAN.md, src/failure.rs |
| 05 | `crates/clean-signals/src/activity.rs`, `crates/clean-signals/src/async_state.rs` | PLAN.md, src/failure.rs, research/SPIKE_NOTES.md |
| 06 | `crates/clean-signals/src/controller.rs`, `crates/clean-signals/src/lib.rs` | everything in crates/clean-signals/src, SPIKE_NOTES.md, research/dart-test-spec.md |
| 07 | `crates/clean-signals-leptos/src/lib.rs`, `src/hooks.rs`, `src/async_view.rs`, `src/failure_listener.rs`, `src/interval.rs` | crates/clean-signals/src/* |
| 08 | `docs/ARCHITECTURE.md`, `docs/CODE_STANDARDS.md`, `docs/DEVELOPMENT.md`, `templates/AGENTS.md` | crates/*, Dart original templates/AGENTS.md |
| 09 | `examples/team-demo/src/*`, `examples/team-demo/index.html` | both framework crates |
| 10 | cupline repo only: `crates/cl-dashboard/src/pages/orders.rs`, new `crates/cl-dashboard/src/orders/*`, `crates/cl-dashboard/Cargo.toml`, cupline root `Cargo.toml` | this repo (read-only path dep), research/cupline-surface.md |

### Overlap Matrix (wave peers only)

| Task Pair | Shared Write Files | Isolation Strategy |
|-----------|-------------------|-------------------|
| 01 + 02 | None | Parallel (worktree) |
| 03 + 04 | None | Parallel (worktree) |
| 03 + 05 | None | Parallel (worktree) |
| 04 + 05 | None | Parallel (worktree) |
| 07 + 08 | None | Parallel (worktree) |
| 09 + 10 | None (different repos) | 09 worktree ∥ 10 main-loop agent in cupline |

## Success Criteria

See PLAN.md Success Criteria. Verify commands (until docs/DEVELOPMENT.md exists): `cargo build --workspace && cargo test --workspace && cargo clippy --workspace --all-targets -- -D warnings`; wasm: `cargo check -p clean-signals --target wasm32-unknown-unknown`.

## Notes

- Task 10 DEFERRED by user (2026-07-06): do not implement anything in the cupline repo in this pipeline. The task file stays as a ready-to-run spec for later. Wave 5 = task 09 only.
- If task 01 refutes a PLAN.md assumption (spawn_local recipe, Effect usability), the conductor updates PLAN.md + task 06 before Wave 3 dispatch.

## Pipeline State (durable — survives context compaction)

- PHASE_BASE: `28d6aaa` (scaffold commit, Wave 0 done)
- WORKING_BRANCH: `master`
- Plan approval: delegated to conductor by user; approved 2026-07-06.
- Cupline retrofit: DEFERRED by user — no cupline writes in this pipeline.
- Push policy: push origin master after every merged wave (user: "push as you go", 2026-07-06).

## Phase Review

| Round | Verdict | Review | Reviewed HEAD |
|-------|---------|--------|---------------|
| 0 | ⚠️ NEEDS_WORK | workflow/reviews/clean-signals-rs/REVIEW.md | f143e2b |

## Wave Log

- Wave 1 (01,02): PASS+PASS, integration PASS (15 tests).
- Wave 2 (03,04,05): validator verdicts FAIL/CONCERN/CONCERN — the FAIL and scope-violation concerns were evidence-invalidated (worktrees forked pre-3548370; merge-base diffs prove each branch touched only its scoped files). Conductor override → merged. Real notes kept: 04 `backoff_factor` default = 2.0 matching Dart source (task text said 1.0 but instructed matching Dart; Dart wins); 05 summary test-count typo (10 not 12) — cosmetic. Integration PASS (48 tests).
- Wave 3 (06): sequential main-loop implementor (opus). Validator PASS (19 controller tests; deviations documented: Params: Clone, on_dispose-after-dispose immediate, no emit_failures arg, no Effect in core).
- Wave 4 (07,08): PASS+PASS, integration PASS (74 tests, both crates wasm-clean). Post-merge: doc_maintainer patched AGENTS.md (+1 line, Arc for provide_controller); conductor fixed pre-existing rustdoc link in async_state.rs (from wave 2).
- Wave 5 (09): blocked once on missing async-trait dep in team-demo (Wave-0 gap; conductor fixed, d7d5615), then Done. Validator PASS (20 tests; layout = AGENTS.md reference). Friction findings carried to review: async-trait re-export gap; reactive_graph-via-leptos-prelude wording in AGENTS.md; view! angle-bracket parsing in closure types.
