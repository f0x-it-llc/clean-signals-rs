//! Composition root: wires the fake backend behind the `TeamRepository`
//! trait object and mounts the `team` feature's single page.

use std::sync::Arc;
use std::time::Duration;

use team_demo::features::team::data::InMemoryTeamRepo;
use team_demo::features::team::domain::repositories::TeamRepository;
use team_demo::features::team::presentation::TeamPage;

fn main() {
    console_error_panic_hook::set_once();

    // 400ms of artificial latency and 2 forced failures before success —
    // demonstrates `TeamController::load`'s `RetryPolicy` absorbing
    // transient `TeamFailure::Network` errors transparently.
    let repo: Arc<dyn TeamRepository + Send + Sync> =
        Arc::new(InMemoryTeamRepo::new(Duration::from_millis(400), 2));

    leptos::mount::mount_to_body(move || {
        leptos::view! { <TeamPage repo=repo.clone() /> }
    });
}
