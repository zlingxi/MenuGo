//! Native (system-drawn) title bar theming through DWM.
//!
//! MenuGo keeps the standard Windows decorations, so the caption bar is
//! painted by DWM and CSS cannot reach it. `DWMWA_USE_IMMERSIVE_DARK_MODE`
//! switches the caption buttons and their glyphs, while `DWMWA_CAPTION_COLOR`
//! / `DWMWA_TEXT_COLOR` (Windows 11) tint the bar to exactly the webview's
//! own `--surface` / `--text` palette. The caption color attributes are
//! ignored on Windows 10, where immersive dark mode still works.

use tauri::WebviewWindow;

#[cfg(windows)]
use std::ffi::c_void;

// `unsafe extern` is required by Rust edition 2024 (and accepted by 2021).
#[cfg(windows)]
#[link(name = "dwmapi")]
unsafe extern "system" {
    fn DwmSetWindowAttribute(
        hwnd: *mut c_void,
        dw_attribute: i32,
        pv_attribute: *const c_void,
        cb_attribute: u32,
    ) -> i32;
}

// DWMWINDOWATTRIBUTE values used below. 20 is also valid (undocumented) on
// Windows 10 1809+, 35/36 are Windows 11 only.
#[cfg(windows)]
const DWMWA_USE_IMMERSIVE_DARK_MODE: i32 = 20;
#[cfg(windows)]
const DWMWA_CAPTION_COLOR: i32 = 35;
#[cfg(windows)]
const DWMWA_TEXT_COLOR: i32 = 36;

/// COLORREF is 0x00BBGGRR. These mirror `--surface` / `--text` in styles.css,
/// so the native caption bar and the webview top bar read as one surface.
#[cfg(windows)]
const LIGHT_CAPTION: u32 = 0x00FF_FFFF; // #ffffff
#[cfg(windows)]
const LIGHT_CAPTION_TEXT: u32 = 0x0030_221A; // #1a2230
#[cfg(windows)]
const DARK_CAPTION: u32 = 0x002B_221E; // #1e222b
#[cfg(windows)]
const DARK_CAPTION_TEXT: u32 = 0x00F6_EFEC; // #eceff6

/// Every attribute used here takes a 4-byte BOOL / COLORREF.
/// Failures (e.g. Win10 lacking CAPTION_COLOR) are cosmetic and ignored.
#[cfg(windows)]
fn set_attribute(handle: *mut c_void, attribute: i32, value: *const c_void) {
    // SAFETY: `handle` comes from `WebviewWindow::hwnd()` and `value` points
    // at a live 4-byte local; DWM only reads `cb_attribute` bytes from it.
    unsafe {
        let _ = DwmSetWindowAttribute(handle, attribute, value, 4);
    }
}

/// Apply the *effective* (already resolved) theme to the native title bar.
pub fn apply(window: &WebviewWindow, dark: bool) {
    #[cfg(windows)]
    {
        // `WebviewWindow::hwnd()` returns windows::Win32::Foundation::HWND,
        // a newtype over `*mut c_void`; `.0` is the raw handle.
        let Ok(hwnd) = window.hwnd() else {
            return;
        };
        let handle = hwnd.0;
        if handle.is_null() {
            return;
        }

        let immersive: i32 = i32::from(dark);
        set_attribute(
            handle,
            DWMWA_USE_IMMERSIVE_DARK_MODE,
            &immersive as *const i32 as *const c_void,
        );

        let caption: u32 = if dark { DARK_CAPTION } else { LIGHT_CAPTION };
        set_attribute(
            handle,
            DWMWA_CAPTION_COLOR,
            &caption as *const u32 as *const c_void,
        );

        let text: u32 = if dark {
            DARK_CAPTION_TEXT
        } else {
            LIGHT_CAPTION_TEXT
        };
        set_attribute(
            handle,
            DWMWA_TEXT_COLOR,
            &text as *const u32 as *const c_void,
        );
    }
    #[cfg(not(windows))]
    let _ = (window, dark);
}
