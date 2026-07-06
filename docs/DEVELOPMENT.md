# clean-signals-rs — Development Guide

## Prerequisites

- Rust (stable toolchain, edition 2024) with the `wasm32-unknown-unknown`
  target:
  ```sh
  rustup target add wasm32-unknown-unknown
  ```
- `trunk` if you want to serve `examples/team-demo` in a browser:
  ```sh
  cargo install trunk
  ```

## Build

```sh
# Whole workspace, native
cargo build --workspace

# Core crate only, checked against the wasm target (no leptos/DOM allowed)
cargo check -p clean-signals --target wasm32-unknown-unknown

# Leptos integration crate, checked against wasm too
cargo check -p clean-signals-leptos --target wasm32-unknown-unknown
```

## Test

```sh
# Whole workspace
cargo test --workspace

# One crate
cargo test -p clean-signals

# Core crate's tests plus its shared fixtures (fake use cases/failures) used
# by the controller/leptos test suites
cargo test -p clean-signals --features test-fixtures

# The reactive_graph/any_spawner environment-assumption suite
# (kept permanently — re-run if you bump reactive_graph/any_spawner versions)
cargo test -p clean-signals --test spike_reactive_graph
```

## Lint

```sh
cargo clippy --workspace --all-targets -- -D warnings

# test-fixtures feature must also be clippy-clean (it's real code, not test-only)
cargo clippy -p clean-signals --all-targets --features test-fixtures -- -D warnings
```

## Definition-of-done commands

Run all of these before declaring any task in this repo complete:

```sh
cargo build --workspace
cargo test --workspace
cargo clippy --workspace --all-targets -- -D warnings
cargo check -p clean-signals --target wasm32-unknown-unknown
```

## Run the example apps

### `team-demo` (CSR)

```sh
cd examples/team-demo
trunk serve --open
```

This is a CSR (client-side-rendered) Leptos app; there is no backend to run
alongside it.

### `team-demo-ssr` (SSR + hydrate)

This example ships no `cargo-leptos` config, so it is verified by compiling
both feature/target combinations plus native tests, rather than served:

```sh
cargo check -p team-demo-ssr --features ssr
cargo check -p team-demo-ssr --features hydrate --target wasm32-unknown-unknown
cargo test  -p team-demo-ssr --features ssr
```

See `examples/team-demo-ssr/src/lib.rs` rustdoc for what a full
`cargo-leptos` setup (`cargo install cargo-leptos`, a
`[package.metadata.leptos]` table, `cargo leptos watch`) would add to serve
it end to end.

## The `effects` feature gotcha

`reactive_graph::Effect::new` **silently never runs** unless the `effects`
feature is enabled on `reactive_graph` for whoever is running the code. In
this workspace:
- `clean-signals`'s own dev-dependencies enable it (needed for its test
  suite), but the *library* code in `clean-signals` does not use `Effect` at
  all — controllers are testable without the feature.
- An app pulling in `leptos` with `csr`/`hydrate` gets `effects` transitively
  from leptos, so this only bites you in a native test or tool that talks to
  `reactive_graph` directly without going through leptos or without adding
  `features = ["effects"]` itself.

If an `Effect` you created appears to do nothing, check for this first
before suspecting a logic bug.

## Driving `watch`/`spawn_local` in tests

`any_spawner::Executor::spawn_local` (what `ControllerCore::watch` uses)
panics if called outside a `tokio::task::LocalSet` on a current-thread
runtime. Copy this recipe verbatim for any test exercising `watch`:

```rust
#[tokio::test(flavor = "current_thread")]
async fn my_watch_test() {
    tokio::task::LocalSet::new().run_until(async {
        any_spawner::Executor::init_tokio().ok(); // idempotent init
        // ... construct controller, call watch(...) ...
        any_spawner::Executor::tick().await; // deterministic pump — call once
                                              // per expected async step, never sleep
    }).await;
}
```

`Executor::init_tokio()` is process-wide and first-writer-wins — always call
it with `.ok()`, never assert it returns `Ok` (an earlier test in the same
binary may have already set it).

## Workflow locations

- Feature plans and task breakdowns: `workflow/plans/feature/clean-signals-rs/`
  (`PLAN.md`, `TASKS.md`, `tasks/*.md`), with spike/research findings under
  its `research/` subdirectory.
- The downstream project template: `templates/AGENTS.md`.

## Troubleshooting

### `spawn_local` panics in a test
Missing the `LocalSet` + current-thread runtime — see the recipe above. A
plain `#[tokio::test]` (multi-thread flavor, no `LocalSet`) always panics on
`spawn_local`.

### An `Effect` never seems to fire
See "The `effects` feature gotcha" above; also confirm you called
`Executor::tick().await` at least once after creating it.

### `cargo check --target wasm32-unknown-unknown` fails on `clean-signals`
A leptos/DOM dependency leaked into the core crate — a layer-boundary bug,
not a target-support bug. Check `Cargo.toml` and any new leptos `use`.
