//! Hertzline desktop host library.
//!
//! Profile lifecycle (HZ-028), shell routes (HZ-029), measure + benchmark UI (HZ-030).
//! The Tauri binary lives in `src-tauri` and stays unprivileged.

pub mod benchmark_ui;
pub mod measure;
pub mod profile_host;
pub mod shell;

pub const CRATE_VERSION: &str = env!("CARGO_PKG_VERSION");

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
