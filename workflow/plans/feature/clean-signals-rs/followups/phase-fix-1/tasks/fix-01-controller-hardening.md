## Task: fix-01 — ControllerCore hardening (FailureSink lock discipline, watch registry pruning, dispose/watch race)

**Objective**: Implement the verified fix approaches for findings F2, F3, F5 in `crates/clean-signals/src/controller.rs` — one coherent change since all three touch the same synchronization core.

**Depends on**: None (runs first; fix-02 rebases its test edits on this)

**Complexity:** high

### Scope

**Files Modified (Write):**
- `crates/clean-signals/src/controller.rs`

**Files Read (Dependencies):**
- `workflow/plans/feature/clean-signals-rs/followups/phase-fix-1/DIAGNOSES.md` (the authoritative fix specs — F2, F3, F5 sections incl. the F5 cross-check dissent)
- `docs/CODE_STANDARDS.md` (try_* rule, test recipes)

### Details

Implement exactly per DIAGNOSES.md:

**F2 (FailureSink):** `Listener<F>` becomes `Arc<dyn Fn(&F) + Send + Sync>`; `emit` locks only to clone the (id, Arc) entries into a local Vec, drops the guard, then invokes — no user code ever runs under the lock. Update rustdoc: the re-entrancy caveat changes (listeners may now subscribe/unsubscribe during emit; removal isn't reflected in the in-flight emission). Consider poison-recovery (`unwrap_or_else(|e| e.into_inner())`) at remaining lock sites so a panicking listener degrades gracefully. New test: a panicking listener (AssertUnwindSafe + catch_unwind) doesn't poison the sink — subsequent subscribe/emit still work.

**F3 (registry pruning):** `watches` becomes id-keyed (`Arc<Mutex<Vec<(u64, AbortHandle)>>>` + AtomicU64). Driver removes its own entry when it finishes (normal completion or abort — e.g. a scope-guard around the Abortable, or retain after `Abortable` resolves); `WatchHandle::cancel()` removes its entry too. New tests: registry empty after stream completes; empty after cancel; dispose still aborts all live watches.

**F5 (dispose/watch race):** single-lock protocol per DIAGNOSES.md incl. the dissent refinement — `dispose()` takes the watches lock, flips `disposed` under it, drains+aborts, releases before running cleanups; `watch()` takes the same lock, checks `is_disposed()` under it, and if disposed returns an inert `WatchHandle` WITHOUT spawning (or aborts immediately if already spawned — keep the critical section to the is_disposed read + push only; never hold the lock across `uc.execute` or the spawn if avoidable, re-check after spawn and abort if disposed meanwhile). New test: watch() on a disposed controller delivers nothing and registers nothing (deterministic version; a genuine two-thread race test is welcome if it can be made non-flaky, otherwise assert the protocol invariants directly).

Keep the public API surface unchanged (`WatchHandle`, `Subscription`, method signatures). All existing 19 controller tests must keep passing unmodified (fix-02 will rework the 4 sleep-based ones separately — do NOT touch those tests here beyond what compiles).

### Acceptance Criteria

1. All existing tests pass unchanged; new tests cover: panicking listener non-poisoning, registry pruned on completion + cancel, watch-after-dispose inert.
2. `cargo test --workspace`, `cargo clippy --workspace --all-targets -- -D warnings`, `cargo check -p clean-signals --target wasm32-unknown-unknown` green.
3. No user closure is ever invoked while any framework Mutex is held (grep-verifiable).
4. Public API unchanged.

### Notes

- F3+F5 interact: design the registry once so both land coherently.
- Rustdoc for `watch`/`dispose`/`FailureSink` must reflect the new guarantees.
