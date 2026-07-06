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
