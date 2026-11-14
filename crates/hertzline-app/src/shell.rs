//! Desktop shell routes (HZ-029).
//!
//! Navigation targets only. Measure/benchmark/action UIs arrive in later tasks.

use serde::{Deserialize, Serialize};

/// Primary shell destinations the unprivileged window must open.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum AppRoute {
    Measure,
    Benchmark,
    Actions,
    Profiles,
    Changelog,
    RestoreAll,
}

impl AppRoute {
    /// Stable order used by nav and smoke checks.
    pub const ALL: [AppRoute; 6] = [
        Self::Measure,
        Self::Benchmark,
        Self::Actions,
        Self::Profiles,
        Self::Changelog,
        Self::RestoreAll,
    ];

    pub const fn id(self) -> &'static str {
        match self {
            Self::Measure => "measure",
            Self::Benchmark => "benchmark",
            Self::Actions => "actions",
            Self::Profiles => "profiles",
            Self::Changelog => "changelog",
            Self::RestoreAll => "restore-all",
        }
    }

    pub const fn title(self) -> &'static str {
        match self {
            Self::Measure => "Measure",
            Self::Benchmark => "Benchmark",
            Self::Actions => "Actions",
            Self::Profiles => "Profiles",
            Self::Changelog => "Change log",
            Self::RestoreAll => "Restore all",
        }
    }

    /// Hash path opened by the webview (`#/measure`, ...).
    pub fn hash_path(self) -> String {
        format!("#/{}", self.id())
    }

    pub fn parse(id: &str) -> Option<Self> {
        Self::ALL.into_iter().find(|route| route.id() == id)
    }
}

/// One nav entry exposed to the UI.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RouteInfo {
    pub id: String,
    pub title: String,
    pub hash_path: String,
}

impl From<AppRoute> for RouteInfo {
    fn from(route: AppRoute) -> Self {
        Self {
            id: route.id().into(),
            title: route.title().into(),
            hash_path: route.hash_path(),
        }
    }
}

/// Unprivileged shell metadata (HZ-029).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ShellInfo {
    pub app_version: String,
    pub core_version: String,
    /// Always true: the desktop process never starts elevated (RULES.md).
    pub unprivileged: bool,
    pub routes: Vec<RouteInfo>,
}

/// Build the shell descriptor the Tauri window loads.
pub fn shell_info(app_version: &str, core_version: &str) -> ShellInfo {
    ShellInfo {
        app_version: app_version.into(),
        core_version: core_version.into(),
        unprivileged: true,
        routes: AppRoute::ALL.into_iter().map(RouteInfo::from).collect(),
    }
}

/// Assert every required route is present (smoke / tests).
pub fn required_route_ids() -> [&'static str; 6] {
    [
        "measure",
        "benchmark",
        "actions",
        "profiles",
        "changelog",
        "restore-all",
    ]
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn all_required_routes_parse() {
        for id in required_route_ids() {
            let route = AppRoute::parse(id).expect(id);
            assert_eq!(route.id(), id);
            assert_eq!(route.hash_path(), format!("#/{id}"));
            assert!(!route.title().is_empty());
        }
        assert_eq!(AppRoute::ALL.len(), 6);
    }

    #[test]
    fn shell_info_is_unprivileged_with_six_routes() {
        let info = shell_info("0.0.0", "0.0.0");
        assert!(info.unprivileged);
        assert_eq!(info.routes.len(), 6);
        let ids: Vec<&str> = info.routes.iter().map(|r| r.id.as_str()).collect();
        assert_eq!(ids, required_route_ids());
    }

    #[test]
    fn unknown_route_is_none() {
        assert!(AppRoute::parse("overlay").is_none());
        assert!(AppRoute::parse("").is_none());
    }
}
