/**
 * Application entry point: state orchestration and event wiring.
 * All Windows work happens in Rust; this file only talks over Tauri IPC.
 */

import { api, LINKS } from "./modules/api.js";
import { LANGUAGES, resolveLang, t } from "./modules/i18n.js";
import * as ui from "./modules/ui.js";

// WebView2 would otherwise show its default browser context menu
// (Reload / Inspect / Save as). This utility has no editable fields, so
// suppress it globally in the capture phase. Remove this listener to restore
// the native menu.
document.addEventListener(
  "contextmenu",
  (event) => event.preventDefault(),
  { capture: true },
);

const state = {
  status: null,
  settings: null,
  lang: "en",
  version: "0.1.0",
};

// Opening index.html in a plain browser has no Tauri bridge; keep the UI fully
// interactive with a representative local state so it can be previewed.
let preview = false;

const T = (key) => t(key, state.lang);
const $ = (id) => document.getElementById(id);
const themePref = () => state.settings?.theme ?? "light";

function currentLanguageLabel() {
  const pref = state.settings?.language ?? "auto";
  if (pref === "auto") return T("autoLabel");
  return LANGUAGES.find((item) => item.id === pref)?.label ?? pref;
}

function render() {
  ui.applyI18n(state.lang, t);
  ui.applyTheme(themePref());
  ui.renderLanguageMenu(state.settings?.language ?? "auto");
  ui.setLanguageLabel(currentLanguageLabel());
  ui.renderTheme(themePref(), state.lang, t);
  if (state.status && state.settings) {
    ui.renderStatus(state.status, state.settings, state.lang, t);
  }
  $("version-label").textContent = `v${state.version}`;
}

function errorText(error) {
  const detail =
    typeof error === "string" ? error : error?.message ?? String(error);
  return `${T("errorGeneric")}: ${detail}`;
}

/** Run a backend mutation, or apply it locally in browser-preview mode. */
async function runCommand(remote, local) {
  if (preview) {
    local();
    render();
    return;
  }
  try {
    await remote();
  } catch (error) {
    ui.toast(errorText(error), "error");
    // A command can fail *after* its side effect succeeded (e.g. a registry
    // delete reporting ERROR_FILE_NOT_FOUND). Re-read the real state so the UI
    // never stays stuck on the previous style and forces a second click.
    try {
      const [status, settings] = await Promise.all([
        api.getStatus(),
        api.getSettings(),
      ]);
      state.status = status;
      state.settings = settings;
    } catch {
      // Keep the last known state if the backend is unreachable.
    }
  }
  render();
}

async function refresh() {
  if (preview) {
    render();
    return;
  }
  try {
    const [status, settings] = await Promise.all([
      api.getStatus(),
      api.getSettings(),
    ]);
    state.status = status;
    state.settings = settings;
    state.lang = resolveLang(settings.language);
    render();
  } catch (error) {
    if (String(error).includes("Tauri bridge unavailable")) {
      preview = true;
      state.status = {
        classic: false,
        windows11: true,
        osBuild: 26100,
        osName: "Windows 11 Pro (26H2)",
        elevated: false,
        taskbarTop: false,
        taskbarAutoHide: false,
        needsRestart: false,
      };
      state.settings = {
        language: "auto",
        theme: "light",
        applyToAllUsers: false,
        // Keep the "restart pending" banner visible in a browser preview.
        autoRestartExplorer: false,
      };
      render();
    } else {
      ui.toast(errorText(error), "error");
    }
  }
}

async function doRestart() {
  // Restart now polls the shell for up to several seconds; block re-entry so
  // a second click cannot kill the explorer that is just coming back.
  const button = $("btn-restart-now");
  button.disabled = true;
  button.setAttribute("aria-busy", "true");
  try {
    await runCommand(
      async () => {
        state.status = await api.restartExplorer();
      },
      () => {
        state.status.needsRestart = false;
      },
    );
  } finally {
    button.disabled = false;
    button.removeAttribute("aria-busy");
  }
}

async function setStyle(classic) {
  if (!state.status || state.status.classic === classic) return;
  const before = state.status.classic;
  await runCommand(
    async () => {
      state.status = await api.setMenuStyle(classic);
    },
    () => {
      state.status.classic = classic;
      state.status.needsRestart = true;
    },
  );
  if (state.status.classic !== before && state.settings?.autoRestartExplorer) {
    await doRestart();
  }
}

