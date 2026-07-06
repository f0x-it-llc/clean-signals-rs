## Task: ssr-01 — team-demo-ssr example (SSR + hydrate, server-fn repository)

**Objective**: Add `examples/team-demo-ssr`: the same team feature slice as the CSR demo, architected as a Leptos 0.8 SSR+hydrate app (axum server), demonstrating that clean-signals architecture is render-mode-agnostic — only the composition roots and the repository transport change.

**Depends on**: None (scaffold pre-created by conductor)

**Complexity:** high

### Scope

**Files Modified (Write):** `examples/team-demo-ssr/**` ONLY (src/**, Cargo.toml feature wiring if needed, index/Trunk assets NOT needed — cargo-leptos style file layout, but see Notes on build tooling). Root Cargo.toml is pre-wired by the conductor — do not edit it or any other crate.

**Files Read:** `examples/team-demo/**` (the CSR reference — reuse its domain/presentation shapes as literally as possible), both framework crates, `docs/DEVELOPMENT.md`, `templates/AGENTS.md`.

### Details

Structure (start-axum single-crate pattern):
- `Cargo.toml` features: `ssr = ["leptos/ssr", "leptos_axum", "axum", "tokio/full", ...]`, `hydrate = ["leptos/hydrate"]`. (Conductor pre-declares leptos_axum/axum/tower in workspace deps and the example manifest — work with what's there; report gaps instead of editing.)
- `src/main.rs` (cfg ssr): axum server, leptos_axum handler, SERVER composition root — constructs the server-side team store (in-memory, `std::sync::RwLock`-backed, seeded like the CSR demo).
- `src/lib.rs`: `App` component + `hydrate` entry (cfg hydrate); CLIENT composition root constructs `ServerFnTeamRepo`.
- Feature slice mirrors the CSR demo layout exactly: `features/team/{domain,data,presentation}`. Domain (entities, `TeamRepository` trait via `clean_signals::async_trait`, LoadTeam/UpdateMember use cases) — ideally copied verbatim from team-demo, proving the point. Presentation (TeamController + page) — same. Data layer is where SSR differs:
  - `#[server]` functions `fetch_team() -> Result<Vec<Member>, ServerFnError>` and `rename_member(id, name)` — the transport. Server-side bodies read the shared store from axum state/context.
  - `ServerFnTeamRepo` implements `TeamRepository` by calling the server functions and mapping `ServerFnError` → `TeamFailure` at the boundary (the single conversion site, per AGENTS.md rules).
- Initial load: SSR-shell pattern — server renders the loading state; controller fires its load on the client only (same gated `try_get_value` spawn as the CSR demo, wasm-gated where needed). Document WHY in a comment: effects/loads don't run during SSR, so no hydration mismatch.
- Native tests: reuse the CSR demo's controller test approach with a fake repository (tests prove the controller/presentation layer is IDENTICAL and never knows about SSR). Test the ServerFnError→TeamFailure mapping in a unit test.
- Rustdoc on lib.rs: a compact "how this differs from the CSR demo" section (roots + transport + load strategy; everything else identical). No Dart references anywhere (standing directive).

### Verification

1. `cargo check -p team-demo-ssr --features ssr` (native server build)
2. `cargo check -p team-demo-ssr --features hydrate --target wasm32-unknown-unknown`
3. `cargo test -p team-demo-ssr --features ssr` (native tests)
4. `cargo clippy -p team-demo-ssr --features ssr --all-targets -- -D warnings` and the hydrate/wasm clippy equivalent
5. `cargo build --workspace && cargo test --workspace && cargo fmt --all -- --check` (workspace stays green; note: workspace-level builds must not force-enable this crate's mutually-exclusive features — verify the no-default-features setup doesn't break `cargo build --workspace`)

### Acceptance Criteria

1. Both feature-target combinations compile; native tests green; workspace DoD unaffected.
2. Domain + presentation modules are verbatim-portable from the CSR demo (any forced deviation documented in the completion summary — each one is a framework finding).
3. Exactly one ServerFnError→TeamFailure conversion site; controllers/pages contain zero cfg(ssr) logic beyond the standard wasm-gated initial-load spawn.
4. Running instructions in lib.rs rustdoc (cargo-leptos or manual two-step build — whichever the scaffold supports; keep honest: if `cargo leptos` isn't installed, say what WOULD run it and verify compilation only).

### Notes

- This example's value is the DIFF against team-demo — resist improving/refactoring shared shapes; sameness is the demonstration.
- If leptos_axum pulls conflicting versions or the SSR feature graph fights the workspace, STOP and report exact cargo errors (conductor owns manifests).
