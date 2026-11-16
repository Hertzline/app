//! Hertzline Tauri shell (HZ-029--HZ-034).
//!
//! Unprivileged by default. Elevation is requested per action, never at startup.

// MSVC emits .lib/.exp notes when linking cdylib; not actionable here.
#![allow(linker_messages)]

use hertzline_app::actions_ui::{
    apply_action, apply_labeled_mock, list_action_catalog, list_change_log, macos_honesty_banner,
    restore_all_explicit, restore_change_entry, ActionCatalogDto, ApplyOutcomeDto, ApplyRequest,
    ChangeLogRowDto, RestoreAllDto,
};
use hertzline_app::benchmark_ui::{
    compare_benchmarks, no_local_benchmark_explanation, run_benchmark_from_windows,
    run_labeled_mock_pair, BenchConfigDto, BenchResultDto, VerdictDto,
};
use hertzline_app::everyday_ui::{
    apply_cleanup, apply_power_preset, everyday_overview, everyday_overview_lite, kill_hog_guard,
    restore_power_preset, weekly_health_report, ApplyPresetRequest, CleanupApplyRequest,
    CleanupOutcomeDto, EverydayOverviewDto, PresetOutcomeDto, WeeklyReportDto,
};
use hertzline_app::measure::{run_measurement, MeasureRequest, MeasureResult};
use hertzline_app::overlay_ui::{
    disable_overlay_globally, labeled_mock_panel_fps, overlay_overview, save_overlay_settings,
    scan_anti_cheat, OverlayOverviewDto, OverlayPanelDto, OverlayScanRequest,
    SaveOverlaySettingsRequest,
};
use hertzline_app::profiles_ui::{
    add_manual_path, apply_profile, apply_restore_labeled_mock, profiles_overview,
    recover_profiles_after_crash, restore_active_profile, save_profile, AddManualPathRequest,
    ApplyProfileRequest, ProfileApplyOutcomeDto, ProfileDto, ProfilesOverviewDto,
    SaveProfileRequest,
};
use hertzline_app::shell::{required_route_ids, shell_info, AppRoute, ShellInfo};
use hertzline_core::ipc::{ping, PingRequest, PingResponse};

#[tauri::command]
fn get_shell_info() -> ShellInfo {
    shell_info(hertzline_app::CRATE_VERSION, hertzline_core::CRATE_VERSION)
}

#[tauri::command]
fn open_route(id: String) -> Result<String, String> {
    let route = AppRoute::parse(&id).ok_or_else(|| format!("unknown route: {id}"))?;
    Ok(route.hash_path())
}

#[tauri::command]
fn core_ping(client: String) -> PingResponse {
    ping(&PingRequest { client })
}

#[tauri::command]
fn measure_run(request: MeasureRequest) -> Result<MeasureResult, String> {
    run_measurement(request)
}

#[tauri::command]
fn benchmark_default_config() -> BenchConfigDto {
    BenchConfigDto::default()
}

#[tauri::command]
fn benchmark_run_windows(
    config: BenchConfigDto,
    windows: Vec<Vec<f64>>,
    source: String,
) -> Result<BenchResultDto, String> {
    let source = if source.trim().is_empty() {
        "supplied_windows"
    } else {
        source.trim()
    };
    run_benchmark_from_windows(config, windows, source)
}

#[tauri::command]
fn benchmark_run_labeled_mock(
    before_frame_ms: f64,
    after_frame_ms: f64,
) -> Result<(BenchResultDto, BenchResultDto, VerdictDto), String> {
    run_labeled_mock_pair(before_frame_ms, after_frame_ms)
}

#[tauri::command]
fn benchmark_compare(before: BenchResultDto, after: BenchResultDto) -> Result<VerdictDto, String> {
    compare_benchmarks(&before, &after)
}

#[tauri::command]
fn benchmark_no_local_note() -> String {
    no_local_benchmark_explanation()
}

#[tauri::command]
fn actions_catalog() -> ActionCatalogDto {
    list_action_catalog()
}

#[tauri::command]
fn actions_apply(request: ApplyRequest) -> ApplyOutcomeDto {
    apply_action(request)
}

#[tauri::command]
fn actions_apply_labeled_mock() -> Result<ApplyOutcomeDto, String> {
    apply_labeled_mock()
}

