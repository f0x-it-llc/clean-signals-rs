# SPIKE NOTES — reactive_graph 0.2.14 + any_spawner 0.3.0 standalone

Empirical validation of every runtime assumption the `clean-signals` framework
rests on. Each `## Q<n>` maps 1:1 to a `#[tokio::test]` in
`crates/clean-signals/tests/spike_reactive_graph.rs` (kept permanently as the
environment-assumption suite). Verdicts below were produced by running
`cargo test -p clean-signals --test spike_reactive_graph` (10/10 pass) against
crates.io `reactive_graph = 0.2.14`, `any_spawner = 0.3.0`.

Downstream consumers: task 05 (activity/async_state), task 06 (controller),
task 07 (leptos integration).

## CONTRADICTIONS

None. Every RESEARCH.md claim the plan is built on held up empirically:

- `effects` feature gates `Effect::new` — confirmed (Q4).
- `Owner::with` / `Owner::cleanup` / `Owner::on_cleanup` exist; signals/effects
  work without a current owner — confirmed (Q1, Q2).
- `.try_set()` / `.try_update()` return `Option` on disposed signals —
  confirmed, with exact semantics pinned below (Q1).
- `RwSignal::new` handles are Send+Sync and cross-thread readable;
  `new_local` panics on cross-thread access — confirmed (Q7).
- `Executor::init_tokio` / `tick` / `spawn_local` exist and behave as
  documented; `spawn_local` needs a current-thread runtime + `LocalSet` (Q5).

One clarification (not a contradiction) for task 06: PLAN §Edge Cases floats a
"return the stream-driving future for the caller to spawn" fallback in case
`spawn_local` proved awkward. It is NOT awkward — the recipe in Q5 is clean and
deterministic — so `watch` can use `Executor::spawn_local` directly. The
fallback is unnecessary.

---

## Q1 — Owner lifecycle & post-dispose write guard

**VERDICT:** `Owner::new()` → create signals in `owner.with(|| ...)` → they are
owned; `owner.cleanup()` disposes them. On a disposed signal `try_set(v)`
returns `Some(v)` (value handed back, unchanged) and `try_update(f)` returns
`None` (closure never runs). Neither panics.

Recipe:
```rust
let owner = Owner::new();
let sig = owner.with(|| RwSignal::new(0i32));  // owned by `owner`
sig.set(1);
assert_eq!(sig.try_update(|n| { *n += 10; *n }), Some(11)); // live -> Some(ret)
owner.cleanup();                                            // dispose
assert_eq!(sig.try_set(99), Some(99));   // disposed -> value returned back
assert_eq!(sig.try_update(|n| *n), None);// disposed -> None, closure skipped
```

Note: `set()` (infallible) on a disposed signal logs a debug-only warning and
is a no-op; prefer `try_set`/`try_update` on paths that can outlive dispose.

**Implications:**
- **Task 05 ActivityTracker:** the "post-dispose decrement suppressed" rule is
  free — implement decrement as `pending.try_update(|n| *n = n.saturating_sub(1))`.
  After dispose it returns `None` and the counter is untouched. No manual
  `disposed` flag is required *for the write-guard itself* (a flag may still be
  wanted to short-circuit `is_loading` reads).
- **Task 06 ControllerCore::dispose:** call `owner.cleanup()` (or dispose each
  owned handle); subsequent writes from in-flight tasks are inert, not panics.

## Q2 — Signals without a current owner

**VERDICT:** Fully functional; they simply leak (never auto-disposed) until the
process exits or they are disposed manually.

```rust
let sig = RwSignal::new(5i32); // no current owner
sig.set(7);
assert_eq!(sig.get_untracked(), 7);
```

**Implications:**
- **Task 05/06:** don't rely on ambient owner-less creation for cleanup. Any
  long-lived signal a controller creates must be created under an `Owner` the
  controller holds, OR be an `ArcRwSignal` with explicit lifetime management.
  Framework structs should own an `Owner` and create their signals in
  `owner.with(...)`.

## Q3 — Memo recomputation

**VERDICT:** `Memo::new(|prev: Option<&T>| ...)` is a synchronous, pull-based
computed. It recomputes lazily on the next `get`/`get_untracked` after a
dependency changed. It requires **neither an Owner nor an initialized
Executor** (it does not spawn). Closure must be `Fn + Send + Sync + 'static`;
`T: Send + Sync + 'static` (SyncStorage). Read with `.get_untracked()` outside a
reactive scope.

