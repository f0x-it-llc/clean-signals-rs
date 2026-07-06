//! clean-signals — clean-architecture framework core for Rust/Leptos apps.
//!
//! Reactivity-aware but DOM-free: this crate depends only on `reactive_graph`
//! and must never import leptos rendering APIs. See docs/ARCHITECTURE.md.
//!
//! Module re-exports are finalized by the controller task (06).

pub mod activity;
pub mod async_state;
pub mod controller;
pub mod failure;
pub mod retry;
pub mod time;
pub mod use_case;
