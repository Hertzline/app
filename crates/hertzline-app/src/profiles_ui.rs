//! Profiles UI host (HZ-032 / AC-3.1--AC-3.4).
//!
//! Detects games from documented launcher paths, accepts manual path adds,
//! saves per-game action sets, and applies/restores via [`ProfileHost`].
//! No undocumented scrape or injection.

use std::fs;
use std::path::{Path, PathBuf};
use std::sync::{Mutex, OnceLock};

use hertzline_core::action::MockToggleAction;
use hertzline_core::games::{detect_games, manual_game, DetectedGame, DetectionRoots, GameSource};
use hertzline_core::paths::hertzline_data_dir;
use hertzline_core::profiles::{game_key, GameProfile};
use hertzline_core::restore::MapActionRegistry;
use serde::{Deserialize, Serialize};

use crate::actions_ui::register_host_actions;
use crate::profile_host::{ProfileHost, ProfileHostError};

/// Env override for the profiles library JSON (tests / smoke).
pub const PROFILES_LIBRARY_ENV: &str = "HERTZLINE_PROFILES_LIBRARY";

const PROFILES_LIBRARY_FILE: &str = "profiles.json";
const LIBRARY_SCHEMA_VERSION: u32 = 1;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct DetectedGameDto {
    pub source: String,
    pub id: String,
    pub name: String,
    pub install_path: String,
    pub game_key: String,
}

