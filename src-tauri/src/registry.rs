//! All Windows registry access lives here.
//!
//! Windows 11 switches between classic and compact context menus through a
//! single CLSID key under `HKCU\Software\Classes`. This module owns that
//! read/write path and adds the "apply to all users" and taskbar options.

use std::io;
// `HKCU` / `HKLM` / `HKU` are re-exported from the crate root, NOT from
// `winreg::enums` (that module only carries the raw HKEY_*/KEY_*/REG_* constants).
// `enum_keys()` became an inherent method on `RegKey` in winreg 0.56, so the
// `EnumKeys` iterator type no longer needs to be imported as a trait.
use winreg::enums::{KEY_ALL_ACCESS, REG_BINARY};
use winreg::{RegKey, RegValue, HKCU, HKLM, HKU};

/// The well-known Windows 11 context menu CLSID.
pub const CLSID: &str = "{86ca1aa0-34aa-4e8b-a509-50c905bae2a2}";

const STUCK_RECTS: &str = "Software\\Microsoft\\Windows\\CurrentVersion\\Explorer\\StuckRects3";
const NT_CURRENT_VERSION: &str = "SOFTWARE\\Microsoft\\Windows NT\\CurrentVersion";
const PERSONALIZE: &str = "Software\\Microsoft\\Windows\\CurrentVersion\\Themes\\Personalize";

/// Byte 8 of StuckRects3\Settings holds the auto-hide flag, byte 12 the edge.
const AUTOHIDE_OFFSET: usize = 8;
const EDGE_OFFSET: usize = 12;
const AUTOHIDE_ON: u8 = 0x03;
const AUTOHIDE_OFF: u8 = 0x02;
const EDGE_TOP: u8 = 0x01;
const EDGE_BOTTOM: u8 = 0x03;

#[derive(Debug, Clone, Copy, Default)]
pub struct TaskbarOptions {
    pub top: bool,
    pub auto_hide: bool,
}

fn clsid_key() -> String {
    format!("Software\\Classes\\CLSID\\{CLSID}")
}

fn user_clsid_key(sid: &str) -> String {
    format!("{sid}\\Software\\Classes\\CLSID\\{CLSID}")
}

fn key_exists(root: &RegKey, path: &str) -> bool {
    root.open_subkey(path).is_ok()
}

/// Classic style == the InprocServer32 subkey exists (with an empty default value).
pub fn is_classic() -> bool {
    key_exists(HKCU, &format!("{}\\InprocServer32", clsid_key()))
}

pub fn set_classic(classic: bool) -> io::Result<()> {
    if classic {
        enable_for(HKCU, &clsid_key())
    } else {
        disable_for(HKCU, &clsid_key())
    }
}

/// Apply the current style to every *loaded* user hive under HKEY_USERS.
/// Returns the number of additional users touched. Requires elevation.
pub fn apply_all_users(classic: bool) -> io::Result<usize> {
    let sids: Vec<String> = HKU
        .enum_keys()
        .filter_map(Result::ok)
        .filter(|s| is_user_sid(s))
        .collect();

    let mut touched = 0usize;
    for sid in &sids {
        // Only hives with an already loaded Classes key are safe to touch.
        if !key_exists(HKU, &format!("{sid}\\Software\\Classes")) {
            continue;
        }
        let path = user_clsid_key(sid);
        if classic {
            enable_for(HKU, &path)?;
        } else {
            disable_for(HKU, &path)?;
        }
        touched += 1;
    }

    // Always keep the interactive user in sync as well.
    if classic {
        enable_for(HKCU, &clsid_key())?;
    } else {
        disable_for(HKCU, &clsid_key())?;
    }
    Ok(touched)
}

fn is_user_sid(value: &str) -> bool {
    value.starts_with("S-1-5-21-") && !value.contains('_')
}

fn enable_for(root: &RegKey, path: &str) -> io::Result<()> {
    let (key, _) = root.create_subkey(format!("{path}\\InprocServer32"))?;
    // An empty REG_SZ default value is what disables the Win11 menu host.
    key.set_value("", &"")?;
    Ok(())
}

/// ERROR_FILE_NOT_FOUND / ERROR_PATH_NOT_FOUND. A key may vanish between the
/// existence check and the delete (Explorer watches this CLSID), and
/// `RegDeleteTreeW` also removes the key itself on some Windows builds.
fn is_missing(e: &io::Error) -> bool {
    matches!(e.raw_os_error(), Some(2) | Some(3))
}

fn disable_for(root: &RegKey, path: &str) -> io::Result<()> {
    if !key_exists(root, path) {
        return Ok(());
    }
    // RegDeleteTreeW wipes every descendant. The follow-up delete is therefore
    // conditional, and "not found" is a success: what matters is the end state,
    // not which of the two calls removed the key. Without this, the *first*
    // switch from classic to default reports "os error 2" even though the
    // registry was already updated, forcing the user to click a second time.
    match root.delete_subkey_all(path) {
        Ok(()) => {}
        Err(e) if is_missing(&e) => {}
        Err(e) => return Err(e),
    }
    if !key_exists(root, path) {
        return Ok(());
    }
    match root.delete_subkey(path) {
        Ok(()) => Ok(()),
        Err(e) if is_missing(&e) => Ok(()),
        Err(e) => Err(e),
    }
}

