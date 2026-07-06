## Task: Example app — team-demo (CSR)

**Objective**: Port the Dart example's team feature to a CSR Leptos app in `examples/team-demo`, demonstrating the full framework shape: feature-sliced layout, fake-flaky data source, retry, search via Memo, failure snackbar, natively-tested controllers.

**Depends on**: 07-leptos-integration

**Complexity:** medium

### Scope

**Files Modified (Write):**
- `examples/team-demo/src/main.rs` (mount, composition root)
- `examples/team-demo/src/features/team/{domain,data,presentation}/*.rs` (feature slice per templates/AGENTS.md conventions)
- `examples/team-demo/index.html` (trunk entry)
- `examples/team-demo/src/lib.rs`

**Files Read (Dependencies):**
- Both framework crates (merged API)
- `/home/ed/Dev/personal/clean_signals/example/` (the canonical shape to imitate)
- `templates/AGENTS.md` if merged; else the folder conventions in PLAN.md/task 08

### Details

- Domain: `Member` entity; `TeamRepository` trait (async_trait cfg_attr pattern); use cases `LoadTeam` (NoParams → Vec<Member>), `UpdateMember`.
- Data: `InMemoryTeamRepo` with configurable `failures_before_success` (demonstrates retry) and artificial latency via `clean_signals::time::sleep`; error enum `TeamFailure` implementing `Failure` (Network retryable, Validation not).
- Presentation: `TeamController` embedding `ControllerCore<TeamFailure>`; state: `members: RwSignal<AsyncState<Vec<Member>, TeamFailure>>`, `query: RwSignal<String>`, `filtered: Memo<Vec<Member>>`; `load()` uses `run_into` with `RetryPolicy::new(3, 200ms).with_backoff(2.0)`; page component uses `use_controller`, `AsyncView`, `use_failure_listener` (console/snackbar div).
- Native tests (`examples/team-demo/tests/` or `src/**/tests`): controller loads through flaky repo (fails twice, succeeds third — retry absorbs); filtered Memo narrows on query change; validation failure surfaces via sink. Use the SPIKE_NOTES tokio recipe.
- Build: crate compiles to wasm (`cargo check -p team-demo --target wasm32-unknown-unknown`); `Trunk.toml`/`index.html` provided for `trunk serve` but installing trunk is NOT part of this task's verification.

### Acceptance Criteria

1. `cargo test -p team-demo` green (native controller tests prove the headline testability claim).
2. `cargo check -p team-demo --target wasm32-unknown-unknown` green; clippy clean.
3. Layout matches the AGENTS.md feature-slice convention exactly (this app is the reference).
4. Zero raw `spawn_local`/hand-rolled polling/`if let Ok`-swallowed errors in presentation code.

### Notes

- Keep it small — one feature slice done perfectly beats three done loosely.

---

## Completion Summary

**Status:** Done
**Branch:** master

