//! `ControllerCore<F>` — the framework's orchestration centerpiece.
//!
//! An app controller (view model) *embeds* a [`ControllerCore`] by composition
//! (there is no base class to extend in Rust) and drives its use cases through
//! [`ControllerCore::run`], [`ControllerCore::run_into`], and
//! [`ControllerCore::watch`], which provide:
//!
//! - ref-counted [`is_loading`](ControllerCore::is_loading) across overlapping
//!   operations (via [`crate::activity::ActivityTracker`]);
//! - a dependency-free [`FailureSink`] of failure events (after retries are
//!   exhausted) for global error handling (snackbars, logging, analytics);
//! - per-call [`RetryPolicy`] retries with only the *final* failure emitted;
//! - lifecycle cleanup: everything registered with
//!   [`on_dispose`](ControllerCore::on_dispose) or subscribed with
//!   [`watch`](ControllerCore::watch) is torn down (LIFO) by
//!   [`dispose`](ControllerCore::dispose), and
//!   [`is_disposed`](ControllerCore::is_disposed) lets the embedding controller
//!   guard its own post-`await` signal writes.
//!
//! # Deviations from the Dart port / PLAN contract
//!
//! - **`U::Params: Clone`** is required on [`run`](ControllerCore::run) and
//!   [`run_into`](ControllerCore::run_into): the retry loop re-invokes the use
//!   case with the same params, and Rust (unlike Dart) moves them into the
//!   first call. [`crate::use_case::NoParams`] is `Copy`, so the common case is
//!   free. (The locked PLAN signature omitted this bound; it is a mechanical
//!   consequence of retries.)
//! - **No `auto_effect`.** Per `research/SPIKE_NOTES.md` Q4, `Effect::new`
//!   forces the `effects` feature + a `LocalSet` on every consumer/test. Core
//!   library code therefore uses [`Memo`] + explicit methods only; any
//!   render-glue effect belongs in the `clean-signals-leptos` crate (task 07),
//!   keeping controllers fully testable without the `effects` feature.
//! - **`on_dispose` after `dispose` runs the callback immediately** (see that
//!   method's docs). Dart silently appends it to a drained list (dropping it);
//!   running it immediately is strictly safer — the resource still gets cleaned
//!   up rather than leaked.

use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::{Arc, Mutex, Weak};

use any_spawner::Executor;
use futures::future::{AbortHandle, Abortable};
use futures::stream::StreamExt;
use reactive_graph::computed::Memo;
use reactive_graph::owner::Owner;
use reactive_graph::signal::RwSignal;
use reactive_graph::traits::{GetUntracked, Set, Update};

use crate::activity::ActivityTracker;
use crate::async_state::{to_reloading, AsyncState};
use crate::failure::Failure;
use crate::retry::RetryPolicy;
use crate::use_case::{StreamUseCase, UseCase};

/// A single failure listener. Stored behind an [`Arc`] (not [`Box`]) so
/// [`FailureSink::emit`] can cheaply clone the live set out from under the lock
/// and invoke listeners with no lock held (see [`FailureSink`]'s re-entrancy
/// docs).
type Listener<F> = Arc<dyn Fn(&F) + Send + Sync>;

/// Shared, id-keyed list of a [`FailureSink`]'s listeners.
type Listeners<F> = Arc<Mutex<Vec<(u64, Listener<F>)>>>;

/// The id-keyed registry of live [`ControllerCore::watch`] subscriptions.
///
/// Each entry pairs a monotonic id with the watch's [`AbortHandle`]. Entries
/// are pruned when a watch's stream completes, when it is aborted, or when its
/// [`WatchHandle::cancel`] is called, so a long-lived controller re-watched many
/// times never accumulates dead handles.
type WatchList = Vec<(u64, AbortHandle)>;

/// Per-call knobs for [`ControllerCore::run`] / [`ControllerCore::run_into`].
///
/// `Default` is `{ retry: RetryPolicy::none(), track_activity: true,
/// emit_failure: true }` — a single-attempt call that counts towards
/// `is_loading` and emits its failure (if any) on the sink.
pub struct RunOptions<F> {
    /// Retry behavior for this call. Defaults to [`RetryPolicy::none`].
    pub retry: RetryPolicy<F>,
    /// Whether this call counts towards [`ControllerCore::is_loading`].
    /// Disable for background work that should not show a spinner.
    pub track_activity: bool,
    /// Whether the *final* failure (after retries) is emitted on the
    /// [`FailureSink`]. Disable when handling the failure locally.
    pub emit_failure: bool,
}

impl<F> Default for RunOptions<F> {
    fn default() -> Self {
        Self {
            retry: RetryPolicy::none(),
            track_activity: true,
            emit_failure: true,
        }
    }
}

