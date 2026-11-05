//! Command line switches, compatible with the original tool:
//!   /C  classic menu, /D  default menu, /R  restart explorer, /?  info
//! Long GNU-style aliases are accepted as a convenience.

use crate::registry;
use crate::system;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LaunchMode {
    /// Normal graphical startup.
    Gui,
    /// Graphical startup with the "Command Line Info" dialog pre-opened.
    ShowCommandInfo,
    /// Switches were executed, exit without showing a window.
    Headless,
}

pub const COMMAND_LINE_INFO: &str = "\
Usage: MenuGo.exe <command>

Commands:
  /C    Enables Win11 classic context menu style
  /D    Enables Win11 default context menu style
  /R    Restarts the explorer if necessary
  /?    Shows this command line info

Examples:
  MenuGo.exe /C
  MenuGo.exe /D
  MenuGo.exe /C /R

Aliases: --classic / --default / --restart / --help

Registry target:
  HKCU\\Software\\Classes\\CLSID\\{86ca1aa0-34aa-4e8b-a509-50c905bae2a2}
";

/// Parse arguments, perform the requested side effects, and tell `main`
/// whether a window should be created.
pub fn handle(args: &[String]) -> LaunchMode {
    let mut style: Option<bool> = None;
    let mut restart = false;
    let mut show_info = false;

    for raw in args {
        match raw.to_ascii_lowercase().as_str() {
            "/c" | "-c" | "--classic" => style = Some(true),
            "/d" | "-d" | "--default" => style = Some(false),
            "/r" | "-r" | "--restart" => restart = true,
            "/?" | "/h" | "-h" | "--help" => show_info = true,
            _ => {}
        }
    }

    if let Some(classic) = style {
        // Headless callers (schedulers, scripts) need a real failure signal,
        // exactly like /R below: silently swallowing a registry error would
        // make the script believe the style has changed.
        if let Err(e) = registry::set_classic(classic) {
            eprintln!("MenuGo: failed to update the context menu registry key: {e}");
            std::process::exit(1);
        }
    }
    if restart {
        // Headless callers (schedulers, scripts) need a real failure signal;
        // silently swallowing it leaves the desktop without a taskbar.
        if let Err(e) = system::restart_explorer() {
            eprintln!("MenuGo: failed to restart Windows Explorer: {e}");
            std::process::exit(1);
        }
    }

    if style.is_some() || restart {
        LaunchMode::Headless
    } else if show_info {
        LaunchMode::ShowCommandInfo
    } else {
        LaunchMode::Gui
    }
}
