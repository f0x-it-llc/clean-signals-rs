# Plan: clean-signals-rs — Rust/Leptos clean-architecture framework

## TL;DR

Port the Dart `clean_signals` framework (v2.1.0, 14 public API items) to a Rust workspace targeting Leptos 0.8 apps: a reactivity-core crate built on `reactive_graph` (no leptos/DOM dependency), a `clean-signals-leptos` integration crate (component-scoped controllers, `AsyncView`, `FailureListener`, polling helper), a CSR example app, core docs + a downstream `AGENTS.md` template, validated by retrofitting cupline's dashboard orders page onto the framework.

---

## Background

`clean_signals` gives Flutter apps strict, testable clean architecture: use cases return `Result`, controllers own signal state and orchestrate via `run`/`runInto`/`watch` with ref-counted loading, per-call retry, and failures-as-events. Cupline's Leptos portals have no equivalent — pages own raw `RwSignal`s, inline effects, hand-rolled polling, and swallowed errors (`cl-dashboard/src/pages/orders.rs` is the reference offender). Research (see `research/RESEARCH.md`) verified that Leptos 0.8's `reactive_graph` works standalone in plain structs and tokio tests, so the framework builds on Leptos's own reactivity — not `futures-signals`.

## Approved design decisions (locked)

1. **Workspace layout** (compile-time layering — core cannot import leptos):
   ```
   Cargo.toml                      # workspace: resolver 3, workspace deps
   crates/clean-signals/           # core: failure, use_case, retry, time, activity, async_state, controller
   crates/clean-signals-leptos/    # integration: use_controller, AsyncView, FailureListener, watch_interval
   examples/team-demo/             # CSR Leptos app porting the Dart example's team feature
   docs/                           # ARCHITECTURE / CODE_STANDARDS / DEVELOPMENT
   templates/AGENTS.md             # downstream architecture-rules template (mirrors Dart's)
   ```
2. **Generic over app failure type**: everything is parameterized by `F: Failure + Clone` (trait: `Debug + Display + Send + Sync + 'static`, methods `user_message()`, `is_retryable()`). No `UnexpectedFailure` port; apps map transport errors via `From` impls at the repository boundary.
3. **`std::result::Result`** everywhere; a `ResultExt::to_async_state()` bridge. No custom Result type.
4. **Async traits**: `#[cfg_attr(not(target_arch="wasm32"), async_trait)] #[cfg_attr(target_arch="wasm32", async_trait(?Send))]` on `UseCase`; framework calls use cases generically. `StreamUseCase` returns a cfg-gated boxed stream alias (`BoxStream` native / `LocalBoxStream` wasm).
5. **Failure events**: dependency-free `FailureSink<F>` callback registry with RAII `Subscription` guards (no broadcast channel).
6. **Timers**: `clean_signals::time::sleep(Duration)` cfg-gated — tokio `time` (native) / gloo-timers `futures` (wasm).
7. **Spawning**: `any_spawner::Executor::spawn_local` for `watch`; apps init via leptos mount, tests via `Executor::init_tokio()` (exact test recipe determined by the spike task and recorded in `research/SPIKE_NOTES.md`).
8. **Post-dispose safety**: framework signal writes use `try_set`/`try_update` + an explicit `is_disposed` flag in `ControllerCore`.
9. **All dependencies pre-declared in the Wave-0 scaffold** so parallel tasks never edit Cargo.toml files.
10. **Cupline retrofit lands on a branch in the cupline repo** (`clean-signals-retrofit`), never merged by the pipeline — user reviews it.

## Core API contract (parallel implementors code against THIS)

