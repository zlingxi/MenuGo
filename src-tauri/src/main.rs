// Prevents additional console window on Windows in release, DO NOT REMOVE!!
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

use menugo_lib::cli::LaunchMode;

fn main() {
    // Finish a previous "clear cache" that hit WebView-locked files. Runs
    // before anything else, so no webview exists to hold locks again.
    menugo_lib::cleanup::sweep_if_pending();

    // CLI switches are handled before Tauri spins up, so `/C`, `/D` and `/R`
    // stay completely headless - exactly like the original tool.
    let args: Vec<String> = std::env::args().skip(1).collect();
    match menugo_lib::cli::handle(&args) {
        LaunchMode::Headless => {}
        LaunchMode::ShowCommandInfo => menugo_lib::run(true),
        LaunchMode::Gui => menugo_lib::run(false),
    }
}
