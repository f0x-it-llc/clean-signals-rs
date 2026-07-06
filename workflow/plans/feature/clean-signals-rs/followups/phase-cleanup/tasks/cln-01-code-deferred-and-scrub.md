## Task: cln-01 — Code deferred items + Dart-reference scrub + fmt

**Objective**: Implement the code-side deferred items from the parent TASKS.md § Deferred Items, remove every Dart/Flutter/`clean_signals`-Dart lineage reference from crates/ and examples/ (standalone lib — comments describe what the code does, never where it came from), and land a repo-wide `cargo fmt`.

**Depends on**: None

**Complexity:** high

### Scope

**Files Modified (Write):** `crates/clean-signals/src/*.rs`, `crates/clean-signals-leptos/src/*.rs`, `examples/team-demo/**` (incl. `examples/team-demo/Cargo.toml` for the async-trait dep removal ONLY), `crates/clean-signals/tests/spike_reactive_graph.rs` (fmt/scrub only)

**Files Read:** `workflow/plans/feature/clean-signals-rs/TASKS.md` (§ Deferred Items), `workflow/plans/feature/clean-signals-rs/followups/phase-cleanup/TASKS.md` (ACCEPTED list)

### Details — deferred items to implement

1. **async_trait re-export**: add `pub use async_trait::async_trait;` to `crates/clean-signals/src/lib.rs` (with rustdoc: downstream traits/impls use `#[cfg_attr(not(target_arch = "wasm32"), clean_signals::async_trait)]` / `#[cfg_attr(target_arch = "wasm32", clean_signals::async_trait(?Send))]`). Switch team-demo to the re-export and REMOVE `async-trait` from `examples/team-demo/Cargo.toml` — proving consumers no longer pin it.
2. **RemoveOnDrop rustdoc correction** (controller.rs ~253-257): describe the real mechanism — the guard prunes when its owning future is dropped (normal completion = scope exit; abort case = redundant with `cancel()`/`dispose()`'s own explicit pruning, whenever the executor drops the aborted task). No behavior change.
3. **`lock_recovering` helper**: private `fn lock_recovering<T>(m: &Mutex<T>) -> MutexGuard<'_, T>` replacing all ~7 `.lock().unwrap_or_else(|e| e.into_inner())` sites; also factor the duplicated upgrade+retain pruning shared by `WatchHandle::cancel` and `RemoveOnDrop::drop` into one helper.
4. **Per-listener panic isolation in `FailureSink::emit`** (native): wrap each listener call in `std::panic::catch_unwind(AssertUnwindSafe(..))` so one bad listener doesn't skip the rest or abort a driver task; re-throw nothing, optionally log via a debug_assert-free path. On wasm32 catch_unwind is ineffective for aborts — document that caveat in the rustdoc. Adapt/extend the existing panicking-listener test to assert subsequent listeners in the SAME emit still fire.
5. **StreamUseCase lazy contract**: promote the "execute() must return a lazy, side-effect-free-until-polled stream" requirement from a comment inside `watch` to the `StreamUseCase` trait rustdoc as an explicit contract (watch's disposed fast-path may then keep constructing-and-dropping the stream safely).
6. **run/run_into narrow TOCTOU**: correct the controller.rs rustdoc to state precisely what's guaranteed: post-dispose signal writes are inert via `try_*`; failure EMISSION after a concurrent cross-thread dispose is a narrow accepted limitation (documented, per ACCEPTED list — no lock coordination on the hot path).
7. **SlowTicker fixture tightening**: terminate the stream after item 2 (no unbounded tail) and add a debug assertion in `execute()` that a previously-stashed gate was taken (fail fast instead of silently dropping).
8. **`RetryPolicy::new(0, _)` clamp test**: pin the actual behavior (one attempt still runs — max_attempts 0 behaves as 1 through the run loop) with a test + one rustdoc line on `max_attempts`.
9. **AsyncView clone cost**: one rustdoc paragraph on `AsyncView` noting the state value is cloned per reactive re-render (inherent to owned-children over `Signal<AsyncState<T>>`) and that large `T`s belong behind `Arc`.

### Details — Dart scrub (crates/ + examples/)

`grep -rni 'dart|flutter|AsyncDataReloading'` must go to zero in crates/ and examples/. Rewrite each hit so the text stands alone. Guidance: "Rust delta from the Dart port" sections become plain design rationale ("execute returns Result directly; there is no exception-catching wrapper — Rust has no exceptions"); fixture docs describe behavior, not `usecase_test.dart` mirrors; `examples/team-demo/Cargo.toml` description becomes e.g. "clean-signals example app — a small team-roster feature slice (CSR)"; the retry default-backoff comment cites nothing upstream ("defaults to 2.0"). Do not delete useful semantics — only the lineage framing. Historical pipeline files under workflow/ are OUT of scope (leave them).

### Finish

`cargo fmt --all` as the FINAL edit. Full DoD: build, test --workspace, clippy --workspace --all-targets -D warnings, both wasm checks, test-fixtures clippy, plus `cargo fmt --all -- --check` clean and `cargo test -p clean-signals controller` 3x stable.

### Acceptance Criteria

1. Zero Dart/Flutter/AsyncDataReloading matches in crates/ + examples/ (case-insensitive).
2. Items 1-9 implemented as specified; new/adapted tests green (emit isolation, retry clamp, SlowTicker termination).
3. team-demo no longer depends on async-trait directly; uses the core re-export.
4. Full DoD + fmt check green; public API otherwise unchanged.

---

## Completion Summary

**Status:** Done
**Branch:** worktree-wf_357712b9-098-1

### Files Modified

| File | Changes |
|------|---------|
| `crates/clean-signals/src/lib.rs` | Added `pub use async_trait::async_trait;` re-export with dual-`cfg_attr` rustdoc (item 1). |
| `crates/clean-signals/src/controller.rs` | Added `lock_recovering` + `prune_watch` helpers; replaced all 7 `.lock().unwrap_or_else(...)` sites and both prune sites (item 3); per-listener `catch_unwind` isolation in `FailureSink::emit` with wasm caveat (item 4); RemoveOnDrop rustdoc correction (item 2); `run` TOCTOU rustdoc section (item 6); SlowTicker fixture bounded to 2 items + debug_assert at gate park (item 7); adapted panicking-listener test to assert same-emit isolation; Dart-lineage scrub in module/type docs. |
| `crates/clean-signals/src/use_case.rs` | StreamUseCase laziness contract in trait rustdoc (item 5); Dart-lineage scrub (module doc, Ticker/Counting/fixtures docs). |
| `crates/clean-signals/src/retry.rs` | `max_attempts` clamp rustdoc on field + `new` (item 8); `new(0,_)` clamp test; Dart-lineage scrub on backoff default. |
| `crates/clean-signals/src/activity.rs` | Dart-lineage scrub in module doc. |
| `crates/clean-signals/src/async_state.rs` | Dart-lineage scrub (removed Dart `signals`/`AsyncDataReloading` framing). |
| `crates/clean-signals-leptos/src/async_view.rs` | AsyncView clone-cost rustdoc section (item 9). |
| `examples/team-demo/Cargo.toml` | Removed direct `async-trait` dep; description reworded (Dart scrub). |
| `examples/team-demo/src/**` | Switched all `async_trait::async_trait` → `clean_signals::async_trait` (5 files); Dart-lineage scrub in `lib.rs` and `pages.rs`. |

### Notable Decisions/Tradeoffs

1. **Item 7 debug assertion placement**: Placed the fail-fast `debug_assert!` at the gate-park point inside the SlowTicker stream body (asserting the stashed gate receiver survived to item 2), rather than a "slot must be empty on entry" assert in `execute()`. The latter interpretation would fire in the registry/dispose-all tests, which deliberately construct multiple SlowTicker watches without taking the gate; the park-site assert only triggers on a genuinely lost gate and keeps all existing tests green.
2. **cupline references left as-is**: `cupline`/`orders.rs` mentions in the leptos crate are a different project's pattern reference (design rationale for what a helper replaces), not Dart lineage, and fall outside the AC's `dart|flutter|AsyncDataReloading` grep. Left untouched to avoid scope creep.
3. **emit swallows panics**: `catch_unwind` result is dropped (`let _ =`); the default panic hook still logs during unwind, satisfying the "debug_assert-free logging path" requirement without re-raising.

### Testing Performed

- `cargo build --workspace` - Passed
- `cargo test --workspace` - Passed (all suites; new tests `new_clamps_zero_max_attempts_to_one`, `failure_sink_panicking_listener_is_isolated_and_does_not_poison` green)
- `cargo clippy --workspace --all-targets -- -D warnings` - Passed
- `cargo clippy -p clean-signals --all-targets --features test-fixtures -- -D warnings` - Passed
- `cargo check -p clean-signals --target wasm32-unknown-unknown` - Passed
- `cargo check -p clean-signals-leptos --target wasm32-unknown-unknown` - Passed
- `cargo test -p clean-signals --test spike_reactive_graph` - Passed (10 tests)
- `cargo fmt --all -- --check` - Passed
- `cargo test -p clean-signals controller` x3 - Passed (stable, 24 tests each run)
- `grep -rniE 'dart|flutter|AsyncDataReloading' crates/ examples/` - Zero matches
- `cargo tree -p team-demo -i async-trait` - team-demo reaches async-trait only transitively (via clean-signals re-export + leptos), no direct dep

### Risks/Limitations

1. **wasm32 emit isolation**: `catch_unwind` cannot intercept a panicking listener on wasm32 (panics abort there); documented as a caveat in the `FailureSink` rustdoc. No behavior change on native.
2. **TOCTOU on failure emission**: The cross-thread dispose/emit race remains a narrow accepted limitation (documented per the ACCEPTED list); signal writes are unaffected (all `try_*`).
