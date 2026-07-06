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
