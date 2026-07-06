## Task: UseCase + StreamUseCase traits

**Objective**: Implement `UseCase`, `StreamUseCase`, `NoParams`, and the cfg-gated `UseCaseStream` alias per PLAN.md, with the Dart usecase_test.dart semantics ported.

**Depends on**: 02-failure-trait

**Complexity:** medium

### Scope

**Files Modified (Write):**
- `crates/clean-signals/src/use_case.rs`

**Files Read (Dependencies):**
- `workflow/plans/feature/clean-signals-rs/PLAN.md` (§ Core API contract — trait shapes are locked)
- `crates/clean-signals/src/failure.rs` (trait + fixtures)
- `workflow/plans/feature/clean-signals-rs/research/dart-test-spec.md` (usecase_test.dart section)

### Details

- `UseCase` with the dual `cfg_attr` async_trait pattern from PLAN.md (both on trait and on test impls). Associated types `Params`, `Output`, `Failure: Failure + Clone`.
- `pub struct NoParams;` (unit param type, `Default + Clone + Copy + Debug`).
- `UseCaseStream<T, F>`: `futures::stream::BoxStream<'static, Result<T, F>>` on native, `LocalBoxStream` on wasm (cfg-gated type alias).
- `StreamUseCase` (NOT async_trait — `execute` is sync, returns the stream).
- Rust delta (documented in rustdoc): there is no `call()` wrapper catching exceptions — `execute` already returns `Result`. What IS ported from Dart's StreamUseCase semantics: provide `pub fn guard_stream<T, F>(s: impl Stream<Item = Result<T,F>> ...) -> UseCaseStream<T, F>` helper is NOT needed (no exceptions) — instead document that stream items are already `Result` and cancellation-on-drop is native.
- Test fakes (public under `#[cfg(any(test, feature = "test-fixtures"))] pub mod fixtures`, mirroring Dart helpers for reuse by task 06): `Doubler` (i32 → Ok(2n)), `Failing<F>` (always returns the given failure), `Flaky` (fails N times with NetworkFailure then succeeds — interior `AtomicU32` attempts counter, exposes `attempts()`), `Slow` (awaits a `futures::channel::oneshot` gate then returns Ok(5)), `Ticker(n)` StreamUseCase (yields Ok(1..=n) then Err(NetworkFailure "tick lost")), `Counting(n)` (yields Ok(1..=n)).
- Tests: Doubler executes; Flaky succeeds on attempt N+1 and `attempts()` counts calls; Ticker yields n Ok then one Err then terminates; dropping a Ticker stream mid-iteration stops it (cancellation semantics — use a stream with a side-effect counter to prove no further polls).

### Acceptance Criteria

1. `cargo test -p clean-signals use_case` passes; clippy clean.
2. Trait compiles for BOTH targets: `cargo check -p clean-signals --target wasm32-unknown-unknown` must remain green (the alias + `?Send` gate is the point).
3. Fakes exported for task 06's controller tests.

### Notes

- Keep `StreamUseCase::execute` taking `&self` and returning an owned boxed stream — controller `watch` moves it into a task.
