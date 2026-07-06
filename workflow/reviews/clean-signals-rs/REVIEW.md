# Review — clean-signals-rs (feature phase, round 0)

**Diff range:** `28d6aaa..HEAD` (f143e2b at review time)
**Verdict:** ⚠️ NEEDS_WORK
**Workflow:** review-diff (5 dimensions, adversarial 3-vote verify on every Critical/Major; 26 agents)

## Confirmed findings (6 reported → 5 distinct; no Criticals)

Full text: [findings-confirmed.md](findings-confirmed.md). Deduped list:

| # | Sev | Where | Finding |
|---|-----|-------|---------|
| F1 | Major | controller.rs tests (608-681, 765) | Real-time `tokio::time::sleep` used to synchronize spawned tasks in 4 tests — violates the repo's own no-sleep-to-settle rule; flake risk under CI load |
| F2 | Major | controller.rs 119-141 | `FailureSink::emit` holds its `std::sync::Mutex` while invoking user listeners — a panicking listener poisons the sink permanently and risks double-panic (abort) via `Subscription::drop` during unwind |
| F3 | Major | controller.rs 339-437 | `watch` AbortHandle registry never pruned (completed or cancelled watches linger) — unbounded growth on long-lived app-scoped controllers (reported twice: logic + risks) |
| F4 | Major | team-demo pages.rs 38-40 | Ungated `StoredValue::get_value()` inside fire-and-forget `spawn_local` panics if the page unmounts before the microtask runs — footgun in the stated reference implementation |
| F5 | Major | controller.rs 260-274, 339-437 | `dispose()` vs concurrent `watch()` TOCTOU on a deliberately `Send + Sync` type — a watch registered after the drain is never aborted |

## Minor findings (15, deferred unless a followup touches the same code)

Full list: [findings-minor.md](findings-minor.md). Themes: AGENTS.md wording vs reference-app reality (reactive_graph reachability), missing `async_trait` re-export, `view!` generic-closure parsing sharp edge, `cargo fmt` drift repo-wide, `dispose()` doesn't cancel in-flight `run`, `RetryPolicy::new(0,_)` untested clamp, `AsyncView` clones `T` per reactive read, raw transport text in a user-facing message (example app).

## Ledger

Recorded in TASKS.md § Phase Review as Round 0 → NEEDS_WORK. Followup round 1 targets F1–F5 via followup-investigate → fix tasks.