#[tauri::command]
fn actions_change_log() -> Result<Vec<ChangeLogRowDto>, String> {
    list_change_log()
}

#[tauri::command]
fn actions_restore_entry(entry_id: u64) -> Result<ChangeLogRowDto, String> {
    restore_change_entry(entry_id)
}

#[tauri::command]
fn actions_restore_all() -> Result<RestoreAllDto, String> {
    restore_all_explicit()
}

#[tauri::command]
fn actions_macos_honesty() -> String {
    macos_honesty_banner()
}

#[tauri::command]
fn profiles_get_overview() -> Result<ProfilesOverviewDto, String> {
    profiles_overview()
}

#[tauri::command]
fn profiles_add_manual(request: AddManualPathRequest) -> Result<ProfilesOverviewDto, String> {
    add_manual_path(request)
}

#[tauri::command]
fn profiles_save(request: SaveProfileRequest) -> Result<ProfileDto, String> {
    save_profile(request)
}

#[tauri::command]
fn profiles_apply(request: ApplyProfileRequest) -> Result<ProfileApplyOutcomeDto, String> {
    apply_profile(request)
}

#[tauri::command]
fn profiles_restore(labeled_mock: bool) -> Result<ProfileApplyOutcomeDto, String> {
    restore_active_profile(labeled_mock)
}

#[tauri::command]
fn profiles_recover_crash() -> Result<bool, String> {
    recover_profiles_after_crash()
}

#[tauri::command]
fn profiles_apply_restore_labeled_mock(
) -> Result<(ProfileApplyOutcomeDto, ProfileApplyOutcomeDto), String> {
    apply_restore_labeled_mock()
}

#[tauri::command]
fn everyday_get_overview() -> Result<EverydayOverviewDto, String> {
    everyday_overview()
}

#[tauri::command]
fn everyday_get_overview_lite() -> Result<EverydayOverviewDto, String> {
    everyday_overview_lite()
}

#[tauri::command]
fn everyday_apply_preset(request: ApplyPresetRequest) -> PresetOutcomeDto {
    apply_power_preset(request)
}

#[tauri::command]
fn everyday_restore_preset(
    entry_id: u64,
) -> Result<hertzline_app::everyday_ui::ChangeRowDto, String> {
    restore_power_preset(entry_id)
}

#[tauri::command]
fn everyday_apply_cleanup(request: CleanupApplyRequest) -> CleanupOutcomeDto {
    apply_cleanup(request)
}

#[tauri::command]
fn everyday_kill_guard(confirm: bool) -> Result<(), String> {
    kill_hog_guard(confirm)
}

#[tauri::command]
fn everyday_weekly_report() -> Result<WeeklyReportDto, String> {
    weekly_health_report()
}

#[tauri::command]
fn overlay_get_overview(install_path: Option<String>) -> Result<OverlayOverviewDto, String> {
    overlay_overview(install_path.as_deref())
}

#[tauri::command]
fn overlay_save_settings(
    request: SaveOverlaySettingsRequest,
) -> Result<OverlayOverviewDto, String> {
    save_overlay_settings(request)
}

#[tauri::command]
fn overlay_disable_globally() -> Result<OverlayOverviewDto, String> {
    disable_overlay_globally()
}

#[tauri::command]
fn overlay_scan_anti_cheat(request: OverlayScanRequest) -> Result<OverlayOverviewDto, String> {
    scan_anti_cheat(request)
}

#[tauri::command]
fn overlay_labeled_mock_panel() -> OverlayPanelDto {
    labeled_mock_panel_fps(60.0)
}

