//! The `#[server]` functions — this app's *transport*. They are the SSR
//! analogue of the CSR demo's `InMemoryTeamSource`: the wire over which the
//! data layer talks to the backend. On the client (hydrate) build the
//! `#[server]` macro compiles these into an HTTP call to `/api/...`; on the
//! server (ssr) build it compiles the bodies below, which run in-process
//! against the shared [`TeamStore`] pulled from Leptos context.
//!
//! Server functions are Leptos's transport primitive, so this is the one place
//! the data layer imports from `leptos` — analogous to a `reqwest`/`tonic`
//! client in a networked app. The import is kept narrow (the `server` macro +
//! `ServerFnError`); no reactive/DOM/view types cross into the data layer.
//! Server-only dependencies (the store, `use_context`) are imported *inside*
//! the bodies, which only exist on the `ssr` build.

use super::models::MemberDto;
use leptos::prelude::{ServerFnError, server};

/// Fetches every team member. Transport-level failures surface as
/// [`ServerFnError`]; [`super::repositories::ServerFnTeamRepo`] maps them to
/// `TeamFailure` at the one conversion site.
#[server]
pub async fn fetch_team() -> Result<Vec<MemberDto>, ServerFnError> {
    use super::store::TeamStore;
    use leptos::prelude::use_context;
    use std::sync::Arc;

    let Some(store) = use_context::<Arc<TeamStore>>() else {
        return Err(ServerFnError::ServerError(
            "team store missing from server context".to_string(),
        ));
    };
    store.fetch_members().await
}

/// Renames the member with `id`, returning the updated wire row. Input
/// validation (empty/too-short names) lives in the `UpdateMember` use case and
/// never reaches this transport; the only failure here is an unknown `id`.
#[server]
pub async fn rename_member(id: String, name: String) -> Result<MemberDto, ServerFnError> {
    use super::store::TeamStore;
    use leptos::prelude::use_context;
    use std::sync::Arc;

    let Some(store) = use_context::<Arc<TeamStore>>() else {
        return Err(ServerFnError::ServerError(
            "team store missing from server context".to_string(),
        ));
    };
    store.patch_member_name(&id, &name).await
}
