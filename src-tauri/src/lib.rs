//! Hertzline Tauri shell (HZ-029 / HZ-030).
//!
//! Unprivileged by default. Elevation is requested per action later, never at startup.

// MSVC emits .lib/.exp notes when linking cdylib; not actionable here.
#![allow(linker_messages)]

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

fn run_smoke_and_exit() -> ! {
    let info = get_shell_info();
    if !info.unprivileged {
        eprintln!("hz030 smoke failed: shell must be unprivileged");
        std::process::exit(1);
    }
    let expected = required_route_ids();
    let got: Vec<&str> = info.routes.iter().map(|r| r.id.as_str()).collect();
    if got != expected {
        eprintln!("hz030 smoke failed: routes={got:?} expected={expected:?}");
        std::process::exit(1);
    }
    for id in expected {
        if open_route(id.to_string()).is_err() {
            eprintln!("hz030 smoke failed: open_route({id})");
            std::process::exit(1);
        }
    }
    let core = core_ping("hz030-smoke".into());
    if !core.ok {
        eprintln!("hz030 smoke failed: core_ping={core:?}");
        std::process::exit(1);
    }

    let measure = measure_run(MeasureRequest {
        target_process: Some("hz030-smoke".into()),
        network_host: None,
        presentmon_csv: None,
        mangohud_log: None,
        notes: Some("smoke".into()),
    });
    let measure = match measure {
        Ok(m) => m,
        Err(e) => {
            eprintln!("hz030 smoke failed: measure_run={e}");
            std::process::exit(1);
        }
    };
    if !measure.csv.contains("avg_fps") || !measure.json.contains("avg_fps") {
        eprintln!("hz030 smoke failed: export missing avg_fps");
        std::process::exit(1);
    }

    let (_b, _a, verdict) = match benchmark_run_labeled_mock(16.0, 16.0) {
        Ok(v) => v,
        Err(e) => {
            eprintln!("hz030 smoke failed: labeled_mock={e}");
            std::process::exit(1);
        }
    };
    if verdict.message != "no measurable effect" || !verdict.offer_revert {
        eprintln!("hz030 smoke failed: verdict={verdict:?}");
        std::process::exit(1);
    }
    let note = benchmark_no_local_note();
    if note.is_empty() {
        eprintln!("hz030 smoke failed: empty AC-8.2 note");
        std::process::exit(1);
    }

    let route_list = expected.join(",");
    println!(
        "hz030_smoke_ok unprivileged=true routes={route_list} export=csv+json verdict={} app={} core={}",
        verdict.message, info.app_version, info.core_version
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
            benchmark_no_local_note
        ])
        .run(tauri::generate_context!())
        .expect("error while running Hertzline");
}