```rust
// failure.rs
pub trait Failure: std::fmt::Debug + std::fmt::Display + Send + Sync + 'static {
    fn user_message(&self) -> String { self.to_string() }
    fn is_retryable(&self) -> bool { false }
}

// result_ext.rs (inside async_state.rs module or its own — owner: task 05)
pub trait ResultExt<T, F> { fn to_async_state(self) -> AsyncState<T, F>; }

// use_case.rs
#[cfg_attr(not(target_arch = "wasm32"), async_trait::async_trait)]
#[cfg_attr(target_arch = "wasm32", async_trait::async_trait(?Send))]
pub trait UseCase {
    type Params;
    type Output;
    type Failure: Failure + Clone;
    async fn execute(&self, params: Self::Params) -> Result<Self::Output, Self::Failure>;
}
pub struct NoParams;                       // unit param
pub type UseCaseStream<T, F> = /* cfg-gated: BoxStream<'static, Result<T,F>> | LocalBoxStream */;
pub trait StreamUseCase {
    type Params; type Output; type Failure: Failure + Clone;
    fn execute(&self, params: Self::Params) -> UseCaseStream<Self::Output, Self::Failure>;
}

// retry.rs
#[derive(Clone)]
pub struct RetryPolicy<F> {
    pub max_attempts: u32,          // total attempts, not retries
    pub delay: std::time::Duration,
    pub backoff_factor: f64,        // delay_for(n) = delay * backoff_factor^(n-1)
    /* retry_if: Option<Arc<dyn Fn(&F) -> bool + Send + Sync>> */
}
impl<F: Failure> RetryPolicy<F> {
    pub fn none() -> Self;                       // max_attempts = 1
    pub fn delay_for(&self, attempt: u32) -> std::time::Duration;
    pub fn should_retry(&self, f: &F) -> bool;   // retry_if else f.is_retryable()
}

// time.rs
pub async fn sleep(d: std::time::Duration);      // tokio::time::sleep | gloo_timers::future::sleep

// activity.rs
pub struct ActivityTracker { /* pending: RwSignal<u32>, is_loading: Memo<bool>, disposed flag */ }
impl ActivityTracker {
    pub fn new() -> Self;
    pub fn begin(&self) -> ActivityGuard;        // RAII: decrements on drop; post-dispose no-op
    pub fn pending(&self) -> u32;
    pub fn is_loading(&self) -> Memo<bool>;
    pub fn dispose(&self);
}

// async_state.rs
#[derive(Clone, Debug, PartialEq)]
pub enum AsyncState<T, F> {
    Loading,
    Data(T),
    Reloading(T),                                // stale data visible during refresh
    Error { failure: F, stale: Option<T> },
}
impl<T, F> AsyncState<T, F> {
    pub fn value(&self) -> Option<&T>;           // Data | Reloading | Error{stale:Some}
    pub fn has_value(&self) -> bool;
    pub fn is_loading(&self) -> bool;            // Loading | Reloading
    pub fn failure(&self) -> Option<&F>;
}
pub fn async_state_signal<T, F>() -> RwSignal<AsyncState<T, F>>   // seeded Loading
    where T: Send + Sync + 'static, F: Send + Sync + 'static;

// controller.rs — composition, not inheritance: app controllers EMBED a ControllerCore
pub struct RunOptions<F> { pub retry: RetryPolicy<F>, pub track_activity: bool, pub emit_failure: bool } // Default: none/true/true
pub struct ControllerCore<F: Failure + Clone> { /* activity, failures: FailureSink<F>, cleanups, disposed */ }
impl<F: Failure + Clone> ControllerCore<F> {
    pub fn new() -> Self;
    pub async fn run<U>(&self, uc: &U, params: U::Params, opts: RunOptions<F>) -> Result<U::Output, F>
        where U: UseCase<Failure = F>;
    pub async fn run_into<U>(&self, uc: &U, params: U::Params, into: RwSignal<AsyncState<U::Output, F>>, opts: RunOptions<F>) -> Result<U::Output, F>
        where U: UseCase<Failure = F>, U::Output: Clone + Send + Sync + 'static;
    pub fn watch<S>(&self, uc: &S, params: S::Params, on_data: impl Fn(S::Output) + 'static) -> WatchHandle
        where S: StreamUseCase<Failure = F>;     // Failed events -> failures sink; aborted on dispose
    pub fn on_dispose(&self, f: impl FnOnce() + Send + 'static);
    pub fn is_disposed(&self) -> bool;
    pub fn is_loading(&self) -> Memo<bool>;
    pub fn failures(&self) -> &FailureSink<F>;
    pub fn dispose(&self);                        // idempotent; LIFO cleanups; aborts watches
}
pub struct FailureSink<F> { /* Mutex<Vec<(u64, Box<dyn Fn(&F) + Send + Sync>)>> */ }
impl<F> FailureSink<F> {
    pub fn subscribe(&self, f: impl Fn(&F) + Send + Sync + 'static) -> Subscription;  // RAII unsubscribe
    pub fn emit(&self, f: &F);
}
```

