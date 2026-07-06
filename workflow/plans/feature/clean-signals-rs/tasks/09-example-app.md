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