/// A dependency-free registry of failure listeners with RAII unsubscription.
///
/// Listeners are invoked synchronously, in subscription order, each time
/// [`emit`](FailureSink::emit) is called. Cheap to [`Clone`] — all clones
/// share the same listener list and id counter.
///
/// # Re-entrancy and panics
///
/// [`emit`](FailureSink::emit) snapshots the current listeners under its lock
/// and then invokes them with **no lock held**. Consequences:
///
/// - A listener may safely [`subscribe`](FailureSink::subscribe) to or drop a
///   [`Subscription`] of the *same* sink from within its own body — this no
///   longer deadlocks.
/// - Unsubscribing mid-`emit` does *not* retroactively cancel the in-flight
///   pass: a listener removed during an `emit` still fires once for that
///   emission (the snapshot was already taken); only *subsequent* emits skip
///   it. Likewise a listener subscribed mid-`emit` first fires on the next
///   emission.
/// - A panicking listener propagates the panic out of `emit` but never poisons
///   the sink's internal lock, so later `subscribe`/`emit`/unsubscribe calls
///   keep working. Remaining lock sites also recover from poisoning defensively.
pub struct FailureSink<F> {
    listeners: Listeners<F>,
    next_id: Arc<AtomicU64>,
}

impl<F: 'static> FailureSink<F> {
    /// Creates an empty sink with no listeners.
    pub fn new() -> Self {
        Self {
            listeners: Arc::new(Mutex::new(Vec::new())),
            next_id: Arc::new(AtomicU64::new(0)),
        }
    }

    /// Registers `f` to be called on every subsequent [`emit`](Self::emit).
    ///
    /// Returns a [`Subscription`] whose `Drop` unsubscribes the listener. Keep
    /// the guard alive for as long as the listener should fire; call
    /// [`Subscription::forget`] to leak it (fire until the sink is dropped).
    pub fn subscribe(&self, f: impl Fn(&F) + Send + Sync + 'static) -> Subscription {
        let id = self.next_id.fetch_add(1, Ordering::SeqCst);
        self.listeners
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .push((id, Arc::new(f)));

        // Type-erase removal so `Subscription` need not be generic over `F`.
        let listeners = Arc::downgrade(&self.listeners);
        Subscription {
            remove: Some(Box::new(move || {
                if let Some(listeners) = listeners.upgrade() {
                    listeners
                        .lock()
                        .unwrap_or_else(|e| e.into_inner())
                        .retain(|(lid, _)| *lid != id);
                }
            })),
        }
    }

    /// Invokes every subscribed listener with `failure`, synchronously and in
    /// subscription order. A no-op when there are no listeners.
    ///
    /// Listeners are snapshotted under the lock and invoked with **no lock
    /// held**, so a panicking or re-entrant listener can neither poison nor
    /// deadlock the sink. See the [type docs](FailureSink#re-entrancy-and-panics)
    /// for the exact mid-`emit` subscribe/unsubscribe semantics.
    pub fn emit(&self, failure: &F) {
        let snapshot: Vec<Listener<F>> = self
            .listeners
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .iter()
            .map(|(_, listener)| Arc::clone(listener))
            .collect();
        for listener in &snapshot {
            listener(failure);
        }
    }
}

impl<F: 'static> Default for FailureSink<F> {
    fn default() -> Self {
        Self::new()
    }
}

impl<F> Clone for FailureSink<F> {
    fn clone(&self) -> Self {
        Self {
            listeners: Arc::clone(&self.listeners),
            next_id: Arc::clone(&self.next_id),
        }
    }
}

/// RAII handle for a [`FailureSink`] listener. Dropping it unsubscribes the
/// listener; [`forget`](Self::forget) detaches it so the listener fires for the
/// lifetime of the sink.
#[must_use = "dropping the Subscription immediately unsubscribes the listener; \
              hold it or call `.forget()`"]
pub struct Subscription {
    remove: Option<Box<dyn FnOnce() + Send + Sync>>,
}

impl Subscription {
    /// Detaches this guard from the listener: the listener keeps firing (until
    /// the sink itself is dropped) and dropping the returned unit has no
    /// effect. Fire-and-forget subscriptions.
    pub fn forget(mut self) {
        self.remove = None;
    }
}

impl Drop for Subscription {
    fn drop(&mut self) {
        if let Some(remove) = self.remove.take() {
            remove();
        }
    }
}

/// Handle to a running [`ControllerCore::watch`] subscription.
///
/// Dropping the handle does **not** cancel the watch (matching the Dart
/// `StreamSubscription` return, which callers routinely ignore); the watch runs
/// until the controller is [`dispose`](ControllerCore::dispose)d or
/// [`cancel`](Self::cancel) is called explicitly.
pub struct WatchHandle {
    abort: AbortHandle,
    /// Back-reference to the owning controller's watch registry, so `cancel`
    /// can prune this watch's entry synchronously. `Weak` avoids keeping the
    /// controller alive; a disposed-controller (inert) handle holds an empty
    /// `Weak` and prunes nothing.
    registry: Weak<Mutex<WatchList>>,
    /// This watch's registry id (see [`WatchList`]).
    id: u64,
}

impl WatchHandle {
    /// Cancels just this watch immediately (aborts its driving task at the next
    /// poll) and prunes its registry entry synchronously. Idempotent.
    pub fn cancel(&self) {
        self.abort.abort();
        if let Some(registry) = self.registry.upgrade() {
            registry
                .lock()
                .unwrap_or_else(|e| e.into_inner())
                .retain(|(id, _)| *id != self.id);
        }
    }
}

