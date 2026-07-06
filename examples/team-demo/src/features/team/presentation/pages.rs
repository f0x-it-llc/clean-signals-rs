//! [`TeamPage`] — the team feature's single route component. Wires
//! [`TeamController`] up with `clean-signals-leptos`'s helpers: no raw
//! signals of its own, no inline refetch effect, no hand-rolled polling, no
//! swallowed errors (see `clean_signals_leptos`'s crate docs for the "after"
//! shape this follows).

use crate::failure::TeamFailure;
use crate::features::team::domain::entities::Member;
use crate::features::team::domain::repositories::TeamRepository;
use crate::features::team::presentation::components::member_row;
use crate::features::team::presentation::controllers::TeamController;
use clean_signals::Failure;
use clean_signals_leptos::{AsyncView, use_controller, use_failure_listener};
use leptos::prelude::*;
use std::rc::Rc;
use std::sync::Arc;

/// The team list screen. `repo` is injected by the composition root
/// (`main.rs`) — constructor injection, never looked up by the page itself.
#[component]
pub fn TeamPage(repo: Arc<dyn TeamRepository + Send + Sync>) -> impl IntoView {
    let controller =
        use_controller::<TeamController, TeamFailure>(move || TeamController::new(repo));

    // Surface failures once, scoped to this page, as a simple text banner —
    // the idiomatic replacement for a hand-rolled snackbar per-failure-site.
    let banner = RwSignal::new(None::<String>);
    controller.with_value(|c| {
        use_failure_listener(c.failures(), move |f: TeamFailure| {
            banner.set(Some(f.user_message()));
        });
    });

    // Kick off the initial load once — the CSR equivalent of Dart's
    // `initState`. `spawn_local` is the unavoidable, minimal glue needed to
    // invoke async work from synchronous component setup; `load()` itself is
    // still fully routed through `ControllerCore::run_into`.
    //
    // Gate the controller read with `try_get_value()` to avoid panicking if
    // the component unmounts before this spawned microtask runs (a disposal race).
    leptos::task::spawn_local(async move {
        let Some(c) = controller.try_get_value() else { return; };
        c.load().await;
    });

    let members_state = controller.with_value(|c| c.members);
    let filtered = controller.with_value(|c| c.filtered);
    let query = controller.with_value(|c| c.query);

    view! {
        <div class="team-page">
            <h1>"Team"</h1>
            <Show when=move || banner.get().is_some()>
                <div class="cs-snackbar">{move || banner.get().unwrap_or_default()}</div>
            </Show>
            <input
                type="text"
                placeholder="Search by name or role"
                on:input=move |ev| query.set(event_target_value(&ev))
            />
            <AsyncView
                state=members_state
                children=move |_all| team_list(filtered, controller).into_any()
            />
        </div>
    }
}

/// Renders the (already-filtered) member list. Factored out of the
/// `view!` above — a closure param annotated with a generic type
/// (`Vec<Member>`) confuses the `view!` macro's tag scanner, so this reads
/// the members off `filtered` itself instead of taking them as a
/// parameter.
fn team_list(
    filtered: Memo<Vec<Member>>,
    controller: StoredValue<Rc<TeamController>, LocalStorage>,
) -> impl IntoView {
    view! {
        <ul class="team-list">
            <For
                each=move || filtered.get()
                key=|m| m.id.clone()
                children=move |member| member_row(member, controller)
            />
        </ul>
    }
}