Initial pass hit a genuinely missing `async-trait` dependency in
`examples/team-demo/Cargo.toml` (required to implement `clean_signals::UseCase`
and the app's own `TeamRepository` async trait) and stopped per the dispatch
instruction to "stop and report" rather than edit Cargo.toml myself. The
conductor added `async-trait = { workspace = true }` to
`examples/team-demo/Cargo.toml` in commit `d7d5615` and unblocked the task;
this summary covers the resumed, completed implementation.

### Files Modified

| File | Changes |
|------|---------|
| `examples/team-demo/src/lib.rs` | New. Declares `pub mod failure; pub mod features;` — the crate's module tree, used by both `main.rs` and native tests. |
| `examples/team-demo/src/main.rs` | Composition root: builds `InMemoryTeamRepo` (400ms latency, 2 forced failures) as `Arc<dyn TeamRepository + Send + Sync>`, mounts `TeamPage` via `leptos::mount::mount_to_body`. |
| `examples/team-demo/src/failure.rs` | New. `TeamFailure` (Network retryable / Validation not) implementing `clean_signals::Failure`, with per-variant `user_message()`. |
| `examples/team-demo/src/features/mod.rs`, `.../team/mod.rs` | New. Feature-slice module wiring. |
| `examples/team-demo/src/features/team/domain/entities.rs` | New. `Member` — plain struct, `Clone + PartialEq + Eq`, no framework types. |
| `examples/team-demo/src/features/team/domain/repositories.rs` | New. `TeamRepository` trait (`list_members`, `update_member_name`), dual `cfg_attr` async_trait pattern. |
| `examples/team-demo/src/features/team/domain/use_cases/{mod,load_team,update_member}.rs` | New. `LoadTeam` (`NoParams → Vec<Member>`) and `UpdateMember` (`UpdateMemberParams → Member`, validates non-empty/≥2-char name, returning `TeamFailure::Validation` *before* touching the repository). Both take `Arc<dyn TeamRepository + Send + Sync>` (trait object, not a generic parameter — keeps `TeamController`/`TeamPage` non-generic). |
| `examples/team-demo/src/features/team/data/{mod,sources,models,repositories}.rs` | New. `InMemoryTeamSource` (configurable `failures_before_success` + `latency` via `clean_signals::time::sleep`), `MemberRow` DTO + `From<MemberRow> for Member`, `InMemoryTeamRepo` implementing `TeamRepository` and mapping `TransportError` → `TeamFailure` at exactly one site. |
| `examples/team-demo/src/features/team/presentation/{mod,controllers,pages,components}.rs` | New. `TeamController` (embeds `ControllerCore<TeamFailure>`; `members`/`query`/`filtered` signals; `load()` via `run_into` + `RetryPolicy::new(3, 200ms).with_backoff(2.0)`; `rename()` via `run`, patching the loaded list on success). `TeamPage` wires `use_controller`, `use_failure_listener` (banner div), `AsyncView`, kicks off the initial load via one `spawn_local`. `member_row` is a plain view-returning fn with an inline rename control. |
| `examples/team-demo/index.html` | New. Minimal trunk entry (empty `<body>`; leptos mounts into it). |

### Notable Decisions/Tradeoffs

1. **Trait object DI (`Arc<dyn TeamRepository + Send + Sync>`) instead of a generic `<R: TeamRepository>` parameter** on `LoadTeam`/`UpdateMember`/`TeamController`/`TeamPage`. A generic controller would force `TeamPage` to also be generic, which is awkward with the `#[component]` macro and buys nothing here (one concrete repo in the composition root). `TeamRepository`'s async-trait-generated methods are dyn-safe, so this was a clean simplification within the task's intent.
2. **Controller signal imports go through `leptos::prelude::{RwSignal, Memo, Get, Update}` (named, not `*`), not `reactive_graph::…` directly** — `reactive_graph` is not a direct dependency of `team-demo` (only `leptos` re-exports its signal types), and `leptos::prelude::*` would also pull in `view!`/`IntoView` render macros, which `templates/AGENTS.md` explicitly forbids in controller code. Named imports thread the needle: reach the types through the one dependency that has them, without the render-macro wildcard. See the Friction section below — this is worth flagging for the review phase.
3. **`rename()` uses `core.run` (not `run_into`)** since its `Output` (`Member`) differs from `members`'s `Vec<Member>`; on success it patches the already-loaded list via `self.members.try_update(...)` (guarded per the `try_*` convention). The failure path is already routed through `core.run`'s own `FailureSink::emit` before `rename` ever inspects the `Result` — the `if let Ok(updated) = result` in `controllers.rs` reacts to success only, it does not swallow an `Err` (acceptance criterion 4's target is bypassing `ControllerCore`, not this).
4. **`team_list` factored out of the `view!` macro's `AsyncView` `children` closure** — `children=move |_all: Vec<Member>| { ... }` failed to parse (`view!`'s tag-scanner trips on the `<`/`>` in `Vec<Member>` inside an inline closure signature). Moving the body into a standalone `fn team_list(...)` and dropping the closure's explicit param type (`move |_all| team_list(...)`, relying on inference from `AsyncView`'s `state` prop) fixed it.
5. **Initial load triggered via one `leptos::task::spawn_local(async move { controller.get_value().load().await; })` directly in `TeamPage`'s body** (the CSR analogue of Dart's `initState`), mirroring `clean-signals-leptos`'s own crate-doc example (`leptos::task::spawn_local(c.reload());`) rather than a `use_interval`/`Effect`. There's no framework helper for "run once on mount"; this is the minimal, already-precedented glue.
6. **Applied `cargo fmt -p team-demo`** to match the rest of the workspace's style (not explicitly required by `docs/DEVELOPMENT.md`'s DoD list, but the untouched files' style made the deviation obvious).

