//! Built-in desk modes UI host (Study / Programming / Performance).

use hertzline_core::everyday::{read_power_source, DeskMode, PowerSource};
use hertzline_core::paths::snapshot_store_path;
use serde::{Deserialize, Serialize};
use std::fs;
use std::path::PathBuf;

use crate::everyday_ui::{apply_power_preset, ApplyPresetRequest, PresetOutcomeDto};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PowerSourceDto {
    pub status: String,
    pub label: String,
    pub on_ac: Option<bool>,
    pub note: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct DeskModeDto {
    pub id: String,
    pub title: String,
    pub blurb: String,
    pub fans_note: String,
    pub adapts_to_power: bool,
    pub resolved_preset: String,
    pub active: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ModesOverviewDto {
    pub power_source: PowerSourceDto,
    pub modes: Vec<DeskModeDto>,
    pub active_mode_id: Option<String>,
    pub note: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ApplyModeRequest {
    pub id: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "status", rename_all = "snake_case")]
pub enum ModeApplyOutcomeDto {
    Applied {
        mode_id: String,
        preset: String,
        power_source: String,
        before: String,
        after: String,
        entry_id: u64,
    },
    Failed {
        detail: String,
    },
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Default)]
struct ModesStateFile {
    active_mode_id: Option<String>,
}

fn modes_state_path() -> PathBuf {
    snapshot_store_path()
        .parent()
        .map(|p| p.join("desk_modes.json"))
        .unwrap_or_else(|| PathBuf::from("desk_modes.json"))
}

fn load_state() -> ModesStateFile {
    let path = modes_state_path();
    let Ok(raw) = fs::read_to_string(&path) else {
        return ModesStateFile::default();
    };
    serde_json::from_str(&raw).unwrap_or_default()
}

fn save_state(state: &ModesStateFile) -> Result<(), String> {
    let path = modes_state_path();
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).map_err(|e| e.to_string())?;
    }
    let raw = serde_json::to_string_pretty(state).map_err(|e| e.to_string())?;
    fs::write(path, raw).map_err(|e| e.to_string())
}

fn map_power_source(source: PowerSource) -> PowerSourceDto {
    let (on_ac, note) = match source {
        PowerSource::PluggedIn => (
            Some(true),
            "AC adapter detected. Adaptive modes use the plugged-in plan.".into(),
        ),
        PowerSource::OnBattery => (
            Some(false),
            "On battery. Adaptive modes pick a quieter / saver plan.".into(),
        ),
        PowerSource::Unknown => (
            None,
            "Power source could not be read on this OS; modes use a safe default.".into(),
        ),
    };
    PowerSourceDto {
        status: source.as_str().into(),
        label: source.label().into(),
        on_ac,
        note,
    }
}

fn map_mode(mode: DeskMode, source: PowerSource, active_id: Option<&str>) -> DeskModeDto {
    let preset = mode.resolve(source);
    DeskModeDto {
        id: mode.id().into(),
        title: mode.title().into(),
        blurb: mode.blurb().into(),
        fans_note: mode.fans_note().into(),
        adapts_to_power: mode.adapts_to_power(),
        resolved_preset: preset.as_str().into(),
        active: active_id == Some(mode.id()),
    }
}

pub fn modes_overview() -> ModesOverviewDto {
    let source = read_power_source();
    let state = load_state();
    let active = state.active_mode_id.clone();
    let modes = DeskMode::ALL
        .into_iter()
        .map(|m| map_mode(m, source, active.as_deref()))
        .collect();
    ModesOverviewDto {
        power_source: map_power_source(source),
        modes,
        active_mode_id: active,
        note: "Built-in modes set the OS power plan only. Hertzline never edits fan curves."
            .into(),
    }
}

pub fn apply_desk_mode(request: ApplyModeRequest) -> ModeApplyOutcomeDto {
    let Some(mode) = DeskMode::parse(&request.id) else {
        return ModeApplyOutcomeDto::Failed {
            detail: format!("unknown mode: {}", request.id),
        };
    };
    let source = read_power_source();
    let preset = mode.resolve(source);
    let outcome = apply_power_preset(ApplyPresetRequest {
        preset: preset.as_str().into(),
    });
    match outcome {
        PresetOutcomeDto::Applied {
            entry_id,
            before,
            after,
        } => {
            let _ = save_state(&ModesStateFile {
                active_mode_id: Some(mode.id().into()),
            });
            ModeApplyOutcomeDto::Applied {
                mode_id: mode.id().into(),
                preset: preset.as_str().into(),
                power_source: source.as_str().into(),
                before,
                after,
                entry_id,
            }
        }
        PresetOutcomeDto::Failed { detail } => ModeApplyOutcomeDto::Failed { detail },
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use hertzline_core::everyday::{
        EverydayPreset, MODE_PERFORMANCE, MODE_PROGRAMMING, MODE_STUDY,
    };

    #[test]
    fn overview_lists_three_modes() {
        let overview = modes_overview();
        assert_eq!(overview.modes.len(), 3);
        assert_eq!(overview.modes[0].id, MODE_STUDY);
        assert_eq!(overview.modes[1].id, MODE_PROGRAMMING);
        assert_eq!(overview.modes[2].id, MODE_PERFORMANCE);
        assert!(!overview.modes[0].resolved_preset.is_empty());
    }

    #[test]
    fn unknown_mode_fails() {
        let outcome = apply_desk_mode(ApplyModeRequest {
            id: "nope".into(),
        });
        assert!(matches!(outcome, ModeApplyOutcomeDto::Failed { .. }));
    }

    #[test]
    fn study_resolves_to_battery_saver() {
        assert_eq!(
            DeskMode::Study.resolve(PowerSource::PluggedIn),
            EverydayPreset::BatterySaver
        );
    }
}
