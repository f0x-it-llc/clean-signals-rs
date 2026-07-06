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

---

## Completion Summary

**Status:** Done
**Branch:** worktree-wf_d366217c-c00-3

### Files Modified

| File | Changes |
|------|---------|
| `crates/clean-signals/src/activity.rs` | `ActivityTracker` (pending `RwSignal<u32>`, `is_loading: Memo<bool>`, `disposed: Arc<AtomicBool>`), `begin() -> ActivityGuard` RAII, `pending()`, `is_loading()`, `dispose()` (idempotent, flag-only); `ActivityGuard` decrements via `try_update` on `Drop`, short-circuiting when `disposed` is set; 4 unit tests (ref-counted overlap, pending counting, post-dispose suppression, dispose idempotency). |
| `crates/clean-signals/src/async_state.rs` | `AsyncState<T, F>` enum (`Loading`/`Data`/`Reloading`/`Error{failure,stale}`) + `value()`/`has_value()`/`is_loading()`/`failure()`; `async_state_signal::<T,F>()` seeded `Loading`; pure `to_reloading` transition helper for task 06; `ResultExt::to_async_state()` (Ok→Data, Err→Error{stale:None}); 12 unit tests + 1 doctest covering all variants/transitions. |

### Notable Decisions/Tradeoffs

1. **Explicit `disposed: Arc<AtomicBool>` flag, checked in `ActivityGuard::Drop` before the `try_update` decrement**: per the task's stated semantics (not strictly required by SPIKE_NOTES Q1/Q8 alone, since `try_update` on an arena-disposed signal already returns `None` safely). The task spec explicitly calls for both the flag check AND `try_update`, since `ActivityTracker::dispose()` never disposes the underlying signal itself (that's the owning arena/`Owner`'s job per the task's own wording) — the flag is the only thing that actually flips at `dispose()` time in this task's scope, so it's load-bearing for the "post-dispose decrement suppressed" contract tested here. No deviation from SPIKE_NOTES; this is additive.
2. **`begin()` increments via `try_update` (not `update`)**: defensive consistency with the "use `try_*` writes exclusively" note — makes the increment path inert rather than panicking/logging if a caller manages to invoke `begin()` after some external arena disposal of the signal itself (distinct from the tracker's own `disposed` flag).
3. **`to_reloading`'s `T: Clone` bound, `F` unconstrained** — matches the task's exact signature; the `Error` arm only touches `stale.clone()`, never `failure`, so no `Clone` bound is needed on `F`.
4. **No deviation from SPIKE_NOTES** — Q3 (Memo needs neither Owner nor Executor) and Q8 (Drop-time signal writes are reentrancy-safe and post-dispose inert) were followed directly; `ActivityTracker` intentionally holds no `Owner` (signals are created owner-less per Q2, matching the task's note that dispose() must not itself dispose the signals).

### Testing Performed

- `cargo test -p clean-signals` (lib + spike + doctests) - Passed (19 lib tests incl. all new activity/async_state tests, 10 spike tests, 1 doctest)
- `cargo test --workspace` - Passed
- `cargo clippy -p clean-signals --all-targets -- -D warnings` - Passed (no warnings)
- `cargo clippy --workspace --all-targets -- -D warnings` - Passed
- `cargo check -p clean-signals --target wasm32-unknown-unknown` - Passed
- `cargo build --workspace` - Passed

### Risks/Limitations

1. **`docs/ARCHITECTURE.md`, `docs/CODE_STANDARDS.md`, `docs/DEVELOPMENT.md` do not exist yet** in this repo (confirmed against TASKS.md: they're created by Wave 4's task 08, which depends on Wave 3's task 06 — this task is Wave 2). Per TASKS.md's own fallback verification commands, used `cargo build/test/clippy --workspace` + the wasm check instead. Not a blocker for this task; flagging per the stopping-rules protocol since the dispatch prompt referenced these files as mandatory reading.
2. **`cargo fmt -- --check` reports diffs** on both my new files and the pre-existing `failure.rs`/`spike_reactive_graph.rs` (import-ordering, brace style) — no `rustfmt.toml` exists in the repo and no prior task appears to have run `cargo fmt`, so this is a pre-existing repo-wide state, not something introduced here. Left as-is since it isn't part of the stated acceptance criteria or TASKS.md verify commands.
