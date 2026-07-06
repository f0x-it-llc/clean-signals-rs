//! The `team` feature's data layer — the one layer that differs from the CSR
//! demo. Instead of an in-process source, the transport is a pair of
//! `#[server]` functions ([`server_fns`]); the repository ([`ServerFnTeamRepo`])
//! adapts them to the domain [`super::domain::repositories::TeamRepository`]
//! trait, and on the server their bodies hit an in-memory [`TeamStore`].

pub mod models;
pub mod repositories;
pub mod server_fns;

/// Server-only: the in-memory backing store lives (and is referenced) solely on
/// the `ssr` build, inside the server-function bodies. See [`store`].
#[cfg(feature = "ssr")]
pub mod store;

pub use repositories::ServerFnTeamRepo;

#[cfg(feature = "ssr")]
pub use store::TeamStore;
