//! `team-demo-ssr` — the `team-demo` feature slice, rendered as a Leptos 0.8
//! **SSR + hydrate** app (axum server) instead of CSR.
//!
//! The whole point of this crate is the *diff* against `examples/team-demo`.
//! Clean-signals architecture is render-mode-agnostic: the domain and
//! presentation layers are ported **verbatim** from the CSR demo, and only two
//! things change.
//!
//! # How this differs from the CSR demo
//!
//! 1. **Composition roots.** CSR has one root (`main.rs`, `mount_to_body`).
//!    SSR has two:
//!    - the **server** root ([`main`](../main/index.html), `#[cfg(ssr)]`) — an
//!      axum + `leptos_axum` server that constructs the in-memory
//!      [`TeamStore`](features::team::data::TeamStore) and provides it to every
//!      server-function invocation via context;
//!    - the **client** root ([`App`]) — constructs the
//!      [`ServerFnTeamRepo`](features::team::data::ServerFnTeamRepo) and mounts
//!      the same [`TeamPage`](features::team::presentation::TeamPage). `App` is
//!      rendered on the server (inside [`shell`]) *and* hydrated on the client
//!      ([`hydrate`]).
//!
//! 2. **Repository transport.** CSR's `InMemoryTeamRepo` called an in-process
//!    source and mapped a fake `TransportError`. SSR's `ServerFnTeamRepo` calls
//!    two `#[server]` functions (`fetch_team` / `rename_member`) — real HTTP on
//!    the client, in-process on the server — and maps `ServerFnError` →
//!    `TeamFailure` at one conversion site. See `features::team::data`.
//!
//! 3. **Initial-load (SSR-shell) strategy.** The initial `controller.load()`
//!    fires on the client **only** (a `wasm32`-target-gated `spawn_local` in
//!    `presentation::pages`). The server renders the `AsyncState::Loading`
//!    shell and never touches the transport, so the hydrated client DOM matches
//!    the server-rendered HTML (no hydration mismatch). Everything else — the
//!    controller, use cases, entities, failure enum, retry policy, failure sink
//!    — is identical to the CSR demo, byte for byte.
//!
//! # Running it
//!
//! This repository intentionally ships **no `cargo-leptos` configuration**
//! (no `[package.metadata.leptos]`, no `Trunk.toml`), because the example's
//! job is to demonstrate the *architecture* diff, not to be a deployable
//! server. What is verified here is that all three builds compile:
//!
//! ```sh
//! cargo check -p team-demo-ssr --features ssr                                   # native server
//! cargo check -p team-demo-ssr --features hydrate --target wasm32-unknown-unknown  # wasm client
//! cargo test  -p team-demo-ssr --features ssr                                   # native tests
//! ```
//!
//! To actually *run* it end to end you would add a `cargo-leptos` setup:
//! install it (`cargo install cargo-leptos`), add a `[package.metadata.leptos]`
//! table (naming this crate's `output-name`, `site-root`, `site-addr`, etc.),
//! then `cargo leptos watch`. `cargo-leptos` builds the server binary with
//! `--features ssr` and the wasm client with `--features hydrate`, runs
//! `wasm-bindgen`, and serves the two together — at which point [`main`] and
//! [`hydrate`] below become the two entry points it drives. Without that
//! tooling, `main` still compiles and constructs a valid axum app; it simply
//! has no generated wasm/JS assets to serve, so we verify compilation only.

pub mod failure;
pub mod features;

use crate::features::team::data::ServerFnTeamRepo;
use crate::features::team::domain::repositories::TeamRepository;
use crate::features::team::presentation::TeamPage;
use leptos::prelude::*;
use std::sync::Arc;

/// The client composition root: constructs the server-function-backed
/// repository and mounts the team page. Rendered on the server (via [`shell`])
/// and hydrated on the client (via [`hydrate`]) — the same component both ways,
/// which is exactly what makes the SSR/CSR difference invisible to the
/// presentation layer.
#[component]
pub fn App() -> impl IntoView {
    let repo: Arc<dyn TeamRepository + Send + Sync> = Arc::new(ServerFnTeamRepo::new());
    view! { <TeamPage repo=repo /> }
}

/// The server-rendered HTML document shell. Emits the `<!DOCTYPE>`, the head
/// (with Leptos's hydration + auto-reload scripts, wired from
/// [`LeptosOptions`]), and `<App/>` in the body — the markup the client then
/// hydrates. Server-only.
#[cfg(feature = "ssr")]
pub fn shell(options: LeptosOptions) -> impl IntoView {
    view! {
        <!DOCTYPE html>
        <html lang="en">
            <head>
                <meta charset="utf-8" />
                <meta name="viewport" content="width=device-width, initial-scale=1" />
                <AutoReload options=options.clone() />
                <HydrationScripts options=options />
                <title>"clean-signals — team-demo (SSR)"</title>
            </head>
            <body>
                <App />
            </body>
        </html>
    }
}

/// The wasm hydration entry point. `cargo-leptos`/`wasm-bindgen` calls this
/// exported function on page load; it hydrates the server-rendered [`App`]
/// markup in place (rather than mounting fresh, as the CSR demo does). Only
/// compiled for the `hydrate` build.
#[cfg(feature = "hydrate")]
#[wasm_bindgen::prelude::wasm_bindgen]
pub fn hydrate() {
    console_error_panic_hook::set_once();
    leptos::mount::hydrate_body(App);
}
