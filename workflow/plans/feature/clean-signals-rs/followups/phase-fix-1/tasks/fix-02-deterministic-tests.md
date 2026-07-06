## Task: fix-02 — Deterministic test synchronization (kill the real sleeps)

**Objective**: Remove all four wall-clock synchronization sleeps from controller tests (F1) using a started-signal on the `Slow` fixture and a gated `SlowTicker`, per DIAGNOSES.md F1.

**Depends on**: fix-01 (same file; rebase on its merged controller.rs)

**Complexity:** medium

### Scope

**Files Modified (Write):**
- `crates/clean-signals/src/use_case.rs` (fixtures only: additive `Slow::with_started_signal()` — existing `Slow::new()` signature untouched)
- `crates/clean-signals/src/controller.rs` (the 4 named tests + the local `SlowTicker` fixture ONLY — no production code)

**Files Read (Dependencies):**
- `workflow/plans/feature/clean-signals-rs/followups/phase-fix-1/DIAGNOSES.md` (F1 section: exact lines, root cause, approach)
- `docs/CODE_STANDARDS.md` (the no-sleep rule being restored)

### Details

Per DIAGNOSES.md F1:
1. `Slow::with_started_signal() -> (Self, oneshot::Sender<()>, oneshot::Receiver<()>)`; `execute()` best-effort-sends 'started' on entry before parking on the gate. (`run`/`run_into` flip loading state synchronously before `execute`, so awaiting 'started' is race-free proof the observable state changed.)
2. Replace the three `tokio::time::sleep(30ms)` calls in `run_tracks_is_loading_across_the_operation`, `run_into_drives_loading_to_data`, `run_into_keeps_stale_data_visible_while_reloading` with `started_rx.await`.
3. Rework `SlowTicker` to park on test-controlled gates instead of `crate::time::sleep(50ms)`; restructure `watch_dispose_stops_further_on_data` to pump → observe item 1 → dispose → release gate → pump → assert still `[1]`, eliminating the trailing 90ms sleep. Preserve the property that dispose fires while the driver is genuinely parked mid-stream (not merely not-started).
4. Result: zero `tokio::time::sleep`/wall-clock waits used for synchronization anywhere in controller tests (retry-delay timing tests that measure `delay_for` math are unaffected — only synchronization sleeps go).

### Acceptance Criteria

1. `grep -n 'time::sleep' crates/clean-signals/src/controller.rs` shows no synchronization sleeps in tests (document any remaining legitimate use inline).
2. The four tests still pin the SAME behaviors (loading transitions, stale-data reload, no on_data after dispose) — assertions unchanged in substance.
3. Full DoD green: `cargo test --workspace`, clippy `-D warnings`, wasm check. Run the controller test binary 5x locally to demonstrate stability.
4. `Slow::new()` untouched; `slow_resolves_only_after_gate_is_released` (use_case.rs) passes unmodified.

---

## Completion Summary

**Status:** Done
**Branch:** master

### Files Modified

| File | Changes |
|------|---------|
| `crates/clean-signals/src/use_case.rs` | Additive `Slow::with_started_signal() -> (Self, oneshot::Sender<()>, oneshot::Receiver<()>)` fixture: `execute()` best-effort-sends on a new `started` oneshot the moment it's entered, before parking on the existing `gate`. `Slow::new()`'s signature and behavior are byte-for-byte unchanged (its `started` field is `None`, so the send is skipped). |
| `crates/clean-signals/src/controller.rs` | (1) `SlowTicker` reworked: instead of an internal `crate::time::sleep(50ms)` between stream items, each `execute()` call creates a fresh `oneshot` gate and stashes the `Sender` in a thread-local slot (`SLOW_TICKER_GATE`), retrievable via new `SlowTicker::take_gate()`. Stream still yields `Ok(1)` immediately, then awaits the gate before `Ok(2)`. This is a test-fixture-only change: `SlowTicker` remains a bare unit struct, so the 4 fix-01 tests that already construct/use `&SlowTicker` (`watch_registry_empties_after_cancel`, `watch_dispose_aborts_all_live_watches`, plus the reused import) needed zero edits — they never poll past registration so the gate is inert for them. (2) The 4 named sleep-based tests rewritten to await deterministic signals instead of sleeping (see below). |

