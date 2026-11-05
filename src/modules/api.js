/**
 * Thin wrapper around the Tauri IPC bridge.
 * `withGlobalTauri: true` in tauri.conf.json exposes window.__TAURI__.core,
 * so no npm package / bundler is needed (vanilla ESM all the way down).
 */

const invoke = window.__TAURI__?.core?.invoke;

function call(command, args = {}) {
  if (typeof invoke !== "function") {
    return Promise.reject(new Error("Tauri bridge unavailable"));
  }
  return invoke(command, args);
}

export const api = {
  getStatus: () => call("get_status"),
  getSettings: () => call("get_settings"),
  setMenuStyle: (classic) => call("set_menu_style", { classic }),
  restartExplorer: () => call("restart_explorer"),
  setLanguage: (language) => call("set_language", { language }),
  setTheme: (theme, dark) => call("set_theme", { theme, dark }),
  syncWindowTheme: (dark) => call("sync_window_theme", { dark }),
  setApplyToAllUsers: (enabled) => call("set_apply_to_all_users", { enabled }),
  setAutoRestart: (enabled) => call("set_auto_restart", { enabled }),
  setTaskbarTop: (enabled) => call("set_taskbar_top", { enabled }),
  setTaskbarAutoHide: (enabled) => call("set_taskbar_auto_hide", { enabled }),
  openUrl: (url) => call("open_url", { url }),
  commandLineInfo: () => call("command_line_info"),
  launchMode: () => call("launch_mode"),
  appVersion: () => call("app_version"),
  clearCache: () => call("clear_cache"),
};

// Keep in sync with the allow list in src-tauri/src/commands.rs.
export const LINKS = {
  donate: "https://www.aizzx.top/sponsor",
  blog: "https://www.aizzx.top/2930.html",
  github: "https://github.com/zlingxi/MenuGo",
};