fn read_stuck_rects() -> io::Result<Vec<u8>> {
    let key = HKCU.open_subkey(STUCK_RECTS)?;
    let value = key.get_raw_value("Settings")?;
    if value.vtype != REG_BINARY {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            "StuckRects3\\Settings is not REG_BINARY",
        ));
    }
    Ok(value.bytes.into_owned())
}

fn write_stuck_rects(bytes: &[u8]) -> io::Result<()> {
    let key = HKCU.open_subkey_with_flags(STUCK_RECTS, KEY_ALL_ACCESS)?;
    // `RegValue<'a>` borrows its bytes; hand it an owned Vec so the value is
    // unambiguously 'static and no hidden lifetime param leaks into the call.
    let value = RegValue {
        bytes: bytes.to_vec().into(),
        vtype: REG_BINARY,
    };
    key.set_raw_value("Settings", &value)
}

pub fn taskbar_options() -> TaskbarOptions {
    match read_stuck_rects() {
        Ok(bytes) if bytes.len() > EDGE_OFFSET => TaskbarOptions {
            top: bytes[EDGE_OFFSET] == EDGE_TOP,
            auto_hide: bytes[AUTOHIDE_OFFSET] == AUTOHIDE_ON,
        },
        _ => TaskbarOptions::default(),
    }
}

pub fn set_taskbar_top(top: bool) -> io::Result<()> {
    let mut bytes = read_stuck_rects()?;
    if bytes.len() <= EDGE_OFFSET {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            "Unsupported StuckRects3 layout",
        ));
    }
    bytes[EDGE_OFFSET] = if top { EDGE_TOP } else { EDGE_BOTTOM };
    write_stuck_rects(&bytes)
}

pub fn set_taskbar_auto_hide(auto_hide: bool) -> io::Result<()> {
    let mut bytes = read_stuck_rects()?;
    if bytes.len() <= AUTOHIDE_OFFSET {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            "Unsupported StuckRects3 layout",
        ));
    }
    bytes[AUTOHIDE_OFFSET] = if auto_hide { AUTOHIDE_ON } else { AUTOHIDE_OFF };
    write_stuck_rects(&bytes)
}

/// Returns (build number, friendly OS name). Windows 11 starts at build 22000.
///
/// `ProductName` is deliberately NOT trusted for the "Windows 10 / 11" part:
/// Microsoft never updated that value for Windows 11, so it still reads
/// "Windows 10 Pro for Workstations" on a 26H2 machine. The build number is
/// authoritative; ProductName only supplies the edition suffix.
pub fn os_info() -> (u32, String) {
    let key = match HKLM.open_subkey(NT_CURRENT_VERSION) {
        Ok(key) => key,
        Err(_) => return (0, "Windows".into()),
    };
    let build = read_build(&key);
    let product = key
        .get_value::<String, _>("ProductName")
        .unwrap_or_else(|_| "Windows".into());
    let display = key
        .get_value::<String, _>("DisplayVersion")
        .ok()
        .map(|v| v.trim().to_string())
        .filter(|v| !v.is_empty());

    let edition = edition_of(&product);
    let base = if build >= 22000 {
        if edition.is_empty() {
            "Windows 11".to_string()
        } else {
            format!("Windows 11 {edition}")
        }
    } else {
        product.trim().to_string()
    };
    let label = match display {
        Some(version) => format!("{base} ({version})"),
        None => base,
    };
    (build, label)
}

/// `CurrentBuild` is REG_SZ on most systems but can be REG_DWORD; fall back to
/// `CurrentBuildNumber` so a failed read never mislabels Windows 11 as Win10.
fn read_build(key: &RegKey) -> u32 {
    for name in ["CurrentBuild", "CurrentBuildNumber"] {
        if let Ok(raw) = key.get_value::<String, _>(name) {
            if let Ok(build) = raw.trim().parse::<u32>() {
                return build;
            }
        }
        if let Ok(build) = key.get_value::<u32, _>(name) {
            return build;
        }
    }
    0
}

/// Strip the (possibly stale) "Windows 10 " prefix, keeping "Pro", "Home", ...
fn edition_of(product: &str) -> String {
    let text = product.trim();
    for prefix in ["Windows 11 ", "Windows 10 ", "Windows "] {
        if let Some(rest) = text.strip_prefix(prefix) {
            return rest.trim().to_string();
        }
    }
    String::new()
}

/// OS-wide "choose your default app mode" preference. Used only when the user
/// picked the `system` theme and at startup, before the webview can report its
/// own `prefers-color-scheme` result. Missing/odd values default to light.
pub fn apps_use_light_theme() -> bool {
    HKCU
        .open_subkey(PERSONALIZE)
        .and_then(|key| key.get_value::<u32, _>("AppsUseLightTheme"))
        .map(|value| value != 0)
        .unwrap_or(true)
}
