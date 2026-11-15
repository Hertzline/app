//! HZ-057 app integration smoke on the host OS (AC-1--AC-8 shell surface).
//!
//! Unprivileged shell, measure/benchmark, action catalog, restore-all,
//! everyday and overlay policy. Env-mutating steps run in one serial test.

use std::fs;
use std::path::PathBuf;
use std::sync::atomic::{AtomicU64, Ordering};

use hertzline_app::{
    apply_labeled_mock, disable_overlay_globally, everyday_overview, kill_hog_guard,
    labeled_mock_panel_fps, list_action_catalog, no_local_benchmark_explanation,
    required_route_ids, restore_all_explicit, run_labeled_mock_pair, run_measurement,
    save_overlay_settings, shell_info, MeasureRequest, SaveOverlaySettingsRequest, CRATE_VERSION,
};
use hertzline_core::overlay::OVERLAY_SETTINGS_ENV;
use hertzline_core::paths::{MEASUREMENT_ARCHIVE_ENV, SNAPSHOT_STORE_ENV};

fn temp_dir(label: &str) -> PathBuf {
    static N: AtomicU64 = AtomicU64::new(0);
    let n = N.fetch_add(1, Ordering::Relaxed);
    let dir = std::env::temp_dir().join(format!(
        "hertzline-app-hz057-{label}-{}-{n}",
        std::process::id()
    ));
    let _ = fs::remove_dir_all(&dir);
    fs::create_dir_all(&dir).expect("temp dir");
    dir
}

#[test]
fn hz057_host_banner() {
    println!(
        "hz057_app_host os={} arch={} app={} core={}",
        std::env::consts::OS,
        std::env::consts::ARCH,
        CRATE_VERSION,
        hertzline_core::CRATE_VERSION
    );
}

#[test]
fn ac_shell_unprivileged_routes() {
    let info = shell_info(CRATE_VERSION, hertzline_core::CRATE_VERSION);
    assert!(info.unprivileged);
    let ids: Vec<&str> = info.routes.iter().map(|r| r.id.as_str()).collect();
    assert_eq!(ids, required_route_ids());
}

#[test]
fn ac1_ac8_measure_export_and_no_fake_fps() {
    let result = run_measurement(MeasureRequest {
        target_process: Some("hz057".into()),
        network_host: None,
        presentmon_csv: None,
        mangohud_log: None,
        notes: Some("integration".into()),
    })
    .expect("measure");
    assert!(!result.csv.is_empty());
    assert!(!result.json.is_empty());
    assert!(!result.local_measurement_note.trim().is_empty());
}

#[test]
fn ac2_benchmark_labeled_mock_verdict() {
    let (_before, _after, verdict) = run_labeled_mock_pair(16.0, 16.0).expect("mock pair");
    assert_eq!(verdict.verdict, "no_measurable_effect");
    assert!(verdict.offer_revert);
    assert!(verdict.message.contains("no measurable effect"));
    assert!(!no_local_benchmark_explanation().is_empty());
}

#[test]
fn ac4_ac8_catalog_explanations() {
    let catalog = list_action_catalog();
    assert!(
        catalog.actions.len() >= 23,
        "expected >=23 AC-4.x cards, got {}",
        catalog.actions.len()
    );
    for card in &catalog.actions {
        assert!(!card.explanation.what.trim().is_empty());
        assert!(!card.explanation.expected_effect.trim().is_empty());
        assert!(!card.explanation.risk.trim().is_empty());
        assert!(!card.explanation.how_to_undo.trim().is_empty());
        assert!(!card.explanation.local_measurement.trim().is_empty());
    }
}

#[test]
fn ac5_ac6_ac7_serial_env_smoke() {
    // Serial: env path overrides must not race with other tests in this binary.
    let dir = temp_dir("serial");
    std::env::set_var(SNAPSHOT_STORE_ENV, dir.join("snapshots.json").as_os_str());
    std::env::set_var(
        MEASUREMENT_ARCHIVE_ENV,
        dir.join("archive.json").as_os_str(),
    );
    std::env::set_var(OVERLAY_SETTINGS_ENV, dir.join("overlay.json").as_os_str());

    assert!(kill_hog_guard(false).is_err());

    let overview = everyday_overview().expect("everyday");
    assert!(!overview.hogs.note.is_empty());
    assert!(!overview.thermal.note.is_empty());
    assert!(!overview.cleanup.note.is_empty());

    apply_labeled_mock().expect("mock apply");
    let report = restore_all_explicit().expect("restore all");
    assert!(!report.requires_ui);
    assert!(report.ok);

    let game = dir.join("game");
    fs::create_dir_all(game.join("EasyAntiCheat")).expect("eac");
    let ac = save_overlay_settings(SaveOverlaySettingsRequest {
        globally_disabled: false,
        user_enabled: true,
        allow_with_anti_cheat: false,
        install_path: Some(game.display().to_string()),
    })
    .expect("overlay save");
    assert!(ac.anti_cheat_detected);
    assert!(!ac.decision_enabled);
    assert!(!ac.injects_into_game);

    let disabled = disable_overlay_globally().expect("global disable");
    assert!(disabled.settings.globally_disabled);
    assert!(!disabled.decision_enabled);

    let panel = labeled_mock_panel_fps(60.0);
    assert_eq!(panel.status, "available");
    assert!(panel.avg_fps.is_some());

    std::env::remove_var(SNAPSHOT_STORE_ENV);
    std::env::remove_var(MEASUREMENT_ARCHIVE_ENV);
    std::env::remove_var(OVERLAY_SETTINGS_ENV);
    let _ = fs::remove_dir_all(&dir);
}
