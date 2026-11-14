//! Unprivileged profile lifecycle host (HZ-028 / AC-3.3, AC-3.4).
//!
//! Wraps `hertzline-core::profiles` with default snapshot + marker paths.
//! Crash recovery delegates to HZ-018 `recover_from_crash`.

use std::path::{Path, PathBuf};

use hertzline_core::action::ActionMap;
use hertzline_core::paths::{active_profile_path, snapshot_store_path};
use hertzline_core::profiles::{
    recover_active_profile_after_crash, GameProfile, GameRunning, ProfileError, ProfilePhase,
    ProfileSession,
};
use hertzline_core::restore::RestoreAllReport;
use hertzline_core::snapshot::{unix_ms_now, SnapshotError, SnapshotStore};

/// Errors from the desktop profile host.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ProfileHostError {
    Profile(ProfileError),
    Snapshot(SnapshotError),
    AlreadyActive,
    NoActiveSession,
}

impl std::fmt::Display for ProfileHostError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Profile(err) => write!(f, "profile: {err}"),
            Self::Snapshot(err) => write!(f, "snapshot: {err}"),
            Self::AlreadyActive => write!(f, "a profile session is already active"),
            Self::NoActiveSession => write!(f, "no active profile session"),
        }
    }
}

impl std::error::Error for ProfileHostError {}

impl From<ProfileError> for ProfileHostError {
    fn from(value: ProfileError) -> Self {
        Self::Profile(value)
    }
}

impl From<SnapshotError> for ProfileHostError {
    fn from(value: SnapshotError) -> Self {
        Self::Snapshot(value)
    }
}

/// Owns store/marker paths and at most one active [`ProfileSession`].
#[derive(Debug)]
pub struct ProfileHost {
    store_path: PathBuf,
    marker_path: PathBuf,
    session: Option<ProfileSession>,
}

impl ProfileHost {
    /// Default user-data paths from `hertzline-core::paths`.
    pub fn with_default_paths() -> Self {
        Self {
            store_path: snapshot_store_path(),
            marker_path: active_profile_path(),
            session: None,
        }
    }

    /// Explicit paths (tests / non-default installs).
    pub fn with_paths(store_path: impl Into<PathBuf>, marker_path: impl Into<PathBuf>) -> Self {
        Self {
            store_path: store_path.into(),
            marker_path: marker_path.into(),
            session: None,
        }
    }

    pub fn store_path(&self) -> &Path {
        &self.store_path
    }

    pub fn marker_path(&self) -> &Path {
        &self.marker_path
    }

    pub fn session(&self) -> Option<&ProfileSession> {
        self.session.as_ref()
    }

    pub fn is_active(&self) -> bool {
        matches!(
            self.session.as_ref().map(ProfileSession::phase),
            Some(ProfilePhase::Active)
        )
    }

    /// On app start: if a marker remains from a prior crash, restore via HZ-018 (AC-3.4).
    pub fn recover_from_prior_crash(
        &mut self,
        actions: &mut dyn ActionMap,
    ) -> Result<Option<RestoreAllReport>, ProfileHostError> {
        let report =
            recover_active_profile_after_crash(&self.marker_path, &self.store_path, actions)?;
        self.session = None;
        Ok(report)
    }

    /// Apply the profile when the game starts (AC-3.3).
    pub fn apply_on_start(
        &mut self,
        profile: GameProfile,
        actions: &mut dyn ActionMap,
    ) -> Result<&ProfileSession, ProfileHostError> {
        if self.is_active() {
            return Err(ProfileHostError::AlreadyActive);
        }
        let mut store = SnapshotStore::open(&self.store_path)?;
        let session = ProfileSession::apply_on_start(
            profile,
            &mut store,
            actions,
            unix_ms_now(),
            Some(&self.marker_path),
        )?;
        self.session = Some(session);
        Ok(self.session.as_ref().expect("just set"))
    }

    /// Restore every applied setting on normal game exit (AC-3.3).
    pub fn restore_on_exit(&mut self, actions: &mut dyn ActionMap) -> Result<(), ProfileHostError> {
        let Some(session) = self.session.as_mut() else {
            return Err(ProfileHostError::NoActiveSession);
        };
        if session.phase() != ProfilePhase::Active {
            return Err(ProfileHostError::NoActiveSession);
        }
        let mut store = SnapshotStore::open(&self.store_path)?;
        session.restore_on_exit(&mut store, actions)?;
        self.session = None;
        Ok(())
    }

