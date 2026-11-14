//! Everyday mode UI host (HZ-033 / AC-5.1--AC-5.6).

use hertzline_core::action::{apply_with_snapshot, restore_with_log, Action};
use hertzline_core::collectors::{HardwareSampler, SysinfoHardwareSampler};
use hertzline_core::everyday::{
    apply_everyday_cleanup, assess_thermal, generate_weekly_report, list_resource_hogs,
    list_startup_impact, load_measurement_archive, preview_everyday_cleanup,
    refuse_kill_without_confirm, EverydayPreset, HostEverydayCleanupBackend,
    HostPowerPresetBackend, PowerPresetAction, PowerPresetBackend, SysinfoResourceHogs,
    HIGH_TEMP_CELSIUS,
};
use hertzline_core::paths::snapshot_store_path;
use hertzline_core::snapshot::{unix_ms_now, ChangeLogEntry, SnapshotStore};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct StartupRowDto {
    pub name: String,
    pub command: String,
    pub enabled: bool,
    pub impact_kind: String,
    pub impact_summary: String,
    pub reversible: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct StartupImpactDto {
    pub host_os: String,
    pub source: String,
    pub note: String,
    pub rows: Vec<StartupRowDto>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ResourceHogDto {
    pub pid: u32,
    pub name: String,
    pub cpu_percent: f32,
    pub ram_bytes: u64,
    pub disk_read_bytes: Option<u64>,
    pub disk_write_bytes: Option<u64>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ResourceHogsDto {
    pub host_os: String,
    pub note: String,
    pub hogs: Vec<ResourceHogDto>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ExplanationLite {
    pub what: String,
    pub expected_effect: String,
    pub risk: String,
    pub how_to_undo: String,
    pub local_measurement: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PowerPresetDto {
    pub available: bool,
    pub message: String,
    pub backend: Option<String>,
    pub preset: Option<String>,
    pub raw: Option<String>,
    pub explanation: ExplanationLite,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CleanupPathDto {
    pub label: String,
    pub path: String,
    pub bytes: u64,
    pub source: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CleanupPreviewDto {
    pub host_os: String,
    pub note: String,
    pub total_bytes: u64,
    pub entries: Vec<CleanupPathDto>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ThermalWarningDto {
    pub source: String,
    pub message: String,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ThermalDto {
    pub level: String,
    pub cpu_temp_celsius: Option<f64>,
    pub gpu_temp_celsius: Option<f64>,
    pub throttling: Option<String>,
    pub high_temp_celsius: f64,
    pub warnings: Vec<ThermalWarningDto>,
    pub note: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct MeasurementSummaryDto {
    pub session_id: String,
    pub started_at_unix_ms: i64,
    pub ended_at_unix_ms: Option<i64>,
    pub target_process: Option<String>,
    pub frame_status: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ChangeRowDto {
    pub id: u64,
    pub time_unix_ms: i64,
    pub action: String,
    pub before: String,
    pub after: String,
    pub restore_status: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct WeeklyReportDto {
    pub window_start_unix_ms: i64,
    pub window_end_unix_ms: i64,
    pub note: String,
    pub measurements: Vec<MeasurementSummaryDto>,
    pub applied_actions: Vec<ChangeRowDto>,
    pub rollbacks: Vec<ChangeRowDto>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct EverydayOverviewDto {
    pub local_measurement_note: String,
    pub startup: StartupImpactDto,
    pub hogs: ResourceHogsDto,
    pub power: PowerPresetDto,
    pub cleanup: CleanupPreviewDto,
    pub thermal: ThermalDto,
    pub weekly: WeeklyReportDto,
}

fn explanation_lite(action: &dyn Action) -> ExplanationLite {
    let e = action.explanation();
    ExplanationLite {
        what: e.what.clone(),
        expected_effect: e.expected_effect.clone(),
        risk: e.risk.clone(),
        how_to_undo: e.how_to_undo.clone(),
        local_measurement: crate::measure::explanation_with_evidence(None),
    }
}

fn change_row(entry: ChangeLogEntry) -> ChangeRowDto {
    ChangeRowDto {
        id: entry.id,
        time_unix_ms: entry.time_unix_ms,
        action: entry.action,
        before: entry.before,
        after: entry.after,
        restore_status: entry.restore_status.as_str().into(),
    }
}

fn read_power_preset() -> PowerPresetDto {
    let probe = PowerPresetAction::new(HostPowerPresetBackend, EverydayPreset::Balanced);
    let explanation = explanation_lite(&probe);
    match HostPowerPresetBackend.read() {
        Ok(state) => PowerPresetDto {
            available: true,
            message: "Mapped to the OS power API for this host (AC-5.3).".into(),
            backend: Some(state.backend),
            preset: Some(state.preset.as_str().into()),
            raw: Some(state.raw),
            explanation,
        },
        Err(err) => PowerPresetDto {
            available: false,
            message: err.to_string(),
            backend: None,
            preset: None,
            raw: None,
            explanation,
        },
    }
}

/// Full everyday overview for the UI route.
pub fn everyday_overview() -> Result<EverydayOverviewDto, String> {
    let startup = list_startup_impact().map_err(|e| e.to_string())?;
    let mut hogs_backend = SysinfoResourceHogs::new();
    let hogs = list_resource_hogs(&mut hogs_backend, 15).map_err(|e| e.to_string())?;
    let power = read_power_preset();
    let cleanup =
        preview_everyday_cleanup(&HostEverydayCleanupBackend).map_err(|e| e.to_string())?;
    let mut hw = SysinfoHardwareSampler::new();
    let thermal = assess_thermal(&hw.sample());
    let store = SnapshotStore::open(snapshot_store_path()).map_err(|e| e.to_string())?;
    let archive = load_measurement_archive()?;
    let weekly = generate_weekly_report(&store, &archive, unix_ms_now());

    Ok(EverydayOverviewDto {
        local_measurement_note: crate::measure::NO_LOCAL_ACTION_MEASUREMENT.to_string(),
        startup: StartupImpactDto {
            host_os: startup.host_os,
            source: startup.source.as_str().into(),
            note: startup.note,
            rows: startup
                .rows
                .into_iter()
                .map(|r| StartupRowDto {
                    name: r.name,
                    command: r.command,
                    enabled: r.enabled,
                    impact_kind: r.impact_kind.as_str().into(),
                    impact_summary: r.impact_summary,
                    reversible: r.reversible,
                })
                .collect(),
        },
        hogs: ResourceHogsDto {
            host_os: hogs.host_os,
            note: hogs.note,
            hogs: hogs
                .hogs
                .into_iter()
                .map(|h| ResourceHogDto {
                    pid: h.pid,
                    name: h.name,
                    cpu_percent: h.cpu_percent,
                    ram_bytes: h.ram_bytes,
                    disk_read_bytes: h.disk_read_bytes,
                    disk_write_bytes: h.disk_write_bytes,
                })
                .collect(),
        },
        power,
        cleanup: CleanupPreviewDto {
            host_os: cleanup.host_os,
            note: cleanup.note,
            total_bytes: cleanup.total_bytes,
            entries: cleanup
                .entries
                .into_iter()
                .map(|e| CleanupPathDto {
                    label: e.label,
                    path: e.path,
                    bytes: e.bytes,
                    source: e.source,
                })
                .collect(),
        },
        thermal: ThermalDto {
            level: thermal.level.as_str().into(),
            cpu_temp_celsius: thermal.cpu_temp_celsius,
            gpu_temp_celsius: thermal.gpu_temp_celsius,
            throttling: thermal.throttling,
            high_temp_celsius: HIGH_TEMP_CELSIUS,
            warnings: thermal
                .warnings
                .into_iter()
                .map(|w| ThermalWarningDto {
                    source: w.source,
                    message: w.message,
                })
                .collect(),
            note: thermal.note,
        },
        weekly: WeeklyReportDto {
            window_start_unix_ms: weekly.window_start_unix_ms,
            window_end_unix_ms: weekly.window_end_unix_ms,
            note: weekly.note,
            measurements: weekly
                .measurements
                .into_iter()
                .map(|m| MeasurementSummaryDto {
                    session_id: m.session_id,
                    started_at_unix_ms: m.started_at_unix_ms,
                    ended_at_unix_ms: m.ended_at_unix_ms,
                    target_process: m.target_process,
                    frame_status: m.frame_status,
                })
                .collect(),
            applied_actions: weekly.applied_actions.into_iter().map(change_row).collect(),
            rollbacks: weekly.rollbacks.into_iter().map(change_row).collect(),
        },
    })
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ApplyPresetRequest {
    pub preset: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "status", rename_all = "snake_case")]
pub enum PresetOutcomeDto {
    Applied {
        entry_id: u64,
        before: String,
        after: String,
    },
    Failed {
        detail: String,
    },
}

/// Apply an everyday power preset through the Action + snapshot path (AC-5.3).
pub fn apply_power_preset(request: ApplyPresetRequest) -> PresetOutcomeDto {
    let preset = match EverydayPreset::parse(&request.preset) {
        Ok(p) => p,
        Err(e) => {
            return PresetOutcomeDto::Failed {
                detail: e.to_string(),
            }
        }
    };
    let mut action = PowerPresetAction::new(HostPowerPresetBackend, preset);
    let mut store = match SnapshotStore::open(snapshot_store_path()) {
        Ok(s) => s,
        Err(e) => {
            return PresetOutcomeDto::Failed {
                detail: e.to_string(),
            }
        }
    };
    match apply_with_snapshot(&mut action, &mut store, unix_ms_now()) {
        Ok(entry) => PresetOutcomeDto::Applied {
            entry_id: entry.id,
            before: entry.before,
            after: entry.after,
        },
        Err(e) => PresetOutcomeDto::Failed {
            detail: e.to_string(),
        },
    }
}

/// Restore the last everyday power preset change by entry id.
pub fn restore_power_preset(entry_id: u64) -> Result<ChangeRowDto, String> {
    let mut action = PowerPresetAction::new(HostPowerPresetBackend, EverydayPreset::Balanced);
    let mut store = SnapshotStore::open(snapshot_store_path()).map_err(|e| e.to_string())?;
    let entry = restore_with_log(&mut action, &mut store, entry_id).map_err(|e| e.to_string())?;
    Ok(change_row(entry))
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CleanupApplyRequest {
    pub confirm: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "status", rename_all = "snake_case")]
pub enum CleanupOutcomeDto {
    PreviewRequired { message: String },
    Applied { bytes_removed: u64 },
    Failed { detail: String },
}

/// Preview-gated cleanup (AC-5.4).
pub fn apply_cleanup(request: CleanupApplyRequest) -> CleanupOutcomeDto {
    if !request.confirm {
        return CleanupOutcomeDto::PreviewRequired {
            message: "Confirm after reviewing path sizes. Deletion is irreversible (AC-5.4)."
                .into(),
        };
    }
    match apply_everyday_cleanup(&HostEverydayCleanupBackend, true) {
        Ok(bytes) => CleanupOutcomeDto::Applied {
            bytes_removed: bytes,
        },
        Err(e) => CleanupOutcomeDto::Failed {
            detail: e.to_string(),
        },
    }
}

/// AC-5.2: everyday mode never kills; always errors with an honest message.
pub fn kill_hog_guard(confirm: bool) -> Result<(), String> {
    refuse_kill_without_confirm(confirm).map_err(|e| e.to_string())
}

/// Weekly report alone (AC-5.6).
pub fn weekly_health_report() -> Result<WeeklyReportDto, String> {
    let overview = everyday_overview()?;
    Ok(overview.weekly)
}

#[cfg(test)]
mod tests {
    use super::*;
    use hertzline_core::everyday::{
        apply_everyday_cleanup, preview_everyday_cleanup, CleanupPath, MockEverydayCleanupBackend,
        MockPowerPresetBackend, PRESET_PERFORMANCE,
    };
    use std::cell::RefCell;
    use std::path::PathBuf;
    use std::sync::atomic::{AtomicU64, Ordering};

    fn temp_dir(label: &str) -> PathBuf {
        static N: AtomicU64 = AtomicU64::new(0);
        let n = N.fetch_add(1, Ordering::Relaxed);
        let dir = std::env::temp_dir().join(format!("hertzline-app-hz033-{label}-{n}"));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).expect("temp dir");
        dir
    }

    #[test]
    fn labeled_mock_preset_and_cleanup_paths() {
        // labeled mock preset
        let backend = RefCell::new(MockPowerPresetBackend::new(
            "mock",
            EverydayPreset::Balanced,
            "mock:balanced",
        ));
        let mut action = PowerPresetAction::new(backend, EverydayPreset::Performance);
        let mut store = SnapshotStore::new();
        let entry = apply_with_snapshot(&mut action, &mut store, 1).unwrap();
        assert!(entry.after.contains(PRESET_PERFORMANCE));

        // labeled mock cleanup requires confirm
        let cleanup = RefCell::new(MockEverydayCleanupBackend::with_entries(vec![
            CleanupPath {
                label: "Temp".into(),
                path: "/tmp/mock".into(),
                bytes: 10,
                source: "mock".into(),
            },
        ]));
        let preview = preview_everyday_cleanup(&cleanup).unwrap();
        assert_eq!(preview.total_bytes, 10);
        assert!(apply_everyday_cleanup(&cleanup, false).is_err());
        assert_eq!(apply_everyday_cleanup(&cleanup, true).unwrap(), 10);
    }

    #[test]
    fn kill_guard_never_succeeds() {
        assert!(kill_hog_guard(false).is_err());
        assert!(kill_hog_guard(true).is_err());
    }

    #[test]
    fn overview_builds_on_host() {
        let _guard = crate::test_env::lock();
        let dir = temp_dir("overview");
        let prev_store = std::env::var_os(hertzline_core::paths::SNAPSHOT_STORE_ENV);
        let prev_archive = std::env::var_os(hertzline_core::paths::MEASUREMENT_ARCHIVE_ENV);
        std::env::set_var(
            hertzline_core::paths::SNAPSHOT_STORE_ENV,
            dir.join("snapshots.json").as_os_str(),
        );
        std::env::set_var(
            hertzline_core::paths::MEASUREMENT_ARCHIVE_ENV,
            dir.join("archive.json").as_os_str(),
        );
        let overview = everyday_overview().expect("overview");
        match prev_store {
            Some(v) => std::env::set_var(hertzline_core::paths::SNAPSHOT_STORE_ENV, v),
            None => std::env::remove_var(hertzline_core::paths::SNAPSHOT_STORE_ENV),
        }
        match prev_archive {
            Some(v) => std::env::set_var(hertzline_core::paths::MEASUREMENT_ARCHIVE_ENV, v),
            None => std::env::remove_var(hertzline_core::paths::MEASUREMENT_ARCHIVE_ENV),
        }
        let _ = std::fs::remove_dir_all(&dir);
        assert!(!overview.startup.host_os.is_empty());
        assert!(!overview.hogs.note.is_empty());
        assert!(!overview.cleanup.note.is_empty());
        assert!(!overview.thermal.level.is_empty());
        assert!(!overview.weekly.note.is_empty());
        assert!(overview.local_measurement_note.contains("No local"));
    }
}
