//! MenuGo - a small Tauri 2 utility for Windows 11 context menu styles.
//!
//! Layout of the backend:
//!   cli       - command line switches (/C /D /R /?)
//!   cleanup   - removing MenuGo's own files from the user profile
//!   commands  - IPC commands exposed to the webview
//!   config    - INI style settings persistence
//!   models    - shared DTOs
//!   registry  - every registry read/write
//!   shell     - taskbar-window probe used to verify explorer restarts
//!   system    - explorer restart, shell helpers, elevation check
//!   titlebar  - native (DWM) caption bar theming

pub mod cli;
pub mod cleanup;
mod commands;
mod config;
pub mod models;
mod registry;
mod shell;
mod system;
mod titlebar;

use std::sync::atomic::AtomicBool;
use std::sync::Mutex;
use tauri::Manager;

/// Process-wide state shared by all commands.
pub struct AppState {
    pub settings: Mutex<models::Settings>,
    /// A registry edit is pending until explorer is restarted.
    pub needs_restart: AtomicBool,
    /// True when launched with /? so the frontend opens the info dialog.
    pub show_command_info: bool,
    /// Computed once at startup; `net session` is relatively expensive.
    pub elevated: bool,
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run(show_command_info: bool) {
    tauri::Builder::default()
        .manage(AppState {
            settings: Mutex::new(config::load()),
            needs_restart: AtomicBool::new(false),
            show_command_info,
            elevated: system::is_elevated(),
        })
        .invoke_handler(tauri::generate_handler![
            commands::get_status,
            commands::get_settings,
            commands::set_menu_style,
            commands::restart_explorer,
            commands::set_language,
            commands::set_theme,
            commands::sync_window_theme,
            commands::set_apply_to_all_users,
            commands::set_auto_restart,
            commands::set_taskbar_top,
            commands::set_taskbar_auto_hide,
            commands::clear_cache,
            commands::open_url,
            commands::command_line_info,
            commands::launch_mode,
            commands::app_version,
        ])
        // Paint the native caption bar before the user sees the window, using
        // the persisted theme. The webview re-syncs once it has loaded.
        // The window is created hidden (visible = false in tauri.conf.json):
        // it is placed on the monitor the cursor is on, then shown, so it
        // never flashes centered on the primary screen first.
        .setup(|app| {
            let theme = config::load().theme;
            if let Some(window) = app.get_webview_window("main") {
                let dark = match theme.as_str() {
                    "dark" => true,
                    "system" => !registry::apps_use_light_theme(),
                    _ => false,
                };
                titlebar::apply(&window, dark);
                place_on_cursor_monitor(&window);
                // A failed show() must not abort the whole startup; the same
                // call succeeds on every normal Windows session anyway.
                let _ = window.show();
            }
            Ok(())
        })
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}

/// Center the window on the monitor the mouse cursor is currently on,
/// falling back to the primary monitor when the cursor position cannot be
/// resolved. All coordinates are physical pixels, so no DPI scaling is
/// involved.
fn place_on_cursor_monitor(window: &tauri::WebviewWindow) {
    let Ok(cursor) = window.cursor_position() else {
        return;
    };
    let monitor = window
        .available_monitors()
        .ok()
        .and_then(|monitors| {
            monitors.into_iter().find(|monitor| {
                let (x, y) = (
                    f64::from(monitor.position().x),
                    f64::from(monitor.position().y),
                );
                let (w, h) = (
                    f64::from(monitor.size().width),
                    f64::from(monitor.size().height),
                );
                cursor.x >= x && cursor.x < x + w && cursor.y >= y && cursor.y < y + h
            })
        })
        .or_else(|| window.primary_monitor().ok().flatten());
    let Some(monitor) = monitor else {
        return;
    };
    if let Ok(size) = window.outer_size() {
        let x = monitor.position().x + (monitor.size().width as i32 - size.width as i32) / 2;
        let y = monitor.position().y + (monitor.size().height as i32 - size.height as i32) / 2;
        let _ = window.set_position(tauri::PhysicalPosition::new(x, y));
    }
}
