//! Small presentational pieces for the team page. Plain view-returning
//! functions (not `#[component]`s) — no state of their own beyond a local
//! text-input signal, no business logic (that lives in the use cases).

use crate::features::team::domain::entities::Member;
use crate::features::team::presentation::controllers::TeamController;
use leptos::prelude::*;
use std::rc::Rc;

/// Renders one member row with an inline rename control.
///
/// `controller` is the `use_controller`-scoped handle from the page; renaming
/// goes through `controller.rename(...)`, which is itself routed through
/// `ControllerCore::run` (retry/activity/failure routing), never called
/// directly against a use case.
pub fn member_row(
    member: Member,
    controller: StoredValue<Rc<TeamController>, LocalStorage>,
) -> impl IntoView {
    let id = member.id.clone();
    let name_input = RwSignal::new(member.name.clone());

    view! {
        <li class="member-row">
            <span class="member-name">{member.name.clone()}</span>
            " — "
            <span class="member-role">{member.role.clone()}</span>
            " ("
            <span class="member-email">{member.email.clone()}</span>
            ")"
            <input
                type="text"
                prop:value=move || name_input.get()
                on:input=move |ev| name_input.set(event_target_value(&ev))
            />
            <button on:click=move |_| {
                let id = id.clone();
                let name = name_input.get_untracked();
                // Gate the controller read with `try_get_value()` to avoid
                // panicking if the component unmounts before this spawned microtask
                // runs (a disposal race).
                leptos::task::spawn_local(async move {
                    let Some(c) = controller.try_get_value() else { return; };
                    c.rename(id, name).await;
                });
            }>
                "Rename"
            </button>
        </li>
    }
}
