//! Cleanup of MenuGo's own files in the user profile.
//!
//! Exactly two app-owned locations exist outside the executable directory:
//!   %APPDATA%\MenuGo                 - settings.ini (see config.rs)
//!   %LOCALAPPDATA%\com.lingxi.menugo - WebView2 user data (EBWebView cache)
//! Both names are app-exclusive (the WebView folder is keyed by the bundle
//! identifier), and every delete goes through an exact last-component check,
//! so nothing outside these two directories can ever be touched. Files that
//! the running WebView keeps locked survive `clear_now()`; a sweep marker
//! tells the next launch to finish the job before any webview exists again.

use std::io;
use std::path::{Path, PathBuf};
use std::time::Duration;

/// Directory created by config.rs under %APPDATA%.
const SETTINGS_DIR: &str = "MenuGo";
/// WebView2 user data folder, named after the identifier in tauri.conf.json.
const WEBVIEW_DIR: &str = "com.lingxi.menugo";
/// Sibling marker telling the next launch to finish a partial cleanup.
const SWEEP_FLAG: &str = "com.lingxi.menugo.cleanup-pending";

/// Delete both app-owned directories. Returns `true` when nothing of ours is
/// left behind; otherwise a sweep marker is written for the next launch.
pub fn clear_now() -> bool {
    let mut all_gone = true;
    if let Some(dir) = settings_dir() {
        all_gone &= remove_tree(&dir, 1);
    }
    if let Some(dir) = own_dir("LOCALAPPDATA", WEBVIEW_DIR) {
        if dir.exists() {
            all_gone &= remove_tree(&dir, 1);
        }
    }
    if !all_gone {
        let _ = write_sweep_flag();
    }
    all_gone
}

/// Called from `main()` before the Tauri builder runs: no webview exists yet,
/// so files locked last session can be removed. The brief retry absorbs the
/// hand-over window while a previous instance is still shutting down.
pub fn sweep_if_pending() {
    let Some(flag) = sweep_flag_path() else {
        return;
    };
    if !flag.exists() {
        return;
    }
    // Retry BOTH directories: settings.ini can equally have been locked when
    // the marker was written, and a leftover one would silently restore the
    // old settings on this very launch.
    let settings_gone = match settings_dir() {
        Some(dir) if dir.exists() => remove_tree(&dir, 10),
        _ => true,
    };
    let webview_gone = match own_dir("LOCALAPPDATA", WEBVIEW_DIR) {
        Some(dir) if dir.exists() => remove_tree(&dir, 10),
        _ => true,
    };
    // Keep the marker while something is still locked, so a later launch
    // retries instead of silently abandoning the leftovers.
    if settings_gone && webview_gone {
        let _ = std::fs::remove_file(flag);
    }
}

/// Mirror config.rs's resolution: `%APPDATA%\MenuGo`, falling back to the
/// working directory when APPDATA is unset. In portable mode this directory
/// only holds stale files (settings live beside the exe) - still ours to
/// delete.
fn settings_dir() -> Option<PathBuf> {
    let base = std::env::var_os("APPDATA")
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from("."));
    let dir = base.join(SETTINGS_DIR);
    if dir.file_name()?.to_str()? != SETTINGS_DIR {
        return None;
    }
    Some(dir)
}

/// Resolve `%BASE%\<name>` and refuse anything whose last component is not
/// exactly the expected directory name - the guard against ever touching
/// something that is not ours.
fn own_dir(base_env: &str, name: &str) -> Option<PathBuf> {
    let dir = Path::new(&std::env::var_os(base_env)?).join(name);
    if dir.file_name()?.to_str()? != name {
        return None;
    }
    Some(dir)
}

fn remove_tree(dir: &Path, attempts: u32) -> bool {
    for _ in 0..attempts {
        match std::fs::remove_dir_all(dir) {
            Ok(()) => return true,
            Err(e) if e.kind() == io::ErrorKind::NotFound => return true,
            Err(_) => std::thread::sleep(Duration::from_millis(250)),
        }
    }
    false
}

fn sweep_flag_path() -> Option<PathBuf> {
    Some(Path::new(&std::env::var_os("LOCALAPPDATA")?).join(SWEEP_FLAG))
}

fn write_sweep_flag() -> io::Result<()> {
    let path = sweep_flag_path().ok_or_else(|| {
        io::Error::new(io::ErrorKind::NotFound, "LOCALAPPDATA is not set")
    })?;
    std::fs::write(path, b"menugo cleanup pending")
}
