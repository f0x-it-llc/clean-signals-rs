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

---

## Completion Summary

**Status:** Done
**Branch:** master

### Verbatim-portability verdict (per module)

| Module | Verdict |
|--------|---------|
| `failure.rs` | **Byte-identical** to CSR demo |
| `features/mod.rs`, `features/team/mod.rs` | **Byte-identical** |
| `domain/` (entities, repositories, use_cases/*, mods) | **Byte-identical** — 9/9 files, zero deviations. The domain layer is fully render-mode-agnostic. |
| `presentation/mod.rs`, `presentation/components.rs` | **Byte-identical** |
| `presentation/controllers.rs` | **Production code byte-identical.** Only the `#[cfg(test)]` module differs (see deviation #1). |
| `presentation/pages.rs` | **One-line deviation:** the initial-load `spawn_local` gained a `#[cfg(target_arch = "wasm32")]` gate (see deviation #2). Everything else verbatim. |
| `data/` | New — this is the intended SSR diff (server-fn transport). No CSR counterpart to be verbatim against. |

### Forced deviations (framework findings)

1. **Controller tests require an active reactive `Owner`/arena on SSR+hydrate builds.** SSR/hydrate pull `leptos_axum` → `leptos_integration_utils`, which enables `reactive_graph`'s `sandboxed-arenas` feature. Under it, `RwSignal::new` / `async_state_signal()` panic with *"the `sandboxed-arenas` feature is active, but no Arena is active"* unless created inside an `Owner`. The CSR demo (csr feature) never activates `sandboxed-arenas`, so its identical controller tests need no owner. Mitigation: a `ssr_test_owner()` helper (`Owner::new(); owner.set()`), one `let _owner = ssr_test_owner();` line per test (4 tests). `#[tokio::test]`'s current-thread runtime keeps the thread-local owner active across `.await`s. This is test-harness-only; no production controller code changed. **Finding:** downstream SSR apps testing controllers natively need this owner scaffolding; worth a note in `templates/AGENTS.md` testing rules or a shared test helper in `clean-signals-leptos`.

2. **The initial-load spawn is target-gated (`#[cfg(target_arch = "wasm32")]`), not feature-gated.** This is the sanctioned SSR-shell load strategy: the server renders the `AsyncState::Loading` shell and never touches the transport; the client fires `controller.load()` post-hydration. Target-gating (not `cfg(feature = "ssr")`) keeps the page render-mode-agnostic per the spec. This is the *only* production-code difference in the whole presentation layer.

3. **The data layer imports `leptos` (narrowly).** `templates/AGENTS.md` says `data/` must not import `leptos`. But Leptos server functions ARE the transport here (the SSR analogue of a `reqwest`/`tonic` client), so `data/server_fns.rs` imports `leptos::prelude::{server, ServerFnError}` and (server-body-only) `use_context`, and `data/repositories.rs`/`data/store.rs` use `ServerFnError`. No reactive/DOM/view types cross into `data/`. **Finding:** the layer rule's intent ("no reactive/render types in data") holds; the letter ("no leptos import") needs an explicit server-functions carve-out for SSR apps in `templates/AGENTS.md`.

4. **Server-side flakiness ports cleanly** (kept, not dropped). The store's `failures_before_success` counter lives in the shared `RwLock`-backed store, so it counts down across the *separate HTTP retry requests* the controller's `RetryPolicy` issues — a stronger demonstration than CSR's in-process retries. Latency via `clean_signals::time::sleep` retained.

5. **`ServerFnError` → `TeamFailure` maps on the variant, not the message string.** `ServerFnError::Args`/`MissingArg` → non-retryable `Validation`; every other variant → retryable `Network`. Server bodies choose the variant (`ServerError` for transient, `Args` for unknown-id), so retryability is decided structurally, honoring the "never string-match on error text" rule. Note: `ServerFnError`'s generic `E` + blanket `From` impls make `?`-with-`ok_or_else` ambiguous; server-fn bodies use `let-else` + explicit `return Err(...)` so `E = NoCustomError` is inferred from the signature (minor ergonomic finding).

6. **No `serde_json` dev-dependency available**, so `models.rs` proves the DTO conversion via a `Member → MemberDto → Member` round-trip rather than a serde JSON round-trip. No manifest edit made (out of scope).

### Composition roots & load strategy

- **Server root** (`main.rs`, `#[cfg(ssr)]`): axum + `leptos_axum`, constructs `Arc<TeamStore>` (400ms latency, 2 forced failures — mirrors CSR `main.rs`), provides store + `LeptosOptions` via context to BOTH the SSR renderer (`render_app_to_stream_with_context`) and the standalone server-fn handler (`handle_server_fns_with_context`). `LeptosOptions` built directly (no `get_configuration`, since no cargo-leptos config → no `LEPTOS_*` env).
- **Client root** (`App` in `lib.rs`): constructs `ServerFnTeamRepo`, mounts `TeamPage`. Rendered on server inside `shell` and hydrated on client via `hydrate` (`#[cfg(hydrate)]`, `wasm_bindgen`).
- Exactly **one** `ServerFnError → TeamFailure` conversion site (`map_server_fn_error` in `data/repositories.rs`).

### Files Modified

| File | Changes |
|------|---------|
| `src/lib.rs` | `App` component (client root), `shell` (cfg ssr), `hydrate` (cfg hydrate), rustdoc "differences from CSR" + honest run story |
| `src/main.rs` | Server composition root (axum + leptos_axum, store construction, dual-handler context) |
| `src/failure.rs` | Copied verbatim from CSR |
| `src/features/**` (domain: 9 files) | Copied verbatim from CSR |
| `src/features/team/presentation/{mod,components}.rs` | Copied verbatim |
| `src/features/team/presentation/controllers.rs` | Verbatim production code; test module gains `ssr_test_owner()` + `let _owner` (deviation #1) |
| `src/features/team/presentation/pages.rs` | Verbatim except `#[cfg(target_arch = "wasm32")]` on initial-load spawn (deviation #2) |
| `src/features/team/data/models.rs` | New — `MemberDto` (serde) + `From` conversions |
| `src/features/team/data/store.rs` | New (cfg ssr) — seeded `RwLock` store, latency + flakiness |
| `src/features/team/data/server_fns.rs` | New — `#[server]` `fetch_team`/`rename_member` transport |
| `src/features/team/data/repositories.rs` | New — `ServerFnTeamRepo` + single `map_server_fn_error` site |
| `src/features/team/data/mod.rs` | New — data module wiring |

### Testing Performed

- `cargo check -p team-demo-ssr --features ssr` — Passed
- `cargo check -p team-demo-ssr --features hydrate --target wasm32-unknown-unknown` — Passed
- `cargo test -p team-demo-ssr --features ssr` — Passed (**21 tests**: 5 domain use-case/entity, 2 models, 4 repo mapping, 3 store, 3 failure, 4 controller)
- `cargo clippy -p team-demo-ssr --features ssr --all-targets -- -D warnings` — Passed
- `cargo clippy -p team-demo-ssr --features hydrate --target wasm32-unknown-unknown -- -D warnings` — Passed
- `cargo build --workspace` / `cargo test --workspace` / `cargo fmt --all -- --check` — Passed (featureless default build of this crate compiles; no forced-feature breakage)
- `cargo clippy --workspace --all-targets -- -D warnings` + `cargo check -p clean-signals --target wasm32-unknown-unknown` (repo DoD) — Passed

### No manifest changes needed

The pre-wired `examples/team-demo-ssr/Cargo.toml` was sufficient: featureless build compiles because `server_fn/browser` is always enabled transitively (via `clean-signals-leptos` → `leptos` defaults) and `#[server]` bodies are dropped when `ssr` is off; ssr build gets the browser client via the same path plus `server_fn/axum`. No gaps to report.

### Doc Updates Needed

- `templates/AGENTS.md` — two SSR carve-outs would help downstream SSR apps: (a) a server-functions exception to the "data must not import leptos" rule (findings #3), and (b) a testing note that native controller tests on SSR/hydrate builds must run inside a reactive `Owner` due to `sandboxed-arenas` (finding #1). Not edited (core doc; flagged for `doc_maintainer`).

### Risks/Limitations

1. **Runtime not exercised.** Per spec, this repo ships no cargo-leptos config; verification is compile + native-test only. `main.rs` constructs a valid axum app but has no generated wasm/JS assets to serve. The run story (what a cargo-leptos setup would look like) is documented in `lib.rs` rustdoc.
2. **`ServerFnError` non-exhaustive** — `map_server_fn_error` uses a catch-all `other =>` arm (required; the enum is `#[non_exhaustive]`). New transport-error variants default to retryable `Network`, which is the safe default.