fn run_smoke_and_exit() -> ! {
    let info = get_shell_info();
    if !info.unprivileged {
        eprintln!("hz034 smoke failed: shell must be unprivileged");
        std::process::exit(1);
    }
    let expected = required_route_ids();
    let got: Vec<&str> = info.routes.iter().map(|r| r.id.as_str()).collect();
    if got != expected {
        eprintln!("hz034 smoke failed: routes={got:?} expected={expected:?}");
        std::process::exit(1);
    }
    for id in expected {
        if open_route(id.to_string()).is_err() {
            eprintln!("hz034 smoke failed: open_route({id})");
            std::process::exit(1);
        }
    }
    let core = core_ping("hz034-smoke".into());
    if !core.ok {
        eprintln!("hz034 smoke failed: core_ping={core:?}");
        std::process::exit(1);
    }

    let dir = std::env::temp_dir().join(format!(
        "hertzline-app-hz034-smoke-{}-{}",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_millis())
            .unwrap_or(0)
    ));
    let _ = std::fs::create_dir_all(&dir);
    let store = dir.join("snapshots.json");
    let archive = dir.join("archive.json");
    let overlay_settings = dir.join("overlay_settings.json");
    std::env::set_var(hertzline_core::paths::SNAPSHOT_STORE_ENV, store.as_os_str());
    std::env::set_var(
        hertzline_core::paths::MEASUREMENT_ARCHIVE_ENV,
        archive.as_os_str(),
    );
    std::env::set_var(
        hertzline_core::overlay::OVERLAY_SETTINGS_ENV,
        overlay_settings.as_os_str(),
    );

    let game = dir.join("game");
    let _ = std::fs::create_dir_all(game.join("EasyAntiCheat"));

    let overview = match overlay_get_overview(None) {
        Ok(v) => v,
        Err(e) => {
            eprintln!("hz034 smoke failed: overview={e}");
            std::process::exit(1);
        }
    };
    if overview.injects_into_game {
        eprintln!("hz034 smoke failed: injects_into_game must be false");
        std::process::exit(1);
    }
    if overview.decision_enabled {
        eprintln!("hz034 smoke failed: default overlay must be off");
        std::process::exit(1);
    }

    let ac = match overlay_save_settings(SaveOverlaySettingsRequest {
        globally_disabled: false,
        user_enabled: true,
        allow_with_anti_cheat: false,
        install_path: Some(game.display().to_string()),
    }) {
        Ok(v) => v,
        Err(e) => {
            eprintln!("hz034 smoke failed: save={e}");
            std::process::exit(1);
        }
    };
    if !ac.anti_cheat_detected || ac.decision_enabled {
        eprintln!("hz034 smoke failed: anti-cheat must keep overlay off by default");
        std::process::exit(1);
    }

    let disabled = match overlay_disable_globally() {
        Ok(v) => v,
        Err(e) => {
            eprintln!("hz034 smoke failed: global disable={e}");
            std::process::exit(1);
        }
    };
    if !disabled.settings.globally_disabled || disabled.decision_enabled {
        eprintln!("hz034 smoke failed: global disable did not stick");
        std::process::exit(1);
    }

    let mock = overlay_labeled_mock_panel();
    if mock.status != "available" || mock.avg_fps.is_none() {
        eprintln!("hz034 smoke failed: labeled mock panel={mock:?}");
        std::process::exit(1);
    }

    let _ = std::fs::remove_dir_all(&dir);

    let route_list = expected.join(",");
    println!(
        "hz034_smoke_ok unprivileged=true routes={route_list} inject=0 ac_default_off=1 global_disable=1 mock_panel=ok method={} app={} core={}",
        overview.method_id,
        info.app_version,
        info.core_version
    );
    std::process::exit(0);
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    // Headless CI: no window / WebView.
    if std::env::var_os("HERTZLINE_APP_SMOKE").is_some() {
        run_smoke_and_exit();
    }

    let _ = recover_profiles_after_crash();

    tauri::Builder::default()
        .invoke_handler(tauri::generate_handler![
            get_shell_info,
            open_route,
            core_ping,
            measure_run,
            benchmark_default_config,
            benchmark_run_windows,
            benchmark_run_labeled_mock,
            benchmark_compare,
            benchmark_no_local_note,
            actions_catalog,
            actions_apply,
            actions_apply_labeled_mock,
            actions_change_log,
            actions_restore_entry,
            actions_restore_all,
            actions_macos_honesty,
            profiles_get_overview,
            profiles_add_manual,
            profiles_save,
            profiles_apply,
            profiles_restore,
            profiles_recover_crash,
            profiles_apply_restore_labeled_mock,
            everyday_get_overview,
            everyday_get_overview_lite,
            everyday_apply_preset,
            everyday_restore_preset,
            everyday_apply_cleanup,
            everyday_kill_guard,
            everyday_weekly_report,
            overlay_get_overview,
            overlay_save_settings,
            overlay_disable_globally,
            overlay_scan_anti_cheat,
            overlay_labeled_mock_panel
        ])
        .run(tauri::generate_context!())
        .expect("error while running Hertzline");
}
