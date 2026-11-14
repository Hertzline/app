//! Hertzline desktop host library.
//!
//! Profile lifecycle (HZ-028) plus shell route model (HZ-029).
//! The Tauri binary lives in `src-tauri` and stays unprivileged.

pub mod profile_host;
pub mod shell;

pub const CRATE_VERSION: &str = env!("CARGO_PKG_VERSION");

pub use profile_host::{ProfileHost, ProfileHostError};
pub use shell::{shell_info, AppRoute, RouteInfo, ShellInfo};
