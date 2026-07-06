//! Server-side, in-memory stand-in for a real team database — the SSR analogue
//! of the CSR demo's `InMemoryTeamSource`.
//!
//! This module is **server-only** (`#[cfg(feature = "ssr")]`): it is reached
//! exclusively from the bodies of the `#[server]` functions in
//! [`super::server_fns`], which the `#[server]` macro compiles only for the
//! `ssr` build. The composition root in `main.rs` constructs one
//! `Arc<TeamStore>` and provides it via Leptos context so every server-function
//! invocation (SSR render *and* subsequent client calls) reads the same store.
//!
//! It reproduces what makes real backends annoying, exactly like the CSR
//! source: per-call [`TeamStore::latency`] (via `clean_signals::time::sleep`,
//! never a raw `tokio`/`gloo` sleep — see `docs/CODE_STANDARDS.md`) and the
//! first `failures_before_success` list fetches failing with a transient
//! upstream error. Because the flakiness counter lives in the shared store,
//! it survives across the *separate HTTP retry requests* the client's
//! `RetryPolicy` issues — so the retry demonstration ports cleanly to SSR,
//! where each retry is a real round-trip rather than an in-process call.

use crate::features::team::data::models::MemberDto;
use leptos::prelude::ServerFnError;
use std::sync::RwLock;
use std::sync::atomic::{AtomicU32, Ordering};
use std::time::Duration;

/// The seeded, mutable server-side team store. Holds [`MemberDto`] rows (the
/// wire shape) behind an `RwLock`, plus the latency/flakiness knobs the demo
/// uses to exercise the controller's retry policy.
pub struct TeamStore {
    latency: Duration,
    failures_before_success: u32,
    list_fetches: AtomicU32,
    rows: RwLock<Vec<MemberDto>>,
}

impl TeamStore {
    /// `latency` is applied to every call; the first `failures_before_success`
    /// calls to [`Self::fetch_members`] fail with a transient
    /// [`ServerFnError::ServerError`] (retryable at the repository boundary)
    /// before the seeded data is ever returned.
    pub fn new(latency: Duration, failures_before_success: u32) -> Self {
        Self {
            latency,
            failures_before_success,
            list_fetches: AtomicU32::new(0),
            rows: RwLock::new(seed_rows()),
        }
    }

    /// Returns every member, after simulating latency and the configured
    /// transient failures. A transient failure is a
    /// [`ServerFnError::ServerError`] — the repository maps it to a *retryable*
    /// `TeamFailure::Network`.
    pub async fn fetch_members(&self) -> Result<Vec<MemberDto>, ServerFnError> {
        clean_signals::time::sleep(self.latency).await;
        let attempt = self.list_fetches.fetch_add(1, Ordering::SeqCst);
        if attempt < self.failures_before_success {
            return Err(ServerFnError::ServerError(
                "temporary upstream error".to_string(),
            ));
        }
        Ok(self.rows.read().unwrap().clone())
    }

    /// Renames the member with `id`, returning the updated row. An unknown
    /// `id` is a bad-argument error ([`ServerFnError::Args`]) — the repository
    /// maps it to a *non-retryable* `TeamFailure::Validation`.
    pub async fn patch_member_name(
        &self,
        id: &str,
        name: &str,
    ) -> Result<MemberDto, ServerFnError> {
        clean_signals::time::sleep(self.latency).await;
        let mut rows = self.rows.write().unwrap();
        match rows.iter_mut().find(|row| row.id == id) {
            Some(row) => {
                row.name = name.to_string();
                Ok(row.clone())
            }
            None => Err(ServerFnError::Args(format!("no member {id}"))),
        }
    }
}

fn seed_rows() -> Vec<MemberDto> {
    [
        ("u1", "Ava Chen", "Mobile Engineer", "ava@team.dev"),
        ("u2", "Bruno Costa", "Backend Engineer", "bruno@team.dev"),
        ("u3", "Chidi Okafor", "Product Designer", "chidi@team.dev"),
        ("u4", "Dana Weiss", "Engineering Manager", "dana@team.dev"),
        ("u5", "Emre Yilmaz", "QA Engineer", "emre@team.dev"),
        ("u6", "Freja Lund", "Mobile Engineer", "freja@team.dev"),
    ]
    .into_iter()
    .map(|(id, name, role, email)| MemberDto {
        id: id.to_string(),
        name: name.to_string(),
        role: role.to_string(),
        email: email.to_string(),
    })
    .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn fetch_members_fails_configured_times_then_succeeds() {
        let store = TeamStore::new(Duration::from_millis(0), 2);

        assert!(store.fetch_members().await.is_err());
        assert!(store.fetch_members().await.is_err());
        let rows = store.fetch_members().await.unwrap();
        assert_eq!(rows.len(), 6);
    }

    #[tokio::test]
    async fn patch_member_name_updates_matching_row() {
        let store = TeamStore::new(Duration::from_millis(0), 0);
        let row = store.patch_member_name("u1", "New Name").await.unwrap();
        assert_eq!(row.name, "New Name");
    }

    #[tokio::test]
    async fn patch_member_name_errors_for_unknown_id() {
        let store = TeamStore::new(Duration::from_millis(0), 0);
        let err = store
            .patch_member_name("nope", "New Name")
            .await
            .unwrap_err();
        assert!(matches!(err, ServerFnError::Args(_)));
    }
}
