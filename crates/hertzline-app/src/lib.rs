//! Hertzline desktop host library.
//!
//! Profile lifecycle (HZ-028), shell routes (HZ-029), measure + benchmark UI (HZ-030),
//! actions + explanations + restore UI (HZ-031), profiles UI (HZ-032), everyday mode (HZ-033),
//! optional overlay (HZ-034).
//! The Tauri binary lives in `src-tauri` and stays unprivileged.

pub mod actions_ui;
pub mod benchmark_ui;
pub mod everyday_ui;
pub mod measure;
pub mod overlay_ui;
pub mod profile_host;
pub mod profiles_ui;
pub mod shell;

#[cfg(test)]
mod test_env;

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
pub use everyday_ui::{
    apply_cleanup, apply_power_preset, everyday_overview, kill_hog_guard, restore_power_preset,
    weekly_health_report, ApplyPresetRequest, CleanupApplyRequest, CleanupOutcomeDto,
    EverydayOverviewDto, PresetOutcomeDto, WeeklyReportDto,
};
pub use measure::{
    explanation_with_evidence, run_measurement, MeasureRequest, MeasureResult,
    NO_LOCAL_ACTION_MEASUREMENT,
};
pub use overlay_ui::{
    disable_overlay_globally, labeled_mock_panel_fps, overlay_overview, save_overlay_settings,
    scan_anti_cheat, OverlayOverviewDto, OverlayPanelDto, OverlayScanRequest, OverlaySettingsDto,
    SaveOverlaySettingsRequest,
};
pub use profile_host::{ProfileHost, ProfileHostError};
pub use profiles_ui::{
    add_manual_path, apply_profile, apply_restore_labeled_mock, profiles_overview,
    recover_profiles_after_crash, restore_active_profile, save_profile, ActiveSessionDto,
    AddManualPathRequest, ApplyProfileRequest, DetectedGameDto, ManualPathDto,
    ProfileApplyOutcomeDto, ProfileDto, ProfilesOverviewDto, SaveProfileRequest,
    PROFILES_LIBRARY_ENV,
};
pub use shell::{shell_info, AppRoute, RouteInfo, ShellInfo};
