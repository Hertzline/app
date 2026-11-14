//! Hertzline desktop host library.
//!
//! Profile lifecycle (HZ-028), shell routes (HZ-029), measure + benchmark UI (HZ-030),
//! actions + explanations + restore UI (HZ-031).
//! The Tauri binary lives in `src-tauri` and stays unprivileged.

pub mod actions_ui;
pub mod benchmark_ui;
pub mod measure;
pub mod profile_host;
pub mod shell;

pub const CRATE_VERSION: &str = env!("CARGO_PKG_VERSION");

pub use actions_ui::{
    apply_action, apply_labeled_mock, list_action_catalog, list_change_log, macos_honesty_banner,
    restore_all_explicit, restore_change_entry, ActionCatalogDto, ApplyOutcomeDto, ApplyRequest,
    ChangeLogRowDto, RestoreAllDto,
};
pub use benchmark_ui::{
    compare_benchmarks, no_local_benchmark_explanation, run_benchmark_from_windows,
    run_labeled_mock_pair, BenchConfigDto, BenchResultDto, VerdictDto,
};
pub use measure::{
    explanation_with_evidence, run_measurement, MeasureRequest, MeasureResult,
    NO_LOCAL_ACTION_MEASUREMENT,
};
pub use profile_host::{ProfileHost, ProfileHostError};
pub use shell::{shell_info, AppRoute, RouteInfo, ShellInfo};
