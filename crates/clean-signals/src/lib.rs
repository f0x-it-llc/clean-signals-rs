//! clean-signals — clean-architecture framework core for Rust/Leptos apps.
//!
//! Reactivity-aware but DOM-free: this crate depends only on `reactive_graph`
//! and must never import leptos rendering APIs. See docs/ARCHITECTURE.md.
//!
//! # What it gives you
//!
//! Apps implement [`UseCase`]s that return `Result<Output, Failure>`, and
//! presentation-layer *controllers* embed a [`ControllerCore`] that orchestrates
//! them through [`run`](ControllerCore::run) /
//! [`run_into`](ControllerCore::run_into) / [`watch`](ControllerCore::watch),
//! providing ref-counted loading, per-call retries, failures-as-events, and
//! lifecycle-scoped cleanup.
//!
//! # Example
//!
//! ```rust
//! use clean_signals::{
//!     async_state_signal, AsyncState, ControllerCore, Failure, NoParams, RunOptions, UseCase,
//! };
//! use reactive_graph::signal::RwSignal;
//! use reactive_graph::traits::GetUntracked;
//! use std::fmt;
//!
//! // 1. An app failure enum.
//! #[derive(Clone, Debug, PartialEq)]
//! enum AppFailure {
//!     NotFound,
//! }
//! impl fmt::Display for AppFailure {
//!     fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
//!         f.write_str("not found")
//!     }
//! }
//! impl Failure for AppFailure {}
//!
//! // 2. A use case.
//! struct GetGreeting;
//! #[async_trait::async_trait]
//! impl UseCase for GetGreeting {
//!     type Params = NoParams;
//!     type Output = String;
//!     type Failure = AppFailure;
//!     async fn execute(&self, _p: NoParams) -> Result<String, AppFailure> {
//!         Ok("hello".to_string())
//!     }
//! }
//!
//! // 3. A controller embedding a ControllerCore.
//! struct GreetingController {
//!     core: ControllerCore<AppFailure>,
//!     greeting: RwSignal<AsyncState<String, AppFailure>>,
//! }
//! impl GreetingController {
//!     fn new() -> Self {
//!         Self { core: ControllerCore::new(), greeting: async_state_signal() }
//!     }
//!     async fn load(&self) {
//!         let _ = self
//!             .core
//!             .run_into(&GetGreeting, NoParams, self.greeting, RunOptions::default())
//!             .await;
//!     }
//! }
//!
//! // 4. Drive it (no retry/sleep/spawn, so a plain block_on suffices).
//! let controller = GreetingController::new();
//! futures::executor::block_on(controller.load());
//! assert_eq!(
//!     controller.greeting.get_untracked(),
//!     AsyncState::Data("hello".to_string())
//! );
//! controller.core.dispose();
//! ```

pub mod activity;
pub mod async_state;
pub mod controller;
pub mod failure;
pub mod retry;
pub mod time;
pub mod use_case;

// Flat public surface — the framework's vocabulary, re-exported at the crate
// root so downstream code can `use clean_signals::{ControllerCore, Failure, ...}`.
pub use activity::{ActivityGuard, ActivityTracker};
pub use async_state::{async_state_signal, to_reloading, AsyncState, ResultExt};
pub use controller::{ControllerCore, FailureSink, RunOptions, Subscription, WatchHandle};
pub use failure::Failure;
pub use retry::RetryPolicy;
pub use time::sleep;
pub use use_case::{NoParams, StreamUseCase, UseCase, UseCaseStream};
