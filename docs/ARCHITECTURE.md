# clean-signals-rs — Architecture

## Overview

`clean-signals-rs` is a clean-architecture framework for Leptos 0.8 apps. It
gives use cases that return `Result`, and presentation-layer controllers that
orchestrate them with ref-counted loading, per-call retry, and
failures-as-events — built on Leptos's own reactive primitives
(`reactive_graph`), not a bespoke signals library.

## Workspace / Crate Structure

| Crate | Responsibility |
|-------|---------------|
| `crates/clean-signals` | Core framework: `Failure`, `UseCase`/`StreamUseCase`, `RetryPolicy`, `time::sleep`, `ActivityTracker`, `AsyncState`, `ControllerCore`. Reactivity-aware but DOM-free — depends only on `reactive_graph`, `any_spawner`, `async-trait`, `futures`. Never imports leptos. |
| `crates/clean-signals-leptos` | Leptos 0.8 integration: component-scoped controller lifecycle (`use_controller`), `AsyncView`, failure listening, an interval helper. The only crate permitted to depend on `leptos`. |
| `examples/team-demo` | CSR Leptos app exercising both crates end to end: a small team-roster feature slice. |

## Layer Dependencies

```
examples/team-demo
      └── crates/clean-signals-leptos
              └── crates/clean-signals
                      └── reactive_graph, any_spawner
```

Compile-time enforced: `clean-signals` has no `leptos` dependency in its
`Cargo.toml`, so any accidental leptos import fails to build. `cargo check -p
clean-signals --target wasm32-unknown-unknown` additionally proves the core
crate is portable to the browser target without pulling in DOM APIs.

## Core Abstractions

- **`Failure`** — trait every app-specific failure enum implements
  (`Debug + Display + Send + Sync + 'static`, `user_message()`,
  `is_retryable()`). Apps define one closed enum per app; there is no shared
  "unexpected failure" catch-all type (see Design Deltas).
- **`UseCase` / `StreamUseCase`** — the single-call and streaming execution
  contracts. `execute` returns `Result<Output, Failure>` directly (single-call)
  or a boxed stream of `Result` items (streaming). Async trait methods are
  `Send` on native targets and `?Send` on `wasm32` (single-threaded event
  loop) via a dual `cfg_attr`.
- **`RetryPolicy<F>`** — declarative, per-call retry: max attempts, delay,
  backoff factor, and an optional custom retry predicate (defaults to
  `Failure::is_retryable`). Pure policy math; the retry loop itself lives in
  `ControllerCore::run`.
- **`ActivityTracker`** — ref-counted "is something loading" state exposed as
  a `Memo<bool>`, incremented/decremented via an RAII `ActivityGuard` so
  overlapping operations don't cause loading-state flicker.
- **`AsyncState<T, F>`** — `Loading | Data(T) | Reloading(T) | Error{failure,
  stale}`, the signal-friendly shape controllers drive UI-bound state through.
  `Reloading` keeps the previous value visible during a refresh.
- **`ControllerCore<F: Failure + Clone>`** — the framework's centerpiece.
  Presentation-layer controllers embed one (composition, not inheritance) and
  drive their use cases exclusively through it:
  - `run` — execute a `UseCase`, apply retry, track activity, route the final
    failure (only) to the failure sink.
  - `run_into` — same, driving an `AsyncState` signal through
    Loading/Reloading → Data/Error.
  - `watch` — subscribe a `StreamUseCase`, forwarding items to a callback;
    aborted immediately on dispose.
  - `dispose` — idempotent; runs registered cleanups LIFO, aborts all watches,
    disposes the activity tracker; all framework signal writes after dispose
    are inert (see Data Flow).
- **`FailureSink<F>`** — dependency-free callback registry (no broadcast
  channel); `subscribe` returns an RAII `Subscription` that removes itself on
  drop.

## Data Flow

1. A repository (app-owned, outside this framework) performs I/O and maps
   transport errors to the app's `Failure` enum via `From` at the boundary.
2. A `UseCase`/`StreamUseCase` calls the repository and returns
   `Result<Output, Failure>` (or a stream of them) — no exceptions cross this
   line.
3. A controller's `ControllerCore` calls the use case through `run`/`run_into`/
   `watch`, which layers on activity tracking, retry, and failure routing.
4. `run_into` drives an `AsyncState<T, F>` signal that UI code reads
   reactively; `watch` drives an arbitrary callback per stream item.
5. Post-dispose: `ControllerCore` and the signals it owns use `try_set`/
   `try_update` so any write racing a teardown (an in-flight `.await`
   completing after `dispose()`) becomes a silent no-op instead of a panic.

## Design Rationale

- **App-defined failure types, not a shared catch-all.** Every framework type
  is generic over `F: Failure` rather than one built-in error type, so apps
  get a single closed, exhaustively-matchable `Failure` enum instead of
  falling back to string-matching an "unexpected failure" variant.
- **`std::result::Result` everywhere.** No custom result/success/failure
  union type; `ResultExt::to_async_state()` bridges a `Result` straight into
  `AsyncState`.
- **Composition, not inheritance, for controllers.** App controllers *embed*
  a `ControllerCore<F>` field; Rust has no base-class mechanism for this
  shape.
- **`Memo`-based core, `Effect` kept out.** `reactive_graph::Effect` requires
  the `effects` feature plus a `LocalSet`-driven executor; core uses `Memo`
  and explicit methods so controllers stay testable without that feature.
  Effect-style render glue lives in `clean-signals-leptos` instead.
- **Failure fan-out without a channel dependency.** `FailureSink` (see Core
  Abstractions) is a plain callback registry, not a broadcast channel —
  keeps the core crate free of an async-channel dependency.
- **Cancellation is drop-based, not token-based.** `watch`'s abort happens
  when its `WatchHandle` (or the owning `ControllerCore`) is dropped — no
  separate cancellation-token type to thread through call sites.

## Key Types

| Type | Purpose |
|------|---------|
| `Failure` (trait) | App failure contract: message + retryability |
| `UseCase` / `StreamUseCase` (traits) | Single-call / streaming execution contract |
| `RetryPolicy<F>` | Declarative per-call retry policy |
| `ActivityTracker` / `ActivityGuard` | Ref-counted loading state |
| `AsyncState<T, F>` | UI-bound async data enum |
| `ControllerCore<F>` | Owns activity, failures, cleanups; exposes `run`/`run_into`/`watch`/`dispose` |
| `FailureSink<F>` / `Subscription` | Failure event fan-out with RAII unsubscribe |
| `RunOptions<F>` | Per-call knobs: retry policy, activity tracking, failure emission |

See `docs/CODE_STANDARDS.md` for conventions and `docs/DEVELOPMENT.md` for
build/test commands.
