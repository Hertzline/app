//! Overlay settings UI host (HZ-034 / AC-6.1--AC-6.3).

use hertzline_core::overlay::{
    decide_for_install_path, load_settings, panel_from_frame_times_ms, probe_overlay_method,
    resolve_panel, save_settings, scan_install_path, OverlayDecision, OverlayPanel,
    OverlaySettings,
};
use serde::{Deserialize, Serialize};
use std::path::PathBuf;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct OverlaySettingsDto {
    pub globally_disabled: bool,
    pub user_enabled: bool,
    pub allow_with_anti_cheat: bool,
}

impl From<&OverlaySettings> for OverlaySettingsDto {
    fn from(s: &OverlaySettings) -> Self {
        Self {
            globally_disabled: s.globally_disabled,
            user_enabled: s.user_enabled,
            allow_with_anti_cheat: s.allow_with_anti_cheat,
        }
    }
}

impl From<OverlaySettingsDto> for OverlaySettings {
    fn from(d: OverlaySettingsDto) -> Self {
        Self {
            globally_disabled: d.globally_disabled,
            user_enabled: d.user_enabled,
            allow_with_anti_cheat: d.allow_with_anti_cheat,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct OverlayOverviewDto {
    pub settings: OverlaySettingsDto,
    pub method_id: String,
    pub method_summary: String,
    pub method_status: String,
    pub method_detail: String,
    pub injects_into_game: bool,
    pub decision_enabled: bool,
    pub decision_message: String,
    pub block_reason: Option<String>,
    pub anti_cheat_detected: bool,
    pub anti_cheat_note: String,
    pub panel: OverlayPanelDto,
    pub note: String,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct OverlayPanelDto {
    pub status: String,
    pub avg_fps: Option<f64>,
    pub avg_frame_time_ms: Option<f64>,
    pub low_1pct_fps: Option<f64>,
    pub sample_count: usize,
    pub note: String,
}

impl From<&OverlayPanel> for OverlayPanelDto {
    fn from(p: &OverlayPanel) -> Self {
        Self {
            status: match p.status {
                hertzline_core::overlay::OverlayPanelStatus::Available => "available".into(),
                hertzline_core::overlay::OverlayPanelStatus::Unavailable => "unavailable".into(),
                hertzline_core::overlay::OverlayPanelStatus::Disabled => "disabled".into(),
            },
            avg_fps: p.avg_fps,
            avg_frame_time_ms: p.avg_frame_time_ms,
            low_1pct_fps: p.low_1pct_fps,
            sample_count: p.sample_count,
            note: p.note.clone(),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SaveOverlaySettingsRequest {
    pub globally_disabled: bool,
    pub user_enabled: bool,
    pub allow_with_anti_cheat: bool,
    /// Optional install path to evaluate anti-cheat defaults.
    pub install_path: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct OverlayScanRequest {
    pub install_path: String,
}

fn settings_path() -> PathBuf {
    hertzline_core::overlay::overlay_settings_path()
}

fn decision_to_overview(
    settings: &OverlaySettings,
    decision: &OverlayDecision,
    anti_cheat_note: String,
    frame_times_ms: &[f64],
) -> OverlayOverviewDto {
    let method = probe_overlay_method();
    let panel = resolve_panel(decision, frame_times_ms);
    OverlayOverviewDto {
        settings: OverlaySettingsDto::from(settings),
        method_id: method.method_id,
        method_summary: method.method.summary().into(),
        method_status: match method.status {
            hertzline_core::frame_capture::ProbeStatus::Available => "available".into(),
            hertzline_core::frame_capture::ProbeStatus::Unavailable => "unavailable".into(),
            hertzline_core::frame_capture::ProbeStatus::NotPossible => "not_possible".into(),
        },
        method_detail: method.detail,
        injects_into_game: false,
        decision_enabled: decision.enabled,
        decision_message: decision.message.clone(),
        block_reason: decision.block_reason.map(|r| r.as_str().into()),
        anti_cheat_detected: decision.anti_cheat_detected,
        anti_cheat_note,
        panel: OverlayPanelDto::from(&panel),
        note: "Overlay uses supported methods only and never injects into the game. Off by default when anti-cheat install markers are found. One global disable control.".into(),
    }
}

/// Full overlay settings overview for the UI route.
pub fn overlay_overview(install_path: Option<&str>) -> Result<OverlayOverviewDto, String> {
    let settings = load_settings(&settings_path()).map_err(|e| e.to_string())?;
    let path = install_path
        .map(str::trim)
        .filter(|s| !s.is_empty())
        .map(PathBuf::from);
    let anti_note = match &path {
        Some(p) => scan_install_path(p).note,
        None => "no install path supplied; anti-cheat not scanned".into(),
    };
    let decision = decide_for_install_path(&settings, path.as_deref());
    Ok(decision_to_overview(&settings, &decision, anti_note, &[]))
}

/// Persist settings and return the refreshed overview.
pub fn save_overlay_settings(
    request: SaveOverlaySettingsRequest,
) -> Result<OverlayOverviewDto, String> {
    let mut settings = OverlaySettings {
        globally_disabled: request.globally_disabled,
        user_enabled: request.user_enabled,
        allow_with_anti_cheat: request.allow_with_anti_cheat,
    };
    // Global disable is the single kill switch (AC-6.3).
    if settings.globally_disabled {
        settings.user_enabled = false;
    }
    save_settings(&settings_path(), &settings).map_err(|e| e.to_string())?;
    overlay_overview(request.install_path.as_deref())
}

/// Disable overlay globally in one control (AC-6.3).
pub fn disable_overlay_globally() -> Result<OverlayOverviewDto, String> {
    let mut settings = load_settings(&settings_path()).map_err(|e| e.to_string())?;
    settings.disable_globally();
    save_settings(&settings_path(), &settings).map_err(|e| e.to_string())?;
    overlay_overview(None)
}

/// Scan an install path for anti-cheat markers (filesystem only).
pub fn scan_anti_cheat(request: OverlayScanRequest) -> Result<OverlayOverviewDto, String> {
    overlay_overview(Some(request.install_path.trim()))
}

/// Labeled mock panel from constant frame times (tests / smoke).
pub fn labeled_mock_panel_fps(fps: f64) -> OverlayPanelDto {
    // mock: synthetic constant frame times; not live capture
    let frame_ms = 1000.0 / fps.max(1.0);
    let frames = vec![frame_ms; 60];
    OverlayPanelDto::from(&panel_from_frame_times_ms(&frames))
}

#[cfg(test)]
mod tests {
    use super::*;
    use hertzline_core::overlay::OVERLAY_SETTINGS_ENV;
    use std::fs;
    use std::sync::atomic::{AtomicU64, Ordering};
    use std::sync::{Mutex, OnceLock};

    fn env_lock() -> &'static Mutex<()> {
        static LOCK: OnceLock<Mutex<()>> = OnceLock::new();
        LOCK.get_or_init(|| Mutex::new(()))
    }

    fn with_temp_settings<F: FnOnce()>(label: &str, f: F) {
        let _guard = env_lock().lock().unwrap();
        static N: AtomicU64 = AtomicU64::new(0);
        let n = N.fetch_add(1, Ordering::Relaxed);
        let dir = std::env::temp_dir().join(format!(
            "hertzline-app-hz034-{}-{}-{}",
            label,
            std::process::id(),
            n
        ));
        let _ = fs::remove_dir_all(&dir);
        fs::create_dir_all(&dir).unwrap();
        let path = dir.join("overlay_settings.json");
        let previous = std::env::var_os(OVERLAY_SETTINGS_ENV);
        std::env::set_var(OVERLAY_SETTINGS_ENV, path.as_os_str());
        f();
        match previous {
            Some(value) => std::env::set_var(OVERLAY_SETTINGS_ENV, value),
            None => std::env::remove_var(OVERLAY_SETTINGS_ENV),
        }
        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn overview_defaults_off() {
        with_temp_settings("defaults", || {
            let overview = overlay_overview(None).expect("overview");
            assert!(!overview.settings.user_enabled);
            assert!(!overview.settings.globally_disabled);
            assert!(!overview.decision_enabled);
            assert!(!overview.injects_into_game);
            assert_eq!(overview.panel.status, "disabled");
        });
    }

    #[test]
    fn global_disable_one_control() {
        with_temp_settings("global", || {
            let _ = save_overlay_settings(SaveOverlaySettingsRequest {
                globally_disabled: false,
                user_enabled: true,
                allow_with_anti_cheat: false,
                install_path: None,
            })
            .unwrap();
            let disabled = disable_overlay_globally().unwrap();
            assert!(disabled.settings.globally_disabled);
            assert!(!disabled.settings.user_enabled);
            assert!(!disabled.decision_enabled);
            assert_eq!(disabled.block_reason.as_deref(), Some("globally_disabled"));
        });
    }

    #[test]
    fn anti_cheat_path_defaults_off() {
        with_temp_settings("ac", || {
            let dir = std::env::temp_dir()
                .join(format!("hertzline-app-hz034-acgame-{}", std::process::id()));
            let _ = fs::remove_dir_all(&dir);
            fs::create_dir_all(dir.join("BattlEye")).unwrap();
            let overview = save_overlay_settings(SaveOverlaySettingsRequest {
                globally_disabled: false,
                user_enabled: true,
                allow_with_anti_cheat: false,
                install_path: Some(dir.display().to_string()),
            })
            .unwrap();
            assert!(overview.anti_cheat_detected);
            assert!(!overview.decision_enabled);
            assert_eq!(
                overview.block_reason.as_deref(),
                Some("anti_cheat_default_off")
            );
            let _ = fs::remove_dir_all(&dir);
        });
    }

    #[test]
    fn labeled_mock_panel_has_fps() {
        // mock: labeled synthetic panel
        let panel = labeled_mock_panel_fps(60.0);
        assert_eq!(panel.status, "available");
        assert!(panel.avg_fps.unwrap() > 59.0 && panel.avg_fps.unwrap() < 61.0);
    }
}
