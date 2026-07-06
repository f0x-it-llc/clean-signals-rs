//! Portable async sleep.
//!
//! This is the **only** sanctioned sleep in the framework — and in
//! downstream apps' presentation layers. Reaching for `tokio::time::sleep`
//! or `gloo_timers` directly outside this module bypasses the
//! target-gating this crate exists to centralize; use [`sleep`] instead.

use std::time::Duration;

/// Suspend the current task for `d`.
///
/// Native targets delegate to `tokio::time::sleep`; `wasm32` targets
/// delegate to `gloo_timers::future::sleep`.
#[cfg(not(target_arch = "wasm32"))]
pub async fn sleep(d: Duration) {
    tokio::time::sleep(d).await;
}

/// Suspend the current task for `d`.
///
/// Native targets delegate to `tokio::time::sleep`; `wasm32` targets
/// delegate to `gloo_timers::future::sleep`.
#[cfg(target_arch = "wasm32")]
pub async fn sleep(d: Duration) {
    gloo_timers::future::sleep(d).await;
}

#[cfg(all(test, not(target_arch = "wasm32")))]
mod tests {
    use super::*;
    use std::time::Instant;

    #[tokio::test]
    async fn sleep_waits_at_least_the_requested_duration() {
        let d = Duration::from_millis(20);
        let start = Instant::now();
        sleep(d).await;
        assert!(start.elapsed() >= d);
    }
}
