## Task: fix-03 — Gate StoredValue access in team-demo async closures (F4)

**Objective**: Eliminate the unmount-panic footgun in the reference example: no ungated `StoredValue::get_value()` inside spawned futures.

**Depends on**: None (disjoint files; runs in parallel with fix-01)

**Complexity:** low

### Scope

**Files Modified (Write):**
- `examples/team-demo/src/features/team/presentation/pages.rs` (initial-load spawn at ~38-40)
- `examples/team-demo/src/features/team/presentation/components.rs` (same pattern in the rename click handler at ~39 — audit and fix if it has the same ungated `get_value()` in a spawned future)

**Files Read (Dependencies):**
- `workflow/plans/feature/clean-signals-rs/followups/phase-fix-1/DIAGNOSES.md` (F4 section)
- `crates/clean-signals-leptos/src/hooks.rs` (what use_controller returns; try_get_value availability)

### Details

Replace `controller.get_value()` inside `spawn_local` futures with the gated pattern: `let Some(c) = controller.try_get_value() else { return; };` — no-op when the page was disposed before the microtask ran. Synchronous (non-spawned) `get_value()`/`with_value()` calls during render are fine and stay. Add a short comment stating WHY (disposal race), since this file is the reference implementation downstream apps copy verbatim. Do NOT add new framework helpers (deferred — see followup TASKS.md Deferred list); do NOT touch Cargo.tomls or framework crates.

### Acceptance Criteria

1. No `get_value()` (or `.with_value()` that can panic) on a `StoredValue` inside any spawned future in team-demo without a `try_*` liveness gate.
2. `cargo test -p team-demo` (20 tests) still green; `cargo clippy --workspace --all-targets -- -D warnings`; `cargo check -p team-demo --target wasm32-unknown-unknown` green.
3. Comment explains the disposal race in one or two lines.
