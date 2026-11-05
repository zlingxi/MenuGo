//! Shared data structures exchanged between the Rust backend and the webview.

use serde::{Deserialize, Serialize};

/// Persisted user preferences (language, theme and the two toggle switches).
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Settings {
    /// "auto" or one of the language ids exposed by the frontend.
    pub language: String,
    /// "light" (default), "dark" or "system".
    pub theme: String,
    /// Apply every change to all loaded user hives instead of HKCU only.
    pub apply_to_all_users: bool,
    /// Ask to restart explorer automatically after a style switch.
    pub auto_restart_explorer: bool,
}

impl Default for Settings {
    fn default() -> Self {
        Self {
            language: "auto".into(),
            theme: "light".into(),
            apply_to_all_users: false,
            auto_restart_explorer: true,
        }
    }
}

/// Snapshot returned to the UI. Keeping it as one object means the frontend
/// only needs a single `get_status` round trip per refresh.
#[derive(Debug, Clone, Serialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct AppStatus {
    /// True when the classic (Windows 10 style) menu is enabled.
    pub classic: bool,
    /// True when running on Windows 11 (build >= 22000).
    pub windows11: bool,
    pub os_build: u32,
    pub os_name: String,
    /// True when the process runs with an elevated token.
    pub elevated: bool,
    pub taskbar_top: bool,
    pub taskbar_auto_hide: bool,
    /// A registry change is waiting for an explorer restart.
    pub needs_restart: bool,
}
