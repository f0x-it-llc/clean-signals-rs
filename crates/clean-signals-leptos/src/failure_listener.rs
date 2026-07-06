//! Component-scoped subscription to a controller's [`FailureSink`].
//!
//! [`use_failure_listener`] wires a handler to a controller's failure events
//! for as long as the current component is mounted, then unsubscribes on
//! cleanup — the idiomatic replacement for swallowing errors or manually
//! tracking a [`Subscription`](clean_signals::Subscription) guard per page.

use clean_signals::{Failure, FailureSink};
use leptos::prelude::*;

/// Subscribes `handler` to `sink` for the lifetime of the current component.
///
/// Each emitted failure is cloned and passed to `handler` (by value). The
/// underlying [`Subscription`](clean_signals::Subscription) is moved into an
/// [`on_cleanup`] callback, so it is dropped — and the listener removed — when
/// the component unmounts.
///
/// # Handler contract
///
/// - The handler runs **outside** any reactive tracking scope: reading a
///   signal inside it does not subscribe the surrounding effect. Use it to
///   push a snackbar, log, or `set` a signal imperatively.
/// - It must be `Send + Sync` because the emitting controller may run its use
///   cases on a worker thread (native). Leptos `SyncStorage` signals — the
///   default — are `Send + Sync`, so the common "show the error in the UI"
///   handler satisfies this. (This tightens the PLAN's `Fn(F)` sketch to
///   `Fn(F) + Send + Sync`, dictated by [`FailureSink::subscribe`]'s bound.)
///
/// # Example
///
/// ```rust,ignore
/// use_failure_listener(controller.core.failures(), move |f: AppFailure| {
///     toasts.error(f.user_message());
/// });
/// ```
pub fn use_failure_listener<F>(sink: &FailureSink<F>, handler: impl Fn(F) + Send + Sync + 'static)
where
    F: Failure + Clone,
{
    let subscription = sink.subscribe(move |failure: &F| handler(failure.clone()));
    // Moving the guard into the cleanup closure keeps the subscription alive
    // until unmount, then drops it (unsubscribing).
    on_cleanup(move || drop(subscription));
}

#[cfg(test)]
mod tests {
    use super::*;
    use clean_signals::failure::fixtures::NetworkFailure;
    use std::sync::Arc;
    use std::sync::atomic::{AtomicU32, Ordering};

    #[test]
    fn listener_fires_while_mounted_and_stops_after_cleanup() {
        let owner = Owner::new();
        let sink = FailureSink::<NetworkFailure>::new();
        let hits = Arc::new(AtomicU32::new(0));

        let counter = Arc::clone(&hits);
        owner.with(|| {
            use_failure_listener(&sink, move |_f: NetworkFailure| {
                counter.fetch_add(1, Ordering::SeqCst);
            });
        });

        sink.emit(&NetworkFailure::new("boom"));
        assert_eq!(
            hits.load(Ordering::SeqCst),
            1,
            "listener fires while mounted"
        );

        owner.cleanup();

        sink.emit(&NetworkFailure::new("after"));
        assert_eq!(
            hits.load(Ordering::SeqCst),
            1,
            "listener is unsubscribed on owner cleanup"
        );
    }
}