async function setTheme(value) {
  if (!state.settings) return;
  if (state.settings.theme === value) return;
  // Resolve "system" here so CSS and the native title bar never disagree.
  const dark = ui.effectiveTheme(value) === "dark";
  if (!preview) {
    try {
      state.settings = await api.setTheme(value, dark);
    } catch (error) {
      ui.toast(errorText(error), "error");
    }
  }
  state.settings.theme = value;
  render();
}

/** Keep the DWM caption bar in step with the CSS theme after OS changes. */
async function syncWindowTheme() {
  if (preview) return;
  try {
    await api.syncWindowTheme(ui.effectiveTheme(themePref()) === "dark");
  } catch {
    // The native bar is cosmetic; never surface an error for it.
  }
}

async function showCommandInfo() {
  let text = "Usage: MenuGo.exe /C | /D | /R | /?";
  if (!preview) {
    try {
      text = await api.commandLineInfo();
    } catch (error) {
      ui.toast(errorText(error), "error");
      return;
    }
  }
  await ui.infoDialog({
    title: T("cmdInfoTitle"),
    message: text,
    okText: T("ok"),
    monospace: true,
  });
}

async function showAbout() {
  await ui.infoDialog({
    title: T("aboutTitle"),
    message: `${T("aboutBody")}\nVersion: ${state.version}`,
    okText: T("ok"),
  });
}

async function clearCache() {
  if (preview) {
    ui.toast(T("clearCacheDone"), "info");
    return;
  }
  const confirmed = await ui.confirmDialog({
    title: T("clearCache"),
    message: T("clearCacheConfirm"),
    okText: T("ok"),
    cancelText: T("cancel"),
  });
  if (!confirmed) return;
  // Block re-entry while the (potentially multi-second) deletion runs; the
  // page reloads afterwards anyway.
  const button = $("btn-clear-cache");
  button.disabled = true;
  try {
    const allGone = await api.clearCache();
    ui.toast(allGone ? T("clearCacheDone") : T("clearCachePartial"), "info");
  } catch (error) {
    ui.toast(errorText(error), "error");
    button.disabled = false;
    return;
  }
  // Reload so the UI reflects the reset defaults (light theme, auto language)
  // without restarting the process. Locked leftovers are swept next launch.
  setTimeout(() => location.reload(), 900);
}

