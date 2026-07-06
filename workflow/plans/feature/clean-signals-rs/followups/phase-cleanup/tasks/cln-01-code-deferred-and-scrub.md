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