/// RAII guard that removes a watch's entry from the registry when its driver
/// future is dropped — which happens on normal stream completion *and* on abort
/// (`futures::future::Abortable` drops its wrapped future in place the moment it
/// observes the abort). Lives inside the driver's async frame so both paths
/// prune. Idempotent with [`WatchHandle::cancel`]'s proactive prune.
struct RemoveOnDrop {
    registry: Weak<Mutex<WatchList>>,
    id: u64,
}

impl Drop for RemoveOnDrop {
    fn drop(&mut self) {
        if let Some(registry) = self.registry.upgrade() {
            registry
                .lock()
                .unwrap_or_else(|e| e.into_inner())
                .retain(|(id, _)| *id != self.id);
        }
    }
}

/// The orchestration core an app controller embeds by composition.
///
/// Generic over the app's failure type `F`. See the module docs for the full
/// contract and its deviations from the Dart original.
pub struct ControllerCore<F: Failure + Clone> {
    /// Owns the activity signals so `dispose` can release them.
    owner: Owner,
    activity: ActivityTracker,
    failures: FailureSink<F>,
    cleanups: Mutex<Vec<Box<dyn FnOnce() + Send>>>,
    /// Id-keyed registry of live watches, shared (`Arc`) so driver tasks and
    /// [`WatchHandle`]s can prune their own entries via a [`Weak`] back-ref.
    watches: Arc<Mutex<WatchList>>,
    /// Monotonic id source for watch registry entries.
    next_watch_id: AtomicU64,
    disposed: Arc<AtomicBool>,
}

impl<F: Failure + Clone> ControllerCore<F> {
    /// Creates a fresh controller core: not loading, no listeners, not
    /// disposed.
    pub fn new() -> Self {
        let owner = Owner::new();
        // Create the activity signals under this controller's `Owner` so
        // `dispose` (which calls `owner.cleanup()`) releases them rather than
        // leaking arena slots (SPIKE_NOTES Q1/Q2).
        let activity = owner.with(ActivityTracker::new);
        Self {
            owner,
            activity,
            failures: FailureSink::new(),
            cleanups: Mutex::new(Vec::new()),
            watches: Arc::new(Mutex::new(Vec::new())),
            next_watch_id: AtomicU64::new(0),
            disposed: Arc::new(AtomicBool::new(false)),
        }
    }

    /// Executes `uc` with `params`, applying `opts` (retry / activity /
    /// failure-emission), returning the use case's `Result`.
    ///
    /// While running, [`is_loading`](Self::is_loading) is `true` (across retry
    /// waits too) unless `opts.track_activity` is `false`. On a retryable
    /// failure (per `opts.retry`), the call is re-attempted up to
    /// `opts.retry.max_attempts` *total* attempts with backoff. Only the final
    /// failure is emitted on the [`FailureSink`] (never intermediate ones), and
    /// only when `opts.emit_failure` is `true` and the controller is not
    /// disposed.
    pub async fn run<U>(&self, uc: &U, params: U::Params, opts: RunOptions<F>) -> Result<U::Output, F>
    where
        U: UseCase<Failure = F>,
        U::Params: Clone,
    {
        // Begin activity once around the whole loop (retries included), mirroring
        // Dart's `_activity.track(_runWithRetry(...))`. Held until `run` returns.
        let _guard = if opts.track_activity {
            Some(self.activity.begin())
        } else {
            None
        };

        let mut attempt: u32 = 1;
        loop {
            match uc.execute(params.clone()).await {
                Ok(value) => return Ok(value),
                Err(failure) => {
                    if attempt < opts.retry.max_attempts && opts.retry.should_retry(&failure) {
                        crate::time::sleep(opts.retry.delay_for(attempt)).await;
                        attempt += 1;
                        continue;
                    }
                    if opts.emit_failure && !self.is_disposed() {
                        self.failures.emit(&failure);
                    }
                    return Err(failure);
                }
            }
        }
    }

    /// Like [`run`](Self::run), but additionally drives `into` through the
    /// operation's lifecycle:
    ///
    /// - before running: [`AsyncState::Reloading`] if `into` already holds a
    ///   value (existing content stays visible during the refresh), otherwise
    ///   [`AsyncState::Loading`];
    /// - after running: [`AsyncState::Data`] on success, or
    ///   [`AsyncState::Error`] carrying the previous value as `stale` on
    ///   failure.
    ///
    /// Every write to `into` goes through `try_*` and is additionally gated on
    /// `!is_disposed()`, so a controller disposed while an `await` is in flight
    /// never mutates the (possibly still-live, caller-owned) signal.
    pub async fn run_into<U>(
        &self,
        uc: &U,
        params: U::Params,
        into: RwSignal<AsyncState<U::Output, F>>,
        opts: RunOptions<F>,
    ) -> Result<U::Output, F>
    where
        U: UseCase<Failure = F>,
        U::Params: Clone,
        U::Output: Clone + Send + Sync + 'static,
    {
        // Snapshot the value to carry as `stale` on failure, then flip to the
        // reloading/loading state — both gated on liveness.
        let prev_value = if self.is_disposed() {
            None
        } else {
            into.try_get_untracked()
                .and_then(|state| state.value().cloned())
        };
        if !self.is_disposed() {
            into.try_update(|state| *state = to_reloading(state));
        }

        let result = self.run(uc, params, opts).await;

        if !self.is_disposed() {
            let next = match &result {
                Ok(value) => AsyncState::Data(value.clone()),
                Err(failure) => AsyncState::Error {
                    failure: failure.clone(),
                    stale: prev_value,
                },
            };
            into.try_set(next);
        }
        result
    }

