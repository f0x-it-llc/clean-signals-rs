## Task: RetryPolicy + portable time module

**Objective**: Implement `RetryPolicy<F>` (declarative per-call retry) and `time::sleep` (cfg-gated tokio/gloo) per PLAN.md.

**Depends on**: 02-failure-trait

**Complexity:** medium

### Scope

**Files Modified (Write):**
- `crates/clean-signals/src/retry.rs`
- `crates/clean-signals/src/time.rs`

**Files Read (Dependencies):**
- `workflow/plans/feature/clean-signals-rs/PLAN.md` (§ Core API contract)
- `crates/clean-signals/src/failure.rs`
- `workflow/plans/feature/clean-signals-rs/research/dart-test-spec.md` (activity_test.dart RetryPolicy section)

### Details

`time.rs`:
- `pub async fn sleep(d: Duration)` — native: `tokio::time::sleep(d).await`; wasm32: `gloo_timers::future::sleep(d).await`. Both deps pre-declared target-gated in Cargo.toml.
- Rustdoc: this is the ONLY sanctioned sleep in the framework and in downstream apps' presentation layers.

`retry.rs` (semantics pinned by dart-test-spec):
- Struct per PLAN.md contract; `retry_if: Option<Arc<dyn Fn(&F) -> bool + Send + Sync>>` private, builder method `pub fn retry_if(self, f: impl Fn(&F) -> bool + Send + Sync + 'static) -> Self`.
- `RetryPolicy::none()` → `max_attempts: 1`; `Default` impl = `none()`. Constructor `pub fn new(max_attempts: u32, delay: Duration) -> Self` with `backoff_factor` defaulting to 1.0 (constant delay) and a `with_backoff(factor)` builder. Formula (pinned by dart-test-spec): `delay_for(1) = delay`, `delay_for(n) = delay * backoff_factor^(n-1)`. Check the Dart source (`/home/ed/Dev/personal/clean_signals/lib/src/retry.dart`) for its default factor and match it; tests must set the factor explicitly either way.
- `delay_for(attempt)` per formula (attempt is 1-based; guard attempt=0).
- `should_retry(&self, f: &F) -> bool` — `retry_if` if set, else `f.is_retryable()`.
- IMPORTANT: the retry LOOP lives in `ControllerCore::run` (task 06), not here. This module is pure policy math + predicate.
- Tests: delay_for exponential progression (factor 2.0: d, 2d, 4d); factor 1.0 constant; should_retry honors is_retryable via NetworkFailure/ValidationFailure fixtures; custom retry_if overrides is_retryable both directions; none() → max_attempts 1. `time::sleep` smoke test under tokio with a small duration + `Instant` lower-bound assert.

### Acceptance Criteria

1. `cargo test -p clean-signals retry` and the sleep smoke test pass; clippy clean.
2. `cargo check -p clean-signals --target wasm32-unknown-unknown` stays green (gloo path compiles).
3. `RetryPolicy<F>` is `Clone` even when `F` isn't (manual Clone impl — Arc field).

### Notes

- No global/shared retry state — the policy is a value, cloned per call.

---

## Completion Summary

**Status:** Done
**Branch:** worktree-wf_d366217c-c00-2

### Files Modified

| File | Changes |
|------|---------|
| `crates/clean-signals/src/retry.rs` | Implemented `RetryPolicy<F>`: `none()`, `new(max_attempts, delay)`, `with_backoff(factor)`, `retry_if(predicate)`, `delay_for(attempt)`, `should_retry(&F)`; manual `Clone` impl (no `F: Clone` bound) via a `RetryIf<F>` type alias for the `Arc<dyn Fn(&F) -> bool + Send + Sync>` field (also fixes a clippy `type_complexity` lint); `Default` = `none()`. Full test suite per task spec: exponential progression, constant-factor, `should_retry` default/override in both directions, `none()`/`Default`, attempt-0 guard, and a `Clone` smoke test. |
| `crates/clean-signals/src/time.rs` | `pub async fn sleep(Duration)` cfg-gated: `tokio::time::sleep` (native) / `gloo_timers::future::sleep` (wasm32). Rustdoc marks it the only sanctioned sleep. Tokio smoke test asserting `Instant::elapsed() >= d`. |

### Notable Decisions/Tradeoffs

1. **`backoff_factor` default in `new()` set to `2.0`, not `1.0`**: the task text has an internal contradiction — it states `backoff_factor` defaults to `1.0` ("constant delay") but in the very next sentence instructs "Check the Dart source... for its default factor and match it." I read the actual Dart source at `/home/ed/Dev/personal/clean_signals/lib/src/retry.dart`: its `RetryPolicy` constructor defaults `backoffFactor: 2.0`. Per the explicit "match it" instruction (the more specific, verifiable directive) I used `2.0` as the default for `new()`, matching upstream Dart exactly. All tests set the factor explicitly via `with_backoff`, per the task's own caveat that "tests must set the factor explicitly either way" — so this choice does not affect any pinned test behavior. `none()`'s `delay`/`backoff_factor` fields are also set to Dart's literal defaults (300ms / 2.0) though irrelevant since `max_attempts = 1` means no retry ever occurs.
2. **Manual `Clone` via type alias**: `#[derive(Clone)]` on a struct with a raw `Option<Arc<dyn Fn(&F) -> bool + Send + Sync>>` field would add a spurious `F: Clone` bound (a known derive-macro limitation) and additionally trips clippy's `type_complexity` lint. Introduced a private `type RetryIf<F> = Arc<dyn Fn(&F) -> bool + Send + Sync>;` alias, used in the field and hand-wrote `impl<F> Clone for RetryPolicy<F>` — satisfies acceptance criterion 3 exactly and keeps clippy `-D warnings` clean.
3. **`should_retry` scoped to `impl<F: Failure>`**, the rest of the API (`none`, `new`, `with_backoff`, `retry_if`, `delay_for`) lives under an unconstrained `impl<F> RetryPolicy<F>` block since none of those need the `Failure` bound — slightly more precise than PLAN.md's single `impl<F: Failure> RetryPolicy<F>` block, but behaviorally identical since every real usage in this codebase carries `F: Failure`.
4. **Doctest fixture**: the crate doc-example for `RetryPolicy` defines a small local `NetworkFailure` type implementing `Failure` inline rather than reaching for `clean_signals::failure::fixtures::NetworkFailure`, because that module is `#[cfg(any(test, feature = "test-fixtures"))]`-gated and isn't visible to an ordinary doctest compilation (which doesn't enable the `test-fixtures` feature by default).

### Testing Performed

- `cargo test -p clean-signals retry` - Passed (12 tests, includes all `failure` tests since name matches; 9 are retry-specific)
- `cargo test -p clean-signals time` - Passed (sleep smoke test with `Instant` lower-bound assert)
- `cargo test -p clean-signals` (full crate, incl. doctests) - Passed
- `cargo test --workspace` - Passed
- `cargo clippy -p clean-signals --all-targets -- -D warnings` - Passed (clean)
- `cargo check -p clean-signals --target wasm32-unknown-unknown` - Passed (gloo path compiles)
- `cargo build --workspace` - Passed

### Risks/Limitations

1. **Backoff-factor default divergence from task text**: see Notable Decision #1 above — flagging in case the plan author intended the literal `1.0` value despite the contradictory Dart-matching instruction. Low risk since no test depends on the default (all set the factor explicitly), and `RetryPolicy::none()` (the only zero-config path used elsewhere) is unaffected by `backoff_factor` since it never retries.
