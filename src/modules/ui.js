/** DOM rendering plus the popover / dialog / toast primitives. */

import { LANGUAGES } from "./i18n.js";

const $ = (id) => document.getElementById(id);

const THEMES = ["light", "dark", "system"];

let toastTimer = null;
let dialogResolver = null;

/* ---------- theme ---------- */

export function effectiveTheme(pref) {
  if (pref === "system") {
    const dark = window.matchMedia?.("(prefers-color-scheme: dark)")?.matches;
    return dark ? "dark" : "light";
  }
  return pref === "dark" ? "dark" : "light";
}

export function applyTheme(pref) {
  const value = THEMES.includes(pref) ? pref : "light";
  document.documentElement.dataset.theme = value;
  document.body.classList.toggle("is-dark", effectiveTheme(value) === "dark");
}

export function renderTheme(pref, lang, t) {
  const value = THEMES.includes(pref) ? pref : "light";
  document.querySelectorAll("[data-theme-value]").forEach((button) => {
    button.setAttribute(
      "aria-checked",
      String(button.dataset.themeValue === value),
    );
  });
  $("hint-theme").textContent = t("themeHint", lang);
  const toggle = $("btn-theme");
  toggle.setAttribute("aria-label", t("ariaTheme", lang));
  toggle.title =
    effectiveTheme(value) === "dark"
      ? t("switchToLight", lang)
      : t("switchToDark", lang);
}

export function applyI18n(lang, t) {
  document.documentElement.lang = lang;
  document.querySelectorAll("[data-i18n]").forEach((el) => {
    el.textContent = t(el.dataset.i18n, lang);
  });
  document.querySelectorAll("[data-i18n-aria]").forEach((el) => {
    el.setAttribute("aria-label", t(el.dataset.i18nAria, lang));
  });
}

export function renderLanguageMenu(currentPreference) {
  const menu = $("language-menu");
  menu.replaceChildren();
  for (const language of LANGUAGES) {
    const item = document.createElement("button");
    item.className = "lang-item";
    item.type = "button";
    item.setAttribute("role", "option");
    item.setAttribute("aria-selected", String(language.id === currentPreference));
    item.dataset.value = language.id;

    const tick = document.createElement("span");
    tick.className = "tick";
    tick.textContent = language.id === currentPreference ? "✓" : "";

    const label = document.createElement("span");
    label.textContent = language.label;

    item.append(tick, label);
    menu.append(item);
  }
}

export function setLanguageLabel(text) {
  $("language-label").textContent = text;
}

export function renderStatus(status, settings, lang, t) {
  document.body.classList.toggle("classic", Boolean(status.classic));

  $("style-title").textContent = t(
    status.classic ? "classicMenu" : "defaultMenu",
    lang,
  );
  $("style-desc").textContent = t(
    status.classic ? "classicDesc" : "defaultDesc",
    lang,
  );
  $("seg-classic").setAttribute("aria-checked", String(Boolean(status.classic)));
  $("seg-default").setAttribute("aria-checked", String(!status.classic));

  $("restart-banner").classList.toggle("hidden", !status.needsRestart);
  $("os-label").textContent = status.osName || "Windows";
  $("os-label").title = status.osBuild ? `Build ${status.osBuild}` : "";
  $("elevated-label").classList.toggle("hidden", !status.elevated);

  const supported = Boolean(status.windows11);
  $("os-warning").classList.toggle("hidden", supported);
  $("os-warning-text").textContent = t("osWarning", lang);

  renderSettings(status, settings, lang, t);
}

