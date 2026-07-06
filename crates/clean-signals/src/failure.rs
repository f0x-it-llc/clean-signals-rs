//! `Failure` — the trait every app-specific failure enum implements.
//!
//! # Layer rule
//!
//! Repositories are the only place raw transport/IO errors are allowed to
//! exist. At the repository boundary they must be converted into the
//! consuming app's `Failure` enum via `From` impls (e.g. `impl From<reqwest::Error>
//! for AppFailure`). Failures never cross a layer boundary as raw errors —
//! use cases, controllers, and views only ever see `Self::Failure: Failure`.

use std::fmt::{Debug, Display};

/// A domain failure. Implementors are typically small enums covering the
/// distinct failure modes an app's use cases can produce.
///
/// See the module-level docs for the layer rule this trait exists to enforce.
pub trait Failure: Debug + Display + Send + Sync + 'static {
    /// A message safe to show to an end user. Defaults to [`Display`]'s
    /// output; override when the `Display` impl is meant for logs/debugging
    /// instead.
    fn user_message(&self) -> String {
        self.to_string()
    }

    /// Whether the operation that produced this failure is safe to retry.
    /// Defaults to `false`; [`crate::retry::RetryPolicy`] consults this
    /// unless a custom `retry_if` predicate is supplied.
    fn is_retryable(&self) -> bool {
        false
    }
}

/// Shared test-fixture failure types used across the crate's (and sibling
/// crates') test suites. Enabled for unit tests and via the `test-fixtures`
/// cargo feature for downstream integration tests.
#[cfg(any(test, feature = "test-fixtures"))]
pub mod fixtures {
    use super::Failure;
    use std::fmt;

    /// A retryable failure standing in for a transient network/transport error.
    #[derive(Clone, Debug, PartialEq, Eq)]
    pub struct NetworkFailure {
        pub message: String,
    }

    impl NetworkFailure {
        pub fn new(message: impl Into<String>) -> Self {
            Self {
                message: message.into(),
            }
        }
    }

    impl fmt::Display for NetworkFailure {
        fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
            f.write_str(&self.message)
        }
    }

    impl Failure for NetworkFailure {
        fn is_retryable(&self) -> bool {
            true
        }
    }

    /// A non-retryable failure standing in for input/validation errors.
    #[derive(Clone, Debug, PartialEq, Eq)]
    pub struct ValidationFailure {
        pub message: String,
    }

    impl ValidationFailure {
        pub fn new(message: impl Into<String>) -> Self {
            Self {
                message: message.into(),
            }
        }
    }

    impl fmt::Display for ValidationFailure {
        fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
            f.write_str(&self.message)
        }
    }

    impl Failure for ValidationFailure {
        fn is_retryable(&self) -> bool {
            false
        }
    }
}

#[cfg(test)]
mod tests {
    use super::fixtures::{NetworkFailure, ValidationFailure};
    use super::Failure;
    use std::fmt;

    #[test]
    fn default_user_message_mirrors_display() {
        let f = NetworkFailure::new("connection reset");
        assert_eq!(f.user_message(), f.to_string());
        assert_eq!(f.user_message(), "connection reset");
    }

    #[test]
    fn default_is_retryable_is_false() {
        #[derive(Debug)]
        struct Plain;

        impl fmt::Display for Plain {
            fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
                f.write_str("plain failure")
            }
        }

        impl Failure for Plain {}

        assert!(!Plain.is_retryable());
        assert_eq!(Plain.user_message(), "plain failure");
    }

    #[test]
    fn network_failure_is_retryable() {
        let f = NetworkFailure::new("timeout");
        assert!(f.is_retryable());
        assert_eq!(f.to_string(), "timeout");
        assert_eq!(f.clone(), f);
    }

    #[test]
    fn validation_failure_is_not_retryable() {
        let f = ValidationFailure::new("field required");
        assert!(!f.is_retryable());
        assert_eq!(f.to_string(), "field required");
        assert_eq!(f.clone(), f);
    }

    /// Blanket-usage test: an app-style enum implementing `Failure` with
    /// exhaustive matching, per PLAN.md design delta #2 (no `UnexpectedFailure`
    /// port — apps define their own closed failure enum).
    #[derive(Debug, Clone, PartialEq, Eq)]
    enum AppFailure {
        Network(String),
        Validation(String),
    }

    impl fmt::Display for AppFailure {
        fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
            match self {
                AppFailure::Network(msg) => write!(f, "network: {msg}"),
                AppFailure::Validation(msg) => write!(f, "validation: {msg}"),
            }
        }
    }

    impl Failure for AppFailure {
        fn is_retryable(&self) -> bool {
            match self {
                AppFailure::Network(_) => true,
                AppFailure::Validation(_) => false,
            }
        }
    }

    #[test]
    fn app_failure_enum_implements_trait_with_exhaustive_matching() {
        let network = AppFailure::Network("timeout".to_string());
        let validation = AppFailure::Validation("bad input".to_string());

        assert!(network.is_retryable());
        assert!(!validation.is_retryable());
        assert_eq!(network.user_message(), "network: timeout");
        assert_eq!(validation.user_message(), "validation: bad input");
    }
}
