//! Spike / environment-assumption suite for `reactive_graph` + `any_spawner`.
//!
//! This file is KEPT permanently: it pins every runtime assumption the
//! `clean-signals` framework rests on. If a `reactive_graph`/`any_spawner`
//! upgrade breaks one of these, the framework's own invariants are at risk.
//!
//! Each `q<n>_*` test answers exactly one question from task 01; the verdicts
//! and downstream implications live in
//! `workflow/plans/feature/clean-signals-rs/research/SPIKE_NOTES.md`.
//!
//! Determinism note: async effects/spawns are driven with
//! `Executor::tick().await` — never sleeps — so ordering is reproducible.

use std::panic;
use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
use std::sync::{Arc, Mutex};
use std::thread;

use any_spawner::Executor;
use reactive_graph::computed::Memo;
use reactive_graph::effect::Effect;
use reactive_graph::owner::Owner;
use reactive_graph::signal::RwSignal;
use reactive_graph::traits::{Get, GetUntracked, Set, Update};
use tokio::task::LocalSet;

/// Ensure the global `any_spawner` executor is configured exactly once.
///
/// `Executor::init_tokio()` returns `Err` if a global executor is already set
/// (see Q6), so every test that needs spawning calls this idempotent helper
/// instead of `init_tokio()` directly.
fn ensure_executor() {
    // `.ok()` swallows the "already set" error from any earlier test in this
    // binary. The global executor is process-wide, so first-writer-wins.
    let _ = Executor::init_tokio();
}

/// Run `f` in `catch_unwind` with the default panic hook silenced, so expected
/// panics don't spam the test output. Returns whether `f` panicked.
fn panicked(f: impl FnOnce() + panic::UnwindSafe) -> bool {
    let prev = panic::take_hook();
    panic::set_hook(Box::new(|_| {}));
    let result = panic::catch_unwind(f);
    panic::set_hook(prev);
    result.is_err()
}

// ---------------------------------------------------------------------------
// Q1 — Owner lifecycle: create under owner, cleanup disposes, post-dispose
//      writes are guarded (try_set/try_update signal disposal).
// ---------------------------------------------------------------------------
#[tokio::test]
async fn q1_owner_lifecycle_and_post_dispose_writes() {
    let owner = Owner::new();

    // Signals created inside `owner.with(..)` are owned by `owner`.
    let sig = owner.with(|| RwSignal::new(0i32));

    // Reads/writes work while the owner is alive.
    sig.set(1);
    assert_eq!(sig.get_untracked(), 1, "write then read must round-trip");
    assert_eq!(
        sig.try_update(|n| {
            *n += 10;
            *n
        }),
        Some(11),
        "try_update on a live signal returns Some(returned value)"
    );

    // Dispose everything owned by this owner.
    owner.cleanup();

    // Post-dispose: `try_set` hands the value back (Some), `try_update`
    // returns None. Neither panics — this is the RAII post-dispose guard the
    // ActivityTracker (task 05) relies on.
    assert_eq!(
        sig.try_set(99),
        Some(99),
        "try_set on a disposed signal returns the value back unchanged"
    );
    assert_eq!(
        sig.try_update(|n| *n),
        None,
        "try_update on a disposed signal returns None (no mutation ran)"
    );
}

// ---------------------------------------------------------------------------
// Q2 — Signals with no current owner: usable, just never auto-disposed.
// ---------------------------------------------------------------------------
#[tokio::test]
async fn q2_signal_without_owner_works_but_leaks() {
    // No `Owner::new()` / `owner.with(..)` here: there is no current owner.
    let sig = RwSignal::new(5i32);
    sig.set(7);
    assert_eq!(
        sig.get_untracked(),
        7,
        "an owner-less signal still reads/writes; it just leaks (never \
         auto-disposed until process exit / manual dispose)"
    );
}

