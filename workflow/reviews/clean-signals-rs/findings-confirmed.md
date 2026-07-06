### [Major / quality] Real-time sleeps used to synchronize concurrency in async tests, violating the project's own no-sleep-to-settle testing rule

**Where:** `crates/clean-signals/src/controller.rs:608-681 (tests run_tracks_is_loading_across_the_operation, run_into_drives_loading_to_data, run_into_keeps_stale_data_visible_while_reloading, and the trailing sleep in watch_dispose_stops_further_on_data at line 765)`

**Problem:** docs/CODE_STANDARDS.md states plainly: 'Never sleep to wait for async work to settle in a test; drive it deterministically instead (see Testing Patterns)' and DEVELOPMENT.md's testing patterns reiterate 'never sleep to wait for async work to settle in a test; drive it deterministically via Executor::tick().await instead'. Three of the new `run`/`run_into` tests spawn a task on a multi-threaded tokio runtime, then call `tokio::time::sleep(Duration::from_millis(30)).await` purely to give the spawned task 'time to begin (and park on the gate)' before asserting `is_loading()`. This is exactly the anti-pattern the project's own docs forbid: a real wall-clock sleep as a synchronization mechanism for concurrent test state, which is inherently non-deterministic and can flake under CI load (a loaded runner may not schedule the spawned task within 30ms, causing `assert!(controller.is_loading()...)` to fail spuriously).

**Recommendation:** Replace the blind sleep with a deterministic signal: e.g. have the `Slow` fixture (or a small wrapper) send a 'started' notification over a second oneshot/mpsc channel just before it awaits its gate, and have the test `await` that notification instead of sleeping. This keeps the test fully deterministic and immune to scheduler timing, in line with the project's stated testing standard.

### [Major / quality] FailureSink::emit holds its Mutex while invoking arbitrary user listeners — a panicking listener poisons the lock for the sink's entire lifetime

**Where:** `crates/clean-signals/src/controller.rs:134-141 (FailureSink::emit), 119-132 (subscribe's removal closure also locks the same Mutex)`

