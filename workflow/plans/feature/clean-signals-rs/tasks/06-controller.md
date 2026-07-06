## Task: ControllerCore — run / run_into / watch / dispose

**Objective**: Implement the framework's centerpiece: `ControllerCore<F>`, `RunOptions<F>`, `FailureSink<F>`, `Subscription`, `WatchHandle` per PLAN.md, porting all controller_test.dart semantics. Finalize `src/lib.rs` re-exports + crate rustdoc.

**Depends on**: 01,02,03,04,05 — read `research/SPIKE_NOTES.md` FIRST (Q4 Effect, Q5 spawn_local recipe, Q6 Executor init pattern are load-bearing here).

**Complexity:** high

### Scope

**Files Modified (Write):**
- `crates/clean-signals/src/controller.rs`
- `crates/clean-signals/src/lib.rs` (re-export polish + crate-level rustdoc; module declarations already exist)

**Files Read (Dependencies):**
- All other `crates/clean-signals/src/*.rs`
- `workflow/plans/feature/clean-signals-rs/PLAN.md` (§ Core API contract — locked)
- `workflow/plans/feature/clean-signals-rs/research/SPIKE_NOTES.md`
- `workflow/plans/feature/clean-signals-rs/research/dart-test-spec.md` (controller_test.dart sections — the acceptance contract)
- Dart source for reference: `/home/ed/Dev/personal/clean_signals/lib/src/controller.dart`

### Details

**FailureSink<F>**: `Mutex<Vec<(u64, Box<dyn Fn(&F) + Send + Sync>)>>` + `AtomicU64` id counter. `subscribe` returns `Subscription` (RAII: removes its entry on Drop; `Subscription::forget()` for fire-and-forget). `emit` clones the failure ref to each listener synchronously in subscription order. No listeners → no-op.

**ControllerCore<F: Failure + Clone>**: owns `ActivityTracker`, `FailureSink<F>`, `disposed: AtomicBool`, `cleanups: Mutex<Vec<Box<dyn FnOnce() + Send>>>`, watch abort registry.

**`run`** (semantics pinned by dart-test-spec controller_test.dart run section):
```
attempt = 1
loop {
  guard = if opts.track_activity { Some(activity.begin()) } else { None }   // NOTE: Dart begins once for the whole run incl. retries — check dart source; if ambiguous, begin once around the whole loop and document
  result = uc.execute(params.clone()).await                                  // Params: Clone bound needed for retry; require U::Params: Clone
  on Ok -> return Ok
  on Err(f) ->
    if attempt < opts.retry.max_attempts && opts.retry.should_retry(&f) {
      time::sleep(opts.retry.delay_for(attempt)).await; attempt += 1; continue
    }
    if opts.emit_failure && !self.is_disposed() { self.failures.emit(&f) }
    return Err(f)
}
```
Pinned: only the FINAL failure is emitted (never intermediates); `max_attempts` counts attempts; success emits nothing; `emit_failure: false` suppresses; `is_loading` true while running (including across retry sleeps), false after.

**`run_into`**: before awaiting, `into.try_update(|s| *s = to_reloading(s))` (first load: Loading; reload: Reloading(stale)). Then delegate to `run` logic. On Ok → `try_set(Data)`; on final Err → `try_set(Error { failure, stale: prev_value })`. ALL writes via `try_*` and gated on `!is_disposed()` — a controller can be disposed while an await is in flight.

**`watch`**: takes `S: StreamUseCase<Failure = F>`, spawns via the SPIKE_NOTES Q5 recipe (`Executor::spawn_local`), forwarding `Ok(v)` → `on_data(v)`, `Err(f)` → failures sink (respect an `emit_failures: bool` arg if you add one — Dart has it; default true). Registers an abort handle (use `futures::future::AbortHandle` or drive via a oneshot cancel select) in the watch registry; `dispose()` aborts ALL watches immediately (pinned: no further `on_data` after dispose). Returns `WatchHandle` allowing individual early cancel.

**`dispose`**: idempotent (AtomicBool swap); aborts watches; disposes activity tracker; drains cleanups and runs them **LIFO**. `on_dispose` after dispose → run the callback immediately (document) or drop it — match Dart (check `controller.dart`; Dart asserts/ignores — mirror it and document).

**lib.rs**: re-export the public surface flat (`pub use failure::Failure;` etc.), crate rustdoc with a 20-line usage example (an app failure enum, one use case, one controller struct embedding ControllerCore, a run_into call) — the example compiles as a doctest using the SPIKE_NOTES tokio recipe.

**Tests** (port dart-test-spec controller sections test-for-test, using task 03 fixtures):
run: success → no failure emitted; final-failure-only after retries (Flaky(2) + max_attempts 3 → Ok on 3rd, attempts()==3; Flaky(5) + max_attempts 2 → Err after 2); ValidationFailure (non-retryable) → no retry even with max_attempts 3; emit_failure=false → sink silent; is_loading transitions with Slow + oneshot gate (assert true while gated, false after release).
run_into: Loading → Data; second call → Reloading(stale) visible mid-flight (gate with Slow-style use case) → Data(new); failure path → Error{stale: Some(old)}.
watch: Ticker(3) → on_data 1,2,3 then sink gets "tick lost"; dispose mid-stream → no further on_data (prove with a gated ticker).
dispose: LIFO order (Vec push 1,2,3 → observe 3,2,1); idempotent (cleanups run once); is_disposed flips; run_into after dispose → signal untouched.
FailureSink: two subscribers both fire; dropped Subscription stops firing.

### Acceptance Criteria

1. `cargo test -p clean-signals` fully green; clippy `-D warnings` clean; wasm `cargo check` green.
2. Every controller behavior listed in dart-test-spec has a named equivalent test.
3. Public API matches PLAN.md contract (deviations forced by SPIKE_NOTES documented in completion summary + rustdoc).
4. Doctest in lib.rs compiles and runs.

### Notes

- `U::Params: Clone` bound on `run`/`run_into` is accepted (retry needs it). NoParams is Copy.
- Do NOT implement `auto_effect` here if SPIKE_NOTES shows Effect needs the leptos runtime nuances — it moves to task 07. Record the decision.
- Prefer `Memo`/explicit state over internal Effects — controllers must be fully testable without the `effects` feature.
