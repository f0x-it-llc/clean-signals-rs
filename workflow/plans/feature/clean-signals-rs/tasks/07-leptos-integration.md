## Task: clean-signals-leptos integration crate

**Objective**: Implement the Leptos 0.8 bridge: component-scoped controller lifecycle, `AsyncView`, failure listening, and a safe polling helper — replacing cupline-style hand-rolled patterns.

**Depends on**: 06-controller

**Complexity:** high

### Scope

**Files Modified (Write):**
- `crates/clean-signals-leptos/src/lib.rs`
- `crates/clean-signals-leptos/src/hooks.rs`
- `crates/clean-signals-leptos/src/async_view.rs`
- `crates/clean-signals-leptos/src/failure_listener.rs`
- `crates/clean-signals-leptos/src/interval.rs`

**Files Read (Dependencies):**
- `crates/clean-signals/src/*` (final API)
- `workflow/plans/feature/clean-signals-rs/PLAN.md` (§ Leptos integration crate API)
- `workflow/plans/feature/clean-signals-rs/research/SPIKE_NOTES.md`
- Cupline anti-pattern reference: `/home/ed/Dev/personal/cupline/crates/cl-dashboard/src/pages/orders.rs` (read-only — what these helpers must make obsolete)

### Details

Deps pre-declared: leptos 0.8 (no default features + `effects`? — leptos is brought in by the APP with csr/hydrate; this crate should depend on the narrowest thing that gives `on_cleanup`, `component`, `IntoView`: use `leptos = { workspace = true }` as declared; do not edit Cargo.toml, work with what's there and note issues).

- `hooks.rs`:
  - `pub fn use_controller<C, F>(factory: impl FnOnce() -> C) -> StoredValue<C, LocalStorage>` where `C: AsRef<ControllerCore<F>> + 'static, F: Failure + Clone` — constructs once, `on_cleanup` calls `.as_ref().dispose()`. Return type may be adjusted (e.g. `StoredValue<Rc<C>>`) — pick what's ergonomic in `view!` closures and document. App controllers implement `AsRef<ControllerCore<F>>`.
  - `pub fn provide_controller<C>(c: C)` / `pub fn expect_controller<C: Clone>() -> C` — thin wrappers over leptos context for app-scoped controllers (documented: pages never dispose these).
- `async_view.rs`: `#[component] AsyncView<T, F>` over `Signal<AsyncState<T, F>>` with `children: … Fn(T) -> AnyView`, optional `loading`/`error` slot closures (sane defaults: simple div for loading; error slot receives `&F` and shows `user_message()`; `Reloading` renders children WITH data plus an optional `reloading_indicator`). Get the generic component to compile with leptos 0.8 `#[component]` (may need `<T: Send + Sync + Clone + 'static, F: Failure + Clone>` and `ViewFn`-style props — follow leptos idioms).
- `failure_listener.rs`: `pub fn use_failure_listener<F: Failure + Clone>(sink: &FailureSink<F>, handler: impl Fn(F) + 'static)` — subscribes; the `Subscription` is stored and dropped via `on_cleanup`. Handler runs outside any reactive tracking. (Component wrapper `#[component] FailureListener` optional — only if it adds value.)
- `interval.rs`: `pub fn use_interval(period: Duration, f: impl Fn() + 'static)` — wasm: spawn_local loop with `clean_signals::time::sleep`, alive-flag flipped in `on_cleanup` (the pattern orders.rs hand-rolls, done once, correctly); non-wasm (SSR): no-op. Also `pub fn use_polling<…>(period, controller-ish callback)` NOT required — keep surface minimal.
- Tests: what's testable natively without a DOM (hook logic factored to be Owner-testable): use_controller disposes on owner cleanup (construct Owner::new(), run factory, cleanup, assert controller.is_disposed()); failure_listener subscription drops on cleanup; interval alive-flag stops the loop (native test with tokio recipe if feasible, else document as wasm-only and gate). AsyncView: compile-test via a `#[cfg(test)]` usage + `cargo check`; rendering assertions are out of scope.

### Acceptance Criteria

1. `cargo build -p clean-signals-leptos` (native) and `cargo check -p clean-signals-leptos --target wasm32-unknown-unknown` green; clippy clean.
2. Owner-cleanup → controller.dispose() covered by a native test.
3. Rustdoc on every public item; crate-level doc shows the page-controller pattern end-to-end (the orders.rs "after" shape).

### Notes

- This crate is the ONLY place leptos APIs may be touched (layer rule) — core stays leptos-free.
- If `#[component]` generics fight back, an `AsyncView` free function returning `impl IntoView` is an acceptable fallback — document the choice.

---

## Completion Summary

**Status:** Done
**Branch:** worktree-wf_98ec4f7c-170-1

### Files Modified

| File | Changes |
|------|---------|
| `crates/clean-signals-leptos/src/lib.rs` | Crate-level doc showing the `orders.rs` "after" page-controller pattern end-to-end; public re-exports. |
| `crates/clean-signals-leptos/src/hooks.rs` | `use_controller` (StoredValue + on_cleanup dispose), `provide_controller`/`expect_controller`; native owner-cleanup + context round-trip tests. |
| `crates/clean-signals-leptos/src/async_view.rs` | Generic `#[component] AsyncView<T, F, Ch>` over `Signal<AsyncState<T,F>>` with optional `loading`/`error`/`reloading_indicator` slots + defaults; `ErrorSlot<F>` slot type; two compile-tests. |
| `crates/clean-signals-leptos/src/failure_listener.rs` | `use_failure_listener` — subscribe, drop `Subscription` on cleanup; native fire/stop test. |
| `crates/clean-signals-leptos/src/interval.rs` | `use_interval` (wasm spawn_local loop w/ Arc alive-flag + on_cleanup; native no-op); testable `run_interval` helper + native stop test. |

### Notable Decisions/Tradeoffs

1. **`use_controller` returns `StoredValue<Rc<C>, LocalStorage>`** (the task's sanctioned adjustment). The cleanup closure captures the `Send + Sync` `StoredValue` handle — not the `!Send` `Rc` — because `reactive_graph`'s `on_cleanup` requires `FnOnce() + Send + Sync`. Verified in `owner.rs` that cleanups run *before* stored values are torn down, so the handle is live at dispose time (`try_with_value` used as a belt-and-braces guard).
2. **`AsyncView` kept as a generic `#[component]`** (no free-function fallback needed). `children: Ch = Fn(T) -> AnyView` is a required generic prop; `loading`/`reloading_indicator` are `Option<ViewFn>` and `error` is `Option<ErrorSlot<F>>`, all `#[prop(optional, into)]` so callers pass bare closures. Omitting them yields sane defaults (loading/error divs). The error slot receives the **owned** failure `F` (task said `&F`; owned clone is strictly more flexible and `user_message()` takes `&self`).
3. **`use_failure_listener` handler bound tightened to `Fn(F) + Send + Sync`** (PLAN sketch said `Fn(F)`). Forced by `FailureSink::subscribe`'s `Send + Sync` bound; leptos default `SyncStorage` signals are `Send + Sync`, so typical UI handlers satisfy it. Documented.
4. **`provide_controller` bound is `C: Send + Sync + 'static`** (leptos context requires it) → app-scoped controllers use `Arc`, not `Rc`. `use_controller` (component-scoped, LocalStorage) still accepts `!Send` controllers.
5. **`use_interval` uses `Arc<AtomicBool>`** for the alive-flag (again the `on_cleanup` Send+Sync requirement). The loop body is factored into `run_interval` (gated `#[cfg(any(target_arch = "wasm32", test))]`) so its stop-on-cleanup behavior is testable natively.

### Testing Performed

- `cargo build -p clean-signals-leptos` (native) — Passed
- `cargo check -p clean-signals-leptos --target wasm32-unknown-unknown` — Passed
- `cargo test -p clean-signals-leptos` — Passed (6 unit tests: use_controller dispose, provide/expect round-trip, failure listener fire/stop, interval stop, 2 AsyncView compile-tests)
- `cargo clippy -p clean-signals-leptos --all-targets -- -D warnings` (native + wasm) — Passed
- `RUSTDOCFLAGS="-D warnings" cargo doc -p clean-signals-leptos --no-deps` — Passed (all public items documented)

### Risks/Limitations

1. **AsyncView rendering not asserted**: DOM rendering assertions are out of scope (no headless browser). Coverage is compile-tests + the four-state match arms are exercised only for type-checking, per the task.
2. **`use_interval`/`use_failure_listener` cleanup behavior on real unmount** is validated via native `Owner::cleanup()` (which shares the same cleanup path), not a real wasm mount/unmount.

### Doc Updates Needed

None for core docs (`docs/` does not exist in this repo yet — task 08 owns doc creation). Two API deviations that task 08 / a downstream `AGENTS.md` should capture: `use_failure_listener` handler must be `Send + Sync`, and app-scoped controllers via `provide_controller` must be `Arc` (Send+Sync), while `use_controller` accepts `!Send` `Rc` controllers.
