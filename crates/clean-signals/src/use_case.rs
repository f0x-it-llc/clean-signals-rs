//! `UseCase` and `StreamUseCase` — the single-call and streaming execution
//! contracts every app-specific use case implements.
//!
//! # Failures as values, not exceptions
//!
//! `execute` returns `Result<Output, Failure>` directly — there is no
//! exception-catching wrapper, because Rust has no exceptions. A use case that
//! can't produce a value returns `Err` like any other fallible Rust function,
//! and apps map transport errors to their own `Failure` enum at the repository
//! boundary (see `PLAN.md` design decision #2); there is deliberately no
//! catch-all "unexpected failure" type.
//!
//! Streaming use cases follow the same rule: [`StreamUseCase::execute`] returns
//! a stream whose items are individually `Result<Output, Failure>`, so an
//! implementation that can fail mid-stream simply yields an `Err` item itself
//! (see the `Ticker` fixture below). There is no guard/wrapper to write, and
//! stream cancellation is Rust's ordinary `Drop` semantics — dropping (or no
//! longer polling) a [`UseCaseStream`] halts it immediately, with no extra
//! bookkeeping required.

use crate::failure::Failure;

/// A single-call use case: takes `Params`, asynchronously produces either
/// `Output` or a domain `Failure`.
///
/// Async trait methods need different `Send` bounds on native vs. `wasm32`
/// targets (the wasm event loop is single-threaded, so futures there need not
/// be `Send`). The dual `cfg_attr` below is the pattern every async trait in
/// this crate uses — apply the SAME pair of attributes to any `impl` block
/// for this trait.
#[cfg_attr(not(target_arch = "wasm32"), async_trait::async_trait)]
#[cfg_attr(target_arch = "wasm32", async_trait::async_trait(?Send))]
pub trait UseCase {
    /// Input the use case is invoked with.
    type Params;
    /// Value produced on success.
    type Output;
    /// Domain failure type produced on error.
    type Failure: Failure + Clone;

    /// Run the use case once, returning its result.
    async fn execute(&self, params: Self::Params) -> Result<Self::Output, Self::Failure>;
}

/// Unit parameter type for use cases that take no input.
#[derive(Default, Clone, Copy, Debug)]
pub struct NoParams;

/// The stream type [`StreamUseCase::execute`] returns: a boxed stream of
/// `Result<T, F>` items.
///
/// `wasm32` gets [`futures::stream::LocalBoxStream`] (no `Send` bound,
/// matching the single-threaded browser event loop); every other target gets
/// [`futures::stream::BoxStream`] (`Send`, so it can be polled from a spawned
/// task on a multi-threaded runtime).
#[cfg(not(target_arch = "wasm32"))]
pub type UseCaseStream<T, F> = futures::stream::BoxStream<'static, Result<T, F>>;

/// See the native definition of [`UseCaseStream`] above; this is the `wasm32`
/// variant.
#[cfg(target_arch = "wasm32")]
pub type UseCaseStream<T, F> = futures::stream::LocalBoxStream<'static, Result<T, F>>;

/// A streaming use case: takes `Params`, synchronously returns a stream whose
/// items are individually `Result<Output, Failure>`.
///
/// Unlike [`UseCase`], this trait is NOT `async_trait` — `execute` itself is
/// a plain synchronous method that hands back an owned, boxed stream (the
/// asynchronous work happens as that stream is polled). Keeping `execute`
/// taking `&self` and returning an owned stream lets a controller's `watch`
/// move the stream into a spawned task while the use case itself outlives
/// the call.
///
/// # Contract: `execute` must be lazy
///
/// Implementations **must** return a stream that is *side-effect-free until
/// polled*: constructing the stream (the body of `execute`) must not itself
/// start any I/O, subscribe to anything, or otherwise cause observable effects
/// — all work must happen only as the returned stream is driven. Controllers
/// rely on this: `watch` may construct the stream and then immediately drop it
/// without ever polling it (e.g. when the controller is already disposed), and
/// that must be a no-op. An eager `execute` that fires effects on construction
/// would leak work in exactly that case.
pub trait StreamUseCase {
    /// Input the use case is invoked with.
    type Params;
    /// Value produced by each successful stream item.
    type Output;
    /// Domain failure type produced by a failed stream item.
    type Failure: Failure + Clone;

    /// Start the stream. Dropping (or simply no longer polling) the returned
    /// stream cancels it immediately — see the module-level "Rust delta"
    /// docs.
    fn execute(&self, params: Self::Params) -> UseCaseStream<Self::Output, Self::Failure>;
}

/// Shared test fakes (fake use cases with deterministic, controllable
/// behavior) reused by this crate's own tests and — via the `test-fixtures`
/// feature — by the controller and leptos test suites.
#[cfg(any(test, feature = "test-fixtures"))]
pub mod fixtures {
    use super::{NoParams, StreamUseCase, UseCase, UseCaseStream};
    use crate::failure::Failure;
    use crate::failure::fixtures::NetworkFailure;
    use futures::stream::{self, StreamExt};
    use std::sync::Mutex;
    use std::sync::atomic::{AtomicU32, Ordering};