impl From<&DetectedGame> for DetectedGameDto {
    fn from(game: &DetectedGame) -> Self {
        Self {
            source: game.source.as_str().into(),
            id: game.id.clone(),
            name: game.name.clone(),
            install_path: game.install_path.display().to_string(),
            game_key: game_key(game.source, &game.id),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ProfileDto {
    pub game_key: String,
    pub name: String,
    pub action_ids: Vec<String>,
}

impl From<&GameProfile> for ProfileDto {
    fn from(p: &GameProfile) -> Self {
        Self {
            game_key: p.game_key.clone(),
            name: p.name.clone(),
            action_ids: p.action_ids.clone(),
        }
    }
}

impl From<&ProfileDto> for GameProfile {
    fn from(p: &ProfileDto) -> Self {
        GameProfile {
            game_key: p.game_key.clone(),
            name: p.name.clone(),
            action_ids: p.action_ids.clone(),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ManualPathDto {
    pub name: String,
    pub install_path: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ActiveSessionDto {
    pub active: bool,
    pub game_key: Option<String>,
    pub name: Option<String>,
    pub action_ids: Vec<String>,
    pub phase: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ProfilesOverviewDto {
    pub games: Vec<DetectedGameDto>,
    pub profiles: Vec<ProfileDto>,
    pub manual_paths: Vec<ManualPathDto>,
    pub active: ActiveSessionDto,
    pub note: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AddManualPathRequest {
    pub name: String,
    pub install_path: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SaveProfileRequest {
    pub game_key: String,
    pub name: String,
    pub action_ids: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ApplyProfileRequest {
    pub game_key: String,
    /// When true, use labeled `mock_toggle` only (tests / smoke).
    pub labeled_mock: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "status", rename_all = "snake_case")]
pub enum ProfileApplyOutcomeDto {
    Applied {
        game_key: String,
        name: String,
        action_ids: Vec<String>,
    },
    Restored {
        game_key: String,
        name: String,
    },
    Failed {
        detail: String,
    },
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
struct ProfilesLibraryFile {
    schema_version: u32,
    manual_paths: Vec<ManualPathDto>,
    profiles: Vec<ProfileDto>,
}

impl Default for ProfilesLibraryFile {
    fn default() -> Self {
        Self {
            schema_version: LIBRARY_SCHEMA_VERSION,
            manual_paths: Vec::new(),
            profiles: Vec::new(),
        }
    }
}

fn library_path() -> PathBuf {
    if let Ok(custom) = std::env::var(PROFILES_LIBRARY_ENV) {
        let trimmed = custom.trim();
        if !trimmed.is_empty() {
            return PathBuf::from(trimmed);
        }
    }
    hertzline_data_dir().join(PROFILES_LIBRARY_FILE)
}

fn load_library(path: &Path) -> Result<ProfilesLibraryFile, String> {
    if !path.exists() {
        return Ok(ProfilesLibraryFile::default());
    }
    let text = fs::read_to_string(path).map_err(|e| e.to_string())?;
    let file: ProfilesLibraryFile = serde_json::from_str(&text).map_err(|e| e.to_string())?;
    if file.schema_version != LIBRARY_SCHEMA_VERSION {
        return Err(format!(
            "unsupported profiles library schema {}",
            file.schema_version
        ));
    }
    Ok(file)
}

fn save_library(path: &Path, file: &ProfilesLibraryFile) -> Result<(), String> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).map_err(|e| e.to_string())?;
    }
    let json = serde_json::to_string_pretty(file).map_err(|e| e.to_string())?;
    fs::write(path, json).map_err(|e| e.to_string())
}

fn host_lock() -> &'static Mutex<ProfileHost> {
    static HOST: OnceLock<Mutex<ProfileHost>> = OnceLock::new();
    HOST.get_or_init(|| Mutex::new(ProfileHost::with_default_paths()))
}

fn active_dto(host: &ProfileHost) -> ActiveSessionDto {
    match host.session() {
        Some(session) => ActiveSessionDto {
            active: host.is_active(),
            game_key: Some(session.profile().game_key.clone()),
            name: Some(session.profile().name.clone()),
            action_ids: session.profile().action_ids.clone(),
            phase: Some(match session.phase() {
                hertzline_core::profiles::ProfilePhase::Active => "active".into(),
                hertzline_core::profiles::ProfilePhase::Restored => "restored".into(),
            }),
        },
        None => ActiveSessionDto {
            active: false,
            game_key: None,
            name: None,
            action_ids: Vec::new(),
            phase: None,
        },
    }
}

fn detection_roots_with_manual(manual: &[ManualPathDto]) -> DetectionRoots {
    let mut roots = DetectionRoots::from_os_defaults();
    for entry in manual {
        roots
            .manual
            .push((entry.name.clone(), PathBuf::from(&entry.install_path)));
    }
    roots
}

fn list_games(manual: &[ManualPathDto]) -> Vec<DetectedGameDto> {
    detect_games(&detection_roots_with_manual(manual))
        .iter()
        .map(DetectedGameDto::from)
        .collect()
}

/// Overview for the Profiles route (AC-3.1, AC-3.2).
pub fn profiles_overview() -> Result<ProfilesOverviewDto, String> {
    let path = library_path();
    let library = load_library(&path)?;
    let games = list_games(&library.manual_paths);
    let host = host_lock()
        .lock()
        .map_err(|_| "profile host lock poisoned".to_string())?;
    Ok(ProfilesOverviewDto {
        games,
        profiles: library.profiles,
        manual_paths: library.manual_paths,
        active: active_dto(&host),
        note: "Games are detected from documented launcher paths only. Manual paths are user-supplied. Undocumented scrape and injection are never used.".into(),
    })
}

/// Add a manual install path (AC-3.1).
pub fn add_manual_path(request: AddManualPathRequest) -> Result<ProfilesOverviewDto, String> {
    let name = request.name.trim().to_string();
    let install_path = request.install_path.trim().to_string();
    if name.is_empty() {
        return Err("manual game name is empty".into());
    }
    if install_path.is_empty() {
        return Err("manual install path is empty".into());
    }
    let path = PathBuf::from(&install_path);
    if !path.exists() {
        return Err(format!("path does not exist: {install_path}"));
    }

    let library_file = library_path();
    let mut library = load_library(&library_file)?;
    let game = manual_game(&name, &path);
    let key = game_key(GameSource::Manual, &game.id);
    if library
        .manual_paths
        .iter()
        .any(|m| m.install_path == install_path)
    {
        return Err(format!("manual path already added: {install_path}"));
    }
    library.manual_paths.push(ManualPathDto {
        name: name.clone(),
        install_path: install_path.clone(),
    });
    if !library.profiles.iter().any(|p| p.game_key == key) {
        library.profiles.push(ProfileDto {
            game_key: key,
            name,
            action_ids: Vec::new(),
        });
    }
    save_library(&library_file, &library)?;
    profiles_overview()
}

/// Save or replace a profile's action set.
pub fn save_profile(request: SaveProfileRequest) -> Result<ProfileDto, String> {
    let game_key = request.game_key.trim().to_string();
    let name = request.name.trim().to_string();
    if game_key.is_empty() {
        return Err("game_key is empty".into());
    }
    if name.is_empty() {
        return Err("profile name is empty".into());
    }
    let action_ids: Vec<String> = request
        .action_ids
        .into_iter()
        .map(|id| id.trim().to_string())
        .filter(|id| !id.is_empty())
        .collect();

    let library_file = library_path();
    let mut library = load_library(&library_file)?;
    let dto = ProfileDto {
        game_key: game_key.clone(),
        name,
        action_ids,
    };
    if let Some(existing) = library.profiles.iter_mut().find(|p| p.game_key == game_key) {
        *existing = dto.clone();
    } else {
        library.profiles.push(dto.clone());
    }
    save_library(&library_file, &library)?;
    Ok(dto)
}

fn build_registry(labeled_mock: bool, action_ids: &[String]) -> Result<MapActionRegistry, String> {
    let mut registry = MapActionRegistry::new();
    if labeled_mock {
        registry.insert(Box::new(MockToggleAction::new("mock_toggle", "off", "on")));
        return Ok(registry);
    }
    register_host_actions(&mut registry);
    if action_ids.iter().any(|id| id == "mock_toggle") {
        registry.insert(Box::new(MockToggleAction::new("mock_toggle", "off", "on")));
    }
    for id in action_ids {
        if registry.get(id).is_none() {
            return Err(format!("action `{id}` is not available on this host"));
        }
    }
    Ok(registry)
}

/// Apply a saved profile when the game starts (AC-3.3). Visible apply path.
pub fn apply_profile(request: ApplyProfileRequest) -> Result<ProfileApplyOutcomeDto, String> {
    let game_key = request.game_key.trim().to_string();
    if game_key.is_empty() {
        return Ok(ProfileApplyOutcomeDto::Failed {
            detail: "game_key is empty".into(),
        });
    }

    let library = load_library(&library_path())?;
    let profile = library
        .profiles
        .iter()
        .find(|p| p.game_key == game_key)
        .cloned()
        .ok_or_else(|| format!("no saved profile for {game_key}"))?;

    let action_ids = if request.labeled_mock {
        vec!["mock_toggle".into()]
    } else if profile.action_ids.is_empty() {
        return Ok(ProfileApplyOutcomeDto::Failed {
            detail: "profile has no actions; save at least one action id first".into(),
        });
    } else {
        profile.action_ids.clone()
    };

    let game_profile = GameProfile {
        game_key: profile.game_key.clone(),
        name: profile.name.clone(),
        action_ids: action_ids.clone(),
    };

    let mut registry = build_registry(request.labeled_mock, &action_ids)?;
    let mut host = host_lock()
        .lock()
        .map_err(|_| "profile host lock poisoned".to_string())?;
    match host.apply_on_start(game_profile, &mut registry) {
        Ok(session) => Ok(ProfileApplyOutcomeDto::Applied {
            game_key: session.profile().game_key.clone(),
            name: session.profile().name.clone(),
            action_ids: session.profile().action_ids.clone(),
        }),
        Err(ProfileHostError::AlreadyActive) => Ok(ProfileApplyOutcomeDto::Failed {
            detail: "a profile session is already active; restore it first".into(),
        }),
        Err(err) => Ok(ProfileApplyOutcomeDto::Failed {
            detail: err.to_string(),
        }),
    }
}

/// Restore every setting applied by the active profile (AC-3.3). Visible restore path.
pub fn restore_active_profile(labeled_mock: bool) -> Result<ProfileApplyOutcomeDto, String> {
    let mut host = host_lock()
        .lock()
        .map_err(|_| "profile host lock poisoned".to_string())?;
    let Some(session) = host.session() else {
        return Ok(ProfileApplyOutcomeDto::Failed {
            detail: "no active profile session".into(),
        });
    };
    let game_key = session.profile().game_key.clone();
    let name = session.profile().name.clone();
    let action_ids = session.profile().action_ids.clone();

    let mut registry = if labeled_mock || action_ids.iter().any(|id| id == "mock_toggle") {
        let mut registry = MapActionRegistry::new();
        // After apply, mock state is "on"; restore reads before from snapshot.
        registry.insert(Box::new(MockToggleAction::new("mock_toggle", "on", "on")));
        if !labeled_mock {
            register_host_actions(&mut registry);
        }
        registry
    } else {
        build_registry(false, &action_ids)?
    };

    match host.restore_on_exit(&mut registry) {
        Ok(()) => Ok(ProfileApplyOutcomeDto::Restored { game_key, name }),
        Err(err) => Ok(ProfileApplyOutcomeDto::Failed {
            detail: err.to_string(),
        }),
    }
}

/// Crash recovery on shell start (AC-3.4); returns whether a marker was cleared.
pub fn recover_profiles_after_crash() -> Result<bool, String> {
    let mut registry = MapActionRegistry::new();
    register_host_actions(&mut registry);
    registry.insert(Box::new(MockToggleAction::new("mock_toggle", "on", "on")));
    let mut host = host_lock()
        .lock()
        .map_err(|_| "profile host lock poisoned".to_string())?;
    let report = host
        .recover_from_prior_crash(&mut registry)
        .map_err(|e| e.to_string())?;
    Ok(report.is_some())
}

/// Labeled mock round-trip for smoke / unit tests (not a real OS tweak).
pub fn apply_restore_labeled_mock(
) -> Result<(ProfileApplyOutcomeDto, ProfileApplyOutcomeDto), String> {
    #[cfg(test)]
    let _guard = crate::test_env::lock();
    apply_restore_labeled_mock_unlocked()
}

fn apply_restore_labeled_mock_unlocked(
) -> Result<(ProfileApplyOutcomeDto, ProfileApplyOutcomeDto), String> {
    let dir = std::env::temp_dir().join(format!(
        "hertzline-app-hz032-{}-{}",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_millis())
            .unwrap_or(0)
    ));
    fs::create_dir_all(&dir).map_err(|e| e.to_string())?;
    let library = dir.join("profiles.json");
    let store = dir.join("snapshots.json");
    let marker = dir.join("active.json");

    let prev_lib = std::env::var_os(PROFILES_LIBRARY_ENV);
    let prev_store = std::env::var_os(hertzline_core::paths::SNAPSHOT_STORE_ENV);
    let prev_marker = std::env::var_os(hertzline_core::paths::ACTIVE_PROFILE_ENV);
    std::env::set_var(PROFILES_LIBRARY_ENV, library.as_os_str());
    std::env::set_var(hertzline_core::paths::SNAPSHOT_STORE_ENV, store.as_os_str());
    std::env::set_var(
        hertzline_core::paths::ACTIVE_PROFILE_ENV,
        marker.as_os_str(),
    );

    {
        let mut host = host_lock()
            .lock()
            .map_err(|_| "profile host lock poisoned".to_string())?;
        *host = ProfileHost::with_paths(&store, &marker);
    }

    let mut file = ProfilesLibraryFile::default();
    file.profiles.push(ProfileDto {
        game_key: "manual:hz032-mock".into(),
        name: "HZ-032 labeled mock".into(),
        action_ids: vec!["mock_toggle".into()],
    });
    let save_result = save_library(&library, &file);
    let outcome = save_result.and_then(|_| {
        let applied = apply_profile(ApplyProfileRequest {
            game_key: "manual:hz032-mock".into(),
            labeled_mock: true,
        })?;
        let restored = restore_active_profile(true)?;
        Ok((applied, restored))
    });

    match prev_lib {
        Some(v) => std::env::set_var(PROFILES_LIBRARY_ENV, v),
        None => std::env::remove_var(PROFILES_LIBRARY_ENV),
    }
    match prev_store {
        Some(v) => std::env::set_var(hertzline_core::paths::SNAPSHOT_STORE_ENV, v),
        None => std::env::remove_var(hertzline_core::paths::SNAPSHOT_STORE_ENV),
    }
    match prev_marker {
        Some(v) => std::env::set_var(hertzline_core::paths::ACTIVE_PROFILE_ENV, v),
        None => std::env::remove_var(hertzline_core::paths::ACTIVE_PROFILE_ENV),
    }
    {
        let mut host = host_lock()
            .lock()
            .map_err(|_| "profile host lock poisoned".to_string())?;
        *host = ProfileHost::with_default_paths();
    }
    let _ = fs::remove_dir_all(&dir);
    outcome
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::atomic::{AtomicU64, Ordering};

    fn temp_dir(label: &str) -> PathBuf {
        static N: AtomicU64 = AtomicU64::new(0);
        let n = N.fetch_add(1, Ordering::Relaxed);
        let dir = std::env::temp_dir().join(format!("hertzline-app-hz032-{label}-{n}"));
        let _ = fs::remove_dir_all(&dir);
        fs::create_dir_all(&dir).expect("temp dir");
        dir
    }

    fn with_isolated_paths<T>(dir: &Path, f: impl FnOnce() -> T) -> T {
        let _guard = crate::test_env::lock();
        let library = dir.join("profiles.json");
        let store = dir.join("snapshots.json");
        let marker = dir.join("active.json");
        let prev_lib = std::env::var_os(PROFILES_LIBRARY_ENV);
        let prev_store = std::env::var_os(hertzline_core::paths::SNAPSHOT_STORE_ENV);
        let prev_marker = std::env::var_os(hertzline_core::paths::ACTIVE_PROFILE_ENV);
        std::env::set_var(PROFILES_LIBRARY_ENV, library.as_os_str());
        std::env::set_var(hertzline_core::paths::SNAPSHOT_STORE_ENV, store.as_os_str());
        std::env::set_var(
            hertzline_core::paths::ACTIVE_PROFILE_ENV,
            marker.as_os_str(),
        );
        {
            let mut host = host_lock().lock().unwrap();
            *host = ProfileHost::with_paths(&store, &marker);
        }
        let out = f();
        match prev_lib {
            Some(v) => std::env::set_var(PROFILES_LIBRARY_ENV, v),
            None => std::env::remove_var(PROFILES_LIBRARY_ENV),
        }
        match prev_store {
            Some(v) => std::env::set_var(hertzline_core::paths::SNAPSHOT_STORE_ENV, v),
            None => std::env::remove_var(hertzline_core::paths::SNAPSHOT_STORE_ENV),
        }
        match prev_marker {
            Some(v) => std::env::set_var(hertzline_core::paths::ACTIVE_PROFILE_ENV, v),
            None => std::env::remove_var(hertzline_core::paths::ACTIVE_PROFILE_ENV),
        }
        {
            let mut host = host_lock().lock().unwrap();
            *host = ProfileHost::with_default_paths();
        }
        out
    }

    #[test]
    fn overview_lists_manual_path_and_note() {
        // mock: no live launcher roots required; manual path only
        let dir = temp_dir("overview");
        let game_dir = dir.join("MyGame");
        fs::create_dir_all(&game_dir).unwrap();
        with_isolated_paths(&dir, || {
            let overview = add_manual_path(AddManualPathRequest {
                name: "My Game".into(),
                install_path: game_dir.display().to_string(),
            })
            .expect("add manual");
            assert!(overview
                .games
                .iter()
                .any(|g| g.source == "manual" && g.name == "My Game"));
            assert!(!overview.manual_paths.is_empty());
            assert!(overview.note.contains("documented launcher paths"));
            assert!(!overview.active.active);
        });
        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn save_profile_then_apply_and_restore_labeled_mock() {
        // mock: MockToggleAction via labeled_mock; not a real OS tweak
        let dir = temp_dir("apply");
        with_isolated_paths(&dir, || {
            let saved = save_profile(SaveProfileRequest {
                game_key: "manual:fixture".into(),
                name: "Fixture".into(),
                action_ids: vec!["mock_toggle".into()],
            })
            .expect("save");
            assert_eq!(saved.action_ids, vec!["mock_toggle"]);

            let applied = apply_profile(ApplyProfileRequest {
                game_key: "manual:fixture".into(),
                labeled_mock: true,
            })
            .expect("apply");
            match applied {
                ProfileApplyOutcomeDto::Applied {
                    game_key,
                    action_ids,
                    ..
                } => {
                    assert_eq!(game_key, "manual:fixture");
                    assert_eq!(action_ids, vec!["mock_toggle"]);
                }
                other => panic!("expected applied, got {other:?}"),
            }

            let overview = profiles_overview().expect("overview");
            assert!(overview.active.active);
            assert_eq!(overview.active.phase.as_deref(), Some("active"));

            let restored = restore_active_profile(true).expect("restore");
            match restored {
                ProfileApplyOutcomeDto::Restored { game_key, name } => {
                    assert_eq!(game_key, "manual:fixture");
                    assert_eq!(name, "Fixture");
                }
                other => panic!("expected restored, got {other:?}"),
            }
            assert!(!profiles_overview().unwrap().active.active);
        });
        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn reject_empty_manual_path() {
        let dir = temp_dir("reject");
        with_isolated_paths(&dir, || {
            let err = add_manual_path(AddManualPathRequest {
                name: "x".into(),
                install_path: "".into(),
            })
            .expect_err("empty path");
            assert!(err.contains("empty"));
        });
        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn labeled_mock_round_trip_helper() {
        let (applied, restored) = apply_restore_labeled_mock().expect("round trip");
        assert!(matches!(applied, ProfileApplyOutcomeDto::Applied { .. }));
        assert!(matches!(restored, ProfileApplyOutcomeDto::Restored { .. }));
    }
}
