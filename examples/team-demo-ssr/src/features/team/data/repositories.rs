//! [`ServerFnTeamRepo`] — the [`TeamRepository`] implementation this SSR demo
//! wires up. It is the SSR counterpart of the CSR demo's `InMemoryTeamRepo`:
//! same job (own the translation boundary), different transport. Where the CSR
//! repo called an in-process source and mapped `TransportError`, this one calls
//! the `#[server]` functions and maps [`ServerFnError`] → [`TeamFailure`] at
//! exactly one site ([`map_server_fn_error`]) — nothing above this layer knows
//! `ServerFnError` or [`MemberDto`] exist (see `templates/AGENTS.md` data
//! rules).
//!
//! The repo is stateless: the transport is the globally-registered server
//! functions, and (on the server) the store lives in Leptos context, so there
//! is nothing to inject here. The client composition root (`App` in `lib.rs`)
//! constructs it with [`ServerFnTeamRepo::new`].

use crate::failure::TeamFailure;
use crate::features::team::data::server_fns::{fetch_team, rename_member};
use crate::features::team::domain::entities::Member;
use crate::features::team::domain::repositories::TeamRepository;
use leptos::prelude::ServerFnError;

/// [`TeamRepository`] backed by the `#[server]` transport in
/// [`super::server_fns`].
#[derive(Default)]
pub struct ServerFnTeamRepo;

impl ServerFnTeamRepo {
    pub fn new() -> Self {
        Self
    }
}

#[cfg_attr(not(target_arch = "wasm32"), clean_signals::async_trait)]
#[cfg_attr(target_arch = "wasm32", clean_signals::async_trait(?Send))]
impl TeamRepository for ServerFnTeamRepo {
    async fn list_members(&self) -> Result<Vec<Member>, TeamFailure> {
        let dtos = fetch_team().await.map_err(map_server_fn_error)?;
        Ok(dtos.into_iter().map(Into::into).collect())
    }

    async fn update_member_name(&self, id: String, name: String) -> Result<Member, TeamFailure> {
        let dto = rename_member(id, name).await.map_err(map_server_fn_error)?;
        Ok(dto.into())
    }
}

/// The single conversion site from the transport error to the app's failure
/// type — per `docs/CODE_STANDARDS.md`, no raw transport error may cross the
/// repository boundary.
///
/// The mapping keys off the [`ServerFnError`] *variant*, never its message
/// string (per the "never string-match on error text" rule): a bad-argument
/// error is a non-retryable [`TeamFailure::Validation`]; every other transport
/// error (server error, dropped request/response, (de)serialization) is a
/// transient [`TeamFailure::Network`], which the controller's `RetryPolicy`
/// will absorb.
fn map_server_fn_error(err: ServerFnError) -> TeamFailure {
    match err {
        ServerFnError::Args(msg) | ServerFnError::MissingArg(msg) => TeamFailure::Validation(msg),
        other => TeamFailure::Network(other.to_string()),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn args_error_maps_to_non_retryable_validation() {
        use clean_signals::Failure;
        let failure = map_server_fn_error(ServerFnError::Args("no member u9".to_string()));
        assert_eq!(failure, TeamFailure::Validation("no member u9".to_string()));
        assert!(!failure.is_retryable());
    }

    #[test]
    fn missing_arg_error_maps_to_validation() {
        let failure = map_server_fn_error(ServerFnError::MissingArg("id".to_string()));
        assert!(matches!(failure, TeamFailure::Validation(_)));
    }

    #[test]
    fn server_error_maps_to_retryable_network() {
        use clean_signals::Failure;
        let failure = map_server_fn_error(ServerFnError::ServerError(
            "temporary upstream error".to_string(),
        ));
        assert!(matches!(failure, TeamFailure::Network(_)));
        assert!(failure.is_retryable());
    }

    #[test]
    fn transport_response_error_maps_to_retryable_network() {
        use clean_signals::Failure;
        let failure = map_server_fn_error(ServerFnError::Response("connection reset".to_string()));
        assert!(matches!(failure, TeamFailure::Network(_)));
        assert!(failure.is_retryable());
    }
}
