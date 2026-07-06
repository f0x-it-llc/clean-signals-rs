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
