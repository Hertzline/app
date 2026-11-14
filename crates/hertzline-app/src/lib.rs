//! Hertzline desktop host library (HZ-028 profile lifecycle).
//!
//! Full Tauri shell arrives in HZ-029. This crate owns the unprivileged
//! apply-on-start / restore-on-exit / crash-recover host that the UI will call.

pub mod profile_host;

pub const CRATE_VERSION: &str = env!("CARGO_PKG_VERSION");

pub use profile_host::{ProfileHost, ProfileHostError};
