# clean-signals-rs — Code Standards

## Generic-over-`F` failures

Every framework type that touches app errors is generic over `F: Failure +
Clone` rather than a concrete error type — this is how one core crate serves
every downstream app's failure enum. Follow the same pattern in app code:
define one closed `Failure` enum per app and thread it through your own
use cases/controllers as `F`, matching exhaustively rather than
string-matching on messages.

```rust
#[derive(Clone, Debug)]
enum AppFailure { Network(String), Validation(String) }
impl Failure for AppFailure {
    fn is_retryable(&self) -> bool { matches!(self, AppFailure::Network(_)) }
}
```

Map transport/IO errors to your `Failure` enum via `From` impls at the
repository boundary — never let a raw transport error type appear in a use
case or controller signature.

## `try_*` signal writes in framework code

Any write to a signal that a controller (or the type wrapping it) does not
exclusively own for its own stack frame — i.e. any write that could race a
`dispose()` — must use `try_set`/`try_update`, never the infallible
`set`/`update`. `try_update` returns `None` on a disposed target instead of
panicking or logging; treat that as "silently do nothing", not an error to
propagate.

```rust
// BAD: panics/warns if `into` was disposed while this await was in flight
into.set(AsyncState::Data(value));

// GOOD: post-dispose write is an inert no-op
into.try_set(AsyncState::Data(value));
```

RAII `Drop` guards (e.g. an activity guard) follow the same rule: decrement
via `try_update`, and additionally check any owner-level disposed flag before
touching the signal at all.

## No sleeps outside `time::sleep`

Never call `tokio::time::sleep`, `gloo_timers` directly, or
`std::thread::sleep` in framework or app code — always go through
`clean_signals::time::sleep`, cfg-gated to the right backend per target
(`tokio::time` native, `gloo-timers` wasm). This keeps library code portable
to `wasm32`. In tests, never sleep to wait for async work to settle — drive
it deterministically instead (see Testing Patterns).

## `cfg_attr` async_trait pattern

Every async trait in this codebase (`UseCase` and any trait with async
methods) uses the same dual attribute, because `wasm32`'s single-threaded
event loop can't require `Send` futures. Use the `clean_signals::async_trait`
re-export rather than a direct `async-trait` dependency, so downstream crates
never need to pin their own:

```rust
#[cfg_attr(not(target_arch = "wasm32"), clean_signals::async_trait)]
#[cfg_attr(target_arch = "wasm32", clean_signals::async_trait(?Send))]
pub trait UseCase { /* ... */ }
```

Apply the identical pair of attributes to every `impl` block for such a
trait — a mismatch compiles on one target only, easy to miss if you don't
also check wasm.

## Ambient-Owner contract for `use_*` hooks (clean-signals-forgekit)

Every `use_*` helper in `clean-signals-forgekit` that calls `on_cleanup`,
`provide_context`, or `use_context` depends on a reactive `Owner` being
ambient, which is only true inside `Component::init` (or another owner
scope). Outside one, `on_cleanup` silently no-ops instead of erroring — a
controller, subscription, or interval leaks with no panic to catch it. New
hooks in this crate must state this contract explicitly in their own rustdoc
(an `# Ambient-Owner contract` heading), not just rely on the crate-level doc
comment — see `hooks.rs`/`interval.rs`/`failure_listener.rs` for the pattern.

## Naming conventions

| Element | Convention | Example |
|---------|-----------|---------|
| Use case struct | `<Verb><Object>`, one per file downstream | `GetMembers`, `UpdateMemberName` |
| Controller struct | `<Screen>Controller`, embeds `ControllerCore<F>` as a field | `MembersController` |
| Multi-value params | A small named struct, not a tuple | `struct UpdateNameParams { id: String, name: String }` |
| Parameterless use case | `NoParams` | `uc.execute(NoParams)` |
| Fixture types | `test-fixtures` feature, under a `fixtures` submodule | `crate::failure::fixtures::NetworkFailure` |

## `test-fixtures` feature

`clean-signals` gates its fake use cases and fixture failures (`Doubler`,
`Flaky`, `Slow`, `Ticker`, `NetworkFailure`, `ValidationFailure`, ...) behind
`#[cfg(any(test, feature = "test-fixtures"))]`. Downstream crates that need
them in their own tests enable the feature as a dev-dependency:

```toml
[dev-dependencies]
clean-signals = { workspace = true, features = ["test-fixtures"] }
```

Never duplicate these fixtures downstream — extend the shared `pub mod
fixtures` in `crates/clean-signals` if a new one is needed by more than one
test module.

## Anti-patterns

**BAD** — calling a use case directly from a controller method, bypassing
`ControllerCore` (loses retry, activity tracking, failure routing):
```rust
let result = self.get_members.execute(NoParams).await;
```

**GOOD** — always through `run`/`run_into`/`watch`:
```rust
let _ = self.core.run_into(&self.get_members, NoParams, self.members, RunOptions::default()).await;
```

**BAD** — a shared "catch-all" failure variant instead of a typed one per
error mode: `enum AppFailure { Unexpected(String) }`.

## Testing patterns

- Controllers and use cases are tested natively (no DOM); every use case
  gets at least a happy-path and a failure-path test.
- Never sleep to wait for async work to settle in a test; drive it
  deterministically via `Executor::tick().await` instead (once per expected
  async step).
- Anything that drives `watch`/`spawn_local` needs a current-thread runtime +
  `LocalSet` — a plain `#[tokio::test]` panics on `spawn_local`. See
  `docs/DEVELOPMENT.md` for the exact recipe and the commands that run these
  tests.
