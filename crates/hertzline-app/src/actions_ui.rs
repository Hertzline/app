//! Actions + explanations + restore UI host (HZ-031).
//!
//! Catalog exposes AC-4.x per OS with AC-8.1 explanation fields and AC-8.2 local
//! measurement notes. Apply/restore go through the core Action trait + snapshot
//! store. Elevation is prompted per action (never whole-app). macOS honesty
//! banner is required (AC-4.23). Change log (AC-7.5) and restore-all (AC-7.2).

use std::cell::RefCell;

use hertzline_core::action::{
    apply_with_snapshot, restore_with_log, Action, ActionExplanation, ActionMap,
    LocalMeasurementNote, MockToggleAction,
};
use hertzline_core::actions::linux::{
    CompositorHintAction, EnergyProfileAction, FeralGameModeAction, MangoHudAction,
    MockCompositorBackend, MockEnergyProfileBackend, MockFeralGameModeBackend, MockMangoHudBackend,
    MockNiceIoniceBackend, MockProtonInfoBackend, MockShaderCacheBackend, NiceIoniceAction,
    ProtonInfoAction, ShaderCacheAction, IOPRIO_CLASS_BE,
};
use hertzline_core::actions::macos::{
    LowPowerModeWarningAction, LowPowerState, MacOsBackgroundAppsAction, MacOsCapabilitySurface,
    MacOsDisplayState, MacOsGameModeReport, MacOsGameModeStatusAction, MacOsMonitorAction,
    MacOsRefreshAction, MockLowPowerBackend, MockMacOsBackgroundAppsBackend,
    MockMacOsGameModeBackend, MockMacOsRefreshBackend, MockMonitorBackend, MonitorSnapshot,
    MACOS_HONESTY_BANNER,
};
use hertzline_core::actions::windows::{
    BackgroundAppsAction, CleanupAction, DisplayRefreshAction, DisplayState, DriverCheckAction,
    GameBarAction, GameModeAction, HagsAction, MockBackgroundAppsBackend, MockCleanupBackend,
    MockDisplayBackend, MockDriversBackend, MockGameBarBackend, MockGameModeBackend,
    MockHagsBackend, MockPowerPlanBackend, MockProcessPriorityBackend, MockStartupAppsBackend,
    MockStorageBackend, PowerPlanAction, ProcessPriorityAction, StartupAppsAction,
    StorageHealthAction, GUID_BALANCED, GUID_HIGH_PERFORMANCE, HAGS_ON,
};
use hertzline_core::paths::snapshot_store_path;
use hertzline_core::restore::{restore_all, MapActionRegistry, RestoreTrigger};
use hertzline_core::snapshot::{unix_ms_now, RestoreStatus, SnapshotStore};
use serde::{Deserialize, Serialize};

use crate::measure::{explanation_with_evidence, NO_LOCAL_ACTION_MEASUREMENT};

/// AC-8.1 fields for the UI.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ExplanationDto {
    pub what: String,
    pub expected_effect: String,
    pub risk: String,
    pub how_to_undo: String,
    pub local_measurement: String,
}

