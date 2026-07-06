//! `AsyncState<T, F>` — the state shape controllers store `run_into` results
//! in, plus the `ResultExt::to_async_state()` bridge from `Result<T, F>`.
//!
//! A four-state enum — `Loading | Data | Reloading | Error`. The
//! [`AsyncState::Reloading`] variant keeps the previous value visible while a
//! refresh is in flight, so the UI never flashes back to a bare loading state
//! on refetch.

use reactive_graph::signal::RwSignal;

/// The state of an async operation whose result is stored reactively.
///
/// `T` is the success value type, `F` the app's [`crate::failure::Failure`]
/// type. `Error { stale }` retains the last-known-good value (if any) so a
/// failed refresh doesn't discard data the UI was already showing.
#[derive(Clone, Debug, PartialEq)]
pub enum AsyncState<T, F> {
    /// No value has ever loaded yet; a first fetch is in flight.
    Loading,
    /// A value is available and nothing is currently in flight.
    Data(T),
    /// A value is available, but a refresh is currently in flight. Keeps the
    /// stale value visible instead of falling back to `Loading`.
    Reloading(T),
    /// The last (re)fetch failed. `stale` carries a previously-loaded value,
    /// if one existed, so it can still be displayed alongside the error.
    Error { failure: F, stale: Option<T> },
}

impl<T, F> AsyncState<T, F> {
    /// The current value, if any — from `Data`, `Reloading`, or an `Error`
    /// carrying stale data. `None` for `Loading` and a bare (dataless)
    /// `Error`.
    pub fn value(&self) -> Option<&T> {
        match self {
            AsyncState::Loading => None,
            AsyncState::Data(value) => Some(value),
            AsyncState::Reloading(value) => Some(value),
            AsyncState::Error { stale, .. } => stale.as_ref(),
        }
    }

    /// Whether [`value`](Self::value) would return `Some`.
    pub fn has_value(&self) -> bool {
        self.value().is_some()
    }

    /// Whether an operation is currently in flight (`Loading` or
    /// `Reloading`).
    pub fn is_loading(&self) -> bool {
        matches!(self, AsyncState::Loading | AsyncState::Reloading(_))
    }

    /// The failure, if the current state is `Error`.
    pub fn failure(&self) -> Option<&F> {
        match self {
            AsyncState::Error { failure, .. } => Some(failure),
            _ => None,
        }
    }
}

/// Creates a fresh `RwSignal<AsyncState<T, F>>` seeded with [`AsyncState::Loading`].
///
/// The conventional shape for controller state fed by `Result`-returning use
/// cases:
///
/// ```rust
/// use clean_signals::async_state::{async_state_signal, AsyncState};
///
/// let users = async_state_signal::<Vec<String>, std::convert::Infallible>();
/// ```
pub fn async_state_signal<T, F>() -> RwSignal<AsyncState<T, F>>
where
    T: Send + Sync + 'static,
    F: Send + Sync + 'static,
{
    RwSignal::new(AsyncState::Loading)
}

/// Computes the `AsyncState` a controller should transition *into* when it
/// begins reloading, given the *previous* state — a pure, unit-testable
/// helper consumed by `ControllerCore::run_into` (task 06).
///
/// - `Data(d)` | `Reloading(d)` → `Reloading(d)` (keep showing the stale
///   value while the refresh runs).
/// - `Error { stale: Some(d), .. }` → `Reloading(d)` (a previously-loaded
///   value survives a failed refresh, and stays visible through the next
///   retry too).
/// - `Loading` | `Error { stale: None, .. }` → `Loading` (nothing to show
///   yet).
pub fn to_reloading<T: Clone, F>(prev: &AsyncState<T, F>) -> AsyncState<T, F> {
    match prev {
        AsyncState::Data(value) | AsyncState::Reloading(value) => {
            AsyncState::Reloading(value.clone())
        }
        AsyncState::Error {
            stale: Some(value), ..
        } => AsyncState::Reloading(value.clone()),
        AsyncState::Loading | AsyncState::Error { stale: None, .. } => AsyncState::Loading,
    }
}

/// Bridges `Result<T, F>` into `AsyncState<T, F>`.
pub trait ResultExt<T, F> {
    /// `Ok(value)` → `AsyncState::Data(value)`; `Err(failure)` →
    /// `AsyncState::Error { failure, stale: None }`.
    ///
    /// This is a stateless conversion of a bare `Result` — it never knows
    /// about a previous state's stale data. Controllers that need to
    /// preserve stale data across a failed reload use [`to_reloading`]
    /// *before* calling the use case, then apply this conversion only to
    /// pick the failure/data branch (task 06 wires the two together).
    fn to_async_state(self) -> AsyncState<T, F>;
}