    /// Doubles its `i32` input. Never fails.
    #[derive(Default, Clone, Copy, Debug)]
    pub struct Doubler;

    #[cfg_attr(not(target_arch = "wasm32"), async_trait::async_trait)]
    #[cfg_attr(target_arch = "wasm32", async_trait::async_trait(?Send))]
    impl UseCase for Doubler {
        type Params = i32;
        type Output = i32;
        type Failure = NetworkFailure;

        async fn execute(&self, params: i32) -> Result<i32, NetworkFailure> {
            Ok(params * 2)
        }
    }

    /// Always fails with a clone of the configured failure.
    #[derive(Clone, Debug)]
    pub struct Failing<F> {
        failure: F,
    }

    impl<F> Failing<F> {
        pub fn new(failure: F) -> Self {
            Self { failure }
        }
    }

    #[cfg_attr(not(target_arch = "wasm32"), async_trait::async_trait)]
    #[cfg_attr(target_arch = "wasm32", async_trait::async_trait(?Send))]
    impl<F: Failure + Clone> UseCase for Failing<F> {
        type Params = NoParams;
        type Output = ();
        type Failure = F;

        async fn execute(&self, _params: NoParams) -> Result<(), F> {
            Err(self.failure.clone())
        }
    }

    /// Fails with a `NetworkFailure` for the first `fail_times` calls, then
    /// succeeds with `Ok(5)`. Tracks the total number of calls via an
    /// interior atomic counter, exposed through [`Flaky::attempts`].
    #[derive(Debug)]
    pub struct Flaky {
        fail_times: u32,
        attempts: AtomicU32,
    }

    impl Flaky {
        pub fn new(fail_times: u32) -> Self {
            Self {
                fail_times,
                attempts: AtomicU32::new(0),
            }
        }

        /// Total number of times `execute` has been called so far.
        pub fn attempts(&self) -> u32 {
            self.attempts.load(Ordering::SeqCst)
        }
    }

    #[cfg_attr(not(target_arch = "wasm32"), async_trait::async_trait)]
    #[cfg_attr(target_arch = "wasm32", async_trait::async_trait(?Send))]
    impl UseCase for Flaky {
        type Params = NoParams;
        type Output = i32;
        type Failure = NetworkFailure;

        async fn execute(&self, _params: NoParams) -> Result<i32, NetworkFailure> {
            let attempt = self.attempts.fetch_add(1, Ordering::SeqCst) + 1;
            if attempt <= self.fail_times {
                Err(NetworkFailure::new("flaky failure"))
            } else {
                Ok(5)
            }
        }
    }

    /// Awaits a `futures::channel::oneshot` gate before resolving to
    /// `Ok(5)`. [`Slow::new`] returns the fixture paired with the `Sender`
    /// used to release it, letting tests deterministically control when a
    /// "slow" operation completes.
    pub struct Slow {
        gate: Mutex<Option<futures::channel::oneshot::Receiver<()>>>,
        started: Mutex<Option<futures::channel::oneshot::Sender<()>>>,
    }

    impl Slow {
        pub fn new() -> (Self, futures::channel::oneshot::Sender<()>) {
            let (tx, rx) = futures::channel::oneshot::channel();
            (
                Self {
                    gate: Mutex::new(Some(rx)),
                    started: Mutex::new(None),
                },
                tx,
            )
        }

        /// Like [`Slow::new`], but also returns a receiver that fires the
        /// moment `execute` is entered, before it parks on the main gate — a
        /// deterministic, race-free proxy for "the operation has started" in
        /// tests that need to observe intermediate state (e.g.
        /// `is_loading()`, a mid-reload `AsyncState`) without a wall-clock
        /// sleep. `ControllerCore::run`/`run_into` flip activity/state to
        /// `Loading`/`Reloading` synchronously *before* calling `execute`, so
        /// awaiting this receiver is sufficient proof the visible state has
        /// already changed.
        pub fn with_started_signal() -> (
            Self,
            futures::channel::oneshot::Sender<()>,
            futures::channel::oneshot::Receiver<()>,
        ) {
            let (tx, rx) = futures::channel::oneshot::channel();
            let (started_tx, started_rx) = futures::channel::oneshot::channel();
            (
                Self {
                    gate: Mutex::new(Some(rx)),
                    started: Mutex::new(Some(started_tx)),
                },
                tx,
                started_rx,
            )
        }
    }

    #[cfg_attr(not(target_arch = "wasm32"), async_trait::async_trait)]
    #[cfg_attr(target_arch = "wasm32", async_trait::async_trait(?Send))]
    impl UseCase for Slow {
        type Params = NoParams;
        type Output = i32;
        type Failure = NetworkFailure;

        async fn execute(&self, _params: NoParams) -> Result<i32, NetworkFailure> {
            if let Some(started_tx) = self.started.lock().unwrap().take() {
                // Best-effort: a dropped receiver (test doesn't care) is fine.
                let _ = started_tx.send(());
            }
            let rx = self.gate.lock().unwrap().take();
            if let Some(rx) = rx {
                let _ = rx.await;
            }
            Ok(5)
        }
    }

