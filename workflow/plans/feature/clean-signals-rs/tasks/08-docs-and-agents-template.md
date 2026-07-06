## Task: Core docs + downstream AGENTS.md template

**Agent:** doc_maintainer

**Objective**: Write this repo's core docs (ARCHITECTURE, CODE_STANDARDS, DEVELOPMENT) and the `templates/AGENTS.md` downstream-project template that mirrors the Dart original's enforcement role.

**Depends on**: 06-controller (API frozen); 07 runs in parallel — describe the integration crate from its task file + PLAN.md, not from unmerged code.

**Complexity:** medium

### Scope

**Files Modified (Write):**
- `docs/ARCHITECTURE.md`: workspace crates, layer diagram (core ← leptos-integration ← app; core depends only on reactive_graph), the ControllerCore/UseCase/Failure abstractions, design deltas vs Dart (from PLAN.md).
- `docs/CODE_STANDARDS.md`: conventions — generic-over-F failures, `try_*` signal writes in framework code, no sleeps outside `time::sleep`, cfg_attr async_trait pattern, test-fixtures feature usage, naming (`*_use_case` one per file downstream, controller naming).
- `docs/DEVELOPMENT.md`: build/test/lint/wasm-check commands, the `effects` feature gotcha, the SPIKE_NOTES tokio test recipe (Executor init + tick), how to run the example app.
- `templates/AGENTS.md`: the downstream template — port `/home/ed/Dev/personal/clean_signals/templates/AGENTS.md` faithfully to Rust/Leptos: feature-slice folder convention (`src/features/<feature>/{domain,data,presentation}` or crate-per-layer for larger apps), the MAY/MUST-NOT import table per layer, repository-trait rules (return domain types, `From` error conversion at the boundary, one conversion site per repo impl), controller rules (never call use cases directly — always `run`/`run_into`/`watch`), DI conventions (constructor injection; leptos context for app-scoped controllers; `use_controller` for page-scoped), testing rules (controllers tested natively with fake repos; every use case gets a failure-path test).

**Files Read (Dependencies):**
- `~/.claude/skills/doc-standards/schemas.md` (content boundaries)
- `workflow/plans/feature/clean-signals-rs/PLAN.md`, `research/RESEARCH.md`, `research/SPIKE_NOTES.md`
- `/home/ed/Dev/personal/clean_signals/templates/AGENTS.md` + `/home/ed/Dev/personal/clean_signals/README.md` (source material)
- `crates/clean-signals/src/lib.rs` (merged, authoritative API)
- `workflow/plans/feature/clean-signals-rs/tasks/07-leptos-integration.md` (integration API being built in parallel)

### Acceptance Criteria

1. Three docs exist, respect doc-standards content boundaries, each compact (ARCHITECTURE ≤ ~150 lines; others ≤ ~120).
2. `templates/AGENTS.md` covers every enforcement mechanism the Dart template has (layer table, naming, DoD commands) adapted to Rust.
3. All commands in DEVELOPMENT.md are real and copy-pasteable.

### Notes

- The AGENTS.md template is half the framework's value — it is what downstream repos (cupline) copy in. Preserve the Dart template's strictness and tone.