impl<T, F> ResultExt<T, F> for Result<T, F> {
    fn to_async_state(self) -> AsyncState<T, F> {
        match self {
            Ok(value) => AsyncState::Data(value),
            Err(failure) => AsyncState::Error {
                failure,
                stale: None,
            },
        }
    }
}

#[cfg(test)]
mod tests {
    use super::{AsyncState, ResultExt, async_state_signal, to_reloading};
    use reactive_graph::traits::GetUntracked;

    #[derive(Clone, Debug, PartialEq)]
    struct Fixture(&'static str);

    #[test]
    fn value_returns_data_for_data_and_reloading() {
        let data: AsyncState<i32, Fixture> = AsyncState::Data(1);
        assert_eq!(data.value(), Some(&1));

        let reloading: AsyncState<i32, Fixture> = AsyncState::Reloading(2);
        assert_eq!(reloading.value(), Some(&2));
    }

    #[test]
    fn value_returns_stale_for_error_with_stale_and_none_without() {
        let with_stale: AsyncState<i32, Fixture> = AsyncState::Error {
            failure: Fixture("boom"),
            stale: Some(3),
        };
        assert_eq!(with_stale.value(), Some(&3));

        let without_stale: AsyncState<i32, Fixture> = AsyncState::Error {
            failure: Fixture("boom"),
            stale: None,
        };
        assert_eq!(without_stale.value(), None);
    }

    #[test]
    fn value_is_none_for_loading() {
        let loading: AsyncState<i32, Fixture> = AsyncState::Loading;
        assert_eq!(loading.value(), None);
    }

    #[test]
    fn has_value_matches_value_presence() {
        let loading: AsyncState<i32, Fixture> = AsyncState::Loading;
        assert!(!loading.has_value());

        let data: AsyncState<i32, Fixture> = AsyncState::Data(1);
        assert!(data.has_value());
    }

    #[test]
    fn is_loading_true_only_for_loading_and_reloading() {
        assert!(AsyncState::<i32, Fixture>::Loading.is_loading());
        assert!(AsyncState::<i32, Fixture>::Reloading(1).is_loading());
        assert!(!AsyncState::<i32, Fixture>::Data(1).is_loading());
        assert!(
            !AsyncState::<i32, Fixture>::Error {
                failure: Fixture("boom"),
                stale: None,
            }
            .is_loading()
        );
    }

    #[test]
    fn failure_returns_some_only_for_error() {
        let err: AsyncState<i32, Fixture> = AsyncState::Error {
            failure: Fixture("boom"),
            stale: None,
        };
        assert_eq!(err.failure(), Some(&Fixture("boom")));
        assert_eq!(AsyncState::<i32, Fixture>::Data(1).failure(), None);
        assert_eq!(AsyncState::<i32, Fixture>::Loading.failure(), None);
    }

    #[test]
    fn async_state_signal_seeds_loading() {
        let signal = async_state_signal::<i32, Fixture>();
        assert_eq!(signal.get_untracked(), AsyncState::Loading);
    }

    #[test]
    fn to_reloading_covers_all_four_variants() {
        assert_eq!(
            to_reloading(&AsyncState::<i32, Fixture>::Data(1)),
            AsyncState::Reloading(1)
        );
        assert_eq!(
            to_reloading(&AsyncState::<i32, Fixture>::Reloading(2)),
            AsyncState::Reloading(2)
        );
        assert_eq!(
            to_reloading(&AsyncState::Error {
                failure: Fixture("boom"),
                stale: Some(3),
            }),
            AsyncState::Reloading(3)
        );
        assert_eq!(
            to_reloading(&AsyncState::<i32, Fixture>::Error {
                failure: Fixture("boom"),
                stale: None,
            }),
            AsyncState::Loading
        );
        assert_eq!(
            to_reloading(&AsyncState::<i32, Fixture>::Loading),
            AsyncState::Loading
        );
    }

    #[test]
    fn result_ext_ok_maps_to_data() {
        let result: Result<i32, Fixture> = Ok(42);
        assert_eq!(result.to_async_state(), AsyncState::Data(42));
    }

    #[test]
    fn result_ext_err_maps_to_error_with_no_stale() {
        let result: Result<i32, Fixture> = Err(Fixture("boom"));
        assert_eq!(
            result.to_async_state(),
            AsyncState::Error {
                failure: Fixture("boom"),
                stale: None,
            }
        );
    }
}
