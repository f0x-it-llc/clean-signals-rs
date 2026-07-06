## Task: Failure trait + test fixtures

**Objective**: Implement the `Failure` trait exactly per the PLAN.md API contract, plus the shared test-fixture failure types the rest of the suite will use.

**Depends on**: None (Wave 1)

**Complexity:** medium

### Scope

**Files Modified (Write):**
- `crates/clean-signals/src/failure.rs`

**Files Read (Dependencies):**
- `workflow/plans/feature/clean-signals-rs/PLAN.md` (§ Core API contract)
- `workflow/plans/feature/clean-signals-rs/research/dart-test-spec.md`

### Details

- `pub trait Failure: Debug + Display + Send + Sync + 'static` with default `user_message() -> String` (= `to_string()`) and default `is_retryable() -> bool` (= false). Rustdoc on the trait must state the layer rule: repositories convert transport errors into the app's Failure enum via `From` impls; failures never cross a layer as raw errors.
- `#[cfg(any(test, feature = "test-fixtures"))] pub mod fixtures`: `NetworkFailure { message: String }` (`is_retryable = true`), `ValidationFailure { message: String }` (`is_retryable = false`), both `Clone + PartialEq`, Display = message. The `test-fixtures` cargo feature already exists in Cargo.toml — do not edit Cargo.toml.
- Unit tests: default `user_message` mirrors Display; default `is_retryable` false; fixtures behave as specified; a blanket-usage test showing an app-style enum (`enum AppFailure { Network(String), Validation(String) }`) implementing the trait with exhaustive matching.

### Acceptance Criteria

1. `cargo test -p clean-signals failure` passes; `cargo clippy -p clean-signals -- -D warnings` clean on this module.
2. Trait signature matches PLAN.md verbatim (names, bounds, defaults).
3. Fixtures exported for use by sibling modules' tests (they will be used by tasks 03–06).

### Notes

- Do not add `thiserror` usage in the trait itself; fixtures may implement Display by hand to keep the core dependency-light (thiserror is available if it reads cleaner — it is pre-declared).
- Do NOT port Dart's `UnexpectedFailure` — PLAN.md design delta #2.