```rust
let src = RwSignal::new(2i32);
let doubled = Memo::new(move |_| src.get() * 10);
assert_eq!(doubled.get_untracked(), 20);
src.set(3);
assert_eq!(doubled.get_untracked(), 30); // recomputed on read
```

**Implications:**
- **Task 05/06 `is_loading: Memo<bool>`:** safe to construct in a plain struct
  and read synchronously in native tests without any Executor init. `Memo` is
  the right primitive for derived read-only state (prefer it over `Effect`).

## Q4 — Effect

**VERDICT:**
- `Effect::new(f)` requires the **`effects` feature** (enabled in this crate's
  dev-deps via `reactive_graph = { features = ["effects"] }`; apps get it from
  csr/hydrate). It spawns via `Executor::spawn_local`, so it needs an
  initialized executor **and a current-thread runtime + `LocalSet`** (same
  constraint as Q5). It runs on the **next `Executor::tick().await`**, not
  synchronously at creation, and re-runs on dependency change after a tick.
- **Stop:** `effect.stop()` (consumes the handle) halts it; dropping the handle
  also stops it (both dispose the underlying arena item). Owner cleanup stops
  owner-scoped effects too.
- `Effect::new_isomorphic(f)` runs **regardless of the `effects` feature**,
  spawns via `Executor::spawn` (works on a plain multi-thread `#[tokio::test]`,
  no `LocalSet` needed), still fires on a tick. Closure + captures must be
  `Send + Sync + 'static`.

`Effect::new` recipe (LocalSet):
```rust
#[tokio::test(flavor = "current_thread")]
async fn t() {
    LocalSet::new().run_until(async {
        Executor::init_tokio().ok();
        let owner = Owner::new(); owner.set();
        let sig = RwSignal::new(0);
        let e = Effect::new(move |_| { let _ = sig.get(); /* ... */ });
        Executor::tick().await;   // first run
        sig.set(1);
        Executor::tick().await;   // re-run
        e.stop();                 // halt
    }).await;
}
```

**Implications:**
- **Task 06:** per PLAN, prefer `Memo` + explicit methods in core library code;
  if core uses `Effect` at all, it forces the `effects` feature + LocalSet on
  every consumer/test. Recommendation: **keep `Effect` out of core library
  code**; any `auto_effect`/render-glue belongs in the `clean-signals-leptos`
  crate (task 07), where csr/hydrate already provide `effects` and the reactive
  runtime. Core tests may still use `Effect` behind the dev-dep feature.

## Q5 — `any_spawner::Executor::spawn_local` under tokio

**VERDICT:** `init_tokio()` wires `spawn_local` to `tokio::task::spawn_local`,
which **panics** if called outside a `tokio::task::LocalSet`. It works on a
`current_thread` runtime driven through `LocalSet::run_until`. It does **not**
work on a bare multi-thread `#[tokio::test]` (panics synchronously at the call
site).

**THE EXACT RECIPE for any downstream test that drives `spawn_local`/`watch`
(copy verbatim — task 06 controller `watch` tests, and later docs):**
```rust
#[tokio::test(flavor = "current_thread")]
async fn my_watch_test() {
    tokio::task::LocalSet::new().run_until(async {
        any_spawner::Executor::init_tokio().ok();   // idempotent init (Q6)
        // create controller / call watch(...) ;
        // drive the reactive/stream loop deterministically:
        any_spawner::Executor::tick().await;
        // assert on signals via .get_untracked()
    }).await;
}
```
Notes:
- `Executor::tick().await` is the deterministic pump (it uses `Executor::spawn`,
  i.e. `tokio::spawn`, plus a oneshot — works under both current_thread and
  multi-thread). Use it instead of sleeps for ordering.
- A single `tick()` yields once; if a `watch` needs several stream items
  delivered, call `tick()` once per expected step (or await the item's own
  completion channel) — do not sleep.

**Implications:**
- **Task 06 `watch`:** may call `Executor::spawn_local` directly (the PLAN
  fallback of returning a future for the caller to spawn is unnecessary). Its
  test module MUST use the current_thread + LocalSet harness above; a plain
  `#[tokio::test]` will panic.

## Q6 — Executor double-init