    /// Subscribes to the stream `uc` produces for `params`, forwarding each
    /// `Ok(value)` to `on_data` and each `Err(failure)` to the
    /// [`FailureSink`]. The driving task is spawned via
    /// [`any_spawner::Executor::spawn_local`] (per SPIKE_NOTES Q5) and is
    /// aborted immediately by [`dispose`](Self::dispose) — no `on_data` fires
    /// after disposal.
    ///
    /// The watch registers itself in an id-keyed registry and removes its own
    /// entry when its stream completes, when it is aborted, or when
    /// [`WatchHandle::cancel`] is called — so a long-lived controller that is
    /// re-watched many times never accumulates dead handles.
    ///
    /// Registration is performed under the same lock [`dispose`](Self::dispose)
    /// takes, so a `watch` racing a concurrent `dispose` is strictly ordered:
    /// either it registers in time to be aborted by that `dispose`, or it
    /// observes the controller already disposed and returns an **inert**
    /// [`WatchHandle`] without spawning a driver — the stream is never polled
    /// and no `on_data` ever fires. (The stream value is still *constructed* via
    /// `uc.execute`, but constructing it is side-effect-free; only polling
    /// drives it.)
    ///
    /// The returned [`WatchHandle`] can cancel this single watch early; dropping
    /// it does not.
    pub fn watch<S>(
        &self,
        uc: &S,
        params: S::Params,
        on_data: impl Fn(S::Output) + 'static,
    ) -> WatchHandle
    where
        S: StreamUseCase<Failure = F>,
        S::Output: 'static,
    {
        let mut stream = uc.execute(params);
        let failures = self.failures.clone();
        let disposed = Arc::clone(&self.disposed);

        let (abort_handle, abort_reg) = AbortHandle::new_pair();

        // Register (or refuse, if disposed) under the watches lock so this call
        // is strictly ordered against `dispose`'s flag-flip + drain. Only the
        // `is_disposed` read, id allocation and push happen under the lock — no
        // user code (`uc.execute`, `on_data`, the spawn) runs while it is held.
        let id = {
            let mut watches = self.watches.lock().unwrap_or_else(|e| e.into_inner());
            if self.is_disposed() {
                // Already disposed: never spawn a driver. Return an inert handle
                // (already aborted, holds no registry entry) whose `cancel` is a
                // no-op.
                abort_handle.abort();
                return WatchHandle {
                    abort: abort_handle,
                    registry: Weak::new(),
                    id: 0,
                };
            }
            let id = self.next_watch_id.fetch_add(1, Ordering::SeqCst);
            watches.push((id, abort_handle.clone()));
            id
        };

        let driver = {
            let registry = Arc::downgrade(&self.watches);
            async move {
                // Prune this watch's registry entry on normal completion or
                // abort (the guard is dropped in both cases).
                let _guard = RemoveOnDrop { registry, id };
                while let Some(item) = stream.next().await {
                    if disposed.load(Ordering::SeqCst) {
                        break;
                    }
                    match item {
                        Ok(value) => on_data(value),
                        Err(failure) => failures.emit(&failure),
                    }
                }
            }
        };
        let driver = Abortable::new(driver, abort_reg);
        Executor::spawn_local(async move {
            let _ = driver.await;
        });

        WatchHandle {
            abort: abort_handle,
            registry: Arc::downgrade(&self.watches),
            id,
        }
    }

    /// Registers `cleanup` to run when the controller is
    /// [`dispose`](Self::dispose)d. Cleanups run in **reverse** registration
    /// order (LIFO).
    ///
    /// If the controller is already disposed, `cleanup` runs **immediately** —
    /// registering a teardown against an already-torn-down controller still
    /// releases the resource rather than silently dropping it (see module docs).
    pub fn on_dispose(&self, cleanup: impl FnOnce() + Send + 'static) {
        if self.is_disposed() {
            cleanup();
        } else {
            self.cleanups
                .lock()
                .unwrap_or_else(|e| e.into_inner())
                .push(Box::new(cleanup));
        }
    }

    /// Whether [`dispose`](Self::dispose) has run. Use this to guard the
    /// embedding controller's own post-`await` signal writes.
    pub fn is_disposed(&self) -> bool {
        self.disposed.load(Ordering::SeqCst)
    }

    /// Reactive handle: `true` while at least one activity-tracked operation is
    /// in flight.
    pub fn is_loading(&self) -> Memo<bool> {
        self.activity.is_loading()
    }

    /// The controller's failure event sink. Subscribe near the feature's UI
    /// root to surface failures.
    pub fn failures(&self) -> &FailureSink<F> {
        &self.failures
    }