function bindEvents() {
  $("seg-classic").addEventListener("click", () => setStyle(true));
  $("seg-default").addEventListener("click", () => setStyle(false));
  $("btn-restart-now").addEventListener("click", doRestart);

  $("btn-theme").addEventListener("click", () => {
    const next = ui.effectiveTheme(themePref()) === "dark" ? "light" : "dark";
    setTheme(next);
  });

  $("theme-options").addEventListener("click", (event) => {
    const target = event.target instanceof Element ? event.target : null;
    const button = target?.closest("[data-theme-value]");
    if (button) setTheme(button.dataset.themeValue);
  });

  $("btn-settings").addEventListener("click", () => {
    if (ui.isSettingsOpen()) ui.closeSettings();
    else ui.openSettings();
  });
  $("btn-close-settings").addEventListener("click", ui.closeSettings);
  $("btn-clear-cache").addEventListener("click", clearCache);
  $("btn-panel-refresh").addEventListener("click", () => {
    ui.closeSettings();
    refresh();
  });

  $("btn-language").addEventListener("click", () => {
    if (ui.isLanguageMenuOpen()) ui.closeLanguageMenu();
    else ui.openLanguageMenu();
  });

  $("btn-more").addEventListener("click", () => {
    if (ui.isMoreMenuOpen()) ui.closeMoreMenu();
    else ui.openMoreMenu();
  });

  $("more-menu").addEventListener("click", async (event) => {
    const target = event.target instanceof Element ? event.target : null;
    const button = target?.closest("[data-link]");
    if (!button) return;
    const action = button.dataset.link;
    if (action === "cmdinfo") {
      ui.closeMoreMenu();
      return showCommandInfo();
    }
    if (action === "about") {
      ui.closeMoreMenu();
      return showAbout();
    }
    const url = LINKS[action];
    if (!url) return;
    if (preview) {
      ui.toast(url, "info");
      return;
    }
    ui.closeMoreMenu();
    try {
      await api.openUrl(url);
    } catch (error) {
      ui.toast(errorText(error), "error");
    }
  });

  $("language-menu").addEventListener("click", async (event) => {
    if (!state.settings) return; // refresh() has not finished yet
    const target = event.target instanceof Element ? event.target : null;
    const item = target?.closest("[data-value]");
    if (!item) return;
    const value = item.dataset.value;
    if (!preview) {
      try {
        await api.setLanguage(value);
      } catch (error) {
        ui.toast(errorText(error), "error");
        return;
      }
    }
    state.settings.language = value;
    state.lang = resolveLang(value);
    ui.closeLanguageMenu();
    render();
  });

  $("sw-apply-all").addEventListener("change", (event) => {
    if (!state.status || !state.settings) return; // refresh() has not finished yet
    const next = event.target.checked;
    if (!preview && next && !state.status.elevated) {
      ui.toast(T("needAdmin"), "error");
      render();
      return;
    }
    runCommand(
      async () => {
        state.status = await api.setApplyToAllUsers(next);
        state.settings.applyToAllUsers = next;
      },
      () => {
        state.settings.applyToAllUsers = next;
        state.status.needsRestart = true;
      },
    );
  });

  $("sw-auto-hide").addEventListener("change", (event) => {
    if (!state.status) return; // refresh() has not finished yet
    const next = event.target.checked;
    runCommand(
      async () => {
        state.status = await api.setTaskbarAutoHide(next);
      },
      () => {
        state.status.taskbarAutoHide = next;
        state.status.needsRestart = true;
      },
    );
  });

  $("sw-auto-restart").addEventListener("change", (event) => {
    if (!state.settings) return; // refresh() has not finished yet
    const next = event.target.checked;
    runCommand(
      async () => {
        state.settings = await api.setAutoRestart(next);
      },
      () => {
        state.settings.autoRestartExplorer = next;
      },
    );
  });

  $("sw-taskbar-top").addEventListener("change", (event) => {
    if (!state.status) return; // refresh() has not finished yet
    const next = event.target.checked;
    runCommand(
      async () => {
        state.status = await api.setTaskbarTop(next);
      },
      () => {
        state.status.taskbarTop = next;
        state.status.needsRestart = true;
      },
    );
  });

  document.addEventListener("click", (event) => {
    const target = event.target instanceof Element ? event.target : null;
    if (
      ui.isSettingsOpen() &&
      !target?.closest("#settings-panel") &&
      !target?.closest("#btn-settings")
    ) {
      ui.closeSettings();
    }
    if (
      ui.isMoreMenuOpen() &&
      !target?.closest("#more-menu") &&
      !target?.closest("#btn-more")
    ) {
      ui.closeMoreMenu();
    }
    if (
      ui.isLanguageMenuOpen() &&
      !target?.closest("#language-menu") &&
      !target?.closest("#btn-language")
    ) {
      ui.closeLanguageMenu();
    }
  });

  window.addEventListener("keydown", (event) => {
    if (event.key === "Escape") {
      if (ui.isDialogOpen()) ui.dismissDialog();
      else if (ui.isMoreMenuOpen()) ui.closeMoreMenu();
      else if (ui.isSettingsOpen()) ui.closeSettings();
      else if (ui.isLanguageMenuOpen()) ui.closeLanguageMenu();
    } else if (event.key === "F5") {
      event.preventDefault();
      refresh();
    } else if (event.key === "F1") {
      event.preventDefault();
      showAbout();
    } else if (event.altKey && event.key.toLowerCase() === "c") {
      event.preventDefault();
      setStyle(true);
    } else if (event.altKey && event.key.toLowerCase() === "d") {
      event.preventDefault();
      setStyle(false);
    }
  });
}

window.addEventListener("DOMContentLoaded", async () => {
  // Apply the persisted theme before the first paint of dynamic content.
  ui.applyTheme(themePref());
  bindEvents();
  if (window.matchMedia) {
    const media = window.matchMedia("(prefers-color-scheme: dark)");
    media.addEventListener?.("change", () => {
      if (themePref() !== "system") return;
      render();
      syncWindowTheme();
    });
  }
  await refresh();
  syncWindowTheme();
  if (preview) return;
  try {
    state.version = await api.appVersion();
    render();
    const mode = await api.launchMode();
    if (mode === "commandInfo") await showCommandInfo();
  } catch {
    // Browser preview has no backend; the fallback state is already rendered.
  }
});
