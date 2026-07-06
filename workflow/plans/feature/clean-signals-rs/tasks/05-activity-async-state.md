## Task: ActivityTracker + AsyncState

**Objective**: Implement ref-counted loading (`ActivityTracker` with RAII guards) and the `AsyncState<T, F>` enum + `async_state_signal` + `ResultExt::to_async_state` per PLAN.md.

**Depends on**: 02-failure-trait; READ `research/SPIKE_NOTES.md` (Q1 disposed-signal behavior, Q3 Memo recipe, Q8 Drop-guard) before implementing.

**Complexity:** medium

### Scope

**Files Modified (Write):**
- `crates/clean-signals/src/activity.rs`
- `crates/clean-signals/src/async_state.rs`

**Files Read (Dependencies):**
- `workflow/plans/feature/clean-signals-rs/PLAN.md` (§ Core API contract)
- `workflow/plans/feature/clean-signals-rs/research/SPIKE_NOTES.md`
- `workflow/plans/feature/clean-signals-rs/research/dart-test-spec.md` (activity_test.dart + AsyncState transitions)

### Details

`activity.rs` (semantics: dart-test-spec activity_test.dart):
- `pending: RwSignal<u32>`, `is_loading: Memo<bool>` (`pending > 0`), `disposed: Arc<AtomicBool>`.
- `begin() -> ActivityGuard`: increments; guard's `Drop` decrements via `try_update` AND skips when `disposed` is set (post-dispose decrements suppressed — pinned behavior). Guard holds clones of what it needs; must be safe if it outlives the tracker's dispose.
- `dispose()`: sets the flag (idempotent). No signal disposal here — owner/arena handles that; use `try_*` writes exclusively.
- `is_loading()` returns the `Memo<bool>` (Copy handle) so controllers/UI can track it reactively.
- Tests (tokio, per SPIKE_NOTES recipes): two overlapping begins → is_loading stays true until BOTH guards drop; guard dropped after dispose → pending unchanged; pending() counts.

`async_state.rs`:
- Enum + helpers exactly per PLAN.md contract (`value()` returns stale data for `Reloading` and `Error{stale: Some}` too).
- `pub fn async_state_signal<T, F>() -> RwSignal<AsyncState<T, F>>` seeded `Loading` (bounds per contract).
- Transition helper used by task 06 (pure function, unit-testable): `pub fn to_reloading<T: Clone, F>(prev: &AsyncState<T, F>) -> AsyncState<T, F>` — Data(d)|Reloading(d) → Reloading(d); Error{stale:Some(d)} → Reloading(d); else Loading.
- `impl<T, F> ResultExt<T, F> for Result<T, F>`: Ok → Data, Err → `Error{failure, stale: None}`.
- Tests: helper matrix over all variants; seed state is Loading; ResultExt both arms; PartialEq derives work with fixture failures.

### Acceptance Criteria

1. `cargo test -p clean-signals activity async_state` passes; clippy clean; wasm check stays green.
2. Post-dispose decrement suppression has an explicit test.
3. `to_reloading` covers all 4 variants with tests.

### Notes

- If SPIKE_NOTES contradicts the RwSignal/Memo usage assumed here, follow SPIKE_NOTES and note the deviation in your completion summary.