// ---------------------------------------------------------------------------
// Q3 — Memo: recomputes on dependency change, pull-based, needs neither an
//      Owner nor the Executor.
// ---------------------------------------------------------------------------
#[tokio::test]
async fn q3_memo_recomputes_without_owner_or_executor() {
    // Deliberately NOT inside an owner and NOT calling ensure_executor():
    // Memo is a synchronous pull-based computed (it does not spawn).
    let src = RwSignal::new(2i32);
    let doubled = Memo::new(move |_| src.get() * 10);

    assert_eq!(doubled.get_untracked(), 20, "initial memo computation");

    src.set(3);
    assert_eq!(
        doubled.get_untracked(),
        30,
        "memo recomputes lazily on next read after a dependency changes"
    );
}

// ---------------------------------------------------------------------------
// Q4 — Effect: `Effect::new` needs the `effects` feature + a spawned local
//      task + a tick to fire; `.stop()` halts it. Plus `new_isomorphic`.
// ---------------------------------------------------------------------------
#[tokio::test(flavor = "current_thread")]
async fn q4_effect_new_runs_and_stops_under_localset() {
    LocalSet::new()
        .run_until(async {
            ensure_executor();
            let owner = Owner::new();
            owner.set();

            let sig = RwSignal::new(0i32);
            let seen: Arc<Mutex<Vec<i32>>> = Arc::new(Mutex::new(Vec::new()));

            let effect = Effect::new({
                let seen = Arc::clone(&seen);
                move |_| seen.lock().unwrap().push(sig.get())
            });

            // Effects run on the NEXT tick, not synchronously at creation.
            Executor::tick().await;
            assert_eq!(
                &*seen.lock().unwrap(),
                &[0],
                "effect runs once on first tick"
            );

            sig.set(1);
            Executor::tick().await;
            assert_eq!(
                &*seen.lock().unwrap(),
                &[0, 1],
                "effect re-runs on dependency change after a tick"
            );

            // Explicit stop halts the effect (drop of the handle would too).
            effect.stop();
            sig.set(2);
            Executor::tick().await;
            assert_eq!(
                &*seen.lock().unwrap(),
                &[0, 1],
                "a stopped effect no longer re-runs"
            );
        })
        .await;
}

#[tokio::test]
async fn q4_effect_new_isomorphic_runs_on_multithread_runtime() {
    // `new_isomorphic` uses `Executor::spawn` (multi-thread ok) and runs
    // regardless of the `effects` feature — but still fires on a tick.
    ensure_executor();
    let owner = Owner::new();
    owner.set();

    let sig = RwSignal::new(0i32);
    let seen: Arc<Mutex<Vec<i32>>> = Arc::new(Mutex::new(Vec::new()));

    // Closure + captures must be Send + Sync for `new_isomorphic`.
    let _effect = Effect::new_isomorphic({
        let seen = Arc::clone(&seen);
        move |_| seen.lock().unwrap().push(sig.get())
    });

    Executor::tick().await;
    assert_eq!(
        &*seen.lock().unwrap(),
        &[0],
        "isomorphic effect runs on first tick"
    );

    sig.set(1);
    Executor::tick().await;
    assert_eq!(
        &*seen.lock().unwrap(),
        &[0, 1],
        "isomorphic effect re-runs on dependency change"
    );
}

