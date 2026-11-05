//! OS side effects: restarting explorer, opening links, elevation check.

use crate::shell;
use std::io;
use std::path::PathBuf;
use std::process::Command;
use std::time::Duration;

#[cfg(windows)]
use std::os::windows::process::CommandExt;

/// CREATE_NO_WINDOW - keep helper processes from flashing a console window.
#[cfg(windows)]
const CREATE_NO_WINDOW: u32 = 0x0800_0000;

fn hidden(program: impl AsRef<std::ffi::OsStr>) -> Command {
    let mut cmd = Command::new(program);
    hide(&mut cmd);
    cmd
}

#[cfg(windows)]
fn hide(cmd: &mut Command) {
    cmd.creation_flags(CREATE_NO_WINDOW);
}

#[cfg(not(windows))]
fn hide(_cmd: &mut Command) {}

fn windows_dir() -> PathBuf {
    std::env::var_os("SystemRoot")
        .or_else(|| std::env::var_os("WINDIR"))
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from(r"C:\Windows"))
}

/// Absolute path to a system binary. Relying on PATH makes CreateProcess fail
/// with ERROR_FILE_NOT_FOUND when PATH has been trimmed.
fn system_exe(name: &str) -> PathBuf {
    windows_dir().join("System32").join(name)
}

/// The 64-bit shell lives in `%WINDIR%\explorer.exe`, **not** in System32 -
/// there is no explorer.exe there on modern Windows. SysWOW64 holds the 32-bit
/// build; Sysnative is a virtual alias visible only to WOW64 processes. Probe
/// every candidate so a launch failure can never be silently swallowed.
fn explorer_candidates() -> Vec<PathBuf> {
    let root = windows_dir();
    [
        root.join("explorer.exe"),
        root.join("System32").join("explorer.exe"),
        root.join("Sysnative").join("explorer.exe"),
        root.join("SysWOW64").join("explorer.exe"),
    ]
    .into_iter()
    .filter(|path| path.is_file())
    .collect()
}

/// Kill and relaunch the shell. Success means a *new* taskbar window exists,
/// not merely that CreateProcess returned a handle.
pub fn restart_explorer() -> io::Result<()> {
    let before = shell::taskbar_hwnd();
    let kill = hidden(system_exe("taskkill.exe"))
        .args(["/f", "/im", "explorer.exe"])
        .status();
    let kill_note = match &kill {
        Ok(status) if status.success() => String::new(),
        Ok(status) => format!(" (taskkill exited with {status})"),
        Err(e) => format!(" (taskkill failed: {e})"),
    };

    // Winlogon normally relaunches the shell by itself (AutoRestartShell=1).
    // Prefer that: spawning a second explorer against a live shell only opens
    // a stray file-manager window and races with Winlogon.
    if shell::wait_for_new_taskbar(before, Duration::from_millis(2500)) {
        return Ok(());
    }

    let mut launched = false;
    let mut last_error: Option<io::Error> = None;
    for exe in explorer_candidates() {
        match hidden(&exe).spawn() {
            Ok(_) => {
                launched = true;
                break;
            }
            Err(e) => last_error = Some(e),
        }
    }
    if !launched {
        let detail = match last_error {
            Some(e) => format!("could not start explorer.exe: {e}"),
            None => "explorer.exe was not found in any system path".to_string(),
        };
        return Err(io::Error::new(
            io::ErrorKind::NotFound,
            format!("{detail}{kill_note}"),
        ));
    }

    if shell::wait_for_new_taskbar(before, Duration::from_millis(6000)) {
        Ok(())
    } else {
        Err(io::Error::new(
            io::ErrorKind::TimedOut,
            format!("explorer.exe was launched but the taskbar did not return{kill_note}"),
        ))
    }
}

pub fn open_url(url: &str) -> io::Result<()> {
    hidden(system_exe("rundll32.exe"))
        .args(["url.dll,FileProtocolHandler", url])
        .spawn()
        .map(|_| ())
}

/// `net session` only succeeds with an elevated token. Cheap, dependency-free
/// equivalent of CheckTokenMembership(Administrators).
pub fn is_elevated() -> bool {
    hidden(system_exe("net.exe"))
        .arg("session")
        .output()
        .map(|out| out.status.success())
        .unwrap_or(false)
}