    /// Releases everything the controller owns. Idempotent (safe to call more
    /// than once).
    ///
    /// Aborts all [`watch`](Self::watch) subscriptions immediately, marks the
    /// activity tracker disposed, runs registered cleanups **LIFO**, and
    /// releases the owned activity signals.
    ///
    /// The `disposed` flag is flipped **while holding the same lock**
    /// [`watch`](Self::watch) takes to register, then all live watches are
    /// drained and aborted under that lock. This makes `watch`/`dispose` races
    /// (the type is `Send + Sync` for app-scoped `provide_controller` use)
    /// strictly ordered — a concurrent `watch` either registers in time to be
    /// aborted here or observes `disposed` and never spawns, so no watch can
    /// leak past disposal. User cleanups run *after* the lock is released, since
    /// they may call back into the controller.
    pub fn dispose(&self) {
        // Flip `disposed` and drain+abort watches under the watches lock, so a
        // concurrent `watch` cannot interleave its register between the two.
        {
            let mut watches = self.watches.lock().unwrap_or_else(|e| e.into_inner());
            if self.disposed.swap(true, Ordering::SeqCst) {
                return; // already disposed
            }
            for (_, handle) in watches.drain(..) {
                handle.abort();
            }
        }

        // Suppress any in-flight activity guard decrements.
        self.activity.dispose();

        // Run cleanups in reverse registration order.
        let mut cleanups =
            std::mem::take(&mut *self.cleanups.lock().unwrap_or_else(|e| e.into_inner()));
        while let Some(cleanup) = cleanups.pop() {
            cleanup();
        }

        // Release the activity signals owned by this controller.
        self.owner.cleanup();
    }
}

impl<F: Failure + Clone> Default for ControllerCore<F> {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::async_state::async_state_signal;
    use crate::failure::fixtures::{NetworkFailure, ValidationFailure};
    use crate::use_case::fixtures::{Doubler, Failing, Flaky, Slow, Ticker};
    use crate::use_case::{NoParams, UseCaseStream};
    use futures::stream::{self, StreamExt};
    use reactive_graph::traits::GetUntracked;
    use std::cell::RefCell;
    use std::rc::Rc;
    use std::sync::atomic::{AtomicU32, Ordering};
    use std::time::Duration;

    // ---- helpers ------------------------------------------------------------

    fn collector() -> (Arc<Mutex<Vec<NetworkFailure>>>, impl Fn(&NetworkFailure) + Send + Sync) {
        let store = Arc::new(Mutex::new(Vec::new()));
        let sink = Arc::clone(&store);
        (store, move |f: &NetworkFailure| sink.lock().unwrap().push(f.clone()))
    }

    fn ensure_executor() {
        let _ = any_spawner::Executor::init_tokio();
    }

    /// Pump the executor a few times so spawned-local watch tasks make progress.
    async fn pump() {
        for _ in 0..8 {
            any_spawner::Executor::tick().await;
        }
    }

    /// A use case that always fails with the given failure, counting attempts —
    /// used to prove retry/no-retry attempt counts for arbitrary failures.
    struct CountingFail<F> {
        failure: F,
        attempts: AtomicU32,
    }
    impl<F> CountingFail<F> {
        fn new(failure: F) -> Self {
            Self {
                failure,
                attempts: AtomicU32::new(0),
            }
        }
        fn attempts(&self) -> u32 {
            self.attempts.load(Ordering::SeqCst)
        }
    }
    #[async_trait::async_trait]
    impl<F: Failure + Clone> UseCase for CountingFail<F> {
        type Params = NoParams;
        type Output = i32;
        type Failure = F;
        async fn execute(&self, _p: NoParams) -> Result<i32, F> {
            self.attempts.fetch_add(1, Ordering::SeqCst);
            Err(self.failure.clone())
        }
    }

    /// A stream that yields `Ok(1)` immediately then sleeps between subsequent
    /// items — long enough that `dispose` can interrupt it mid-stream.
    struct SlowTicker;
    impl StreamUseCase for SlowTicker {
        type Params = ();
        type Output = u32;
        type Failure = NetworkFailure;
        fn execute(&self, _p: ()) -> UseCaseStream<u32, NetworkFailure> {
            stream::unfold(1u32, |n| async move {
                if n > 1 {
                    crate::time::sleep(Duration::from_millis(50)).await;
                }
                Some((Ok(n), n + 1))
            })
            .boxed()
        }
    }

    fn opts_retry(max: u32) -> RunOptions<NetworkFailure> {
        RunOptions {
            retry: RetryPolicy::new(max, Duration::from_millis(1)),
            ..Default::default()
        }
    }

    // ---- run ----------------------------------------------------------------

    #[tokio::test]
    async fn run_success_emits_no_failure() {
        let controller = ControllerCore::<NetworkFailure>::new();
        let (failures, listener) = collector();
        controller.failures().subscribe(listener).forget();

        let result = controller.run(&Flaky::new(0), NoParams, RunOptions::default()).await;
        assert_eq!(result, Ok(5));
        assert!(failures.lock().unwrap().is_empty());
        controller.dispose();
    }

    #[tokio::test]
    async fn run_retries_retryable_failures_up_to_max_and_succeeds() {
        let controller = ControllerCore::<NetworkFailure>::new();
        let uc = Flaky::new(2);

        let result = controller.run(&uc, NoParams, opts_retry(3)).await;
        assert_eq!(result, Ok(5));
        assert_eq!(uc.attempts(), 3);
        controller.dispose();
    }

