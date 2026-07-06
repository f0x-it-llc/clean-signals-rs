//! Component-scoped controller lifecycle.
//!
//! [`use_controller`] ties an app controller's lifetime to the current
//! reactive owner: it is built once when the component mounts and
//! [`ControllerCore::dispose`]d exactly once when the component unmounts —
//! replacing the manual `StoredValue` + `on_cleanup` juggling pages
//! otherwise hand-roll.
//!
//! [`provide_controller`] / [`expect_controller`] are thin context wrappers
//! for *app-scoped* controllers that outlive any single page. Controllers
//! placed in context are **never** disposed by the pages that read them; the
//! provider owns their lifecycle.

use std::rc::Rc;

use clean_signals::{ControllerCore, Failure};
use leptos::prelude::*;

/// Constructs an app controller once and disposes it on owner cleanup.
///
/// `factory` runs a single time (when the owning component mounts). The
/// controller is stored in a component-scoped [`StoredValue`] and its
/// embedded [`ControllerCore`] is [`dispose`](ControllerCore::dispose)d via
/// [`on_cleanup`] when the component unmounts — aborting in-flight `watch`es,
/// running registered teardowns, and releasing owned signals.
///
/// The controller is wrapped in an [`Rc`] so it can be read out of the
/// [`StoredValue`] as a cheap `Clone` handle. Read it in `view!` closures with
/// `.get_value()` (yields an `Rc<C>`, which derefs to `C`) or
/// `.with_value(|c| …)`. The cleanup closure captures the `StoredValue` handle
/// (which is `Send + Sync`, as [`on_cleanup`] requires) and disposes through
/// it; reactive cleanups run before stored values are torn down, so the
/// handle is still live at that point.
///
/// # Type parameters
///
/// - `C`: the app controller, which must expose its [`ControllerCore`] via
///   `AsRef` (`impl AsRef<ControllerCore<F>> for MyController`).
/// - `F`: the app's [`Failure`] type.
///
/// # Example
///
/// ```rust,ignore
/// let controller = use_controller::<UsersController, AppFailure>(|| {
///     UsersController::new(repo)
/// });
/// // in view: controller.with_value(|c| c.load());
/// ```
pub fn use_controller<C, F>(factory: impl FnOnce() -> C) -> StoredValue<Rc<C>, LocalStorage>
where
    C: AsRef<ControllerCore<F>> + 'static,
    F: Failure + Clone,
{
    let stored = StoredValue::new_local(Rc::new(factory()));
    on_cleanup(move || {
        // Cleanups run before stored values are disposed, so the handle is
        // still live here; `try_with_value` is a belt-and-braces guard.
        let _ = stored.try_with_value(|controller| (**controller).as_ref().dispose());
    });
    stored
}

/// Provides an app-scoped controller through leptos context.
///
/// Use this for controllers whose lifetime spans the whole app (or a large
/// subtree) rather than a single page. The provider is responsible for
/// disposal; pages that [`expect_controller`] must **not** dispose it.
///
/// Controllers are typically shared as `Arc<C>` (leptos context requires
/// `Send + Sync`, so use `Arc`, not `Rc`, for app-scoped controllers) so
/// [`expect_controller`] can hand out cheap clones.
pub fn provide_controller<C: Send + Sync + 'static>(controller: C) {
    provide_context(controller);
}

/// Retrieves an app-scoped controller previously supplied by
/// [`provide_controller`].
///
/// Panics if no controller of type `C` is in context (the same contract as
/// leptos [`expect_context`]). Pages must **not** dispose the returned
/// controller — the provider owns its lifecycle.
pub fn expect_controller<C: Clone + 'static>() -> C {
    expect_context::<C>()
}

#[cfg(test)]
mod tests {
    use super::*;
    use clean_signals::failure::fixtures::NetworkFailure;

    /// A minimal app controller embedding a [`ControllerCore`] by composition.
    struct TestController {
        core: ControllerCore<NetworkFailure>,
    }

    impl TestController {
        fn new() -> Self {
            Self {
                core: ControllerCore::new(),
            }
        }
    }

    impl AsRef<ControllerCore<NetworkFailure>> for TestController {
        fn as_ref(&self) -> &ControllerCore<NetworkFailure> {
            &self.core
        }
    }

    #[test]
    fn use_controller_disposes_on_owner_cleanup() {
        let owner = Owner::new();
        // Build under the owner and keep an Rc handle to observe disposal
        // after the owner is torn down.
        let handle = owner.with(|| {
            let stored = use_controller::<TestController, NetworkFailure>(TestController::new);
            stored.get_value()
        });

        assert!(
            !handle.core.is_disposed(),
            "controller is live while the owner is alive"
        );

        owner.cleanup();

        assert!(
            handle.core.is_disposed(),
            "owner cleanup disposes the controller"
        );
    }

    #[test]
    fn provide_and_expect_controller_round_trip() {
        use std::sync::Arc;

        let owner = Owner::new();
        owner.with(|| {
            // App-scoped controllers go through leptos context, which requires
            // `Send + Sync` — so `Arc`, not `Rc`.
            let controller = Arc::new(TestController::new());
            provide_controller(Arc::clone(&controller));

            let fetched = expect_controller::<Arc<TestController>>();
            assert!(!fetched.core.is_disposed());
            // Context hands out a clone of the same Arc.
            assert!(Arc::ptr_eq(&controller, &fetched));
        });
        owner.cleanup();
    }
}