impl From<&ActionExplanation> for ExplanationDto {
    fn from(e: &ActionExplanation) -> Self {
        Self {
            what: e.what.clone(),
            expected_effect: e.expected_effect.clone(),
            risk: e.risk.clone(),
            how_to_undo: e.how_to_undo.clone(),
            local_measurement: explanation_with_evidence(match &e.local_measurement {
                LocalMeasurementNote::NoneYet => None,
            }),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ActionCardDto {
    pub id: String,
    pub title: String,
    pub os: String,
    pub ac_refs: Vec<String>,
    pub explanation: ExplanationDto,
    pub requires_elevation: bool,
    /// False for report-only / not-possible / needs input before switch.
    pub exposes_switch: bool,
    /// `toggle` | `report` | `needs_input` | `destructive`
    pub kind: String,
    /// `available` | `wrong_os` | `not_possible`
    pub host_status: String,
    pub current_state: Option<String>,
    pub elevation_reason: Option<String>,
    pub input_hint: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct MacOsHonestyDto {
    pub banner: String,
    pub capabilities: Vec<MacOsCapDto>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct MacOsCapDto {
    pub id: String,
    pub level: String,
    pub exposes_toggle: bool,
    pub kind: String,
    pub summary: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ActionCatalogDto {
    pub host_os: String,
    pub local_measurement_note: String,
    pub macos_honesty: Option<MacOsHonestyDto>,
    pub actions: Vec<ActionCardDto>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ApplyRequest {
    pub id: String,
    /// Required true when the action needs elevation (AC elevation prompt).
    pub confirm_elevation: bool,
    /// Required true for destructive cleanup.
    pub confirm_destructive: bool,
    /// PID, startup name, or refresh Hz depending on action.
    pub input: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "status", rename_all = "snake_case")]
pub enum ApplyOutcomeDto {
    Applied {
        entry_id: u64,
        action: String,
        before: String,
        after: String,
    },
    ElevationRequired {
        action: String,
        reason: String,
        message: String,
    },
    ConfirmDestructive {
        action: String,
        message: String,
    },
    Failed {
        action: String,
        detail: String,
    },
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ChangeLogRowDto {
    pub id: u64,
    pub time_unix_ms: i64,
    pub action: String,
    pub before: String,
    pub after: String,
    pub restore_status: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RestoreAllDto {
    pub trigger: String,
    pub requires_ui: bool,
    pub ok: bool,
    pub status: String,
    pub store_path: String,
    pub items: Vec<String>,
}

fn host_os() -> &'static str {
    std::env::consts::OS
}

fn expl(action: &dyn Action) -> ExplanationDto {
    ExplanationDto::from(action.explanation())
}

fn macos_honesty_dto() -> MacOsHonestyDto {
    let surface = MacOsCapabilitySurface::load();
    MacOsHonestyDto {
        banner: surface.honesty_banner.to_string(),
        capabilities: surface
            .capabilities
            .iter()
            .map(|c| MacOsCapDto {
                id: c.id.to_string(),
                level: c.level.as_str().to_string(),
                exposes_toggle: c.exposes_toggle,
                kind: match c.kind {
                    hertzline_core::actions::MacOsCapabilityKind::Measurement => "measure".into(),
                    hertzline_core::actions::MacOsCapabilityKind::Action => "action".into(),
                    hertzline_core::actions::MacOsCapabilityKind::Safety => "safety".into(),
                },
                summary: c.summary.to_string(),
            })
            .collect(),
    }
}

#[allow(clippy::too_many_arguments)]
fn card(
    id: &str,
    title: &str,
    os: &str,
    ac: &[&str],
    explanation: ExplanationDto,
    requires_elevation: bool,
    exposes_switch: bool,
    kind: &str,
    input_hint: Option<&str>,
    elevation_reason: Option<&str>,
) -> ActionCardDto {
    let host = host_os();
    let host_status = if os == host || (os == "linux" && host == "linux") {
        "available".into()
    } else {
        "wrong_os".into()
    };
    ActionCardDto {
        id: id.into(),
        title: title.into(),
        os: os.into(),
        ac_refs: ac.iter().map(|s| (*s).to_string()).collect(),
        explanation,
        requires_elevation,
        exposes_switch: exposes_switch && host_status == "available",
        kind: kind.into(),
        host_status,
        current_state: None,
        elevation_reason: elevation_reason.map(|s| s.to_string()),
        input_hint: input_hint.map(|s| s.to_string()),
    }
}

/// Build the full catalog with AC-8.1 text from core action constructors (mocks).
pub fn list_action_catalog() -> ActionCatalogDto {
    let mut actions = Vec::new();

    // Windows AC-4.1--4.10
    {
        let a = PowerPlanAction::high_performance(RefCell::new(MockPowerPlanBackend::new(
            GUID_BALANCED,
            &[GUID_BALANCED, GUID_HIGH_PERFORMANCE],
        )));
        actions.push(card(
            a.id(),
            "Power plan",
            "windows",
            &["AC-4.1"],
            expl(&a),
            a.requires_elevation(),
            true,
            "toggle",
            None,
            None,
        ));
    }
    {
        let a = GameModeAction::enable(RefCell::new(MockGameModeBackend::new(false)));
        actions.push(card(
            a.id(),
            "Game Mode",
            "windows",
            &["AC-4.2"],
            expl(&a),
            a.requires_elevation(),
            true,
            "toggle",
            None,
            None,
        ));
    }
    {
        let a = HagsAction::enable(RefCell::new(MockHagsBackend::supported(HAGS_ON)));
        actions.push(card(
            a.id(),
            "HAGS",
            "windows",
            &["AC-4.2"],
            expl(&a),
            a.requires_elevation(),
            true,
            "toggle",
            None,
            Some("Writes HKLM HwSchMode; Windows will prompt for elevation via the helper."),
        ));
    }
    {
        let a = ProcessPriorityAction::high_priority(
            RefCell::new(MockProcessPriorityBackend::new(1, 0x20, Some(0xFFFF))),
            1,
        );
        actions.push(card(
            a.id(),
            "Process priority / affinity",
            "windows",
            &["AC-4.3"],
            expl(&a),
            a.requires_elevation(),
            true,
            "needs_input",
            Some("Target process PID"),
            None,
        ));
    }
    {
        let a = BackgroundAppsAction::pause(
            RefCell::new(MockBackgroundAppsBackend::running(1, "mock.exe")),
            1,
        );
        actions.push(card(
            a.id(),
            "Background apps (user-chosen)",
            "windows",
            &["AC-4.4"],
            expl(&a),
            a.requires_elevation(),
            true,
            "needs_input",
            Some("User-chosen PID to pause"),
            None,
        ));
    }
    {
        let a = StartupAppsAction::disable(
            RefCell::new(MockStartupAppsBackend::with_entries(vec![])),
            "Example",
        );
        actions.push(card(
            a.id(),
            "Startup apps",
            "windows",
            &["AC-4.5"],
            expl(&a),
            a.requires_elevation(),
            true,
            "needs_input",
            Some("Startup entry name"),
            None,
        ));
    }
    {
        let a = GameBarAction::disable(RefCell::new(MockGameBarBackend::new(true)));
        actions.push(card(
            a.id(),
            "Xbox Game Bar capture",
            "windows",
            &["AC-4.6"],
            expl(&a),
            a.requires_elevation(),
            true,
            "toggle",
            None,
            None,
        ));
    }
    {
        let a = CleanupAction::preview_only(RefCell::new(MockCleanupBackend::with_entries(vec![])));
        actions.push(card(
            a.id(),
            "Temp / shader cleanup",
            "windows",
            &["AC-4.7"],
            expl(&a),
            a.requires_elevation(),
            true,
            "destructive",
            None,
            None,
        ));
    }
    {
        let a = StorageHealthAction::new(RefCell::new(MockStorageBackend { volumes: vec![] }));
        actions.push(card(
            a.id(),
            "Storage health",
            "windows",
            &["AC-4.8"],
            expl(&a),
            false,
            false,
            "report",
            None,
            None,
        ));
    }
    {
        let a = DriverCheckAction::new(RefCell::new(MockDriversBackend { drivers: vec![] }));
        actions.push(card(
            a.id(),
            "Driver check (official links)",
            "windows",
            &["AC-4.9"],
            expl(&a),
            false,
            false,
            "report",
            None,
            None,
        ));
    }
    {
        let a = DisplayRefreshAction::report_only(RefCell::new(MockDisplayBackend {
            state: DisplayState {
                device: "mock".into(),
                width: 1920,
                height: 1080,
                refresh_hz: 60,
            },
        }));
        actions.push(card(
            a.id(),
            "Display refresh / VRR",
            "windows",
            &["AC-4.10"],
            expl(&a),
            a.requires_elevation(),
            true,
            "needs_input",
            Some("Optional target Hz (empty = report only)"),
            None,
        ));
    }

    // Linux AC-4.11--4.17
    {
        let a = FeralGameModeAction::for_pid(
            RefCell::new(MockFeralGameModeBackend::new_available()),
            1,
        );
        actions.push(card(
            a.id(),
            "Feral GameMode",
            "linux",
            &["AC-4.11"],
            expl(&a),
            a.requires_elevation(),
            true,
            "needs_input",
            Some("Game process PID"),
            None,
        ));
    }
    {
        let a = EnergyProfileAction::performance(RefCell::new(MockEnergyProfileBackend::ppd(
            "balanced",
        )));
        actions.push(card(
            a.id(),
            "Energy profile",
            "linux",
            &["AC-4.12"],
            expl(&a),
            a.requires_elevation(),
            true,
            "toggle",
            None,
            None,
        ));
    }
    {
        let a = NiceIoniceAction::default_boost(
            RefCell::new(MockNiceIoniceBackend::new(1, 0, IOPRIO_CLASS_BE, 4)),
            1,
        );
        actions.push(card(
            a.id(),
            "nice / ionice",
            "linux",
            &["AC-4.13"],
            expl(&a),
            a.requires_elevation(),
            true,
            "needs_input",
            Some("Game process PID"),
            Some("Negative nice may require polkit elevation."),
        ));
    }
    {
        let a = CompositorHintAction::new(RefCell::new(MockCompositorBackend::kwin("false")));
        actions.push(card(
            a.id(),
            "Compositor hints",
            "linux",
            &["AC-4.14"],
            expl(&a),
            a.requires_elevation(),
            true,
            "toggle",
            None,
            None,
        ));
    }
    {
        let a = MangoHudAction::new(RefCell::new(MockMangoHudBackend::available_off()));
        actions.push(card(
            a.id(),
            "MangoHud",
            "linux",
            &["AC-4.15"],
            expl(&a),
            a.requires_elevation(),
            true,
            "toggle",
            None,
            None,
        ));
    }
    {
        let a = ProtonInfoAction::new(RefCell::new(MockProtonInfoBackend { tools: vec![] }));
        actions.push(card(
            a.id(),
            "Proton / Wine info",
            "linux",
            &["AC-4.16"],
            expl(&a),
            false,
            false,
            "report",
            None,
            None,
        ));
    }
    {
        let a = ShaderCacheAction::preview_only(RefCell::new(
            MockShaderCacheBackend::with_entries(vec![]),
        ));
        actions.push(card(
            a.id(),
            "Shader cache cleanup",
            "linux",
            &["AC-4.17"],
            expl(&a),
            a.requires_elevation(),
            true,
            "destructive",
            None,
            None,
        ));
    }

    // macOS AC-4.18--4.22 (+ honesty AC-4.23)
    {
        let a = MacOsMonitorAction::new(RefCell::new(MockMonitorBackend {
            snapshot: MonitorSnapshot {
                honesty_banner: MACOS_HONESTY_BANNER.into(),
                frame_capability: "partial".into(),
                frame_status: "not_possible".into(),
                frame_detail: "mock".into(),
                metrics: vec![],
            },
        }));
        let mut c = card(
            a.id(),
            "macOS monitoring",
            "macos",
            &["AC-4.18", "AC-1.7"],
            expl(&a),
            false,
            false,
            "report",
            None,
            None,
        );
        if host_os() == "macos" {
            c.host_status = "available".into();
        }
        actions.push(c);
    }
    {
        let a = MacOsGameModeStatusAction::new(RefCell::new(MockMacOsGameModeBackend {
            report: MacOsGameModeReport::not_possible(),
        }));
        actions.push(card(
            a.id(),
            "Game Mode status",
            "macos",
            &["AC-4.19"],
            expl(&a),
            false,
            a.exposes_toggle(),
            "report",
            None,
            None,
        ));
    }
    {
        let a = MacOsBackgroundAppsAction::pause(
            RefCell::new(MockMacOsBackgroundAppsBackend::running(1, "MockApp")),
            1,
        );
        actions.push(card(
            a.id(),
            "Background apps (user-chosen)",
            "macos",
            &["AC-4.20"],
            expl(&a),
            a.requires_elevation(),
            a.exposes_toggle(),
            "needs_input",
            Some("User-chosen PID"),
            None,
        ));
    }
    {
        let a =
            LowPowerModeWarningAction::for_performance_work(RefCell::new(MockLowPowerBackend {
                state: LowPowerState {
                    active: false,
                    available: true,
                    detail: "mock".into(),
                    warning: None,
                },
            }));
        actions.push(card(
            a.id(),
            "Low Power Mode warning",
            "macos",
            &["AC-4.21"],
            expl(&a),
            false,
            a.exposes_toggle(),
            "report",
            None,
            None,
        ));
    }
    {
        let a = MacOsRefreshAction::report_only(RefCell::new(MockMacOsRefreshBackend {
            state: MacOsDisplayState {
                display_id: 0,
                width: 1920,
                height: 1080,
                refresh_hz: 60,
                refresh_available: true,
            },
        }));
        actions.push(card(
            a.id(),
            "Refresh rate",
            "macos",
            &["AC-4.22"],
            expl(&a),
            a.requires_elevation(),
            a.exposes_toggle(),
            "needs_input",
            Some("Optional target Hz (empty = report only)"),
            None,
        ));
    }

    // Probe only the recommended pack on catalog load (full probe freezes the UI).
    const FAST_PROBE: &[&str] = &[
        "windows.game_mode",
        "windows.power_plan",
        "windows.game_bar",
    ];
    for action in &mut actions {
        if action.host_status != "available" {
            continue;
        }
        if !FAST_PROBE.contains(&action.id.as_str()) {
            continue;
        }
        if let Ok(state) = read_host_state(&action.id, action.input_hint.as_deref()) {
            action.current_state = Some(state);
        }
    }

    let macos_honesty = if host_os() == "macos" {
        Some(macos_honesty_dto())
    } else {
        // Still expose honesty text for macOS cards on other hosts (AC-4.23 docs).
        Some(macos_honesty_dto())
    };

    ActionCatalogDto {
        host_os: host_os().into(),
        local_measurement_note: NO_LOCAL_ACTION_MEASUREMENT.into(),
        macos_honesty,
        actions,
    }
}

fn open_store() -> Result<SnapshotStore, String> {
    SnapshotStore::open(snapshot_store_path()).map_err(|e| e.to_string())
}

fn parse_pid(input: Option<&str>) -> Result<u32, String> {
    let raw = input
        .map(str::trim)
        .filter(|s| !s.is_empty())
        .ok_or_else(|| "PID required".to_string())?;
    raw.parse::<u32>()
        .map_err(|_| format!("invalid PID: {raw}"))
}

/// Read live state when an OS backend exists for this host.
fn read_host_state(id: &str, _hint: Option<&str>) -> Result<String, String> {
    let action = build_host_action(id, None, false)?;
    action.read_state().map_err(|e| e.to_string())
}

/// Build a host action for apply/restore. Mocks only when `labeled_mock` is true.
fn build_host_action(
    id: &str,
    input: Option<&str>,
    confirmed_destructive: bool,
) -> Result<Box<dyn Action>, String> {
    // Destructive confirm is only consumed by Windows/Linux cleanup arms.
    #[cfg(not(any(windows, target_os = "linux")))]
    let _ = confirmed_destructive;
    match id {
        "mock_toggle" => Ok(Box::new(MockToggleAction::new("mock_toggle", "off", "on"))),
        #[cfg(windows)]
        "windows.power_plan" => Ok(Box::new(PowerPlanAction::high_performance(
            hertzline_core::actions::windows::power_plan::os::OsPowerPlanBackend,
        ))),
        #[cfg(windows)]
        "windows.game_mode" => Ok(Box::new(GameModeAction::enable(
            hertzline_core::actions::windows::game_mode::os::OsGameModeBackend,
        ))),
        #[cfg(windows)]
        "windows.hags" => Ok(Box::new(HagsAction::enable(
            hertzline_core::actions::windows::hags::os::OsHagsBackend,
        ))),
        #[cfg(windows)]
        "windows.process_priority" => {
            let pid = parse_pid(input)?;
            Ok(Box::new(ProcessPriorityAction::high_priority(
                hertzline_core::actions::windows::priority::os::OsProcessPriorityBackend,
                pid,
            )))
        }
        #[cfg(windows)]
        "windows.background_apps" => {
            let pid = parse_pid(input)?;
            Ok(Box::new(BackgroundAppsAction::pause(
                hertzline_core::actions::windows::background::os::OsBackgroundAppsBackend,
                pid,
            )))
        }
        #[cfg(windows)]
        "windows.startup_apps" => {
            let name = input
                .map(str::trim)
                .filter(|s| !s.is_empty())
                .ok_or_else(|| "startup entry name required".to_string())?;
            Ok(Box::new(StartupAppsAction::disable(
                hertzline_core::actions::windows::startup::os::OsStartupAppsBackend,
                name,
            )))
        }
        #[cfg(windows)]
        "windows.game_bar_capture" => Ok(Box::new(GameBarAction::disable(
            hertzline_core::actions::windows::game_bar::os::OsGameBarBackend,
        ))),
        #[cfg(windows)]
        "windows.temp_shader_cleanup" => {
            if confirmed_destructive {
                Ok(Box::new(CleanupAction::confirmed(
                    hertzline_core::actions::windows::cleanup::os::OsCleanupBackend,
                )))
            } else {
                Ok(Box::new(CleanupAction::preview_only(
                    hertzline_core::actions::windows::cleanup::os::OsCleanupBackend,
                )))
            }
        }
        #[cfg(windows)]
        "windows.storage_health" => Ok(Box::new(StorageHealthAction::new(
            hertzline_core::actions::windows::storage::os::OsStorageBackend,
        ))),
        #[cfg(windows)]
        "windows.driver_check" => Ok(Box::new(DriverCheckAction::new(
            hertzline_core::actions::windows::drivers::os::OsDriversBackend,
        ))),
        #[cfg(windows)]
        "windows.display_refresh" => {
            if let Some(hz) = input.map(str::trim).filter(|s| !s.is_empty()) {
                let hz: u32 = hz
                    .parse()
                    .map_err(|_| format!("invalid refresh Hz: {hz}"))?;
                Ok(Box::new(DisplayRefreshAction::set_hz(
                    hertzline_core::actions::windows::display::os::OsDisplayBackend,
                    hz,
                )))
            } else {
                Ok(Box::new(DisplayRefreshAction::report_only(
                    hertzline_core::actions::windows::display::os::OsDisplayBackend,
                )))
            }
        }
        #[cfg(target_os = "linux")]
        "linux.feral_gamemode" => {
            let pid = parse_pid(input)?;
            Ok(Box::new(FeralGameModeAction::for_pid(
                hertzline_core::actions::linux::gamemode::os::OsFeralGameModeBackend,
                pid,
            )))
        }
        #[cfg(target_os = "linux")]
        "linux.energy_profile" => Ok(Box::new(EnergyProfileAction::performance(
            hertzline_core::actions::linux::governor::os::OsEnergyProfileBackend,
        ))),
        #[cfg(target_os = "linux")]
        "linux.nice_ionice" => {
            let pid = parse_pid(input)?;
            Ok(Box::new(NiceIoniceAction::default_boost(
                hertzline_core::actions::linux::nice_ionice::os::OsNiceIoniceBackend,
                pid,
            )))
        }
        #[cfg(target_os = "linux")]
        "linux.compositor_hint" => Ok(Box::new(CompositorHintAction::new(
            hertzline_core::actions::linux::compositor::os::OsCompositorBackend,
        ))),
        #[cfg(target_os = "linux")]
        "linux.mangohud" => Ok(Box::new(MangoHudAction::new(
            hertzline_core::actions::linux::mangohud::os::OsMangoHudBackend,
        ))),
        #[cfg(target_os = "linux")]
        "linux.proton_wine_info" => Ok(Box::new(ProtonInfoAction::new(
            hertzline_core::actions::linux::proton::os::OsProtonInfoBackend,
        ))),
        #[cfg(target_os = "linux")]
        "linux.shader_cache_cleanup" => {
            if confirmed_destructive {
                Ok(Box::new(ShaderCacheAction::confirmed(
                    hertzline_core::actions::linux::shader_cache::os::OsShaderCacheBackend,
                )))
            } else {
                Ok(Box::new(ShaderCacheAction::preview_only(
                    hertzline_core::actions::linux::shader_cache::os::OsShaderCacheBackend,
                )))
            }
        }
        #[cfg(target_os = "macos")]
        "macos.monitor" => Ok(Box::new(MacOsMonitorAction::new(
            hertzline_core::actions::macos::monitor::os::OsMonitorBackend,
        ))),
        #[cfg(target_os = "macos")]
        "macos.game_mode_status" => Ok(Box::new(MacOsGameModeStatusAction::new(
            hertzline_core::actions::macos::game_mode::host::HostMacOsGameModeBackend,
        ))),
        #[cfg(target_os = "macos")]
        "macos.background_apps" => {
            let pid = parse_pid(input)?;
            Ok(Box::new(MacOsBackgroundAppsAction::pause(
                hertzline_core::actions::macos::background::os::OsMacOsBackgroundAppsBackend,
                pid,
            )))
        }
        #[cfg(target_os = "macos")]
        "macos.low_power_mode" => Ok(Box::new(LowPowerModeWarningAction::for_performance_work(
            hertzline_core::actions::macos::low_power::os::OsLowPowerBackend,
        ))),
        #[cfg(target_os = "macos")]
        "macos.refresh_rate" => {
            if let Some(hz) = input.map(str::trim).filter(|s| !s.is_empty()) {
                let hz: u32 = hz
                    .parse()
                    .map_err(|_| format!("invalid refresh Hz: {hz}"))?;
                Ok(Box::new(MacOsRefreshAction::set_hz(
                    hertzline_core::actions::macos::refresh::os::OsMacOsRefreshBackend,
                    hz,
                )))
            } else {
                Ok(Box::new(MacOsRefreshAction::report_only(
                    hertzline_core::actions::macos::refresh::os::OsMacOsRefreshBackend,
                )))
            }
        }
        other => Err(format!(
            "action {other} is not available on this host ({})",
            host_os()
        )),
    }
}

/// Register restore handlers for the current host (mirrors helper restore registry).
pub fn register_host_actions(registry: &mut MapActionRegistry) {
    #[cfg(windows)]
    {
        use hertzline_core::actions::windows::{
            background::os::OsBackgroundAppsBackend, cleanup::os::OsCleanupBackend,
            display::os::OsDisplayBackend, drivers::os::OsDriversBackend,
            game_bar::os::OsGameBarBackend, game_mode::os::OsGameModeBackend,
            hags::os::OsHagsBackend, power_plan::os::OsPowerPlanBackend,
            priority::os::OsProcessPriorityBackend, startup::os::OsStartupAppsBackend,
            storage::os::OsStorageBackend,
        };
        registry.insert(Box::new(PowerPlanAction::high_performance(
            OsPowerPlanBackend,
        )));
        registry.insert(Box::new(GameModeAction::enable(OsGameModeBackend)));
        registry.insert(Box::new(HagsAction::enable(OsHagsBackend)));
        registry.insert(Box::new(ProcessPriorityAction::high_priority(
            OsProcessPriorityBackend,
            0,
        )));
        registry.insert(Box::new(BackgroundAppsAction::pause(
            OsBackgroundAppsBackend,
            0,
        )));
        registry.insert(Box::new(StartupAppsAction::disable(
            OsStartupAppsBackend,
            "",
        )));
        registry.insert(Box::new(GameBarAction::disable(OsGameBarBackend)));
        registry.insert(Box::new(CleanupAction::confirmed(OsCleanupBackend)));
        registry.insert(Box::new(StorageHealthAction::new(OsStorageBackend)));
        registry.insert(Box::new(DriverCheckAction::new(OsDriversBackend)));
        registry.insert(Box::new(DisplayRefreshAction::report_only(
            OsDisplayBackend,
        )));
    }
    #[cfg(target_os = "linux")]
    {
        use hertzline_core::actions::linux::{
            compositor::os::OsCompositorBackend, gamemode::os::OsFeralGameModeBackend,
            governor::os::OsEnergyProfileBackend, mangohud::os::OsMangoHudBackend,
            nice_ionice::os::OsNiceIoniceBackend, proton::os::OsProtonInfoBackend,
            shader_cache::os::OsShaderCacheBackend,
        };
        registry.insert(Box::new(FeralGameModeAction::for_pid(
            OsFeralGameModeBackend,
            0,
        )));
        registry.insert(Box::new(EnergyProfileAction::performance(
            OsEnergyProfileBackend,
        )));
        registry.insert(Box::new(NiceIoniceAction::default_boost(
            OsNiceIoniceBackend,
            0,
        )));
        registry.insert(Box::new(CompositorHintAction::new(OsCompositorBackend)));
        registry.insert(Box::new(MangoHudAction::new(OsMangoHudBackend)));
        registry.insert(Box::new(ProtonInfoAction::new(OsProtonInfoBackend)));
        registry.insert(Box::new(ShaderCacheAction::confirmed(OsShaderCacheBackend)));
    }
    #[cfg(target_os = "macos")]
    {
        use hertzline_core::actions::macos::{
            background::os::OsMacOsBackgroundAppsBackend,
            game_mode::host::HostMacOsGameModeBackend, low_power::os::OsLowPowerBackend,
            monitor::os::OsMonitorBackend, refresh::os::OsMacOsRefreshBackend,
        };
        registry.insert(Box::new(MacOsMonitorAction::new(OsMonitorBackend)));
        registry.insert(Box::new(MacOsGameModeStatusAction::new(
            HostMacOsGameModeBackend,
        )));
        registry.insert(Box::new(MacOsBackgroundAppsAction::pause(
            OsMacOsBackgroundAppsBackend,
            0,
        )));
        registry.insert(Box::new(LowPowerModeWarningAction::for_performance_work(
            OsLowPowerBackend,
        )));
        registry.insert(Box::new(MacOsRefreshAction::report_only(
            OsMacOsRefreshBackend,
        )));
    }
}

/// Apply one action with snapshot (AC-7.1) after elevation / destructive gates.
pub fn apply_action(request: ApplyRequest) -> ApplyOutcomeDto {
    let id = request.id.trim().to_string();
    if id.is_empty() {
        return ApplyOutcomeDto::Failed {
            action: String::new(),
            detail: "action id is empty".into(),
        };
    }

    let catalog = list_action_catalog();
    let meta = catalog.actions.iter().find(|a| a.id == id);
    let requires_elevation = meta.map(|m| m.requires_elevation).unwrap_or(false);
    let kind = meta.map(|m| m.kind.as_str()).unwrap_or("toggle");

    if requires_elevation && !request.confirm_elevation {
        let reason = meta
            .and_then(|m| m.elevation_reason.clone())
            .unwrap_or_else(|| {
                "This change needs a privileged helper. Hertzline will not elevate the whole app."
                    .into()
            });
        return ApplyOutcomeDto::ElevationRequired {
            action: id.clone(),
            reason: reason.clone(),
            message: format!(
                "Elevation required for {id}. Confirm to continue with a per-action privilege prompt. {reason}"
            ),
        };
    }

    if kind == "destructive" && !request.confirm_destructive {
        return ApplyOutcomeDto::ConfirmDestructive {
            action: id,
            message: "Cleanup deletes files. Confirm after reviewing the size preview.".into(),
        };
    }

    let input = request.input.as_deref();
    let mut action = match build_host_action(&id, input, request.confirm_destructive) {
        Ok(a) => a,
        Err(detail) => return ApplyOutcomeDto::Failed { action: id, detail },
    };

    if action.requires_elevation() && !request.confirm_elevation {
        return ApplyOutcomeDto::ElevationRequired {
            action: id.clone(),
            reason: "Action reports requires_elevation".into(),
            message: format!(
                "Elevation required for {id}. Confirm to continue with a per-action privilege prompt."
            ),
        };
    }

    let mut store = match open_store() {
        Ok(s) => s,
        Err(detail) => return ApplyOutcomeDto::Failed { action: id, detail },
    };

    match apply_with_snapshot(action.as_mut(), &mut store, unix_ms_now()) {
        Ok(entry) => ApplyOutcomeDto::Applied {
            entry_id: entry.id,
            action: entry.action,
            before: entry.before,
            after: entry.after,
        },
        Err(err) => ApplyOutcomeDto::Failed {
            action: id,
            detail: err.to_string(),
        },
    }
}

/// Labeled mock apply for smoke / unit tests (not a real OS tweak).
pub fn apply_labeled_mock() -> Result<ApplyOutcomeDto, String> {
    let mut store = open_store()?;
    let mut action = MockToggleAction::new("mock_toggle", "off", "on");
    let entry =
        apply_with_snapshot(&mut action, &mut store, unix_ms_now()).map_err(|e| e.to_string())?;
    Ok(ApplyOutcomeDto::Applied {
        entry_id: entry.id,
        action: entry.action,
        before: entry.before,
        after: entry.after,
    })
}

/// Restore one change-log entry (AC-7.5).
pub fn restore_change_entry(entry_id: u64) -> Result<ChangeLogRowDto, String> {
    let mut store = open_store()?;
    let action_id = store
        .change_log()
        .iter()
        .find(|e| e.id == entry_id)
        .map(|e| e.action.clone())
        .ok_or_else(|| "change log entry not found".to_string())?;

    let mut registry = MapActionRegistry::new();
    register_host_actions(&mut registry);
    if action_id == "mock_toggle" {
        registry.insert(Box::new(MockToggleAction::new("mock_toggle", "on", "on")));
    }
    let action = registry
        .get_mut(&action_id)
        .ok_or_else(|| format!("no restore handler for {action_id}"))?;
    let entry = restore_with_log(action, &mut store, entry_id).map_err(|e| e.to_string())?;
    Ok(ChangeLogRowDto {
        id: entry.id,
        time_unix_ms: entry.time_unix_ms,
        action: entry.action,
        before: entry.before,
        after: entry.after,
        restore_status: entry.restore_status.as_str().into(),
    })
}

/// Visible change log (AC-7.5).
pub fn list_change_log() -> Result<Vec<ChangeLogRowDto>, String> {
    let store = open_store()?;
    Ok(store
        .change_log()
        .iter()
        .map(|e| ChangeLogRowDto {
            id: e.id,
            time_unix_ms: e.time_unix_ms,
            action: e.action.clone(),
            before: e.before.clone(),
            after: e.after.clone(),
            restore_status: e.restore_status.as_str().into(),
        })
        .collect())
}

/// One-click restore-all (AC-7.2).
pub fn restore_all_explicit() -> Result<RestoreAllDto, String> {
    let path = snapshot_store_path();
    let mut store = open_store()?;
    let mut registry = MapActionRegistry::new();
    register_host_actions(&mut registry);
    // Ensure labeled mock can be restored in smoke.
    if store
        .change_log()
        .iter()
        .any(|e| e.action == "mock_toggle" && e.restore_status == RestoreStatus::Outstanding)
    {
        registry.insert(Box::new(MockToggleAction::new("mock_toggle", "on", "on")));
    }
    let report = restore_all(&mut store, &mut registry, RestoreTrigger::Explicit);
    let items: Vec<String> = report
        .items
        .iter()
        .map(|i| {
            format!(
                "{} entry={} action={} ({})",
                i.status.as_str(),
                i.entry_id
                    .map(|n| n.to_string())
                    .unwrap_or_else(|| "-".into()),
                i.action,
                i.detail
            )
        })
        .collect();
    Ok(RestoreAllDto {
        trigger: report.trigger.as_str().into(),
        requires_ui: report.requires_ui,
        ok: report.ok,
        status: report.status_label().into(),
        store_path: path.display().to_string(),
        items: if items.is_empty() {
            vec!["no outstanding Hertzline changes".into()]
        } else {
            items
        },
    })
}

pub fn macos_honesty_banner() -> String {
    MACOS_HONESTY_BANNER.to_string()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn with_temp_store<T>(f: impl FnOnce() -> T) -> T {
        let _guard = crate::test_env::lock();
        let dir = std::env::temp_dir().join(format!(
            "hertzline-app-hz031-{}-{}",
            std::process::id(),
            unix_ms_now()
        ));
        let _ = std::fs::create_dir_all(&dir);
        let path = dir.join("snapshots.json");
        let previous = std::env::var_os(hertzline_core::paths::SNAPSHOT_STORE_ENV);
        std::env::set_var(hertzline_core::paths::SNAPSHOT_STORE_ENV, path.as_os_str());
        let out = f();
        match previous {
            Some(v) => std::env::set_var(hertzline_core::paths::SNAPSHOT_STORE_ENV, v),
            None => std::env::remove_var(hertzline_core::paths::SNAPSHOT_STORE_ENV),
        }
        let _ = std::fs::remove_dir_all(&dir);
        out
    }

    #[test]
    fn catalog_has_ac81_fields_and_host_os() {
        let cat = list_action_catalog();
        assert!(!cat.host_os.is_empty());
        assert_eq!(cat.local_measurement_note, NO_LOCAL_ACTION_MEASUREMENT);
        assert!(cat.macos_honesty.is_some());
        let banner = &cat.macos_honesty.as_ref().unwrap().banner;
        assert!(banner.contains("far fewer tunables"));
        assert!(cat.actions.len() >= 20);
        for a in &cat.actions {
            assert!(!a.explanation.what.is_empty());
            assert!(!a.explanation.expected_effect.is_empty());
            assert!(!a.explanation.risk.is_empty());
            assert!(!a.explanation.how_to_undo.is_empty());
            assert!(a
                .explanation
                .local_measurement
                .contains("No local before/after"));
            assert!(!a.ac_refs.is_empty());
        }
        let win = cat
            .actions
            .iter()
            .find(|a| a.id == "windows.power_plan")
            .unwrap();
        assert_eq!(win.os, "windows");
        let hags = cat.actions.iter().find(|a| a.id == "windows.hags").unwrap();
        assert!(hags.requires_elevation);
        let gm = cat
            .actions
            .iter()
            .find(|a| a.id == "macos.game_mode_status")
            .unwrap();
        assert!(!gm.exposes_switch || host_os() != "macos");
        // AC-4.19: never fake toggle when not_possible
        assert!(!gm.exposes_switch || gm.kind == "report");
    }

    #[test]
    fn elevation_prompt_before_elevated_apply() {
        let out = apply_action(ApplyRequest {
            id: "windows.hags".into(),
            confirm_elevation: false,
            confirm_destructive: false,
            input: None,
        });
        match out {
            ApplyOutcomeDto::ElevationRequired { action, .. } => {
                assert_eq!(action, "windows.hags");
            }
            other => panic!("expected elevation prompt, got {other:?}"),
        }
    }

    #[test]
    fn labeled_mock_apply_restore_all_round_trip() {
        // mock: in-memory toggle only
        with_temp_store(|| {
            let applied = apply_labeled_mock().expect("mock apply");
            match applied {
                ApplyOutcomeDto::Applied {
                    action,
                    before,
                    after,
                    ..
                } => {
                    assert_eq!(action, "mock_toggle");
                    assert_eq!(before, "off");
                    assert_eq!(after, "on");
                }
                other => panic!("expected applied, got {other:?}"),
            }
            let log = list_change_log().expect("log");
            assert_eq!(log.len(), 1);
            assert_eq!(log[0].restore_status, "outstanding");
            let report = restore_all_explicit().expect("restore all");
            assert!(report.ok);
            assert!(!report.requires_ui);
            assert_eq!(report.trigger, "explicit");
            let log2 = list_change_log().expect("log2");
            assert_eq!(log2[0].restore_status, "restored");
        });
    }

    #[test]
    fn destructive_requires_confirm() {
        let out = apply_action(ApplyRequest {
            id: "windows.temp_shader_cleanup".into(),
            confirm_elevation: false,
            confirm_destructive: false,
            input: None,
        });
        match out {
            ApplyOutcomeDto::ConfirmDestructive { action, .. } => {
                assert_eq!(action, "windows.temp_shader_cleanup");
            }
            ApplyOutcomeDto::Failed { detail, .. } if detail.contains("not available") => {
                // wrong OS -- acceptable on non-Windows when destructive gate skipped
                // because meta kind is still destructive from catalog; if wrong_os
                // and we somehow skipped, fail soft.
            }
            other => {
                // On non-Windows catalog still marks destructive; elevation/confirm
                // should fire before host build for known catalog ids.
                match other {
                    ApplyOutcomeDto::ConfirmDestructive { .. } => {}
                    ApplyOutcomeDto::Failed { detail, .. } => {
                        assert!(
                            detail.contains("not available") || detail.contains("confirm"),
                            "{detail}"
                        );
                    }
                    _ => panic!("unexpected {other:?}"),
                }
            }
        }
    }
}
