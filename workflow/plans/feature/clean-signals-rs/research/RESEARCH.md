# Research Synthesis — clean-signals-rs

Sources: two pre-plan deep-research agents (clean_signals architectural map; Leptos ecosystem survey) + a 5-question research-sweep (109 agents, all load-bearing claims adversarially verified). Raw extracts: `RESEARCH-raw.md`, `dart-test-spec.md`, `cupline-surface.md`, `async-and-channels.md`.

## Verified facts the plan is built on

### reactive_graph standalone (Leptos 0.8.x)
- Dep: `reactive_graph = "0.2"` (0.2.14+ pairs with leptos 0.8.19+). Leptos 0.8 has **no default features**; the **`effects` feature must be explicitly enabled** for `Effect::new` to run (csr/hydrate enable it in apps; tests/libs enable `reactive_graph/effects` themselves). `Effect::new_isomorphic` runs regardless of the feature.
- Owner API: `Owner::new()`, `Owner::with(closure)` (NOT `as_current()`), `Owner::cleanup()`, `Owner::on_cleanup(f)` (same fn as `leptos::prelude::on_cleanup`). Effects/signals CAN be created without a current Owner — they just won't be auto-disposed.
- `any_spawner = "0.3"`: `Executor::init_tokio()`, `Executor::init_wasm_bindgen()`, `Executor::tick()` (exists: `pub async fn tick()`), `Executor::spawn_local()`.
- `RwSignal`/`Memo` handles are **unconditionally Send + Sync** (arena index handles). `RwSignal::new(v)` requires `T: Send + Sync + 'static`; `RwSignal::new_local()` pins the value to the creating thread and **panics at runtime on cross-thread access** (the handle itself is still Send).
- `.try_set()`/`.try_update()` return `Option` on disposed signals — the natural Rust "post-dispose write guard".
- `StoredValue` = non-reactive Copy handle; `ArcRwSignal` = reference-counted signal for lifetimes exceeding an Owner.

### Async trait strategy (native + wasm32)
- Native async-fn-in-trait (RPITIT) is still **not dyn-compatible**; `trait_variant` is generic-dispatch only.
- The documented cross-target pattern for dyn-compatible async traits:
  ```rust
  #[cfg_attr(not(target_arch = "wasm32"), async_trait::async_trait)]
  #[cfg_attr(target_arch = "wasm32", async_trait::async_trait(?Send))]
  ```
  applied to both trait definition and impls. Framework core calls use cases **generically** (no dyn needed internally); the async_trait attribute keeps app-side `Arc<dyn Repository>` possible.

### Channels & timers (wasm-portable)
- `tokio::sync` on wasm: the known Atomics.wait panic (tokio#7205) is specific to **mpsc under multithreaded-wasm builds**; still, tokio sync machinery is the wrong dependency for a UI lib.
- `async-broadcast`/`event-listener 5.4.1` are wasm-CI-tested and safe — but **we don't need a broadcast channel at all**: a callback-registry `FailureSink` (Mutex'd listener Vec, RAII unsubscribe) is dependency-free, deterministic, and matches "failures are events".
- Timers: `wasm-timer` is abandoned. The norm is cfg-gating: `tokio::time::sleep` (native, `time` feature) / `gloo-timers::future` (wasm, `futures` feature). gloo-timers exposes `future::TimeoutFuture`, `future::sleep`.

### Dart test-suite porting spec
32 tests across 5 files pin the semantics; full list in `dart-test-spec.md`. Highlights: retry emits only the FINAL failure; `maxAttempts` counts attempts not retries; retries only when `is_retryable` (or custom `retry_if`); `run_into` drives Loading → Data, and reload keeps stale data visible (`Reloading`); dispose is idempotent, runs cleanups LIFO, cancels watches immediately; post-dispose activity decrements are suppressed; `StreamUseCase` converts stream errors into a trailing Failed event then terminates. Helper fakes to port: `FlakyUseCase`, `SlowUseCase` (gate/Completer → oneshot channel), `TickerUseCase`, `CountingStream`, `FailingStream`, `TestController`.

### Cupline retrofit surface
Full map in `cupline-surface.md`. Key types: `OrderDto` (cl-core/src/dto.rs:180-202), `ApiError` (8 variants, cl-ui/src/api/error.rs), `list_orders(shop_id, all) -> Result<Vec<OrderDto>, ApiError>` / `set_order_status(order_id, &str) -> Result<OrderDto, ApiError>` (cl-ui/src/api/dashboard.rs:20-37), `ShopState` context via `use_shop_state()` (cl-dashboard/src/pages/util.rs:32-79), routing in cl-dashboard/src/lib.rs:36-56.

## Design deltas vs the Dart original (Rust-idiomatic, deliberate)
1. **Generic over the app failure type `F: Failure + Clone`** instead of a dynamic `Failure` base class — exhaustive matching without downcasts.
2. **No `Result` reimplementation** — `std::result::Result<T, F>` + an extension trait for `to_async_state()`. Dart's `guard`/`UnexpectedFailure` catch-all disappears: Rust has no exceptions; the equivalent discipline is one `From<TransportError> for AppFailure` per repository impl. No panic-catching.
3. **Layering enforced by crates**, not prose: core crate has zero leptos/DOM deps.
4. **Failures sink = callback registry**, not broadcast stream.
5. **Stream cancellation** comes free from drop semantics (Dart needed a custom StreamTransformer).