    /// Poll injected game presence; restore when the process stops (AC-3.3).
    pub fn poll_game_exit(
        &mut self,
        game: &dyn GameRunning,
        actions: &mut dyn ActionMap,
    ) -> Result<bool, ProfileHostError> {
        let Some(session) = self.session.as_mut() else {
            return Ok(false);
        };
        if session.phase() != ProfilePhase::Active {
            return Ok(false);
        }
        let mut store = SnapshotStore::open(&self.store_path)?;
        let restored = session.poll_game_exit(game, &mut store, actions)?;
        if restored {
            self.session = None;
        }
        Ok(restored)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use hertzline_core::action::MockToggleAction;
    use hertzline_core::profiles::MockGameRunning;
    use hertzline_core::restore::{MapActionRegistry, RestoreTrigger};
    use hertzline_core::snapshot::RestoreStatus;
    use std::fs;
    use std::sync::atomic::{AtomicU64, Ordering};

    fn temp_dir(label: &str) -> PathBuf {
        static N: AtomicU64 = AtomicU64::new(0);
        let n = N.fetch_add(1, Ordering::Relaxed);
        let dir = std::env::temp_dir().join(format!("hertzline-app-hz028-{label}-{n}"));
        let _ = fs::remove_dir_all(&dir);
        fs::create_dir_all(&dir).expect("temp dir");
        dir
    }

    #[test]
    fn host_apply_then_restore_on_exit() {
        // mock: MockToggleAction, not a real OS tweak
        let dir = temp_dir("exit");
        let mut host = ProfileHost::with_paths(dir.join("snapshots.json"), dir.join("active.json"));
        let mut registry = MapActionRegistry::new();
        registry.insert(Box::new(MockToggleAction::new("mock_a", "off", "on")));

        let profile = GameProfile {
            game_key: "steam:480".into(),
            name: "Spacewar".into(),
            action_ids: vec!["mock_a".into()],
        };
        host.apply_on_start(profile, &mut registry).expect("apply");
        assert!(host.is_active());
        assert!(host.marker_path().exists());
        assert_eq!(registry.get("mock_a").unwrap().read_state().unwrap(), "on");

        host.restore_on_exit(&mut registry).expect("restore");
        assert!(!host.is_active());
        assert!(!host.marker_path().exists());
        assert_eq!(registry.get("mock_a").unwrap().read_state().unwrap(), "off");

        let store = SnapshotStore::load_from_path(host.store_path()).unwrap();
        assert!(store
            .change_log()
            .iter()
            .all(|e| e.restore_status == RestoreStatus::Restored));
    }

    #[test]
    fn host_poll_exit_restores() {
        // mock: GameRunning flipped by test
        let dir = temp_dir("poll");
        let mut host = ProfileHost::with_paths(dir.join("snapshots.json"), dir.join("active.json"));
        let mut registry = MapActionRegistry::new();
        registry.insert(Box::new(MockToggleAction::new("mock_toggle", "a", "b")));
        let profile = GameProfile {
            game_key: "manual:x".into(),
            name: "Manual".into(),
            action_ids: vec!["mock_toggle".into()],
        };
        host.apply_on_start(profile, &mut registry).expect("apply");
        assert!(!host
            .poll_game_exit(&MockGameRunning { running: true }, &mut registry)
            .unwrap());
        assert!(host
            .poll_game_exit(&MockGameRunning { running: false }, &mut registry)
            .unwrap());
        assert!(!host.is_active());
        assert_eq!(
            registry.get("mock_toggle").unwrap().read_state().unwrap(),
            "a"
        );
    }

    #[test]
    fn host_crash_recovery_uses_hz018() {
        // mock: leave marker + outstanding change as if the app crashed
        let dir = temp_dir("crash");
        let store_path = dir.join("snapshots.json");
        let marker_path = dir.join("active.json");
        {
            let mut host = ProfileHost::with_paths(&store_path, &marker_path);
            let mut registry = MapActionRegistry::new();
            registry.insert(Box::new(MockToggleAction::new("mock_toggle", "off", "on")));
            let profile = GameProfile {
                game_key: "epic:1".into(),
                name: "Crash".into(),
                action_ids: vec!["mock_toggle".into()],
            };
            host.apply_on_start(profile, &mut registry).expect("apply");
            // Drop without restore_on_exit: simulates Hertzline crash.
        }

        let mut host = ProfileHost::with_paths(&store_path, &marker_path);
        let mut recovery = MapActionRegistry::new();
        recovery.insert(Box::new(MockToggleAction::new("mock_toggle", "on", "on")));
        let report = host
            .recover_from_prior_crash(&mut recovery)
            .expect("recover")
            .expect("marker present");
        assert_eq!(report.trigger, RestoreTrigger::Crash);
        assert!(report.ok);
        assert!(!marker_path.exists());
        assert_eq!(
            recovery.get("mock_toggle").unwrap().read_state().unwrap(),
            "off"
        );
    }
}
