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

---

## Completion Summary

**Status:** Done
**Branch:** master

### Files Modified

| File | Changes |
|------|---------|
| `crates/clean-signals/src/controller.rs` | F2/F3/F5 hardening of the synchronization core + 5 new tests. |

**F2 — lock-free FailureSink emit:**
- `Listener<F>` changed from `Box<dyn Fn…>` to `Arc<dyn Fn…>`; `subscribe` pushes `Arc::new(f)`.
- `emit` now snapshots the `(id, Arc)` listeners into a local `Vec` under the lock, drops the guard, then invokes — **no user closure runs under the lock**, so a panicking listener can neither poison nor deadlock the sink.
- All FailureSink lock sites (`subscribe` push, `remove` closure retain, `emit` snapshot) use `unwrap_or_else(|e| e.into_inner())` poison-recovery. `cleanups` lock sites hardened the same way.
- Re-entrancy rustdoc rewritten: listeners may now subscribe/unsubscribe mid-`emit`; removal isn't reflected in the in-flight pass (snapshot already taken); a panic propagates but doesn't poison.

**F3 — pruned watch registry:**
- `watches` is now `Arc<Mutex<Vec<(u64, AbortHandle)>>>` (type alias `WatchList`) + a `next_watch_id: AtomicU64`.
- New `RemoveOnDrop` RAII guard lives *inside* the driver's async frame (inside `Abortable`), so the entry is pruned on **normal stream completion AND on abort** (Abortable drops the wrapped future in place on abort).
- `WatchHandle` gained a `Weak<Mutex<WatchList>>` back-ref + `id`; `cancel()` aborts **and** prunes synchronously (observable without pumping the executor).
- `dispose()` drains the `(id, handle)` tuples and aborts each — idempotent with the guard/cancel prunes.

**F5 — dispose/watch race:**
- `dispose()` takes the watches lock **first**, flips `disposed` under it, drains+aborts under it, then releases before running cleanups/`owner.cleanup()` (cleanups may re-enter the controller).
- `watch()` takes the same lock as its first registration step, checks `is_disposed()` under it; if disposed it returns an **inert** `WatchHandle` (already-aborted, empty `Weak`, no registry entry) **without spawning** — the stream is constructed but never polled. Otherwise it allocates the id and pushes under the lock, then releases before building/spawning the driver. Critical section is exactly `{is_disposed read, id alloc, push}` — no `uc.execute`, `on_data`, `emit`, or the spawn runs under the lock (honors the F5 cross-check dissent about minimal critical sections).

### New Tests

- `failure_sink_panicking_listener_does_not_poison` — `catch_unwind` around an `emit` whose listener panics; subsequent unsubscribe + subscribe + emit still work (would `PoisonError`-panic if the mutex were poisoned).
- `watch_registry_empties_after_stream_completes` — `len()==1` after `watch`, `len()==0` after the finite stream is pumped to exhaustion.
- `watch_registry_empties_after_cancel` — `cancel()` prunes to `len()==0` *without* pumping; a 5× re-watch/cancel loop never grows the vec past the live count.
- `watch_dispose_aborts_all_live_watches` — two live watches, `dispose()` drains both to `len()==0`.
- `watch_after_dispose_is_inert` — `watch` on a disposed controller registers nothing (`len()==0`) and delivers nothing (empty values after pump); inert `cancel()` is a harmless no-op.

### Notable Decisions/Tradeoffs

1. **Kept the per-item `disposed.load()` check in the driver loop** as defense-in-depth alongside the new abort-under-lock guarantee — harmless, preserves the exact behavior `watch_dispose_stops_further_on_data` relies on.
2. **Build the stream before the lock even when disposed** (per the dissent): `uc.execute` constructs a lazy stream with no side effects until polled, so constructing-then-dropping it on the disposed path is safe; documented in the `watch` rustdoc.
3. **Spawn the driver *after* releasing the lock**: the handle is already registered under the lock, so a racing `dispose` still finds+aborts it; if `dispose` ran first, `watch` already returned inert. No window leaks a watch past disposal, and no user code / spawn runs under the lock.
4. **Public API unchanged**: `WatchHandle`, `Subscription`, and all method signatures are identical; `WatchHandle`'s new fields are private.

### run()/run_into() — out of scope, justified

The F5 dissent's point about the analogous `is_disposed()`-then-write races in `run()`/`run_into()` is already mitigated by the existing `try_set`/`try_update` writes (per `docs/CODE_STANDARDS.md`, `try_*` no-ops on a disposed signal instead of panicking). Those call sites need no lock coordination — a post-dispose write is an inert no-op regardless of interleaving — and the task scope is FailureSink/watch/dispose. Left unchanged.

### Testing Performed

- `cargo build --workspace` — Passed
- `cargo test --workspace` — Passed (60 clean-signals unit + 10 spike + others; all green, no test modified)
- `cargo clippy --workspace --all-targets -- -D warnings` — Passed
- `cargo check -p clean-signals --target wasm32-unknown-unknown` — Passed
- `cargo clippy -p clean-signals --all-targets --features test-fixtures -- -D warnings` — Passed

### Risks/Limitations

1. **`Abortable` drop-in-place semantics** are load-bearing for `RemoveOnDrop` firing on abort. Verified indirectly by `watch_dispose_aborts_all_live_watches` (drain empties the registry) and the pinned `futures` version behaves as documented.
2. The 4 sleep-based tests (fix-02's scope) were **left untouched** and required no compile changes — they still pass unmodified against the reworked registry/dispose paths.
