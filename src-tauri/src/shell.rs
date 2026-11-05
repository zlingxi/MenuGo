//! Detect whether the Windows shell is actually back.
//!
//! `Command::spawn()` succeeding only proves CreateProcess worked. Explorer is
//! single-instance: a fresh process may hand off to an existing shell and exit,
//! or open a file-manager window without taking over the desktop at all. The
//! reliable signal is `Shell_TrayWnd` - the taskbar window owned by the shell
//! process (still the same class on Windows 11). A restart recreates that
//! window, so a changed HWND proves a *new* shell, not the surviving old one.
//! No extra crate is needed; this mirrors `titlebar.rs`' FFI.

use std::thread;
use std::time::{Duration, Instant};

// `unsafe extern` is required by Rust edition 2024 (and accepted by 2021).
#[cfg(windows)]
#[link(name = "user32")]
unsafe extern "system" {
    fn FindWindowW(class_name: *const u16, window_name: *const u16) -> isize;
}

#[cfg(windows)]
fn find_window(class_name: &str) -> isize {
    let name: Vec<u16> = class_name.encode_utf16().chain(Some(0)).collect();
    // SAFETY: `name` is a live NUL-terminated UTF-16 string; a null window
    // name is explicitly allowed. FindWindowW only reads these pointers.
    unsafe { FindWindowW(name.as_ptr(), std::ptr::null()) }
}

/// HWND of the taskbar, or 0 when the shell is not running.
#[cfg(windows)]
pub fn taskbar_hwnd() -> isize {
    find_window("Shell_TrayWnd")
}

#[cfg(not(windows))]
pub fn taskbar_hwnd() -> isize {
    0
}

/// Wait until the taskbar window has been recreated. Comparing HWNDs (rather
/// than polling "gone" and then "back") also handles the case where Winlogon
/// restarts the shell faster than our first poll.
pub fn wait_for_new_taskbar(previous: isize, timeout: Duration) -> bool {
    let start = Instant::now();
    loop {
        let current = taskbar_hwnd();
        if current != 0 && current != previous {
            return true;
        }
        if start.elapsed() >= timeout {
            return false;
        }
        thread::sleep(Duration::from_millis(100));
    }
}