function renderSettings(status, settings, lang, t) {
  const applyAll = $("sw-apply-all");
  applyAll.checked = Boolean(settings.applyToAllUsers);
  applyAll.disabled = !status.elevated;
  $("row-apply-all").classList.toggle("is-disabled", !status.elevated);
  $("hint-apply-all").textContent = t(
    status.elevated ? "hintApplyAll" : "hintApplyAllAdmin",
    lang,
  );

  $("sw-auto-hide").checked = Boolean(status.taskbarAutoHide);
  $("hint-auto-hide").textContent = t("hintAutoHide", lang);

  $("sw-auto-restart").checked = Boolean(settings.autoRestartExplorer);
  $("hint-auto-restart").textContent = t("hintAutoRestart", lang);

  // Windows 11 22H2+ ignores the taskbar edge byte in StuckRects3.
  const topUnsupported =
    Boolean(status.windows11) && (status.osBuild ?? 0) >= 22600;
  const top = $("sw-taskbar-top");
  top.checked = Boolean(status.taskbarTop);
  top.disabled = topUnsupported;
  $("row-taskbar-top").classList.toggle("is-disabled", topUnsupported);
  $("hint-taskbar-top").textContent = t(
    topUnsupported ? "hintTaskbarTopOff" : "hintTaskbarTop",
    lang,
  );
}

export function toast(message, kind = "info") {
  const el = $("toast");
  el.textContent = message;
  el.className = `toast ${kind}`;
  if (toastTimer) clearTimeout(toastTimer);
  toastTimer = setTimeout(() => el.classList.add("hidden"), 2600);
}

/* ---------- settings popover ---------- */

export function openSettings() {
  closeLanguageMenu();
  $("settings-panel").classList.remove("hidden");
  $("settings-scrim").classList.remove("hidden");
  $("btn-settings").setAttribute("aria-expanded", "true");
}

export function closeSettings() {
  $("settings-panel").classList.add("hidden");
  $("settings-scrim").classList.add("hidden");
  $("btn-settings").setAttribute("aria-expanded", "false");
}

export function isSettingsOpen() {
  return !$("settings-panel").classList.contains("hidden");
}

/* ---------- language menu ---------- */

export function openLanguageMenu() {
  closeSettings();
  $("language-menu").classList.remove("hidden");
  $("btn-language").setAttribute("aria-expanded", "true");
}

export function closeLanguageMenu() {
  $("language-menu").classList.add("hidden");
  $("btn-language").setAttribute("aria-expanded", "false");
}

export function isLanguageMenuOpen() {
  return !$("language-menu").classList.contains("hidden");
}

/* ---------- dialog ---------- */

export function infoDialog({ title, message, okText, monospace = false }) {
  if (dialogResolver) dialogResolver();
  $("dialog-title").textContent = title;

  const body = $("dialog-body");
  body.textContent = message;
  body.classList.toggle("monospace", monospace);
  $("dialog-ok").textContent = okText;
  $("dialog-cancel").classList.add("hidden");
  $("modal-backdrop").classList.remove("hidden");
  $("dialog-ok").focus();

  return new Promise((resolve) => {
    dialogResolver = () => {
      dialogResolver = null;
      $("modal-backdrop").classList.add("hidden");
      resolve();
    };
    $("dialog-ok").onclick = () => dialogResolver?.();
  });
}

export function dismissDialog() {
  if (dialogResolver) dialogResolver();
}

export function isDialogOpen() {
  return !$("modal-backdrop").classList.contains("hidden");
}

/// Two-button dialog. Resolves `true` on OK, `false` on cancel/dismiss.
export function confirmDialog({ title, message, okText, cancelText }) {
  if (dialogResolver) dialogResolver();
  $("dialog-title").textContent = title;
  const body = $("dialog-body");
  body.textContent = message;
  body.classList.remove("monospace");
  $("dialog-ok").textContent = okText;
  $("dialog-cancel").textContent = cancelText;
  $("dialog-cancel").classList.remove("hidden");
  $("modal-backdrop").classList.remove("hidden");
  $("dialog-cancel").focus();

  return new Promise((resolve) => {
    dialogResolver = (value) => {
      dialogResolver = null;
      $("modal-backdrop").classList.add("hidden");
      $("dialog-cancel").classList.add("hidden");
      resolve(value);
    };
    $("dialog-ok").onclick = () => dialogResolver?.(true);
    $("dialog-cancel").onclick = () => dialogResolver?.(false);
  });
}