// ---------------------------------------------------------------------------
// Q5 — any_spawner spawn_local under tokio: requires a current-thread runtime
//      + LocalSet. This test pins the EXACT recipe task 06 must copy.
// ---------------------------------------------------------------------------
#[tokio::test(flavor = "current_thread")]
async fn q5_spawn_local_recipe_current_thread_localset() {
    // THE RECIPE (verbatim for downstream `watch` tests):
    //   #[tokio::test(flavor = "current_thread")]
    //   async fn my_test() {
    //       LocalSet::new().run_until(async {
    //           Executor::init_tokio().ok();   // (ensure_executor here)
    //           /* create controller, call watch(...), drive with tick() */
    //       }).await;
    //   }
    LocalSet::new()
        .run_until(async {
            ensure_executor();

            let ran = Arc::new(AtomicBool::new(false));
            Executor::spawn_local({
                let ran = Arc::clone(&ran);
                async move { ran.store(true, Ordering::SeqCst) }
            });

            // Give the spawned local task a chance to run.
            Executor::tick().await;
            assert!(
                ran.load(Ordering::SeqCst),
                "spawn_local task runs under a current_thread runtime + LocalSet"
            );
        })
        .await;
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn q5_spawn_local_panics_without_localset() {
    ensure_executor();
    // On a multi-thread runtime with no LocalSet, `spawn_local` (backed by
    // `tokio::task::spawn_local`) panics synchronously at the call site.
    assert!(
        panicked(|| Executor::spawn_local(async {})),
        "spawn_local must panic on a multi-thread runtime without a LocalSet"
    );
}

// ---------------------------------------------------------------------------
// Q6 — Executor double-init: second init returns Err. `.ok()` is the safe,
//      order-independent pattern.
// ---------------------------------------------------------------------------
#[tokio::test]
async fn q6_executor_double_init_returns_err() {
    ensure_executor(); // guarantees an executor is set (by us or an earlier test)
    assert!(
        Executor::init_tokio().is_err(),
        "init_tokio() on an already-configured executor returns Err; \
         use Executor::init_tokio().ok() to make init idempotent"
    );
}

// ---------------------------------------------------------------------------
// Q7 — Cross-thread arena access: `new()` (SyncStorage) is thread-safe;
//      `new_local()` (LocalStorage/SendWrapper) panics on cross-thread read.
// ---------------------------------------------------------------------------
#[tokio::test]
async fn q7_cross_thread_sync_signal_ok_local_signal_panics() {
    // Default `RwSignal::new` uses SyncStorage: the handle AND the value are
    // reachable from other threads.
    let shared = RwSignal::new(42i32);
    let handle = thread::spawn(move || shared.get_untracked());
    assert_eq!(
        handle.join().unwrap(),
        42,
        "SyncStorage signal is readable from another thread"
    );

    // `new_local` pins the value to the creating thread (SendWrapper). The
    // handle is Send, but dereferencing it off-thread panics. We assert the
    // panic *inside* the spawned thread via catch_unwind so it never unwinds
    // across the arena boundary into the test harness.
    let local = RwSignal::new_local(7i32);
    let cross = thread::spawn(move || {
        panicked(move || {
            let _ = local.get_untracked();
        })
    });
    assert!(
        cross.join().unwrap(),
        "LocalStorage (new_local) signal panics when read from another thread"
    );
}

// ---------------------------------------------------------------------------
// Q8 — RAII guard pattern: a signal write from a Drop impl (ActivityTracker
//      guard design) is reentrancy-safe, and is guarded post-dispose.
// ---------------------------------------------------------------------------
#[tokio::test]
async fn q8_raii_guard_writes_from_drop() {
    struct DecrementOnDrop {
        pending: RwSignal<u32>,
    }
    impl Drop for DecrementOnDrop {
        fn drop(&mut self) {
            // `try_update` == no-op if the signal was already disposed.
            self.pending.try_update(|n| *n = n.saturating_sub(1));
        }
    }

    let owner = Owner::new();
    let pending = owner.with(|| RwSignal::new(0u32));

    // Live case: guard increments on creation, decrements on drop, no
    // reentrancy panic (the write lock is released before Drop runs).
    {
        pending.update(|n| *n += 1);
        let _guard = DecrementOnDrop { pending };
        assert_eq!(pending.get_untracked(), 1, "counter held while guard alive");
    }
    assert_eq!(
        pending.get_untracked(),
        0,
        "guard's Drop decremented the signal without reentrancy issues"
    );

    // Post-dispose case: dispose, then drop a guard — the decrement is
    // silently suppressed (no panic), matching the ActivityTracker contract.
    let runs = AtomicUsize::new(0);
    pending.update(|n| *n += 1);
    let guard = DecrementOnDrop { pending };
    owner.cleanup();
    drop(guard); // try_update returns None; nothing observable, nothing panics
    runs.fetch_add(1, Ordering::SeqCst);
    assert_eq!(
        runs.load(Ordering::SeqCst),
        1,
        "dropping a guard over a disposed signal completes cleanly"
    );
}
