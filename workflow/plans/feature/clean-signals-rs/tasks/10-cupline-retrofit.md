## Task: Cupline orders-page retrofit (validation, branch-only)

**Objective**: In the cupline repo, on a NEW branch `clean-signals-retrofit`, refactor the dashboard orders page onto clean-signals: `OrdersController` + repository trait + dumb view, with native controller tests. This validates the framework against its first real consumer. NEVER merge in cupline.

**Depends on**: 06, 07 (framework merged on clean-signals-rs master)

**Complexity:** high

### Scope  — ALL writes in /home/ed/Dev/personal/cupline on branch `clean-signals-retrofit`

**Files Modified (Write):**
- cupline root `Cargo.toml`: add workspace deps `clean-signals` / `clean-signals-leptos` as PATH deps (`path = "../clean-signals-rs/crates/…"`)
- `crates/cl-dashboard/Cargo.toml`: add the two deps
- `crates/cl-dashboard/src/orders/mod.rs` **NEW**: `OrdersFailure` enum (impl `Failure`; `From<ApiError>` — Network/RateLimited/Server retryable, others not), `OrdersRepository` trait (cfg_attr async_trait: `list_orders(shop_id) -> Result<Vec<OrderDto>, OrdersFailure>`, `set_status(order_id, &str) -> Result<OrderDto, OrdersFailure>`), `ApiOrdersRepository` (wraps `cl_ui::api::dashboard`, wasm-only impl or cfg-stubbed), `OrdersController` (embeds `ControllerCore<OrdersFailure>`; state `orders: RwSignal<AsyncState<Vec<OrderDto>, OrdersFailure>>`; methods `load(shop_id)` via run_into, `advance(order_id, target)` then reload, `cancel(order_id)`), native tests with `FakeOrdersRepository`
- `crates/cl-dashboard/src/pages/orders.rs`: reduce to a view: `use_controller`, effect on `state.selected` → `controller.load`, `use_interval(8s, reload)`, `AsyncView` over orders state, `use_failure_listener` → visible error surface (replaces silent `if let Ok` swallowing)
- `crates/cl-dashboard/src/lib.rs`: `mod orders;` declaration if needed

**Files Read (Dependencies):**
- `/home/ed/Dev/personal/clean-signals-rs/workflow/plans/feature/clean-signals-rs/research/cupline-surface.md` — the verified surface map (signatures, paths, DTO fields)
- cupline `docs/ARCHITECTURE.md`, `crates/cl-ui/src/api/*`, `crates/cl-core/src/dto.rs`
- clean-signals-rs `templates/AGENTS.md` + both crates' rustdoc

### Details

- Branch first: `git -C /home/ed/Dev/personal/cupline checkout -b clean-signals-retrofit` (verify clean tree first; if dirty, STOP and report).
- Preserve behavior exactly: 3 status columns, next_status transitions, cancel, 8s polling, refetch on shop switch. The polling keeps stale orders visible during refetch (Reloading) — improving on today's flash-free behavior, not regressing it.
- OrderDto stays the state type (this retrofit deliberately does NOT introduce a domain entity — minimal-diff validation; note it in the summary).
- Respect cupline conventions (docs/ARCHITECTURE.md, existing code style; money/ticket helpers from cl_ui::theme stay in the view).
- Tests must NOT need a DOM: FakeOrdersRepository with programmable results; assert load transitions, advance→refetch sequencing, failure → sink emission, non-retryable ApiError::Unauthorized NOT retried.
- Verify: `cargo check -p cl-dashboard` with the crate's native feature set AND `cargo check -p cl-dashboard --target wasm32-unknown-unknown` if that's how the portal builds (mirror whatever cupline's docs/DEVELOPMENT.md prescribes — read it); `cargo test -p cl-dashboard`.
- Commit on the branch with a clear message; leave the branch checked out ON `clean-signals-retrofit`? NO — return the repo to its original branch after committing, leave the new branch for review.

### Acceptance Criteria

1. Orders page behavior-identical (columns, actions, polling, shop switching) with errors now user-visible.
2. `OrdersController` fully covered by native tests using the fake repo — zero DOM/browser needed.
3. cupline verification commands green ON THE BRANCH; original branch untouched and restored as checked-out.
4. No other cupline pages/files touched beyond the listed scope.

### Notes

- If leptos version skew (cupline pins 0.8.x) causes reactive_graph version conflicts with clean-signals, record exact cargo error in the completion summary and STOP rather than fork the framework's dep versions — the conductor resolves versioning.
- This is the framework's exam. Friction points (awkward APIs, missing helpers) are FINDINGS — list every one in the completion summary even when worked around.