    /// Stream fixture: yields `Ok(1)..=Ok(n)` then a trailing
    /// `Err(NetworkFailure("tick lost"))` and terminates — a finite stream that
    /// ends in a failure item. `n` is supplied as the `execute` params.
    #[derive(Default, Clone, Copy, Debug)]
    pub struct Ticker;

    impl StreamUseCase for Ticker {
        type Params = u32;
        type Output = u32;
        type Failure = NetworkFailure;

        fn execute(&self, n: u32) -> UseCaseStream<u32, NetworkFailure> {
            let ticks = stream::iter((1..=n).map(Ok));
            let tail = stream::once(async { Err(NetworkFailure::new("tick lost")) });
            let combined = ticks.chain(tail);

            #[cfg(not(target_arch = "wasm32"))]
            {
                combined.boxed()
            }
            #[cfg(target_arch = "wasm32")]
            {
                combined.boxed_local()
            }
        }
    }

    /// Stream fixture: yields `Ok(1)..=Ok(n)` then terminates cleanly (no
    /// trailing failure) — a finite, all-success stream.
    #[derive(Default, Clone, Copy, Debug)]
    pub struct Counting;

    impl StreamUseCase for Counting {
        type Params = u32;
        type Output = u32;
        type Failure = NetworkFailure;

        fn execute(&self, n: u32) -> UseCaseStream<u32, NetworkFailure> {
            let counted = stream::iter((1..=n).map(Ok));

            #[cfg(not(target_arch = "wasm32"))]
            {
                counted.boxed()
            }
            #[cfg(target_arch = "wasm32")]
            {
                counted.boxed_local()
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::fixtures::{Counting, Doubler, Failing, Flaky, Slow, Ticker};
    use super::*;
    use crate::failure::fixtures::NetworkFailure;
    use futures::stream::StreamExt;
    use std::sync::Arc;
    use std::sync::atomic::{AtomicU32, Ordering};

    #[tokio::test]
    async fn doubler_executes() {
        let uc = Doubler;
        assert_eq!(uc.execute(21).await, Ok(42));
    }

    #[tokio::test]
    async fn failing_always_fails_with_configured_failure() {
        let uc = Failing::new(NetworkFailure::new("boom"));
        let err = uc.execute(NoParams).await.unwrap_err();
        assert_eq!(err, NetworkFailure::new("boom"));
    }

    #[tokio::test]
    async fn flaky_succeeds_on_attempt_n_plus_1_and_attempts_counts_calls() {
        let uc = Flaky::new(2);

        assert!(uc.execute(NoParams).await.is_err());
        assert!(uc.execute(NoParams).await.is_err());
        assert_eq!(uc.execute(NoParams).await, Ok(5));
        assert_eq!(uc.attempts(), 3);
    }

    #[tokio::test]
    async fn slow_resolves_only_after_gate_is_released() {
        let (uc, tx) = Slow::new();
        let handle = tokio::spawn(async move { uc.execute(NoParams).await });

        tx.send(()).expect("receiver must still be alive");
        assert_eq!(handle.await.unwrap(), Ok(5));
    }

    #[tokio::test]
    async fn ticker_yields_n_ok_then_one_err_then_terminates() {
        let uc = Ticker;
        let items: Vec<_> = uc.execute(3).collect().await;
        assert_eq!(
            items,
            vec![Ok(1), Ok(2), Ok(3), Err(NetworkFailure::new("tick lost")),]
        );
    }

    #[tokio::test]
    async fn counting_yields_n_ok_then_terminates_with_no_trailing_failure() {
        let uc = Counting;
        let items: Vec<_> = uc.execute(3).collect().await;
        assert_eq!(items, vec![Ok(1), Ok(2), Ok(3)]);
    }

    /// Cancellation semantics: dropping a [`UseCaseStream`] mid-iteration
    /// stops it immediately — no further polls happen, proven here via a
    /// stream whose every poll has an observable side effect (incrementing
    /// a shared counter).
    #[tokio::test]
    async fn dropping_a_stream_mid_iteration_stops_further_polls() {
        let counter = Arc::new(AtomicU32::new(0));
        let mut stream: UseCaseStream<u32, NetworkFailure> = {
            let counter = Arc::clone(&counter);
            futures::stream::unfold(0u32, move |state| {
                let counter = Arc::clone(&counter);
                async move {
                    counter.fetch_add(1, Ordering::SeqCst);
                    if state < 5 {
                        Some((Ok(state), state + 1))
                    } else {
                        None
                    }
                }
            })
            .boxed()
        };

        assert_eq!(stream.next().await, Some(Ok(0)));
        assert_eq!(stream.next().await, Some(Ok(1)));
        assert_eq!(
            counter.load(Ordering::SeqCst),
            2,
            "polled exactly twice so far"
        );

        drop(stream);

        assert_eq!(
            counter.load(Ordering::SeqCst),
            2,
            "dropping the stream must halt it immediately: no further polls"
        );
    }
}
