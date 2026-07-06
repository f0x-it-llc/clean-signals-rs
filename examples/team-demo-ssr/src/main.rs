//! Server composition root (`#[cfg(feature = "ssr")]`): an axum +
//! `leptos_axum` server that constructs the seeded, in-memory
//! [`TeamStore`](team_demo_ssr::features::team::data::TeamStore), provides it to
//! every server-function invocation via Leptos context, and serves the
//! server-rendered [`App`](team_demo_ssr::App) shell.
//!
//! This is the SSR counterpart of the CSR demo's `main.rs` `mount_to_body`
//! root. The binary only exists on the `ssr` build (see `required-features`
//! in `Cargo.toml`); the wasm client entry point is
//! [`team_demo_ssr::hydrate`].

#[cfg(feature = "ssr")]
#[tokio::main]
async fn main() {
    use axum::Router;
    use axum::extract::Request;
    use axum::routing::post;
    use leptos::prelude::*;
    use leptos_axum::{handle_server_fns_with_context, render_app_to_stream_with_context};
    use std::net::SocketAddr;
    use std::sync::Arc;
    use std::time::Duration;
    use team_demo_ssr::features::team::data::TeamStore;
    use team_demo_ssr::shell;

    // The shared, seeded, in-memory backend. 400ms latency + 2 forced transient
    // failures mirror the CSR demo's composition root — except here each retry
    // the controller's `RetryPolicy` issues is a *real HTTP round-trip*, and
    // the flakiness counter lives in this one shared store so it counts down
    // across those separate requests.
    let store = Arc::new(TeamStore::new(Duration::from_millis(400), 2));

    // This repo ships no cargo-leptos config, so build `LeptosOptions` directly
    // instead of via `get_configuration` (which would require `LEPTOS_*` env
    // vars). See the crate-level rustdoc for the full run story.
    let leptos_options = LeptosOptions::builder()
        .output_name("team-demo-ssr")
        .site_addr(SocketAddr::from(([127, 0, 0, 1], 3000)))
        .build();
    let addr = leptos_options.site_addr;

    // The context provider shared by BOTH the SSR renderer and the standalone
    // server-function handler — leptos_axum requires the same context in both,
    // since server functions are invoked by the renderer during SSR and by
    // direct client requests afterwards.
    let provide_ctx = {
        let store = Arc::clone(&store);
        let options = leptos_options.clone();
        move || {
            provide_context(Arc::clone(&store));
            provide_context(options.clone());
        }
    };

    let app = Router::<()>::new()
        .route(
            "/api/{*fn_name}",
            post({
                let provide_ctx = provide_ctx.clone();
                move |req: Request| handle_server_fns_with_context(provide_ctx.clone(), req)
            }),
        )
        .fallback(render_app_to_stream_with_context(provide_ctx, {
            let leptos_options = leptos_options.clone();
            move || shell(leptos_options.clone())
        }));

    let listener = tokio::net::TcpListener::bind(addr).await.unwrap();
    println!("team-demo-ssr listening on http://{addr}");
    axum::serve(listener, app.into_make_service())
        .await
        .unwrap();
}
