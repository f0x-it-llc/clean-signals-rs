//! `RetryPolicy<F>` — declarative per-call retry.
//!
//! # Layer rule
//!
//! This module is pure policy math + predicate. The retry *loop* itself
//! lives in `ControllerCore::run` (task 06) — a `RetryPolicy` describes how
//! many attempts to make and how long to wait between them, it does not
//! drive the loop.
//!
//! A `RetryPolicy` is a plain value: no global/shared retry state, cloned
//! per call.

use crate::failure::Failure;
use std::sync::Arc;
use std::time::Duration;

/// Predicate deciding whether a given failure is worth retrying.
type RetryIf<F> = Arc<dyn Fn(&F) -> bool + Send + Sync>;

/// Declarative retry behavior for a single use-case invocation.
///
/// Passed per call to `ControllerCore::run`, so each operation owns its
/// retry state.
///
/// ```
/// use clean_signals::failure::Failure;
/// use clean_signals::retry::RetryPolicy;
/// use std::fmt;
/// use std::time::Duration;
///
/// #[derive(Debug)]
/// struct NetworkFailure;
/// impl fmt::Display for NetworkFailure {
///     fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
///         f.write_str("network failure")
///     }
/// }
/// impl Failure for NetworkFailure {
///     fn is_retryable(&self) -> bool {
///         true
///     }
/// }
///
/// let policy = RetryPolicy::<NetworkFailure>::new(3, Duration::from_millis(300));
/// ```
pub struct RetryPolicy<F> {
    /// Total number of attempts, including the first one (not the retry
    /// count).
    pub max_attempts: u32,
    /// Delay before the first retry.
    pub delay: Duration,
    /// Multiplier applied to `delay` for each subsequent retry. `1.0`
    /// yields a constant delay.
    pub backoff_factor: f64,
    /// Predicate deciding whether a given failure is worth retrying.
    /// Defaults to [`Failure::is_retryable`] when unset.
    retry_if: Option<RetryIf<F>>,
}

// Manual `Clone` impl: `#[derive(Clone)]` would add a spurious `F: Clone`
// bound even though the only field that mentions `F` is an `Arc<dyn Fn(&F)
// -> bool + Send + Sync>`, which is `Clone` regardless of `F`'s bounds.
impl<F> Clone for RetryPolicy<F> {
    fn clone(&self) -> Self {
        Self {
            max_attempts: self.max_attempts,
            delay: self.delay,
            backoff_factor: self.backoff_factor,
            retry_if: self.retry_if.clone(),
        }
    }
}

impl<F> Default for RetryPolicy<F> {
    fn default() -> Self {
        Self::none()
    }
}

impl<F> RetryPolicy<F> {
    /// No retries: the operation runs exactly once.
    pub fn none() -> Self {
        Self {
            max_attempts: 1,
            delay: Duration::from_millis(300),
            backoff_factor: 2.0,
            retry_if: None,
        }
    }

    /// A policy that attempts the operation up to `max_attempts` times
    /// (including the first attempt), waiting `delay` before the first
    /// retry.
    ///
    /// `backoff_factor` defaults to `2.0` (exponential backoff), matching
    /// the upstream Dart `RetryPolicy`'s default. Use [`Self::with_backoff`]
    /// to override it (e.g. `1.0` for a constant delay).
    pub fn new(max_attempts: u32, delay: Duration) -> Self {
        Self {
            max_attempts: max_attempts.max(1),
            delay,
            backoff_factor: 2.0,
            retry_if: None,
        }
    }

    /// Overrides the backoff multiplier applied between retries.
    pub fn with_backoff(mut self, factor: f64) -> Self {
        self.backoff_factor = factor;
        self
    }

    /// Overrides the retry predicate, replacing the default
    /// [`Failure::is_retryable`] check.
    pub fn retry_if(mut self, f: impl Fn(&F) -> bool + Send + Sync + 'static) -> Self {
        self.retry_if = Some(Arc::new(f));
        self
    }

