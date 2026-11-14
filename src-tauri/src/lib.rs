//! Hertzline Tauri shell (HZ-029 / HZ-030 / HZ-031).
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
use hertzline_app::measure::{run_measurement, MeasureRequest, MeasureResult};
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

fn run_smoke_and_exit() -> ! {
    let info = get_shell_info();
    if !info.unprivileged {
        eprintln!("hz031 smoke failed: shell must be unprivileged");
        std::process::exit(1);
    }
    let expected = required_route_ids();
    let got: Vec<&str> = info.routes.iter().map(|r| r.id.as_str()).collect();
    if got != expected {
        eprintln!("hz031 smoke failed: routes={got:?} expected={expected:?}");
        std::process::exit(1);
    }
    for id in expected {
        if open_route(id.to_string()).is_err() {
            eprintln!("hz031 smoke failed: open_route({id})");
            std::process::exit(1);
        }
    }
    let core = core_ping("hz031-smoke".into());
    if !core.ok {
        eprintln!("hz031 smoke failed: core_ping={core:?}");
        std::process::exit(1);
    }

    let catalog = actions_catalog();
    if catalog.actions.len() < 20 {
        eprintln!(
            "hz031 smoke failed: catalog too small {}",
            catalog.actions.len()
        );
        std::process::exit(1);
    }
    let honesty = actions_macos_honesty();
    if !honesty.contains("far fewer tunables") {
        eprintln!("hz031 smoke failed: macos honesty banner");
        std::process::exit(1);
    }
    let elev = actions_apply(ApplyRequest {
        id: "windows.hags".into(),
        confirm_elevation: false,
        confirm_destructive: false,
        input: None,
    });
    match elev {
        ApplyOutcomeDto::ElevationRequired { .. } => {}
        other => {
            eprintln!("hz031 smoke failed: expected elevation prompt, got {other:?}");
            std::process::exit(1);
        }
    }

    // Isolated store for labeled mock apply + restore-all.
    let dir = std::env::temp_dir().join(format!(
        "hertzline-app-hz031-smoke-{}-{}",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_millis())
            .unwrap_or(0)
    ));
    let _ = std::fs::create_dir_all(&dir);
    let store = dir.join("snapshots.json");
    std::env::set_var(hertzline_core::paths::SNAPSHOT_STORE_ENV, store.as_os_str());

    let applied = match actions_apply_labeled_mock() {
        Ok(v) => v,
        Err(e) => {
            eprintln!("hz031 smoke failed: labeled_mock={e}");
            std::process::exit(1);
        }
    };
    match &applied {
        ApplyOutcomeDto::Applied { action, .. } if action == "mock_toggle" => {}
        other => {
            eprintln!("hz031 smoke failed: mock apply={other:?}");
            std::process::exit(1);
        }
    }
    let log = match actions_change_log() {
        Ok(rows) => rows,
        Err(e) => {
            eprintln!("hz031 smoke failed: change_log={e}");
            std::process::exit(1);
        }
    };
    if log.is_empty() || log[0].restore_status != "outstanding" {
        eprintln!("hz031 smoke failed: change_log={log:?}");
        std::process::exit(1);
    }
    let restore = match actions_restore_all() {
        Ok(r) => r,
        Err(e) => {
            eprintln!("hz031 smoke failed: restore_all={e}");
            std::process::exit(1);
        }
    };
    if !restore.ok || restore.requires_ui {
        eprintln!("hz031 smoke failed: restore={restore:?}");
        std::process::exit(1);
    }

    let _ = std::fs::remove_dir_all(&dir);

    let route_list = expected.join(",");
    println!(
        "hz031_smoke_ok unprivileged=true routes={route_list} catalog={} elev=prompt restore_all=ok app={} core={}",
        catalog.actions.len(),
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
            actions_macos_honesty
        ])
        .run(tauri::generate_context!())
        .expect("error while running Hertzline");
}
