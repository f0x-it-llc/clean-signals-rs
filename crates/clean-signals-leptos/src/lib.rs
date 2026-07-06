//! clean-signals-leptos — the Leptos 0.8 bridge for `clean-signals`.
//!
//! This is the **only** crate where the framework meets leptos rendering
//! APIs; the core [`clean_signals`] crate stays DOM-free. It supplies the
//! four helpers a presentation layer needs to drive controllers safely from
//! components:
//!
//! - [`use_controller`] — build a controller once, dispose it on unmount.
//! - [`AsyncView`] — render an `AsyncState<T, F>` signal declaratively.
//! - [`use_failure_listener`] — surface a controller's failures, scoped to
//!   the component.
//! - [`use_interval`] — poll on a timer that stops when the component
//!   unmounts.
//!
//! Together they replace the hand-rolled patterns a raw-leptos page
//! accumulates (a reference offender: cupline's `cl-dashboard/src/pages/orders.rs`
//! — raw `RwSignal`s, an inline refetch `Effect`, a bespoke `spawn_local`
//! polling loop with a manual alive-flag, and swallowed errors).
//!
//! # The page-controller pattern, end to end
//!
//! A controller (view model) embeds a [`ControllerCore`](clean_signals::ControllerCore)
//! by composition and exposes its state signals; the page wires it up with the
//! helpers below. This is the "after" shape the `orders.rs` offender becomes:
//!
//! ```rust,ignore
//! use clean_signals::{async_state_signal, AsyncState, ControllerCore, RunOptions};
//! use clean_signals_leptos::{use_controller, use_interval, use_failure_listener, AsyncView};
//! use leptos::prelude::*;
//! use std::rc::Rc;
//! use std::time::Duration;
//!
//! // 1. A controller embedding a ControllerCore, owning its state signal.
//! struct OrdersController {
//!     core: ControllerCore<AppFailure>,
//!     orders: RwSignal<AsyncState<Vec<Order>, AppFailure>>,
//!     repo: Rc<OrdersRepo>,
//! }
//!
//! impl OrdersController {
//!     fn new(repo: Rc<OrdersRepo>) -> Self {
//!         Self { core: ControllerCore::new(), orders: async_state_signal(), repo }
//!     }
//!     async fn reload(self: Rc<Self>) {
//!         let _ = self.core
//!             .run_into(&ListOrders(self.repo.clone()), (), self.orders, RunOptions::default())
//!             .await;
//!     }
//! }
//!
//! // Expose the core so `use_controller` can dispose it.
//! impl AsRef<ControllerCore<AppFailure>> for OrdersController {
//!     fn as_ref(&self) -> &ControllerCore<AppFailure> { &self.core }
//! }
//!
//! // 2. The page: no raw signals, no inline effects, no swallowed errors.
//! #[component]
//! fn OrdersPage(repo: Rc<OrdersRepo>) -> impl IntoView {
//!     let controller = use_controller::<OrdersController, AppFailure>(move || {
//!         OrdersController::new(repo)
//!     });
//!
//!     // Surface failures once, scoped to this page.
//!     controller.with_value(|c| {
//!         use_failure_listener(c.core.failures(), move |f: AppFailure| {
//!             leptos::logging::error!("{}", f.user_message());
//!         });
//!     });
//!
//!     // Poll every 8s; the loop stops automatically on unmount.
//!     let poll = controller;
//!     use_interval(Duration::from_secs(8), move || {
//!         let c = poll.get_value();
//!         leptos::task::spawn_local(c.reload());
//!     });
//!
//!     let orders = controller.with_value(|c| c.orders);
//!     view! {
//!         <AsyncView
//!             state=orders
//!             children=move |orders: Vec<Order>| view! { <OrderList orders/> }.into_any()
//!         />
//!     }
//! }
//! ```
//!
//! Everything is torn down deterministically when `OrdersPage` unmounts: the
//! controller is disposed (aborting in-flight work), the failure subscription
//! is dropped, and the polling loop stops.

pub mod async_view;
pub mod failure_listener;
pub mod hooks;
pub mod interval;

pub use async_view::{AsyncView, AsyncViewProps, ErrorSlot};
pub use failure_listener::use_failure_listener;
pub use hooks::{expect_controller, provide_controller, use_controller};
pub use interval::use_interval;
