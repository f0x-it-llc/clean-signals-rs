# clean-signals

A clean-architecture framework for [Leptos](https://leptos.dev) apps. Business
logic lives in small, testable **use cases** that return `Result`; presentation
state lives in **controllers** that orchestrate them with ref-counted loading,
per-call retry, and failures-as-events — all built on Leptos's own reactive
primitives (`reactive_graph`), so there is no second signals system to bridge.

```
your app (CSR or SSR — the architecture is identical)
   └── clean-signals-leptos     use_controller · AsyncView · use_failure_listener · use_interval
          └── clean-signals     Failure · UseCase · RetryPolicy · AsyncState · ControllerCore
                 └── reactive_graph, any_spawner        (no DOM, no leptos)
```

The layering is compile-time enforced: the core crate has no `leptos`
dependency, and everything below the page components is testable in plain
`#[tokio::test]`s — no browser, no DOM.

## What it gives you

- **Use cases** — one struct per business operation, `async fn execute(params)
  -> Result<Output, Failure>`. Dependencies injected as `Arc<dyn Repository>`;
  the compiler is the registry.
- **Failures as data** — you define one closed `enum AppFailure` implementing
  the `Failure` trait (`user_message()`, `is_retryable()`). Transport errors
  convert into it at exactly one boundary per repository. No exceptions, no
  string matching, exhaustive `match` in the UI.
- **Controllers** — view models embedding a `ControllerCore<F>` that runs use
  cases through `run` / `run_into` / `watch`: ref-counted `is_loading`,
  declarative `RetryPolicy` (only the final failure is ever emitted), an
  `AsyncState` that keeps stale data visible during refetches, and idempotent
  disposal that aborts in-flight watches.
- **Leptos glue** — `use_controller` ties a controller's lifetime to the
  component (disposed on unmount), `AsyncView` renders an
  `AsyncState<T, F>` signal declaratively, `use_failure_listener` surfaces
  failure events (snackbars, logging), `use_interval` polls safely and stops
  on unmount.

## Quick start

Define a failure type and a use case (the `async_trait` re-export handles the
native/wasm `Send` split):

```rust
use clean_signals::{Failure, UseCase, NoParams};
use std::{fmt, sync::Arc};

#[derive(Clone, Debug, PartialEq)]
enum TeamFailure { Network(String), Validation(String) }

impl fmt::Display for TeamFailure {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            TeamFailure::Network(msg) | TeamFailure::Validation(msg) => f.write_str(msg),
        }
    }
}
impl Failure for TeamFailure {
    fn is_retryable(&self) -> bool { matches!(self, TeamFailure::Network(_)) }
}

struct LoadTeam { repo: Arc<dyn TeamRepository + Send + Sync> }

#[cfg_attr(not(target_arch = "wasm32"), clean_signals::async_trait)]
#[cfg_attr(target_arch = "wasm32", clean_signals::async_trait(?Send))]
impl UseCase for LoadTeam {
    type Params = NoParams;
    type Output = Vec<Member>;
    type Failure = TeamFailure;
    async fn execute(&self, _: NoParams) -> Result<Vec<Member>, TeamFailure> {
        self.repo.load_team().await
    }
}
```

Drive it from a controller, and wire the controller to a page:

```rust
use clean_signals::{async_state_signal, AsyncState, ControllerCore, RetryPolicy, RunOptions};
use clean_signals_leptos::{use_controller, use_failure_listener, AsyncView};
use leptos::prelude::*;
use std::time::Duration;

struct TeamController {
    core: ControllerCore<TeamFailure>,
    members: RwSignal<AsyncState<Vec<Member>, TeamFailure>>,
    load_team: LoadTeam,
}

impl TeamController {
    async fn load(&self) {
        let opts = RunOptions {
            retry: RetryPolicy::new(3, Duration::from_millis(200)).with_backoff(2.0),
            ..Default::default()
        };
        let _ = self.core.run_into(&self.load_team, NoParams, self.members, opts).await;
    }
}

// `use_controller` needs a way to reach the core for disposal:
impl AsRef<ControllerCore<TeamFailure>> for TeamController {
    fn as_ref(&self) -> &ControllerCore<TeamFailure> { &self.core }
}

#[component]
fn TeamPage() -> impl IntoView {
    // Built once; disposed automatically when the component unmounts.
    let controller = use_controller::<TeamController, TeamFailure>(|| TeamController::new(/* repo */));
    controller.with_value(|c| {
        use_failure_listener(c.core.failures(), |f| show_snackbar(f.user_message()));
    });
    let members = controller.with_value(|c| c.members);
    view! { <AsyncView state=members children=|team| view! { <TeamList team/> }.into_any()/> }
}
```

Controllers are tested natively with hand-written fake repositories — retry
absorption, loading transitions, stale-data reloads, and failure routing all
assert without a browser. See `docs/DEVELOPMENT.md` for the two-line
tokio/`Owner` test recipe.

## ForgeKit

[`clean-signals-forgekit`](crates/clean-signals-forgekit) wires the same
framework to [ForgeKit](https://github.com/f0x-it-llc/forgekit) instead of
Leptos: `use_controller`, `provide_controller`/`expect_controller`,
`use_failure_listener`, `async_view`, `use_interval`.

```rust,ignore
use clean_signals_forgekit::{use_controller, use_failure_listener, async_view};
use forgekit::{AnyView, Component, any, text};
use std::sync::Arc;

impl Component for InboxScreen {
    type State = Arc<InboxController>;

    fn init(&self) -> Arc<InboxController> {
        // An Owner is ambient here (Component::init), so disposal binds to
        // this component's teardown.
        let controller = use_controller::<InboxController, AppFailure>(InboxController::new);
        use_failure_listener(controller.core().failures(), |f: AppFailure| {
            log::error!("{}", f.user_message());
        });
        controller
    }

    fn build(&self, state: &mut Arc<InboxController>) -> AnyView<Arc<InboxController>> {
        // ForgeKit re-runs the whole `build` on change (coarse-grained
        // reactivity, unlike Leptos's fine-grained fragments), so `async_view`
        // is a plain snapshot match over an already-tracked `.get()`.
        async_view(
            state.messages.get(),
            || any(text("Loading…")),
            |messages| any(text(format!("{} messages", messages.len()))),
            |failure: AppFailure| any(text(failure.user_message())),
        )
    }
}
```

`clean-signals-forgekit` is a **standalone package**, not a workspace member:
it path-depends on an unpublished ForgeKit sibling checkout, so it only builds
from a checkout with `../forgekit` next to `../clean-signals-rs`. See
`docs/ARCHITECTURE.md` and `docs/DEVELOPMENT.md` for the gate commands.

## Examples

Both examples implement the **same** team-roster feature slice — the diff
between them is the point:

| Example | Mode | Run |
|---------|------|-----|
| [`examples/team-demo`](examples/team-demo) | CSR | `cd examples/team-demo && trunk serve --open` |
| [`examples/team-demo-ssr`](examples/team-demo-ssr) | SSR + hydrate (axum) | compile/test-verified: `cargo test -p team-demo-ssr --features ssr` (see its rustdoc) |

The domain and presentation layers are byte-identical across the two (one
wasm-gated line excepted); only the composition roots and the repository
transport (Leptos server functions in the SSR app) differ. That is the
framework's claim about render modes, demonstrated: same architecture, CSR or
SSR.

## Adopting it in your app

Copy [`templates/AGENTS.md`](templates/AGENTS.md) into your repo — it carries
the enforcement half of the framework: the per-layer import rules, feature-slice
layout, naming conventions, repository/controller/testing rules, and the
definition-of-done commands.

Further reading: [`docs/ARCHITECTURE.md`](docs/ARCHITECTURE.md) (crate map,
core abstractions, design rationale, SSR applications),
[`docs/CODE_STANDARDS.md`](docs/CODE_STANDARDS.md),
[`docs/DEVELOPMENT.md`](docs/DEVELOPMENT.md).

## Status

Early (0.1.x): APIs may still move, and the crates are not yet published to
crates.io — use a git dependency. The test suite pins the behavioral contract
(100+ tests across the workspace), and `tests/spike_reactive_graph.rs` acts as
a tripwire for upstream `reactive_graph` changes.

## License

MIT
