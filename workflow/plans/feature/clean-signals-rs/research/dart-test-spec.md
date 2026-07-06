SUMMARY: The clean_signals test suite contains 32 tests across 5 files that pin critical behaviors for a Rust port. Key abstractions include Result (Success/Failed with map/flatMap/fold), UseCase (execute wraps thrown Failures as Failed, unknown errors as UnexpectedFailure), ActivityTracker (ref-counted loading), RetryPolicy (exponential backoff with isRetryable checks), and Controller (run/runInto/watch/dispose with failures broadcast stream). Critical semantics: isLoading transitions with fine-grained await ordering; AsyncDataReloading preserves stale data while reloading; failures emit only final result (not intermediates during retries); dispose runs onDispose callbacks in reverse order (idempotent), cancels watch subscriptions, and guards post-dispose signal writes; StreamUseCase catches stream errors and emits as trailing Failed event with immediate cancellation.

CLAIMS:
- result_test.dart: fold(onSuccess, onFailure) routes Success to onSuccess, Failed to onFailure; map preserves failure object identity; flatMap chains Results with short-circuit on failure; Result.guard catches thrown Failures and wraps unknown errors in UnexpectedFailure(cause, isRetryable=false); toAsyncState converts Success→AsyncData, Failed→AsyncError
  [/home/ed/Dev/personal/clean_signals/test/result_test.dart:11-95]
- usecase_test.dart: UseCase.call wraps thrown Failure objects as Failed (preserving the exception), and wraps non-Failure exceptions in Failed<UnexpectedFailure(cause:exception, isRetryable:false)>
  [/home/ed/Dev/personal/clean_signals/test/usecase_test.dart:46-65]
- usecase_test.dart: StreamUseCase.call forwards Success/Failed results from execute stream; catches stream errors (e.g. StateError) and emits a trailing Failed<UnexpectedFailure(cause:error)> event, then terminates
  [/home/ed/Dev/personal/clean_signals/test/usecase_test.dart:67-81]
- activity_test.dart: ActivityTracker.track uses ref-counted isLoading (increments on call, decrements on completion regardless of success/throw, but post-dispose decrements are suppressed); pending signal tracks active operation count
  [/home/ed/Dev/personal/clean_signals/test/activity_test.dart:7-64]
- activity_test.dart: RetryPolicy.delayFor(attempt) applies exponential backoff: delayFor(1) = delay, delayFor(2) = delay*backoffFactor, delayFor(3) = delay*backoffFactor², etc.; shouldRetry checks Failure.isRetryable by default, but custom retryIf predicate overrides
  [/home/ed/Dev/personal/clean_signals/test/activity_test.dart:67-95]
- controller_test.dart run: Success results do not emit to failures stream; run(emitFailure=true, default) emits one final failure after all retries exhausted; run(emitFailure=false) suppresses failure emission; retries only happen if Failure.isRetryable=true, up to maxAttempts total attempts (not retries); isLoading tracks via ActivityTracker (increments on run entry, decrements on run exit)
  [/home/ed/Dev/personal/clean_signals/test/controller_test.dart:85-198]
- controller_test.dart runInto: loads AsyncLoading; on success sets AsyncData; keeps stale data visible during reload (AsyncDataReloading state with hasValue=true, requireValue returns old value); on failure sets AsyncError
  [/home/ed/Dev/personal/clean_signals/test/controller_test.dart:201-245]
- controller_test.dart watch: StreamUseCase subscription routes Success events to onData callback, Failed events to failures broadcast stream, on dispose cancels subscription immediately (no further events)
  [/home/ed/Dev/personal/clean_signals/test/controller_test.dart:248-274]
- controller_test.dart dispose: runs onDispose callbacks in reverse order (LIFO), is idempotent (multiple calls invoke callbacks only once), sets isDisposed flag, cancels all watched stream subscriptions; post-dispose signal writes can be guarded with isDisposed check
  [/home/ed/Dev/personal/clean_signals/test/controller_test.dart:277-314]
- controller_test.dart defines helper: FlakyUseCase(failuresBeforeSuccess, failure) tracks attempts counter, fails first N times with given Failure, succeeds on attempt N+1 (or later); NetworkFailure has isRetryable=true
  [/home/ed/Dev/personal/clean_signals/test/controller_test.dart:18-32, 7-12]
- controller_test.dart defines helper: SlowUseCase(gate:Completer) awaits gate.future, then returns Success(5); simulates long-running operation
  [/home/ed/Dev/personal/clean_signals/test/controller_test.dart:34-44]
- controller_test.dart defines helper: TickerUseCase (StreamUseCase) yields Success(1) through Success(params), then yields Failed(NetworkFailure('tick lost'))
  [/home/ed/Dev/personal/clean_signals/test/controller_test.dart:46-54]
- controller_test.dart defines helper: TestController exposes exec(useCase, params, retry, emitFailure), execInto(useCase, params, into), listen(useCase, params, onData), addCleanup(fn), disposed flag; wraps Controller methods for testing
  [/home/ed/Dev/personal/clean_signals/test/controller_test.dart:56-82]
- usecase_test.dart defines test helpers: Doubler(params:int)→Success(params*2), ThrowsFailure()→throws NetworkFailure, ThrowsError()→throws FormatException(non-Failure), CountingStream(params:int)→yields Success(1..params), FailingStream()→yields Success(1) then throws StateError
  [/home/ed/Dev/personal/clean_signals/test/usecase_test.dart:11-43]
- result_test.dart: asyncStateSignal<T>() initializes to AsyncLoading<T> state (not data or error)
  [/home/ed/Dev/personal/clean_signals/test/result_test.dart:97-100]
- app_test.dart: Integration tests verify retry absorbs first N failures (FakeTeamApi with failuresBeforeSuccess:2, RetryPolicy(maxAttempts:3) succeeds on third attempt); search filter uses computed signal; profile edits propagate via app-scoped shared signal; ValidationFailure surfaces through failures stream as snackbar
  [/home/ed/Dev/personal/clean_signals/example/test/app_test.dart:45-150]

CAVEATS: The specification is extracted from test code only; runtime behavior (e.g., exact timing of async transitions, signal reactivity) is inferred from test assertions but not explicitly verified against implementation source code. Integration tests (app_test.dart) depend on FakeTeamApi and FakeProfileApi which are not analyzed here—their behavior is inferred from test usage. The Rust port must ensure equivalent semantics for: (1) Result type algebra (fold, map, flatMap, pattern matching); (2) UseCase error wrapping rules; (3) StreamUseCase stream error handling and immediate cancellation; (4) ActivityTracker ref-counting with post-dispose guard; (5) RetryPolicy backoff formula and retryable predicate; (6) Controller's isLoading/failures async ordering and dispose LIFO + idempotency.
