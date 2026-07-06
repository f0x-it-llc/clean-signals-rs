## Task: Spike — validate reactive_graph standalone usage

**Objective**: Empirically validate every runtime assumption the framework rests on, in `crates/clean-signals/tests/spike_reactive_graph.rs`, and record the verdicts + exact recipes in `workflow/plans/feature/clean-signals-rs/research/SPIKE_NOTES.md` for tasks 05/06/07 to consume.

**Depends on**: None (Wave 1)

**Complexity:** high

### Scope

**Files Modified (Write):**
- `crates/clean-signals/tests/spike_reactive_graph.rs`: integration tests (this file is KEPT permanently as the framework's environment-assumption suite)
- `workflow/plans/feature/clean-signals-rs/research/SPIKE_NOTES.md`: findings document

**Files Read (Dependencies):**
- `workflow/plans/feature/clean-signals-rs/PLAN.md` (API contract + Edge Cases)
- `workflow/plans/feature/clean-signals-rs/research/RESEARCH.md` (verified doc claims to test empirically)

### Details

Dependencies are already declared in `crates/clean-signals/Cargo.toml` (reactive_graph 0.2, any_spawner 0.3 with tokio feature as dev-dep, tokio dev-dep, futures). Do NOT edit Cargo.toml; if something is genuinely missing, record it in SPIKE_NOTES.md as a blocker instead.

Write focused `#[tokio::test]` (and where needed `#[tokio::test(flavor = "current_thread")]` + `LocalSet`) tests answering, one test per question:

1. **Owner lifecycle**: create `Owner::new()`, create `RwSignal` inside `owner.with(...)`, verify reads/writes; call `owner.cleanup()`, verify `signal.try_set(...)` / `try_update` return `None` (or document actual disposed-signal behavior — panic? None? warn?).
2. **Signals without an Owner**: create a signal with no current owner — works? leaks only?
3. **Memo**: `Memo::new(|_| ...)` recomputation on dependency change, read via `.get_untracked()`; does Memo require `Executor` init or an Owner?
4. **Effect**: with `reactive_graph/effects` enabled in dev-deps, does `Effect::new` run under `Executor::init_tokio()`? Does it need `Executor::tick().await` to fire? Can it be stopped explicitly (what's the API — `.stop()`? drop? Owner cleanup)? Also try `Effect::new_isomorphic`.
5. **any_spawner spawn_local under tokio**: does `Executor::spawn_local` work in a plain multi-thread `#[tokio::test]`, or does it require a current-thread runtime / `tokio::task::LocalSet`? Pin the EXACT recipe a downstream `#[test]` must follow to test controller `watch` (this recipe goes verbatim into SPIKE_NOTES for task 06 and into docs later).
6. **Executor double-init**: calling `Executor::init_tokio()` in multiple tests of one binary — error? idempotent? Determine the safe pattern (e.g. `Executor::init_tokio().ok()` or a `Once`).
7. **Cross-thread arena signal**: create `RwSignal` on one thread, `.get_untracked()` from `std::thread::spawn` — confirm it works for `new()` and panics for `new_local()` (document, don't assert the panic if it's UB-ish — use `catch_unwind` or skip).
8. **RAII guard pattern**: signal writes from a `Drop` impl (the ActivityTracker guard design) — confirm no reentrancy issue.

SPIKE_NOTES.md format: one `## Q<n>` section per question with VERDICT (one line) + the working code recipe + implications for tasks 05/06/07. Flag loudly any result that contradicts PLAN.md's locked decisions.

### Acceptance Criteria

1. `cargo test -p clean-signals --test spike_reactive_graph` passes.
2. Every one of the 8 questions has a test AND a SPIKE_NOTES.md section with a definitive verdict.
3. Any contradiction with PLAN.md is flagged in a `## CONTRADICTIONS` section at the top of SPIKE_NOTES.md (empty section if none).

### Notes

- This is exploratory but ships: keep the tests deterministic (no sleeps for ordering — use `Executor::tick().await`).
- If a question can't be settled, say so explicitly in SPIKE_NOTES with what was tried — do NOT guess.
