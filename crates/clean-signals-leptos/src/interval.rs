//! Component-scoped polling: run a callback every `period` until unmount.
//!
//! [`use_interval`] is the correct, done-once version of the loop cupline's
//! `orders.rs` hand-rolls per page: a `spawn_local` loop that sleeps via the
//! framework's portable [`clean_signals::time::sleep`] and checks an
//! alive-flag flipped in [`on_cleanup`](leptos::prelude::on_cleanup) so it
//! stops the moment the component unmounts (no leaked timers, no callbacks
//! firing into a dead scope).
//!
//! On non-wasm (SSR / native) targets it is a no-op: there is no client-side
//! reactive scope to poll.

use std::time::Duration;

/// Runs `f` every `period` for as long as the current component is mounted.
///
/// On `wasm32`, spawns a local task that sleeps `period`, checks an alive-flag
/// (flipped `false` by an [`on_cleanup`](leptos::prelude::on_cleanup) handler
/// on unmount), then invokes `f` — looping until unmount. The first call fires
/// after the first `period`, not immediately (trigger an initial load
/// separately if you need one).
///
/// On non-wasm targets this is a no-op.
///
/// # Example
///
/// ```rust,ignore
/// // Refetch the order queue every 8 seconds while the page is open.
/// use_interval(Duration::from_secs(8), move || controller.with_value(|c| c.reload()));
/// ```
#[cfg(target_arch = "wasm32")]
pub fn use_interval(period: Duration, f: impl Fn() + 'static) {
    use std::sync::atomic::{AtomicBool, Ordering};
    use std::sync::Arc;

    // `Arc<AtomicBool>` (not `Rc<Cell>`): `on_cleanup` requires a `Send + Sync`
    // closure, so the shared alive-flag must be `Send + Sync` too.
    let alive = Arc::new(AtomicBool::new(true));
    let flag = Arc::clone(&alive);
    leptos::prelude::on_cleanup(move || flag.store(false, Ordering::SeqCst));

    leptos::task::spawn_local(run_interval(
        period,
        move || alive.load(Ordering::SeqCst),
        f,
    ));
}

/// No-op on SSR / native targets (no client-side scope to poll).
#[cfg(not(target_arch = "wasm32"))]
pub fn use_interval(_period: Duration, _f: impl Fn() + 'static) {}

/// The polling loop, factored out of [`use_interval`] so its stop-on-cleanup
/// behavior is testable natively without a DOM: it sleeps `period`, breaks
/// when `alive()` returns `false`, otherwise calls `f`, and repeats.
///
/// Compiled for `wasm32` (where [`use_interval`] drives it) and for tests.
#[cfg(any(target_arch = "wasm32", test))]
async fn run_interval<A, F>(period: Duration, alive: A, f: F)
where
    A: Fn() -> bool,
    F: Fn(),
{
    loop {
        clean_signals::time::sleep(period).await;
        if !alive() {
            break;
        }
        f();
    }
}

#[cfg(all(test, not(target_arch = "wasm32")))]
mod tests {
    use super::*;
    use std::cell::Cell;
    use std::rc::Rc;

    #[tokio::test]
    async fn run_interval_stops_when_alive_flag_clears() {
        let alive = Rc::new(Cell::new(true));
        let count = Rc::new(Cell::new(0u32));

        // The callback flips the alive-flag off after its third invocation;
        // the loop must observe that and stop rather than fire a fourth time.
        let alive_for_f = Rc::clone(&alive);
        let count_for_f = Rc::clone(&count);
        run_interval(
            Duration::from_millis(1),
            {
                let alive = Rc::clone(&alive);
                move || alive.get()
            },
            move || {
                count_for_f.set(count_for_f.get() + 1);
                if count_for_f.get() >= 3 {
                    alive_for_f.set(false);
                }
            },
        )
        .await;

        assert_eq!(count.get(), 3, "loop stops once the alive-flag is cleared");
    }
}
