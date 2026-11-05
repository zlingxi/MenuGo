//! Tauri IPC surface. Commands are intentionally thin: all Windows-specific
//! work stays in `registry` / `system`, all persistence in `config`.

use crate::cli::COMMAND_LINE_INFO;
use crate::config;
use crate::models::{AppStatus, Settings};
use crate::registry;
use crate::cleanup;
use crate::system;
use crate::AppState;
use std::sync::atomic::Ordering;
use tauri::State;

const DONATE: &str = "https://www.aizzx.top/sponsor";
const BLOG: &str = "https://www.aizzx.top/2930.html";
const GITHUB: &str = "https://github.com/zlingxi/MenuGo";

fn status_from(state: &AppState) -> AppStatus {
    let (build, name) = registry::os_info();
    let taskbar = registry::taskbar_options();
    AppStatus {
        classic: registry::is_classic(),
        windows11: build >= 22000,
        os_build: build,
        os_name: name,
        elevated: state.elevated,
        taskbar_top: taskbar.top,
        taskbar_auto_hide: taskbar.auto_hide,
        needs_restart: state.needs_restart.load(Ordering::Relaxed),
    }
}

#[tauri::command]
pub fn get_status(state: State<'_, AppState>) -> AppStatus {
    status_from(&*state)
}

#[tauri::command]
pub fn get_settings(state: State<'_, AppState>) -> Settings {
    state
        .settings
        .lock()
        .map(|guard| guard.clone())
        .unwrap_or_default()
}

/// Switch the menu style. Honours the "apply to all users" preference.
#[tauri::command]
pub fn set_menu_style(state: State<'_, AppState>, classic: bool) -> Result<AppStatus, String> {
    let apply_all = {
        let guard = state.settings.lock().map_err(|e| e.to_string())?;
        guard.apply_to_all_users
    };
    if apply_all {
        registry::apply_all_users(classic).map_err(|e| e.to_string())?;
    } else {
        registry::set_classic(classic).map_err(|e| e.to_string())?;
    }
    state.needs_restart.store(true, Ordering::Relaxed);
    Ok(status_from(&*state))
}

#[tauri::command]
pub fn restart_explorer(state: State<'_, AppState>) -> Result<AppStatus, String> {
    system::restart_explorer().map_err(|e| e.to_string())?;
    state.needs_restart.store(false, Ordering::Relaxed);
    Ok(status_from(&*state))
}

#[tauri::command]
pub fn set_language(state: State<'_, AppState>, language: String) -> Result<Settings, String> {
    let mut guard = state.settings.lock().map_err(|e| e.to_string())?;
    guard.language = language;
    config::save(&guard).map_err(|e| e.to_string())?;
    Ok(guard.clone())
}

/// Persist the theme preference and repaint the native title bar in one call.
/// `dark` is the *effective* value resolved by the webview, so light / dark /
/// follow-system all stay in sync with the CSS palette.
#[tauri::command]
pub fn set_theme(
    state: State<'_, AppState>,
    window: tauri::WebviewWindow,
    theme: String,
    dark: bool,
) -> Result<Settings, String> {
    // Repaint first: the webview switches its CSS even if persistence fails,
    // so the native bar must never be left behind.
    crate::titlebar::apply(&window, dark);
    let mut guard = state.settings.lock().map_err(|e| e.to_string())?;
    guard.theme = normalize_theme(&theme);
    config::save(&guard).map_err(|e| e.to_string())?;
    Ok(guard.clone())
}

/// Re-apply the title bar without touching settings.ini. Called once at
/// startup and whenever the OS switches light/dark while `system` is selected.
#[tauri::command]
pub fn sync_window_theme(window: tauri::WebviewWindow, dark: bool) {
    crate::titlebar::apply(&window, dark);
}

fn normalize_theme(value: &str) -> String {
    let value = value.trim().to_ascii_lowercase();
    match value.as_str() {
        "dark" | "system" => value,
        _ => "light".into(),
    }
}

#[tauri::command]
pub fn set_apply_to_all_users(
    state: State<'_, AppState>,
    enabled: bool,
) -> Result<AppStatus, String> {
    if enabled {
        let classic = registry::is_classic();
        registry::apply_all_users(classic).map_err(|e| e.to_string())?;
    }
    {
        let mut guard = state.settings.lock().map_err(|e| e.to_string())?;
        guard.apply_to_all_users = enabled;
        config::save(&guard).map_err(|e| e.to_string())?;
    }
    state.needs_restart.store(true, Ordering::Relaxed);
    Ok(status_from(&*state))
}

#[tauri::command]
pub fn set_auto_restart(state: State<'_, AppState>, enabled: bool) -> Result<Settings, String> {
    let mut guard = state.settings.lock().map_err(|e| e.to_string())?;
    guard.auto_restart_explorer = enabled;
    config::save(&guard).map_err(|e| e.to_string())?;
    Ok(guard.clone())
}

#[tauri::command]
pub fn set_taskbar_top(state: State<'_, AppState>, enabled: bool) -> Result<AppStatus, String> {
    registry::set_taskbar_top(enabled).map_err(|e| e.to_string())?;
    state.needs_restart.store(true, Ordering::Relaxed);
    Ok(status_from(&*state))
}

#[tauri::command]
pub fn set_taskbar_auto_hide(
    state: State<'_, AppState>,
    enabled: bool,
) -> Result<AppStatus, String> {
    registry::set_taskbar_auto_hide(enabled).map_err(|e| e.to_string())?;
    state.needs_restart.store(true, Ordering::Relaxed);
    Ok(status_from(&*state))
}

/// Wipe every file MenuGo owns in the user profile: the settings directory
/// and the WebView2 data folder. Locked leftovers are swept on the next
/// launch (see cleanup.rs). In-memory settings reset to defaults too, so a
/// later change cannot recreate settings.ini with the old values.
#[tauri::command]
pub fn clear_cache(state: State<'_, AppState>) -> Result<bool, String> {
    let cleared = cleanup::clear_now();
    let mut guard = state.settings.lock().map_err(|e| e.to_string())?;
    *guard = crate::models::Settings::default();
    Ok(cleared)
}

#[tauri::command]
pub fn open_url(url: String) -> Result<(), String> {
    // Only allow our well-known outbound links, never an arbitrary
    // protocol handler supplied by the webview.
    let allowed = [DONATE, BLOG, GITHUB];
    if !allowed.contains(&url.as_str()) {
        return Err("URL is not in the allow list".into());
    }
    system::open_url(&url).map_err(|e| e.to_string())
}

#[tauri::command]
pub fn command_line_info() -> String {
    COMMAND_LINE_INFO.to_string()
}

#[tauri::command]
pub fn launch_mode(state: State<'_, AppState>) -> String {
    if state.show_command_info {
        "commandInfo".into()
    } else {
        "gui".into()
    }
}

#[tauri::command]
pub fn app_version() -> String {
    env!("CARGO_PKG_VERSION").to_string()
}