    #[tokio::test]
    async fn run_exhausts_attempts_and_emits_exactly_one_failure() {
        let controller = ControllerCore::<NetworkFailure>::new();
        let uc = Flaky::new(5);
        let (failures, listener) = collector();
        controller.failures().subscribe(listener).forget();

        let result = controller.run(&uc, NoParams, opts_retry(2)).await;
        assert!(result.is_err());
        assert_eq!(uc.attempts(), 2, "max_attempts counts total attempts");
        assert_eq!(
            failures.lock().unwrap().len(),
            1,
            "only the final failure is emitted"
        );
        controller.dispose();
    }

    #[tokio::test]
    async fn run_does_not_retry_non_retryable_failure() {
        let controller = ControllerCore::<ValidationFailure>::new();
        let uc = CountingFail::new(ValidationFailure::new("bad input"));

        let opts = RunOptions {
            retry: RetryPolicy::new(3, Duration::from_millis(1)),
            ..Default::default()
        };
        let result = controller.run(&uc, NoParams, opts).await;
        assert!(result.is_err());
        assert_eq!(uc.attempts(), 1, "non-retryable failure must not retry");
        controller.dispose();
    }

    #[tokio::test]
    async fn run_emit_failure_false_keeps_failure_local() {
        let controller = ControllerCore::<NetworkFailure>::new();
        let (failures, listener) = collector();
        controller.failures().subscribe(listener).forget();

        let opts = RunOptions {
            emit_failure: false,
            ..Default::default()
        };
        let result = controller
            .run(&Failing::new(NetworkFailure::new("boom")), NoParams, opts)
            .await;
        assert!(result.is_err());
        assert!(failures.lock().unwrap().is_empty());
        controller.dispose();
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn run_tracks_is_loading_across_the_operation() {
        let controller = Arc::new(ControllerCore::<NetworkFailure>::new());
        let (slow, gate) = Slow::new();
        let slow = Arc::new(slow);

        let c = Arc::clone(&controller);
        let s = Arc::clone(&slow);
        let handle = tokio::spawn(async move { c.run(&*s, NoParams, RunOptions::default()).await });

        // Give the spawned task time to begin (and park on the gate).
        tokio::time::sleep(Duration::from_millis(30)).await;
        assert!(controller.is_loading().get_untracked(), "loading while gated");

        gate.send(()).unwrap();
        assert_eq!(handle.await.unwrap(), Ok(5));
        assert!(!controller.is_loading().get_untracked(), "not loading after");
        controller.dispose();
    }

    // ---- run_into -----------------------------------------------------------

    #[tokio::test(flavor = "multi_thread")]
    async fn run_into_drives_loading_to_data() {
        let controller = Arc::new(ControllerCore::<NetworkFailure>::new());
        let state = async_state_signal::<i32, NetworkFailure>();
        let (slow, gate) = Slow::new();
        let slow = Arc::new(slow);

        let c = Arc::clone(&controller);
        let s = Arc::clone(&slow);
        let handle =
            tokio::spawn(async move { c.run_into(&*s, NoParams, state, RunOptions::default()).await });

        tokio::time::sleep(Duration::from_millis(30)).await;
        assert_eq!(state.get_untracked(), AsyncState::Loading);

        gate.send(()).unwrap();
        handle.await.unwrap().unwrap();
        assert_eq!(state.get_untracked(), AsyncState::Data(5));
        controller.dispose();
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn run_into_keeps_stale_data_visible_while_reloading() {
        let controller = Arc::new(ControllerCore::<NetworkFailure>::new());
        let state = async_state_signal::<i32, NetworkFailure>();

        // First load: Doubler(3) -> Data(6).
        controller
            .run_into(&Doubler, 3, state, RunOptions::default())
            .await
            .unwrap();
        assert_eq!(state.get_untracked(), AsyncState::Data(6));

        // Second load via a gated Slow use case: stale 6 stays visible.
        let (slow, gate) = Slow::new();
        let slow = Arc::new(slow);
        let c = Arc::clone(&controller);
        let s = Arc::clone(&slow);
        let handle =
            tokio::spawn(async move { c.run_into(&*s, NoParams, state, RunOptions::default()).await });

        tokio::time::sleep(Duration::from_millis(30)).await;
        let mid = state.get_untracked();
        assert!(mid.is_loading(), "reloading");
        assert!(mid.has_value(), "stale data stays visible");
        assert_eq!(mid.value(), Some(&6));

        gate.send(()).unwrap();
        handle.await.unwrap().unwrap();
        assert_eq!(state.get_untracked(), AsyncState::Data(5));
        controller.dispose();
    }

    #[tokio::test]
    async fn run_into_failure_sets_error_carrying_stale() {
        let controller = ControllerCore::<NetworkFailure>::new();
        let state = async_state_signal::<i32, NetworkFailure>();

        // Seed a value first.
        controller
            .run_into(&Doubler, 3, state, RunOptions::default())
            .await
            .unwrap();
        assert_eq!(state.get_untracked(), AsyncState::Data(6));

        // Now a failure (single attempt): Error preserves the stale 6.
        let result = controller
            .run_into(&Flaky::new(99), NoParams, state, RunOptions::default())
            .await;
        assert!(result.is_err());
        match state.get_untracked() {
            AsyncState::Error { stale, .. } => assert_eq!(stale, Some(6)),
            other => panic!("expected Error with stale, got {other:?}"),
        }
        controller.dispose();
    }

    #[tokio::test]
    async fn run_into_failure_from_fresh_has_no_stale() {
        let controller = ControllerCore::<NetworkFailure>::new();
        let state = async_state_signal::<i32, NetworkFailure>();

        let result = controller
            .run_into(&Flaky::new(99), NoParams, state, RunOptions::default())
            .await;
        assert!(result.is_err());
        match state.get_untracked() {
            AsyncState::Error { stale, .. } => assert_eq!(stale, None),
            other => panic!("expected Error without stale, got {other:?}"),
        }
        controller.dispose();
    }

    // ---- watch --------------------------------------------------------------

    #[tokio::test(flavor = "current_thread")]
    async fn watch_routes_data_and_final_failure() {
        tokio::task::LocalSet::new()
            .run_until(async {
                ensure_executor();
                let controller = ControllerCore::<NetworkFailure>::new();
                let (failures, listener) = collector();
                controller.failures().subscribe(listener).forget();

                let values = Rc::new(RefCell::new(Vec::<u32>::new()));
                let sink = Rc::clone(&values);
                let _watch = controller.watch(&Ticker, 3, move |v| sink.borrow_mut().push(v));

                pump().await;

                assert_eq!(*values.borrow(), vec![1, 2, 3]);
                assert_eq!(failures.lock().unwrap().len(), 1, "trailing 'tick lost'");
                controller.dispose();
            })
            .await;
    }

    #[tokio::test(flavor = "current_thread")]
    async fn watch_dispose_stops_further_on_data() {
        tokio::task::LocalSet::new()
            .run_until(async {
                ensure_executor();
                let controller = ControllerCore::<NetworkFailure>::new();

                let values = Rc::new(RefCell::new(Vec::<u32>::new()));
                let sink = Rc::clone(&values);
                let _watch = controller.watch(&SlowTicker, (), move |v| sink.borrow_mut().push(v));

                // Deliver the first (immediate) item; the task then parks on sleep.
                pump().await;
                assert_eq!(*values.borrow(), vec![1]);

                controller.dispose();

                // Had the task not been aborted, item 2 would arrive after 50ms.
                tokio::time::sleep(Duration::from_millis(90)).await;
                pump().await;
                assert_eq!(*values.borrow(), vec![1], "no on_data after dispose");
            })
            .await;
    }

    #[tokio::test(flavor = "current_thread")]
    async fn watch_registry_empties_after_stream_completes() {
        tokio::task::LocalSet::new()
            .run_until(async {
                ensure_executor();
                let controller = ControllerCore::<NetworkFailure>::new();

                let _watch = controller.watch(&Ticker, 3, |_v| {});
                assert_eq!(
                    controller.watches.lock().unwrap().len(),
                    1,
                    "registered synchronously"
                );

                // Drive the finite stream to exhaustion; the driver's
                // RemoveOnDrop guard then prunes the entry.
                pump().await;
                assert_eq!(
                    controller.watches.lock().unwrap().len(),
                    0,
                    "pruned once the stream completes"
                );
                controller.dispose();
            })
            .await;
    }

    #[tokio::test(flavor = "current_thread")]
    async fn watch_registry_empties_after_cancel() {
        tokio::task::LocalSet::new()
            .run_until(async {
                ensure_executor();
                let controller = ControllerCore::<NetworkFailure>::new();

                let handle = controller.watch(&SlowTicker, (), |_v| {});
                assert_eq!(controller.watches.lock().unwrap().len(), 1);

                // `cancel` prunes synchronously — without pumping the executor,
                // proving it doesn't rely on the aborted task being re-polled.
                handle.cancel();
                assert_eq!(
                    controller.watches.lock().unwrap().len(),
                    0,
                    "cancel prunes synchronously"
                );

                // Re-navigation: repeated watch()+cancel() never grows the vec
                // past the number of currently-live watches.
                for _ in 0..5 {
                    let h = controller.watch(&SlowTicker, (), |_v| {});
                    assert_eq!(controller.watches.lock().unwrap().len(), 1);
                    h.cancel();
                    assert_eq!(controller.watches.lock().unwrap().len(), 0);
                }
                controller.dispose();
            })
            .await;
    }

    #[tokio::test(flavor = "current_thread")]
    async fn watch_dispose_aborts_all_live_watches() {
        tokio::task::LocalSet::new()
            .run_until(async {
                ensure_executor();
                let controller = ControllerCore::<NetworkFailure>::new();

                let _a = controller.watch(&SlowTicker, (), |_v| {});
                let _b = controller.watch(&SlowTicker, (), |_v| {});
                assert_eq!(controller.watches.lock().unwrap().len(), 2);

                controller.dispose();
                assert_eq!(
                    controller.watches.lock().unwrap().len(),
                    0,
                    "dispose drains and aborts all live watches"
                );
            })
            .await;
    }

    #[tokio::test(flavor = "current_thread")]
    async fn watch_after_dispose_is_inert() {
        tokio::task::LocalSet::new()
            .run_until(async {
                ensure_executor();
                let controller = ControllerCore::<NetworkFailure>::new();
                controller.dispose();

                let values = Rc::new(RefCell::new(Vec::<u32>::new()));
                let sink = Rc::clone(&values);
                let handle = controller.watch(&Ticker, 3, move |v| sink.borrow_mut().push(v));

                // Registers nothing: the disposed-controller path never spawns.
                assert_eq!(
                    controller.watches.lock().unwrap().len(),
                    0,
                    "watch on a disposed controller registers nothing"
                );

                // Delivers nothing, even after pumping the executor.
                pump().await;
                assert!(
                    values.borrow().is_empty(),
                    "no on_data after watch on a disposed controller"
                );

                // The inert handle's cancel is a harmless no-op.
                handle.cancel();
                assert_eq!(controller.watches.lock().unwrap().len(), 0);
            })
            .await;
    }

    // ---- dispose ------------------------------------------------------------

    #[test]
    fn dispose_runs_cleanups_lifo_and_is_idempotent() {
        let controller = ControllerCore::<NetworkFailure>::new();
        let order = Arc::new(Mutex::new(Vec::<i32>::new()));

        for n in [1, 2, 3] {
            let order = Arc::clone(&order);
            controller.on_dispose(move || order.lock().unwrap().push(n));
        }

        controller.dispose();
        controller.dispose(); // idempotent: cleanups run once

        assert_eq!(*order.lock().unwrap(), vec![3, 2, 1]);
    }

    #[test]
    fn is_disposed_flips_on_dispose() {
        let controller = ControllerCore::<NetworkFailure>::new();
        assert!(!controller.is_disposed());
        controller.dispose();
        assert!(controller.is_disposed());
    }

    #[test]
    fn on_dispose_after_dispose_runs_immediately() {
        let controller = ControllerCore::<NetworkFailure>::new();
        controller.dispose();

        let ran = Arc::new(AtomicBool::new(false));
        let flag = Arc::clone(&ran);
        controller.on_dispose(move || flag.store(true, Ordering::SeqCst));
        assert!(ran.load(Ordering::SeqCst));
    }

    #[tokio::test]
    async fn run_into_after_dispose_leaves_signal_untouched() {
        let controller = ControllerCore::<NetworkFailure>::new();
        let state = async_state_signal::<i32, NetworkFailure>();
        controller.dispose();

        let result = controller
            .run_into(&Doubler, 3, state, RunOptions::default())
            .await;
        assert_eq!(result, Ok(6), "the use case still runs");
        assert_eq!(
            state.get_untracked(),
            AsyncState::Loading,
            "signal untouched after dispose"
        );
    }

    // ---- FailureSink --------------------------------------------------------

    #[test]
    fn failure_sink_fans_out_to_all_subscribers() {
        let sink = FailureSink::<NetworkFailure>::new();
        let (a, la) = collector();
        let (b, lb) = collector();
        let _sa = sink.subscribe(la);
        let _sb = sink.subscribe(lb);

        sink.emit(&NetworkFailure::new("x"));
        assert_eq!(a.lock().unwrap().len(), 1);
        assert_eq!(b.lock().unwrap().len(), 1);
    }

    #[test]
    fn failure_sink_dropped_subscription_stops_firing() {
        let sink = FailureSink::<NetworkFailure>::new();
        let (store, listener) = collector();
        let sub = sink.subscribe(listener);

        sink.emit(&NetworkFailure::new("first"));
        assert_eq!(store.lock().unwrap().len(), 1);

        drop(sub);
        sink.emit(&NetworkFailure::new("second"));
        assert_eq!(store.lock().unwrap().len(), 1, "no fire after unsubscribe");
    }

    #[test]
    fn failure_sink_panicking_listener_does_not_poison() {
        let sink = FailureSink::<NetworkFailure>::new();
        let panicking = sink.subscribe(|_| panic!("listener boom"));

        // The listener panic propagates out of `emit`, but because listeners are
        // invoked with no lock held it must NOT poison the internal mutex.
        let caught = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            sink.emit(&NetworkFailure::new("x"));
        }));
        assert!(caught.is_err(), "the listener panic propagates out of emit");

        // Unsubscribing the panicking listener still works (would panic on a
        // PoisonError if the mutex had been poisoned).
        drop(panicking);

        // And a fresh subscribe + emit still fires — the sink is fully usable.
        let (store, listener) = collector();
        sink.subscribe(listener).forget();
        sink.emit(&NetworkFailure::new("y"));
        assert_eq!(
            store.lock().unwrap().len(),
            1,
            "sink still delivers after a listener panicked"
        );
    }

    #[test]
    fn failure_sink_forgotten_subscription_keeps_firing() {
        let sink = FailureSink::<NetworkFailure>::new();
        let (store, listener) = collector();
        sink.subscribe(listener).forget();

        sink.emit(&NetworkFailure::new("a"));
        sink.emit(&NetworkFailure::new("b"));
        assert_eq!(store.lock().unwrap().len(), 2);
    }
}
