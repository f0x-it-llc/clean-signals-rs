## async-trait-dyn-strategy
SUMMARY: Native async fn in trait (Rust 1.75+) is NOT dyn-compatible; traits using async fn cannot be used with trait objects. Production patterns: (1) #[async_trait] with ?Send cfg-gated for wasm (async-trait crate, heap allocation), (2) trait_variant for explicit Send/!Send variants (zero allocation), (3) generic-only design (best if viable), (4) dynosaur (experimental Rust 1.75+, zero-cost dyn async). Ecosystem crates avoid dyn dispatch for async: Leptos uses cfg!-gated executors; Gloo uses generic futures. For a framework seeking Arc<dyn UseCase>, recommend trait_variant + #[async_trait] with cfg_attr for Repository (native requires Send, wasm doesn't), or prefer generic dispatch if polymorphism via dyn isn't required. Migrate to dynosaur in 2025+ when stable.
CLAIMS:
- Native async fn in trait (stabilized Rust 1.75) cannot be used with dyn Trait; async fn in traits lack object-safety.
- async_trait crate (#[async_trait] macro) is the production-standard workaround for dyn async traits; uses Pin<Box<dyn Future + Send>> by default.
- async_trait supports #[async_trait(?Send)] to remove Send bounds, enabling wasm/single-threaded executors; must be applied to both trait and impl blocks.
- trait_variant crate (rust-lang org) creates explicit Send/!Send trait variants via #[trait_variant::make(SendVariant: Send)].
- dynosaur is an experimental proc-macro (Rust 1.75+) that enables zero-cost dyn dispatch on async fn traits via #[dynosaur::dynosaur(DynTrait = dyn(box) Trait)].
- maybe-sync crate implements MaybeSend and MaybeSync trait aliases that conditionally resolve to Send/Sync or empty traits, enabling single-codebase native/wasm support.
- Leptos handles wasm/native async divergence via cfg-gated executors (spawn_local for wasm, tokio spawn for native), not dyn traits; avoids async trait objects.
- Gloo crate avoids dyn trait dispatch for async; provides generic future-based APIs (e.g., gloo_timers::future::TimeoutFuture) that work on both wasm and native without Send requirements.
- For Arc<dyn UseCase> supporting both native and wasm, trait_variant + #[async_trait] with cfg_attr on Repository (native: Send, wasm: ?Send) is recommended pattern; generic dispatch preferred if polymorphism not required.
CAVEATS: none

## wasm-portable-channels-timers
SUMMARY: For wasm32-unknown-unknown (browser), **do not use tokio::sync::broadcast or tokio::time::sleep**—both panic due to Atomics/std::time::Instant incompatibilities. Best practice is callback-registry patterns (Leptos signals + callbacks) or async-broadcast for controlled environments. Use wasm-timer v0.2.5 or gloo-timers for cross-platform delays. Cfg-gating is the norm; tokio's "sync" feature is nominally stable on wasm32 but unreliable in practice.
CLAIMS:
- tokio::sync::broadcast causes panics on wasm32-unknown-unknown due to Atomics.wait blocking the main thread
- tokio::time::sleep panics on wasm32-unknown-unknown (std::time::Instant::now is unavailable)
- async-broadcast v0.7.2 (May 2026) also problematic on wasm32-unknown-unknown due to event-listener dependency using Mutex/Atomics
- Leptos and Yew UI frameworks do not use broadcast channels; instead use signal-based reactivity (Leptos) or component re-render patterns (Yew)
- wasm-timer v0.2.5 is a maintained cross-platform sleep abstraction: uses tokio-timer on native targets, gloo/web-sys on wasm
- gloo-timers TimeoutFuture is designed specifically for wasm32-unknown-unknown and is actively used in Leptos ecosystem
- Tokio v1.52.3 (May 2026 latest) nominally supports sync feature on wasm32-unknown-unknown but is unreliable in practice
- cfg-gating with #[cfg(all(target_family = "wasm", target_os = "unknown"))] or #[cfg(not(target_arch = "wasm32"))] is standard practice
- tokio-with-wasm crate available as alternative providing tokio-like API for wasm32-unknown-unknown using JavaScript glue
- futures-timer has fundamental issues with wasm32-unknown-unknown due to Instant API limitations
CAVEATS: **Critical caveat:** tokio::sync and tokio::time on wasm32-unknown-unknown are only stable for wasm32-wasi (not wasm32-unknown-unknown). The search results document many panics and blocking issues. For wasm32-unknown-unknown (browser/JS runtime), **do not rely on any tokio features for critical path code**. If targeting wasm32-wasi (server-side WASM), tokio features are more reliable but still limited. Version tracking: Tokio 1.52.3 (May 2026), async-broadcast 0.7.2 (May 2026), event-listener 5.4.1, wasm-timer 0.2.5. No maintained futures-timer WASM alternative found; wasm-timer is the de facto standard."

