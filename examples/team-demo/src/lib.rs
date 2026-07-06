//! `team-demo` — the reference example app for `clean-signals-rs`.
//!
//! A CSR Leptos app exercising both framework crates end to end: a single
//! `team` feature slice (ported from the Dart `clean_signals` example),
//! laid out exactly per `templates/AGENTS.md`'s feature-slice convention —
//! `domain/ ← data/`, `domain/ ← presentation/`, nothing crossing the other
//! direction. See `src/main.rs` for the composition root.

pub mod failure;
pub mod features;
