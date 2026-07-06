//! [`AsyncView`] — a declarative renderer for a controller's
//! [`AsyncState`] signal.
//!
//! Instead of hand-rolling `<Show when=… fallback=…>` plus a manual error
//! branch on every page (the cupline `orders.rs` pattern), a controller's
//! `RwSignal<AsyncState<T, F>>` is handed to `<AsyncView>` once and the four
//! states — loading, data, reloading, error — are mapped to views in one
//! place:
//!
//! ```rust,ignore
//! view! {
//!     <AsyncView
//!         state=controller.users
//!         children=move |users: Vec<User>| view! { <UserList users/> }.into_any()
//!         // optional overrides (sane defaults provided):
//!         loading=move || view! { <Spinner/> }
//!         error=move |f: AppFailure| view! { <Banner text=f.user_message()/> }.into_any()
//!         reloading_indicator=move || view! { <TopBar/> }
//!     />
//! }
//! ```

use clean_signals::{AsyncState, Failure};
use leptos::prelude::*;
use std::sync::Arc;

/// Optional error-slot closure: `Fn(F) -> impl IntoView`.
///
/// Wraps a caller-supplied error renderer so it can be stored as a concrete,
/// non-generic prop field. Constructed via `From`, so callers just pass a
/// closure — `error=move |f: AppFailure| view! { … }` — and never name this
/// type directly.
pub struct ErrorSlot<F>(Arc<dyn Fn(F) -> AnyView + Send + Sync>);

impl<F, Func, V> From<Func> for ErrorSlot<F>
where
    Func: Fn(F) -> V + Send + Sync + 'static,
    V: IntoView + 'static,
{
    fn from(f: Func) -> Self {
        ErrorSlot(Arc::new(move |failure| f(failure).into_any()))
    }
}

impl<F> ErrorSlot<F> {
    /// Renders the slot for `failure`.
    fn run(&self, failure: F) -> AnyView {
        (self.0)(failure)
    }
}

/// Renders a reactive [`AsyncState`] signal, mapping each of its four states
/// to a view.
///
/// - **`Loading`** → the `loading` slot, or a default `Loading…` div.
/// - **`Data(value)`** → `children(value)`.
/// - **`Reloading(value)`** → `children(value)` (stale data stays visible),
///   optionally preceded by the `reloading_indicator` slot.
/// - **`Error { failure, .. }`** → the `error` slot (receives the owned
///   `failure`), or a default div showing [`Failure::user_message`].
///
/// The whole view is reactive: it re-renders whenever `state` changes.
///
/// # Clone cost
///
/// Each reactive re-render reads the signal with `state.get()`, which **clones**
/// the whole `AsyncState<T, F>` — including the contained value `T` in the
/// `Data`/`Reloading` cases. This is inherent to rendering owned children from a
/// `Signal<AsyncState<T, F>>`: the value is handed to `children(value)` by
/// value. For small `T` this is negligible, but a large `T` (e.g. a big `Vec`)
/// is copied on every re-render — put such payloads behind an `Arc` (`Data<Arc<
/// Big>>`) so each render clones only a pointer.
///
/// # Props
///
/// - `state`: the controller's state signal (accepts `RwSignal`, `Memo`,
///   `Signal`, or a `move || …` closure via `into`).
/// - `children`: `Fn(T) -> AnyView` — the data renderer. Call `.into_any()`
///   on your `view! { … }`.
/// - `loading` *(optional)*: `Fn() -> impl IntoView` shown while first
///   loading.
/// - `error` *(optional)*: `Fn(F) -> impl IntoView` shown on failure; the
///   owned failure is passed so its [`Failure::user_message`] can be shown.
/// - `reloading_indicator` *(optional)*: `Fn() -> impl IntoView` rendered
///   above the still-visible data during a refresh.
#[component]
pub fn AsyncView<T, F, Ch>(
    /// The controller's reactive state signal.
    #[prop(into)]
    state: Signal<AsyncState<T, F>>,
    /// Renders the loaded value. Return `view! { … }.into_any()`.
    children: Ch,
    /// Optional override for the first-load placeholder.
    #[prop(optional, into)]
    loading: Option<ViewFn>,
    /// Optional override for the failure view; receives the owned failure.
    #[prop(optional, into)]
    error: Option<ErrorSlot<F>>,
    /// Optional indicator rendered above stale data during a refresh.
    #[prop(optional, into)]
    reloading_indicator: Option<ViewFn>,
) -> impl IntoView
where
    T: Clone + Send + Sync + 'static,
    F: Failure + Clone,
    Ch: Fn(T) -> AnyView + Send + Sync + 'static,
{
    move || match state.get() {
        AsyncState::Loading => match &loading {
            Some(slot) => slot.run(),
            None => default_loading().into_any(),
        },
        AsyncState::Data(value) => children(value),
        AsyncState::Reloading(value) => {
            let content = children(value);
            match &reloading_indicator {
                Some(slot) => (slot.run(), content).into_any(),
                None => content,
            }
        }
        AsyncState::Error { failure, .. } => match &error {
            Some(slot) => slot.run(failure),
            None => default_error(&failure).into_any(),
        },
    }
}

/// Default first-load placeholder (overridable via the `loading` prop).
fn default_loading() -> impl IntoView {
    view! { <div class="cs-async-loading">"Loading…"</div> }
}

/// Default failure view (overridable via the `error` prop): shows the
/// failure's user-facing message.
fn default_error<F: Failure>(failure: &F) -> impl IntoView {
    view! { <div class="cs-async-error">{failure.user_message()}</div> }
}

#[cfg(test)]
mod tests {
    use super::*;
    use clean_signals::failure::fixtures::NetworkFailure;

    /// Compile-test: exercises every `AsyncView` prop (required + all three
    /// optional slots) so the generic `#[component]` type-checks end to end.
    /// Rendering assertions require a DOM and are out of scope (task 07).
    #[test]
    fn async_view_type_checks_with_all_slots() {
        let owner = Owner::new();
        owner.with(|| {
            let state = RwSignal::new(AsyncState::<i32, NetworkFailure>::Loading);
            let _view = view! {
                <AsyncView
                    state=state
                    children=move |n: i32| view! { <span>{n}</span> }.into_any()
                    loading=move || view! { <p>"loading"</p> }
                    error=move |f: NetworkFailure| {
                        view! { <p>{f.user_message()}</p> }.into_any()
                    }
                    reloading_indicator=move || view! { <span>"↻"</span> }
                />
            };
        });
        owner.cleanup();
    }

    /// Compile-test the minimal form: only the required `state`/`children`
    /// props, relying on the default loading/error slots.
    #[test]
    fn async_view_type_checks_with_defaults() {
        let owner = Owner::new();
        owner.with(|| {
            let state = RwSignal::new(AsyncState::<String, NetworkFailure>::Loading);
            let _view = view! {
                <AsyncView
                    state=state
                    children=move |s: String| view! { <span>{s}</span> }.into_any()
                />
            };
        });
        owner.cleanup();
    }
}