Semantics are pinned by the Dart test suite port — see `research/dart-test-spec.md` (32 behaviors). Non-negotiables: only the FINAL failure is emitted after retries; `max_attempts` counts attempts; `Reloading` keeps stale data; dispose = idempotent + LIFO + immediate watch abort; post-dispose writes suppressed.

## Leptos integration crate API (task 07)

```rust
pub fn use_controller<C, F: Failure + Clone>(factory: impl FnOnce() -> C) -> StoredValue<C>
    where C: AsRef<ControllerCore<F>> + 'static;   // creates once, on_cleanup -> dispose
#[component] pub fn AsyncView<T, F>(state: Signal<AsyncState<T, F>>, children: …, loading: …, error: …) -> impl IntoView;
pub fn use_failure_listener<F>(sink: &FailureSink<F>, handler: impl Fn(&F) + 'static);  // Subscription tied to owner cleanup
pub fn watch_interval(period: Duration, f: impl Fn() + 'static);                        // spawn_local loop, stops on owner cleanup
```

## Development Phases (waves)

- **Wave 0 (conductor, inline):** workspace scaffold — all Cargo.tomls with full dependency set, `lib.rs` with all `pub mod` declarations + empty module files, `.gitignore`, initial git commit.
- **Wave 1 (parallel worktrees):** 01-spike (reactive_graph standalone validation → `SPIKE_NOTES.md`) ∥ 02-failure trait + tests.
- **Wave 2 (parallel worktrees):** 03-use_case ∥ 04-retry+time ∥ 05-activity+async_state.
- **Wave 3 (sequential, main loop):** 06-controller (the heart; consumes everything + SPIKE_NOTES).
- **Wave 4 (parallel worktrees):** 07-leptos-integration ∥ 08-docs+AGENTS-template (doc_maintainer).
- **Wave 5 (parallel across repos):** 09-example-app (implement-wave) ∥ 10-cupline-retrofit (main-loop implementor in the cupline repo, branch only).
- **Review:** `review-diff` over the full phase in this repo + manual review panel over the cupline branch diff.

## Edge Cases & Risks

- **any_spawner `spawn_local` under tokio tests** may require a current-thread runtime/LocalSet; the spike (task 01) resolves the exact recipe before task 06 needs it. Mitigation: `watch` design admits a fallback (return the stream-driving future for the caller to spawn) if spawn_local proves awkward — spike decides.
- **`effects` feature**: `Effect::new` silently never runs if the feature is missing — DEVELOPMENT.md must document it; core enables `reactive_graph/effects` in dev-dependencies for its own tests; whether core's *library* code uses `Effect` at all is decided by task 06 (prefer `Memo` + explicit methods; `auto_effect` may live in the leptos crate instead).
- **wasm32 verification**: `cargo check -p clean-signals --target wasm32-unknown-unknown` requires the rustup target; installed during Wave 0 if missing.
- **Cupline is a separate git repo** — the retrofit must never be squash-merged by this pipeline; branch + report only.
- **Dart→Rust semantic drift**: mitigated by porting the Dart test suite test-for-test (the 32-behavior spec is the acceptance contract).

## Success Criteria

- [ ] `cargo build --workspace && cargo test --workspace && cargo clippy --workspace --all-targets -- -D warnings` green.
- [ ] Core crate has no leptos/DOM dependency; `cargo check -p clean-signals --target wasm32-unknown-unknown` passes.
- [ ] All 32 Dart-pinned behaviors have passing Rust equivalents.
- [ ] Example app: controllers unit-tested natively with fake repos; wasm build check passes.
- [ ] Cupline `clean-signals-retrofit` branch: OrdersPage reduced to a dumb view over `OrdersController`; controller covered by native tests with a fake repository; `cargo check` for cl-dashboard passes on the branch.
- [ ] docs/ + templates/AGENTS.md exist and pass doc-standards boundaries.

## References

- `research/RESEARCH.md` (verified findings), `research/dart-test-spec.md`, `research/cupline-surface.md`, `research/async-and-channels.md`
- Dart original: `/home/ed/Dev/personal/clean_signals`
- First consumer: `/home/ed/Dev/personal/cupline`