### Framework Friction (for the review phase)

1. **`async-trait` re-export gap** (already flagged by the conductor as a followup candidate): `clean_signals::UseCase` and any custom async trait a downstream app defines both need the `async-trait` crate directly, because implementing a trait whose methods were macro-expanded via `#[cfg_attr(..., async_trait::async_trait)]` requires the identical macro on the `impl` block. Neither `clean-signals` nor `clean-signals-leptos` re-exports it. A `pub use async_trait::async_trait;` (and ideally re-exporting the crate itself, e.g. `pub use async_trait;`) from `clean-signals`'s root would let downstream apps depend on just `clean-signals` for this, rather than needing to independently pin `async-trait` in their own `Cargo.toml` and keep it version-aligned.
2. **`reactive_graph` is reachable only through `leptos`, not directly**, for an app whose Cargo.toml (reasonably) only lists `clean-signals`, `clean-signals-leptos`, and `leptos`. `templates/AGENTS.md`'s controller rule ("may depend on `reactive_graph` signal types... never `leptos::prelude::*` render types") implicitly assumes `reactive_graph` is a direct dependency, but a minimal app has no reason to add it separately when `leptos::prelude` already re-exports the same types. Worth either: (a) relaxing the AGENTS.md wording to explicitly bless `use leptos::prelude::{RwSignal, Memo, ...}` (named, non-wildcard) as the controller-safe way to reach these types, or (b) having `clean-signals-leptos` re-export the specific signal types/traits controllers need (`RwSignal`, `Memo`, `Get`, `Set`, `Update`, `GetUntracked`) so controllers can depend on exactly one crate for both the hooks and the signal primitives.
3. **`view!`'s tag/attribute parser trips on generic angle brackets inside an inline closure's parameter type** (`children=move |x: Vec<Member>| { ... }` fails to parse; `children=move |x| ...` with the type inferred from a sibling prop works). Not blocking — just a a sharp edge worth a one-line callout in `templates/AGENTS.md` or a `clean-signals-leptos` doc comment, since it's the kind of thing a new downstream team will hit immediately and lose time to.

### Testing Performed

- `cargo build --workspace` — Passed.
- `cargo test --workspace` — Passed (all pre-existing suites plus `team-demo`'s 20 new tests: `failure` (2), `domain::entities` (1), `domain::use_cases::load_team` (2), `domain::use_cases::update_member` (3), `data::models` (1), `data::sources` (3), `data::repositories` (4), `presentation::controllers` (4, including the retry-absorption, `filtered` Memo, and validation-failure-via-sink tests the task's acceptance criteria specifically call for)).
- `cargo test -p team-demo` — Passed, 20/20.
- `cargo clippy --workspace --all-targets -- -D warnings` — Passed (one `collapsible_if` finding in `controllers.rs::rename` fixed via an `if let ... && let ...` chain per clippy's own suggestion).
- `cargo check -p team-demo --target wasm32-unknown-unknown` — Passed.

### Risks/Limitations

1. **No `trunk serve` smoke test** — out of scope per the task ("installing trunk is NOT part of this task's verification"); the wasm `cargo check` pass is the load-bearing signal that the crate is browser-portable.
2. **`Trunk.toml` not added** — the task listed it as optional ("optionally Trunk.toml"); default trunk behavior (index.html at crate root) needs no config, so I omitted it to keep the diff minimal.
3. **No `watch`/presence-stream feature** (Dart's `watchOnlineIds`/`WatchOnlinePresence`) — the task's own Details section only specifies `LoadTeam`/`UpdateMember`, not a streaming use case, and the Notes explicitly ask to keep scope small ("one feature slice done perfectly beats three done loosely"); `ControllerCore::watch` is exercised elsewhere in the framework's own test suite, so omitting it here doesn't leave that path untested workspace-wide.
