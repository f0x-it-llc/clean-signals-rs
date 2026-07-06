//! `ActivityTracker` — ref-counted "is something loading" tracking with RAII
//! guards.
//!
//! Overlapping operations are counted, so [`ActivityTracker::is_loading`]
//! stays `true` until the *last* in-flight operation completes — no flicker
//! when one operation ends while another is still running. Callers mark an
//! operation as in flight by holding an RAII guard
//! ([`ActivityTracker::begin`]) across its await points, rather than wrapping
//! the work in a closure.
//!
//! # Post-dispose safety
//!
//! [`ActivityTracker::dispose`] only flips an internal flag — it never
//! disposes the underlying signals (that is the owning arena/`Owner`'s job,
//! per `research/SPIKE_NOTES.md` Q1/Q2). A guard that outlives `dispose()`
//! (e.g. an operation still in flight when the tracker is torn down) must
//! decrement without touching a torn-down counter: its `Drop` impl checks the
//! disposed flag first, and additionally writes via `try_update` so a
//! separately-disposed signal (via an owning `Owner::cleanup()`) is also a
//! silent no-op rather than a panic — see SPIKE_NOTES Q1/Q8.

use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};

use reactive_graph::computed::Memo;
use reactive_graph::signal::RwSignal;
use reactive_graph::traits::{Get, GetUntracked, Update};

/// Ref-counted "is loading" tracker.
///
/// Cheaply `Clone`: all clones share the same counter/flag state (the
/// signal handle and the `Arc<AtomicBool>` are both shared, not copied).
#[derive(Clone)]
pub struct ActivityTracker {
    pending: RwSignal<u32>,
    is_loading: Memo<bool>,
    disposed: Arc<AtomicBool>,
}

impl ActivityTracker {
    /// Creates a new tracker with `pending == 0` and `is_loading == false`.
    pub fn new() -> Self {
        let pending = RwSignal::new(0u32);
        let is_loading = Memo::new(move |_| pending.get() > 0);
        Self {
            pending,
            is_loading,
            disposed: Arc::new(AtomicBool::new(false)),
        }
    }

    /// Begins tracking one operation: increments [`pending`](Self::pending)
    /// immediately and returns an RAII guard that decrements it on `Drop`
    /// (regardless of whether the tracked operation succeeds, fails, or
    /// panics — `Drop` always runs).
    ///
    /// Overlapping calls are ref-counted: `is_loading()` stays `true` until
    /// every outstanding guard has been dropped.
    pub fn begin(&self) -> ActivityGuard {
        // `try_update` rather than `update`: if the underlying signal has
        // already been disposed by an owning arena (independent of this
        // tracker's own `disposed` flag), incrementing is a silent no-op
        // instead of a panic/log-warning (SPIKE_NOTES Q1).
        self.pending.try_update(|n| *n += 1);
        ActivityGuard {
            pending: self.pending,
            disposed: Arc::clone(&self.disposed),
        }
    }

    /// Number of operations currently in flight.
    pub fn pending(&self) -> u32 {
        self.pending.get_untracked()
    }

    /// A reactive handle tracking whether at least one operation is in
    /// flight (`pending() > 0`). `Memo` is `Copy`, so this can be freely
    /// stored and read from controllers/UI.
    pub fn is_loading(&self) -> Memo<bool> {
        self.is_loading
    }

    /// Marks the tracker disposed (idempotent). Outstanding
    /// [`ActivityGuard`]s dropped after this call skip their decrement
    /// entirely — the pinned "post-dispose decrements suppressed" behavior.
    ///
    /// This does **not** dispose the underlying `pending`/`is_loading`
    /// signals; that is the owning arena/`Owner`'s responsibility (see
    /// module docs).
    pub fn dispose(&self) {
        self.disposed.store(true, Ordering::SeqCst);
    }
}

impl Default for ActivityTracker {
    fn default() -> Self {
        Self::new()
    }
}

/// RAII guard returned by [`ActivityTracker::begin`]. Decrements the
/// tracker's pending count on `Drop`, unless the tracker was disposed first.
///
/// Holds only clones of what it needs (a `Copy` signal handle and a shared
/// `Arc<AtomicBool>` flag), so it is safe to outlive the `ActivityTracker`
/// value that created it.
pub struct ActivityGuard {
    pending: RwSignal<u32>,
    disposed: Arc<AtomicBool>,
}

impl Drop for ActivityGuard {
    fn drop(&mut self) {
        if self.disposed.load(Ordering::SeqCst) {
            // Post-dispose: suppress the decrement entirely (pinned
            // behavior — see module docs and SPIKE_NOTES Q8).
            return;
        }
        // `try_update`: also inert (no panic) if the signal itself was
        // independently disposed by an owning arena.
        self.pending.try_update(|n| *n = n.saturating_sub(1));
    }
}

#[cfg(test)]
mod tests {
    use super::ActivityTracker;
    use reactive_graph::traits::GetUntracked;

    #[test]
    fn is_loading_reflects_overlapping_operations_ref_counted() {
        let tracker = ActivityTracker::new();
        assert!(!tracker.is_loading().get_untracked());

        let first = tracker.begin();
        let second = tracker.begin();
        assert!(tracker.is_loading().get_untracked());
        assert_eq!(tracker.pending(), 2);

        drop(first);
        assert!(
            tracker.is_loading().get_untracked(),
            "one operation still in flight"
        );
        assert_eq!(tracker.pending(), 1);

        drop(second);
        assert!(!tracker.is_loading().get_untracked());
        assert_eq!(tracker.pending(), 0);

        tracker.dispose();
    }

    #[test]
    fn pending_counts_active_operations() {
        let tracker = ActivityTracker::new();
        assert_eq!(tracker.pending(), 0);

        let guards: Vec<_> = (0..3).map(|_| tracker.begin()).collect();
        assert_eq!(tracker.pending(), 3);

        drop(guards);
        assert_eq!(tracker.pending(), 0);
    }

    #[test]
    fn guard_dropped_after_dispose_leaves_pending_unchanged() {
        let tracker = ActivityTracker::new();
        let guard = tracker.begin();
        assert_eq!(tracker.pending(), 1);

        // An operation in flight during dispose completes without writing
        // to the (now logically disposed) counter.
        tracker.dispose();
        drop(guard);

        assert_eq!(
            tracker.pending(),
            1,
            "post-dispose guard drop must NOT decrement pending"
        );
    }

    #[test]
    fn dispose_is_idempotent() {
        let tracker = ActivityTracker::new();
        tracker.dispose();
        tracker.dispose(); // must not panic
        let guard = tracker.begin();
        drop(guard);
        assert_eq!(
            tracker.pending(),
            1,
            "begin still increments post-dispose (only decrement is suppressed)"
        );
    }
}
