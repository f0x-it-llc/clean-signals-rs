# SSR round — demonstrate the SSR architecture

User request (2026-07-06): show how an SSR app is architected with clean-signals. Answer being demonstrated: same layers/controllers/use-cases as CSR; only the composition roots (cfg-gated), the repository transport (server functions), and the initial-load strategy (SSR-shell) differ.

## Tasks

| # | Task | Status | Complexity | Depends On | Files (Write) |
|---|------|--------|------------|------------|---------------|
| 1 | [ssr-01-team-demo-ssr](tasks/ssr-01-team-demo-ssr.md) | Not Started | high | - | `examples/team-demo-ssr/**`, root `Cargo.toml` (member + workspace deps), conductor pre-approves manifest additions |
| 2 | [ssr-02-docs-ssr-section](tasks/ssr-02-docs-ssr-section.md) | Not Started | medium (Agent: doc_maintainer) | ssr-01 | `docs/ARCHITECTURE.md`, `templates/AGENTS.md` |

## File Overlap Analysis

| Task | Files Modified (Write) | Files Read |
|------|----------------------|------------|
| ssr-01 | examples/team-demo-ssr/** (new crate; scaffold incl. its Cargo.toml is pre-created by conductor), NO other files | examples/team-demo/**, both framework crates, docs/ |
| ssr-02 | docs/ARCHITECTURE.md, templates/AGENTS.md | ssr-01 completion summary + merged example |

### Overlap Matrix

| Task Pair | Shared Write Files | Isolation Strategy |
|-----------|-------------------|-------------------|
| ssr-01 + ssr-02 | None, but ssr-02 documents ssr-01's real shape | Sequential (ssr-02 after ssr-01 merges) |

## Success Criteria

- team-demo-ssr compiles for BOTH targets: server (`--features ssr`) and hydrate wasm (`--features hydrate`); native controller tests pass (same tests as CSR demo prove the controller layer is untouched).
- Diffable story: feature-slice code (domain/data/presentation) is line-for-line reusable vs the CSR demo except the repository transport impl; only main/lib roots and the data-source transport differ.
- Docs gain an "SSR applications" subsection stating the rule (same layers; cfg-gated roots; server functions as repository transport; SSR-shell default; Resource bridge deliberately out of scope/YAGNI until first-paint data is needed).
- Zero Dart references (standing directive); full workspace DoD green.