**Problem:** `emit` does `let listeners = self.listeners.lock().unwrap(); for (_, listener) in listeners.iter() { listener(failure); }` — the standard-library `Mutex` is held across calls into app-supplied closures (e.g. `use_failure_listener`'s handler, which the docs explicitly say may `set` UI signals or do arbitrary work). If any one listener panics, the `Mutex` becomes poisoned; every subsequent `.lock().unwrap()` call anywhere on that `FailureSink` (in `subscribe`, its `Subscription::drop` removal closure, and `emit` itself) will then panic too, silently converting one buggy app-level failure handler into a cascading, permanent breakage of the controller's entire failure-reporting channel — and since `Subscription::drop`'s removal closure also tries to lock the same already-poisoned mutex during unwind, a panicking listener risks a double-panic during drop (which aborts the process instead of merely failing a test/request). The module docs call out the re-entrancy hazard (a listener must not subscribe/unsubscribe synchronously) but say nothing about the panic-poisoning hazard, which is more likely to occur in practice (any listener bug, not just a deliberate reentrant call).

**Recommendation:** Either (a) clone the listener list into a local `Vec` before invoking closures so the lock is released before running user code (also fixes the re-entrancy concern for free), or (b) use `parking_lot::Mutex` (no poisoning) or explicitly recover from poisoning (`.unwrap_or_else(|e| e.into_inner())`) at every lock site so a panicking listener degrades gracefully instead of taking down the whole sink.

### [Major / logic] watches Vec grows unbounded — completed and cancelled watches never removed

**Where:** `crates/clean-signals/src/controller.rs:371-375, 416-437, 195-201`

**Problem:** Every ControllerCore::watch pushes an AbortHandle into self.watches, but nothing ever removes it except dispose() (which drains the whole vec). A watch that completes normally (stream ends), or is explicitly cancelled via WatchHandle::cancel(), leaves its now-dead AbortHandle in the vec for the entire remaining life of the controller. For a component-scoped controller this is bounded by the component lifetime, but the framework explicitly supports app-scoped controllers via provide_controller (hooks.rs) that outlive many pages; such a controller that re-subscribes a watch on each navigation accumulates AbortHandles without bound for the whole app session. WatchHandle::cancel aborts the task but does not unregister the handle, so 'cancel then let it linger' still leaks.

**Recommendation:** Register each watch under a removable id (mirroring FailureSink's id-keyed listener list) and have the driver task / WatchHandle::cancel remove its own entry on completion so the registry only ever holds live watches.

### [Major / risks] watch() never prunes its AbortHandle registry — unbounded growth + orphaned driver tasks

**Where:** `crates/clean-signals/src/controller.rs:339-375, 416-437`

**Problem:** `ControllerCore::watch` pushes an `AbortHandle` into `self.watches: Mutex<Vec<AbortHandle>>` on every call, and nothing ever removes an entry — not when the underlying stream terminates naturally, and not when the caller calls `WatchHandle::cancel()`. Only `dispose()` drains the whole Vec (once, for the controller's lifetime). A controller that calls `watch` repeatedly over its life (e.g. re-subscribing to a stream whose params change — a very plausible pattern for a live feed/search-as-you-type/websocket use case) leaks an ever-growing Vec of dead handles for as long as the controller lives. This is exactly the kind of long-lived-SPA-session leak the framework's own pitch (replacing cupline's hand-rolled leaks) is supposed to prevent, and it isn't covered by any test (all watch tests call `watch` exactly once per controller instance).

**Recommendation:** Store watches keyed by an id and remove the entry when the driver's `Abortable` future completes (success, error, or after `WatchHandle::cancel()`), e.g. via a wrapping future that removes itself from the registry on drop, or document explicitly that `watch` is a one-shot-per-controller-lifetime API and callers must build a new controller (or manually track/dispose) to resubscribe.

### [Major / risks] Ungated get_value() inside a fire-and-forget spawn_local risks a panic on fast unmount

**Where:** `examples/team-demo/src/features/team/presentation/pages.rs:38-40`

**Problem:** `TeamPage` fires the initial load via `leptos::task::spawn_local(async move { controller.get_value().load().await; });` with no liveness check. `StoredValue::get_value()` panics (`unwrap_signal!`) if the value has been disposed (confirmed via `reactive_graph::traits::GetValue::get_value` -> `try_get_value().unwrap_or_else(unwrap_signal!)`). If `TeamPage` unmounts (route change, fast re-render) before this spawned microtask executes, the panic fires. Contrast with the crate's own `use_interval` helper, which is safe by construction because it always checks the `alive` flag immediately before invoking `f()` in the same synchronous step — there is no equivalent gate on this one-shot 'run on mount' pattern, and the task 09 completion summary explicitly acknowledges 'there's no framework helper for run-once-on-mount' but does not flag the disposal race this creates. Since `templates/AGENTS.md` positions team-demo as *the* reference implementation downstream teams (e.g. cupline) will copy, this footgun is likely to propagate.

**Recommendation:** Use `controller.try_get_value()` (or equivalent) inside the spawned closure and no-op on `None`, or give `clean-signals-leptos` an official `use_effect_once`/`on_mount` helper that internally holds the same alive-flag pattern as `use_interval` before invoking the callback.

### [Major / risks] dispose() vs. concurrent watch()/run() is a TOCTOU race on a type explicitly made Send + Sync

**Where:** `crates/clean-signals/src/controller.rs:260-274, 339-375, 416-437`

**Problem:** The controller task 06 completion summary states `ControllerCore<F>` is deliberately `Send + Sync` 'needed for leptos StoredValue'. That invites native/SSR multi-threaded use, but `watch()`'s registration (`self.watches.lock().unwrap().push(...)`) and `dispose()`'s drain (`self.watches.lock().unwrap().drain(..)`) are two independent critical sections, not one atomic operation — a `watch()` call racing a concurrent `dispose()` on another thread can register an `AbortHandle` after the drain already ran, leaving a driver task that is never aborted and (if its stream is effectively unbounded / long-lived) runs forever, violating the pinned 'no on_data after dispose' contract. The per-item `disposed.load()` check inside the driver loop only prevents *processing* stray items post-dispose, it doesn't stop the task from polling forever if the stream keeps producing.

**Recommendation:** Either narrow the safety claim to 'single-threaded use only, Send+Sync is only for `StoredValue`'s API needs, do not call controller methods concurrently from multiple threads' (documented explicitly), or make `watch()` check `is_disposed()` before spawning and use a single lock/atomic swap-style registration that can't race the drain.

