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

---

## Completion Summary

**Status:** Done
**Branch:** master

### Files Modified

| File | Changes |
|------|---------|
| `crates/clean-signals/src/controller.rs` | Implemented `ControllerCore<F>`, `RunOptions<F>`, `FailureSink<F>`, `Subscription`, `WatchHandle`; ported all `controller_test.dart` behaviors as 20 named tests. |
| `crates/clean-signals/src/lib.rs` | Flat crate-root re-exports of the public surface + crate rustdoc with a compiling `block_on` doctest (failure enum → use case → controller embedding `ControllerCore` → `run_into`). |

### Notable Decisions/Tradeoffs

1. **`U::Params: Clone` bound** on `run`/`run_into` (deviation from the locked PLAN signature, accepted by the task note): the retry loop re-invokes the use case with the same params. `NoParams` is `Copy`, so the common case is free. Documented in the module rustdoc.
2. **Activity begins once around the whole retry loop** (not per attempt): confirmed against Dart source — `_activity.track(_runWithRetry(...))` wraps the entire loop, so `is_loading` stays `true` across retry sleeps. Guard held for the lifetime of `run`.
3. **`on_dispose` after `dispose` runs the callback immediately** (documented). Dart silently appends to an already-drained list (effectively dropping it); running immediately is strictly safer (resource still released). The task explicitly permitted either; chose the defensive option.
4. **No `auto_effect` / no `Effect` in core** (per SPIKE_NOTES Q4): `Effect::new` forces the `effects` feature + `LocalSet` on every consumer. Core uses `Memo` (via `ActivityTracker`) + explicit methods only, keeping controllers testable without the `effects` feature. Render-glue deferred to task 07.
5. **`watch` kept to the PLAN signature** (no public `emit_failures`/`onFailure` params): failures always route to the `FailureSink` (the Rust equivalent of Dart's failures stream). The task allowed adding an `emit_failures` arg "if you add one"; omitted to keep the locked surface minimal — a variant can live in the leptos crate.
6. **`ControllerCore` holds an `Owner`** (SPIKE_NOTES Q1/Q2): activity signals are created under it and released by `dispose()` via `owner.cleanup()`, avoiding leaked arena slots. `ControllerCore<F>` is `Send + Sync` (needed for leptos `StoredValue`).
7. **`FailureSink` is `Clone`** (shares `Arc` listener list + id counter) so `watch`'s spawned-local task can own a handle; `Subscription` is non-generic via a type-erased removal closure and is `#[must_use]`.
8. **`FailureSink::emit` holds its lock while invoking listeners** (synchronous, in-order fan-out). Documented the re-entrancy constraint (a listener must not subscribe/unsubscribe the same sink synchronously).

### Testing Performed

- `cargo build --workspace` — Passed
- `cargo test --workspace` — Passed (clean-signals lib: 55 tests incl. 20 new controller tests; spike suite: 10; doctests: 3)
- `cargo clippy --workspace --all-targets -- -D warnings` — Passed
- `cargo check -p clean-signals --target wasm32-unknown-unknown` — Passed
- `cargo clippy -p clean-signals --all-targets --features test-fixtures -- -D warnings` — Passed

Behavior coverage (dart-test-spec controller sections): run success/no-emit, retry-to-success (attempts==3), exhaust-and-emit-one (attempts==2, one failure), non-retryable no-retry (attempts==1), emit_failure=false, is_loading gated transitions; run_into loading→data, reloading keeps stale (mid-flight `value()==Some(6)`), failure→Error{stale:Some}, fresh failure→Error{stale:None}; watch data+trailing failure, dispose stops further on_data (gated ticker via `Executor::spawn_local` + LocalSet recipe); dispose LIFO+idempotent, is_disposed flips, run_into-after-dispose untouched, on_dispose-after-dispose runs immediately; FailureSink fan-out, dropped-subscription-stops, forget-keeps-firing.

### Risks/Limitations

1. **`watch` tests require the current_thread + `LocalSet` harness** (SPIKE_NOTES Q5) — a plain `#[tokio::test]` panics on `spawn_local`. Encoded in the two watch tests; documented for downstream (task 07) test authors.
2. **`FailureSink::emit` re-entrancy** — a listener that mutates the same sink synchronously would deadlock. Documented; not a concern for the intended usage (forward-to-UI listeners).

### Doc Updates Needed

None yet — `docs/ARCHITECTURE.md` etc. are created by task 08. The public surface finalized here (flat re-exports, `RunOptions`/`ControllerCore`/`FailureSink`/`Subscription`/`WatchHandle`, and the two documented PLAN deviations: `U::Params: Clone` and `on_dispose`-after-dispose-runs-immediately) should be reflected when those docs are written.