**VERDICT:** `Executor::init_tokio()` returns `Result<(), ExecutorError>` and
returns `Err(ExecutorError::AlreadySet)` on the second+ call. The global
executor is **process-wide, first-writer-wins** (a `OnceLock`), so it is set at
most once per test binary regardless of runtime flavor.

**Safe pattern:** `Executor::init_tokio().ok();` at the top of every test/entry
that needs spawning. Order-independent, idempotent-safe. (A helper
`fn ensure_executor() { let _ = Executor::init_tokio(); }` is used in the suite.)

**Implications:**
- **Task 05/06/07 tests:** never assert `init_tokio()` is `Ok` — earlier tests
  in the same binary may have already set it. Always `.ok()`.
- One global init serves every runtime flavor in the binary: the wired
  `tokio::task::spawn` / `spawn_local` use the *current* runtime context at
  call time, so mixing multi-thread and current_thread+LocalSet tests is fine.

## Q7 — Cross-thread arena signal access

**VERDICT:**
- `RwSignal::new(v)` (default **SyncStorage**, `T: Send + Sync + 'static`): the
  handle is `Send + Sync + Copy` and the value is readable/writable from other
  OS threads. `std::thread::spawn(move || sig.get_untracked())` works.
- `RwSignal::new_local(v)` (**LocalStorage**, wraps the value in
  `send_wrapper::SendWrapper`): the handle is still `Send`, but any access
  (`get_untracked`, etc.) from a different thread **panics** (SendWrapper
  deref-from-wrong-thread). Confirmed via `catch_unwind` inside a spawned
  thread.

```rust
let shared = RwSignal::new(42i32);
assert_eq!(std::thread::spawn(move || shared.get_untracked()).join().unwrap(), 42);

let local = RwSignal::new_local(7i32);
// reading `local.get_untracked()` from another thread panics.
```

**Implications:**
- **Task 05/06:** framework signals default to `RwSignal::new` (SyncStorage) so
  controllers/use cases can be exercised from tokio worker threads and hold
  `Send` state. Reserve `new_local` for genuinely `!Send` values pinned to the
  UI thread — and never touch such a signal off-thread.

## Q8 — RAII guard writing from `Drop`

**VERDICT:** Writing to a signal from a `Drop` impl (the ActivityTracker guard
design) is reentrancy-safe: `update`/`try_update` acquire and release the
write lock within the call, so a subsequent guard `Drop` writing the same
signal does not deadlock or re-enter. Post-dispose, a guard that decrements via
`try_update` is a clean no-op (returns `None`) — no panic.

```rust
struct DecrementOnDrop { pending: RwSignal<u32> }
impl Drop for DecrementOnDrop {
    fn drop(&mut self) { self.pending.try_update(|n| *n = n.saturating_sub(1)); }
}
// increments on create, decrements on drop; after owner.cleanup() the drop is inert.
```

**Implications:**
- **Task 05 ActivityTracker guard:** implement `pending += 1` on guard creation
  and `pending.try_update(|n| *n = n.saturating_sub(1))` on `Drop`. This
  single primitive delivers both the ref-counted loading behavior AND the
  "post-dispose decrement suppressed" Dart-spec rule with no extra flag on the
  write path.

---

## Summary table for task 06 (the controller)

| Concern | Verdict | Do this |
|---|---|---|
| Signal cleanup | `owner.cleanup()` disposes owned signals | Hold an `Owner`; create signals in `owner.with(..)` |
| Post-dispose writes | `try_set`→`Some(v)`, `try_update`→`None`, no panic | Use `try_*` on paths outliving dispose |
| Derived read state | `Memo` is pull-based, no Owner/Executor needed | Use `Memo<bool>` for `is_loading` |
| Effects | need `effects` feature + LocalSet + tick | Keep out of core; put UI glue in leptos crate |
| `watch` spawning | `spawn_local` needs current_thread + LocalSet | Call `Executor::spawn_local`; test with the Q5 recipe |
| Executor init | `init_tokio()` errs if already set | `Executor::init_tokio().ok()` everywhere |
| Thread safety | `new()` Send+Sync cross-thread; `new_local()` panics off-thread | Default to `RwSignal::new` |
| RAII guard | Drop-time writes are safe + post-dispose inert | ActivityTracker uses `try_update` decrement on Drop |
