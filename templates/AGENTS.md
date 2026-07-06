# Agent Instructions — clean-signals Clean Architecture (Rust/Leptos)

> Copy this file into the root of every project built on
> [clean-signals-rs](https://github.com/edstub/clean-signals-rs) as
> `AGENTS.md` (and/or `CLAUDE.md` for Claude Code). Fill in the
> `<project-specific>` sections at the bottom. The reference implementation
> for every rule here is `examples/team-demo` in the clean-signals-rs repo —
> when in doubt, imitate it.

This project uses **clean-signals + Leptos 0.8** with strict, feature-sliced
clean architecture. These rules are not suggestions; a change that violates
them is wrong even if it compiles and passes tests.

## The dependency rule

Code is organized as feature slices, each with three layers. Dependencies
point inward only:

```
presentation ──▶ domain ◀── data
```

| Layer | MAY import | MUST NOT import |
| --- | --- | --- |
| `domain/` | `clean_signals`, other files in the same feature's domain, the crate's shared failure module | `leptos`, `reactive_graph`, any transport crate (`reqwest`, `tonic`, a DB driver), `data/`, `presentation/` |
| `data/` | `clean_signals`, own feature's `domain/`, shared infra (failure mapping, HTTP/DB clients), transport crates | `leptos`, `reactive_graph`, `presentation/`, other features' `data/` |
| `presentation/` | `clean_signals`, `clean-signals-leptos`, `leptos`, `reactive_graph`, own feature's `domain/`, other features' `presentation/` components | any `data/` file, transport crates, wire-format DTOs |

Cross-feature communication happens through presentation (controllers/
components) or a domain trait provided via DI — never by importing another
feature's `data/` module directly.

## Feature layout

Small apps: `src/features/<feature>/{domain,data,presentation}/` modules in
one crate. Larger apps: crate-per-layer per feature (`<feature>-domain`,
`<feature>-data`, `<feature>-presentation`) so the dependency rule is
compiler-enforced instead of convention-enforced — prefer this once a
feature's domain layer needs to be reused by more than one presentation
target (e.g. CSR + SSR).

```
src/features/<feature>/
├── domain/
│   ├── entities.rs                  # plain structs: Clone, PartialEq, no wire format
│   ├── repositories.rs              # trait(s), return domain types + Result<_, F>
│   └── use_cases/<verb_object>.rs   # one use case per file
├── data/
│   ├── sources.rs                   # transport client(s)
│   ├── models.rs                    # wire-format DTOs, `to_entity()` / `TryFrom`
│   └── repositories.rs              # repository trait impl(s)
└── presentation/
    ├── controllers.rs               # embeds ControllerCore<F>
    ├── pages.rs                     # top-level route components
    └── components.rs
```

## Domain rules

- Entities are plain Rust structs: owned fields, `Clone`, `PartialEq`. No
  serde derives, no framework types.
- Repository traits return domain types and `Result<T, F>` where `F` is the
  app's `Failure` type — never a transport error, never a DTO.
- Every business operation is a use case: `struct GetX; impl UseCase for
  GetX`. One use case per file, named `<verb>_<object>.rs` /
  `<VerbObject>` (e.g. `get_members.rs` → `GetMembers`). Never call
  `execute` directly from outside a `ControllerCore` — see Controller rules.
- Multi-value params are a small named struct per use case, not a tuple:
  `struct UpdateNameParams { pub id: String, pub name: String }`.
- Input validation lives in the use case and returns
  `Err(F::from(ValidationFailure))` — not in the controller, not in the view.
- Use `NoParams` for parameterless use cases.

## Data rules

- DTOs own (de)serialization and expose a conversion to the domain entity
  (`impl From<MemberDto> for Member` or `TryFrom` if conversion is fallible).
  DTOs never leave the data layer.
- Each repository impl maps transport errors to the app's `Failure` type at
  **exactly one** conversion site (a `From<TransportError> for AppFailure`
  impl, or a single private mapping function the impl calls) — no raw
  transport error may cross the repository boundary.
- Transient errors (timeouts, 5xx, dropped connections) map to a failure with
  `is_retryable() == true`; permanent ones (404, validation) do not.

## Controller rules

- Every screen/component's state lives in a controller struct that
  **embeds** `ControllerCore<F>` as a field (composition, not inheritance —
  Rust has no base class to extend here).
- Async data lives in an `AsyncState<T, F>` signal created via
  `async_state_signal()`, driven by `run_into(...)`. Derived state is a
  `Memo`. Keep signals fine-grained so components rebuild only for what they
  read.
- **Never call a use case directly** (`self.get_members.execute(...)`) from a
  controller method — always go through `core.run`, `core.run_into`, or
  `core.watch`. Calling `execute` directly loses activity tracking, failure
  routing, and retries.
- Retries are declared per call via `RunOptions { retry: RetryPolicy { .. },
  .. }`. Never hand-roll a retry loop.
- Register every owned cleanup with `core.on_dispose(...)`; `watch` handles
  auto-cancel on `core.dispose()`.
- Controllers never import `leptos::prelude::*` render types (`view!`,
  `IntoView`) and never touch DOM/browser APIs — that belongs to
  `presentation::pages`/`components`. A controller may depend on
  `reactive_graph` signal types and on `clean-signals-leptos` hooks that
  construct/dispose it, nothing render-specific.

## DI conventions

- Constructor injection: a controller/use case takes its dependencies as
  constructor parameters, never looks them up itself.
- **Page-scoped controllers** (state that lives and dies with one route):
  create via `clean_signals_leptos::use_controller(|| MyController::new(...))`
  inside the page component. Disposed automatically on the component's
  `on_cleanup`.
- **App-scoped controllers** (session, connectivity, settings — outlive any
  one page): provide once near the app root via
  `clean_signals_leptos::provide_controller(controller)`, look up with
  `expect_controller::<T>()`. Pages that look these up **never** dispose
  them.
- Fakes/test doubles are passed as constructor arguments, so tests can swap
  in fake repositories without touching the composition root.

## Testing rules

- Controllers are tested **natively** (no DOM) against fake repositories —
  construct the controller directly, drive it with `futures::executor::block_on`
  or `#[tokio::test]`, assert on signal values via `get_untracked()`.
- Every use case gets a test for its failure path, not just its happy path.
- Anything exercising `watch`/`spawn_local` needs the current-thread +
  `LocalSet` recipe from `clean-signals-rs`'s `docs/CODE_STANDARDS.md` — a
  plain `#[tokio::test]` panics on `spawn_local`.
- Never sleep to wait for async work in a test; use
  `any_spawner::Executor::tick().await` as the deterministic pump.

## Failures

- The app defines one closed `Failure` enum (or a small tree of them) per
  failure domain, implementing the `clean_signals::Failure` trait, with a
  `user_message()` override per variant where the default `Display` isn't
  user-facing. UI matches over it exhaustively; never string-match on error
  text.
- There is no catch-all "unexpected failure" variant — a transport error
  that doesn't fit an existing variant gets its own typed variant, mapped via
  `From` at the repository boundary.

## Forbidden

- Any other state-management layer alongside `clean_signals` /
  `reactive_graph` signals (no Redux-style store, no separate observable
  library).
- `unwrap()`/`expect()` on a `Result<_, F>` outside tests and fixtures —
  route it through `run`/`run_into`/`watch` and let the failure sink/
  `AsyncState::Error` handle it.
- Business logic in components or controllers (belongs in use cases).
- API/database calls from a controller (belongs in `data/`, behind a
  repository trait).
- Manual `.set()`/`.update()` on a signal a controller doesn't exclusively
  own for that call — see `try_*` write rule in `docs/CODE_STANDARDS.md`.
- Global mutable state outside a DI-provided controller.

## Definition of done

Before declaring any task complete, run and pass ALL of:

```sh
cargo build --workspace
cargo test --workspace
cargo clippy --workspace --all-targets -- -D warnings
cargo check -p <core-crate> --target wasm32-unknown-unknown
```

and verify: no layer-rule violations in the imports you added, new public
items documented, failures typed, every use case's failure path tested.

---

## Project specifics (fill in per project)

- **App name / purpose:** `<project-specific>`
- **Features:** `<list feature slices and one-line responsibilities>`
- **Backend / transport:** `<http client, grpc, local db, ...>`
- **App-scoped controllers:** `<e.g. SessionController, ConnectivityController>`
- **Run / build commands:** `<trunk serve, cargo leptos watch, etc.>`
- **Deviations from these rules (with justification):** none
