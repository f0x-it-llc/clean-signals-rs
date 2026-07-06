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

---

## Completion Summary

**Status:** Done
**Branch:** worktree-wf_98ec4f7c-170-2

### Files Modified

| File | Changes |
|------|---------|
| `docs/ARCHITECTURE.md` | New: workspace/crate structure, layer dependency diagram (core ← leptos-integration ← app), core abstractions (`Failure`, `UseCase`/`StreamUseCase`, `RetryPolicy`, `ActivityTracker`, `AsyncState`, `ControllerCore`, `FailureSink`), data flow, design deltas vs. Dart, key-types table. |
| `docs/CODE_STANDARDS.md` | New: generic-over-`F` convention, `try_*` write rule, no-sleeps-outside-`time::sleep`, dual `cfg_attr` async_trait pattern, naming conventions, `test-fixtures` feature usage, anti-patterns, testing patterns (cross-references DEVELOPMENT.md for the LocalSet/tick recipe instead of duplicating it). |
| `docs/DEVELOPMENT.md` | New: prerequisites, build/test/lint/DoD commands, wasm-check commands, example app run instructions, the `effects` feature gotcha, the SPIKE_NOTES `spawn_local`/`LocalSet`/`tick` recipe, workflow locations, troubleshooting. |
| `templates/AGENTS.md` | New: Rust/Leptos port of the Dart downstream template — layer table, feature-slice layout (flat or crate-per-layer), domain/data/controller/DI/testing rules, failures rules, forbidden list, DoD commands, project-specifics footer. |

### Content Boundary Compliance

- All updates within correct document boundaries: YES
- Cross-contamination detected and fixed: YES — an initial draft duplicated the `LocalSet`/`Executor::tick` test recipe code block in both CODE_STANDARDS.md and DEVELOPMENT.md; trimmed CODE_STANDARDS.md's copy to a cross-reference, keeping the one verbatim recipe in DEVELOPMENT.md (where the task explicitly assigns it).
- No content duplicated across docs / into CLAUDE.md: YES (no CLAUDE.md exists in this repo yet; out of this task's scope, not created)

### Size Discipline

| Doc | Lines before | Lines after | Under cap? |
|-----|-------------|------------|-----------|
| `docs/ARCHITECTURE.md` | 0 (new) | 119 | YES (target ≤350, task target ~150) |
| `docs/CODE_STANDARDS.md` | 0 (new) | 122 | YES (target ≤350, task target ~120) |
| `docs/DEVELOPMENT.md` | 0 (new) | 135 | YES (target ≤350, task target ~120) |
| `templates/AGENTS.md` | 0 (new) | 186 | N/A (not a size-budgeted managed doc type; comparable to the 176-line Dart original) |

### Notable Decisions/Tradeoffs

1. **Described the leptos-integration crate from its task file + PLAN.md, not from code**: task 07 runs in parallel and its crate files are still empty scaffold stubs in this worktree at write time, per the task's explicit instruction.
2. **`try_*` write rule and the async-trait `cfg_attr` pattern** were pulled directly from the merged `crates/clean-signals` source (module-level rustdoc in `activity.rs`/`use_case.rs`) to keep CODE_STANDARDS.md accurate to the authoritative, already-merged API rather than to the locked PLAN.md contract alone.
3. **No CLAUDE.md created**: not in this task's Scope/Files-Modified list, and no existing CLAUDE.md was found to update; left untouched.