### What replaced each sleep

1. `run_tracks_is_loading_across_the_operation` — `tokio::time::sleep(30ms)` → `Slow::with_started_signal()`'s `started_rx.await.unwrap()`, awaited right after spawning the `run(...)` task, before asserting `is_loading()`.
2. `run_into_drives_loading_to_data` — same swap, before asserting `state.get_untracked() == AsyncState::Loading`.
3. `run_into_keeps_stale_data_visible_while_reloading` — same swap, before asserting the mid-reload stale-data state.
4. `watch_dispose_stops_further_on_data` — the `SlowTicker` internal `crate::time::sleep(50ms)` + trailing `tokio::time::sleep(90ms)` after `dispose()` replaced with: `pump()` → observe `[1]` → `dispose()` → `SlowTicker::take_gate().send(())` (release, proving the would-be-next item is inert post-abort) → `pump()` → assert still `[1]`. No wall-clock wait anywhere in the test.

All four rely on the documented fact that `ControllerCore::run`/`run_into` flip `is_loading`/the `AsyncState` signal synchronously *before* calling `execute`, so observing "started" (or, for the watch test, observing the first stream item) is race-free proof the visible state has already changed — no heuristic real-time delay needed.

### 5-run stability result

Ran `cargo test -p clean-signals controller` 5 times consecutively: **24 passed, 0 failed** every run, each completing in ~0.01s (down from the prior ~150-200ms of real sleeps per run). No flakiness observed.

### Verification (Definition of Done)

- `cargo build --workspace` — Passed
- `cargo test --workspace` — Passed (all crates, all suites green)
- `cargo clippy --workspace --all-targets -- -D warnings` — Passed, zero warnings
- `cargo check -p clean-signals --target wasm32-unknown-unknown` — Passed
- `cargo clippy -p clean-signals --all-targets --features test-fixtures -- -D warnings` — Passed, zero warnings
- `grep -n 'time::sleep' crates/clean-signals/src/controller.rs` — only line 341 (`crate::time::sleep(opts.retry.delay_for(attempt))`, legitimate production retry-delay code, not a test-synchronization sleep) and a doc-comment mention of the removed pattern; zero synchronization sleeps remain.
- `slow_resolves_only_after_gate_is_released` — Passed, unmodified.
- 5x `cargo test -p clean-signals controller` — Passed every run (24/24), demonstrating stability.

### Notable Decisions/Tradeoffs

1. **Thread-local gate for `SlowTicker` instead of a constructor parameter**: keeping `SlowTicker` a genuine unit struct (no fields) was required so the 4 fix-01 tests that reference `&SlowTicker` bare-literal (out of scope for this task) needed zero changes. A per-call `oneshot::Sender` stashed in a `thread_local!` slot, retrieved via `SlowTicker::take_gate()`, gives the one test that needs it (`watch_dispose_stops_further_on_data`) a real controllable gate while leaving the other 3 SlowTicker-using tests entirely untouched and unaffected (they never poll the stream past registration, so the dangling gate is inert). This is safe because all `SlowTicker`-using tests run single-threaded (`#[tokio::test(flavor = "current_thread")]` + `LocalSet`), so there's no cross-task/cross-thread interleaving on the thread-local within one test body.
2. **`Slow::with_started_signal` is fully additive**: new struct field defaults to `None` for the existing `Slow::new()` path, so the one pre-existing caller (`slow_resolves_only_after_gate_is_released`) required and received zero changes.

### Risks/Limitations

None identified. All four target tests still pin the same behaviors (loading transitions, stale-data-visible-during-reload, no-`on_data`-after-dispose) with assertions unchanged in substance — only the synchronization mechanism changed from real-time sleeps to deterministic channel awaits.