    /// Delay before attempt number `attempt` (1-based).
    ///
    /// `delay_for(1) == delay`, `delay_for(n) == delay *
    /// backoff_factor^(n-1)`. `attempt == 0` is treated as `1`.
    pub fn delay_for(&self, attempt: u32) -> Duration {
        let attempt = attempt.max(1);
        let exponent = (attempt - 1) as i32;
        let scale = self.backoff_factor.powi(exponent);
        Duration::from_secs_f64(self.delay.as_secs_f64() * scale)
    }
}

impl<F: Failure> RetryPolicy<F> {
    /// Whether `failure` should be retried under this policy: the custom
    /// `retry_if` predicate if one was set via [`Self::retry_if`], else
    /// [`Failure::is_retryable`].
    pub fn should_retry(&self, failure: &F) -> bool {
        match &self.retry_if {
            Some(predicate) => predicate(failure),
            None => failure.is_retryable(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::failure::fixtures::{NetworkFailure, ValidationFailure};

    #[test]
    fn none_has_max_attempts_one() {
        let policy = RetryPolicy::<NetworkFailure>::none();
        assert_eq!(policy.max_attempts, 1);
    }

    #[test]
    fn default_is_none() {
        let policy = RetryPolicy::<NetworkFailure>::default();
        assert_eq!(policy.max_attempts, 1);
    }

    #[test]
    fn delay_for_exponential_progression() {
        let policy = RetryPolicy::<NetworkFailure>::new(4, Duration::from_millis(100))
            .with_backoff(2.0);

        assert_eq!(policy.delay_for(1), Duration::from_millis(100));
        assert_eq!(policy.delay_for(2), Duration::from_millis(200));
        assert_eq!(policy.delay_for(3), Duration::from_millis(400));
    }

    #[test]
    fn delay_for_constant_with_factor_one() {
        let policy =
            RetryPolicy::<NetworkFailure>::new(3, Duration::from_millis(50)).with_backoff(1.0);

        assert_eq!(policy.delay_for(1), Duration::from_millis(50));
        assert_eq!(policy.delay_for(2), Duration::from_millis(50));
        assert_eq!(policy.delay_for(3), Duration::from_millis(50));
    }

    #[test]
    fn delay_for_guards_attempt_zero() {
        let policy = RetryPolicy::<NetworkFailure>::new(2, Duration::from_millis(10));
        assert_eq!(policy.delay_for(0), policy.delay_for(1));
    }

    #[test]
    fn should_retry_honors_is_retryable_by_default() {
        let policy = RetryPolicy::<NetworkFailure>::new(3, Duration::from_millis(10));
        assert!(policy.should_retry(&NetworkFailure::new("boom")));

        let policy = RetryPolicy::<ValidationFailure>::new(3, Duration::from_millis(10));
        assert!(!policy.should_retry(&ValidationFailure::new("bad input")));
    }

    #[test]
    fn custom_retry_if_overrides_is_retryable_to_allow() {
        // ValidationFailure.is_retryable() is false; custom predicate flips it to true.
        let policy = RetryPolicy::<ValidationFailure>::new(3, Duration::from_millis(10))
            .retry_if(|_f| true);
        assert!(policy.should_retry(&ValidationFailure::new("bad input")));
    }

    #[test]
    fn custom_retry_if_overrides_is_retryable_to_deny() {
        // NetworkFailure.is_retryable() is true; custom predicate flips it to false.
        let policy = RetryPolicy::<NetworkFailure>::new(3, Duration::from_millis(10))
            .retry_if(|_f| false);
        assert!(!policy.should_retry(&NetworkFailure::new("boom")));
    }

    #[test]
    fn policy_is_clone_via_manual_impl() {
        let policy = RetryPolicy::<NetworkFailure>::new(3, Duration::from_millis(10))
            .retry_if(|f: &NetworkFailure| f.message.len() > 3);
        let cloned = policy.clone();
        assert_eq!(cloned.max_attempts, policy.max_attempts);
        assert_eq!(
            cloned.should_retry(&NetworkFailure::new("abcd")),
            policy.should_retry(&NetworkFailure::new("abcd"))
        );
    }
}
