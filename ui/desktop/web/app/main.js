import { createEventBus } from "../core/event-bus.js";
import { createUiState, persistSelectedFeature } from "../core/state.js";
import { featureRegistry } from "../core/feature-registry.js";
import { loadDictionaries, createI18n } from "../i18n/runtime.js";
import { createDebugLogger } from "../core/debug.js";
import { initEngineDownloadNotifications } from "../core/engine-downloads.js";
import { initDnsBlocklistNotifications } from "../core/dns-blocklist-downloads.js";
import { minimizeWindow, toggleMaximizeWindow, closeWindow } from "../core/window-controls.js";
import { renderHome, hydrateHomeModel, wireHome } from "../features/home/view.js";
import { ensureEngineBinaries } from "../features/profiles/api.js";
import { launchProfile } from "../features/profiles/api.js";
import { updateProfile } from "../features/profiles/api.js";
import { hydrateProfilesModel, wireProfiles } from "../features/profiles/view.js";
import { panicWipeProfile } from "../features/home/api.js";
import { callCommand } from "../core/commands.js";
import { renderIdentity, wireIdentity } from "../features/identity/view.js";
import { renderNetwork, hydrateNetworkModel, wireNetwork } from "../features/network/view.js";
import { renderDns, hydrateDnsModel, wireDns } from "../features/dns/view.js";
import { renderExtensions, hydrateExtensionsModel, wireExtensions } from "../features/extensions/view.js";
import { renderTraffic, hydrateTrafficModel, wireTraffic } from "../features/traffic/view.js";
import { renderSecurity, wireSecurity } from "../features/security/view.js";
import {
  renderSettings,
  hydrateSettingsModel,
  wireSettings,
  renderLinkLaunchModal,
  wireLinkLaunchModal,
  consumePendingLinkLaunch
} from "../features/settings/view.js";

const log = createDebugLogger("app");
const COLLAPSE_BREAKPOINT = 1200;
const DEFAULT_PANIC_FRAME_COLOR = "#ff8652";
const HOME_METRICS_RENDER_DEBOUNCE_MS = 900;
const APP_VERSION = "1.0.6-1";

function renderBrandLogo(kind = "full") {
  const src = kind === "compact" ? "./assets/brand/logo-32.png" : "./assets/brand/logo-64.png";
  const alt = kind === "compact" ? "Cerbena compact logo" : "Cerbena logo";
  return `<img class='brand-logo-image brand-logo-image--${kind}' src='${src}' alt='${alt}' draggable='false' />`;
}

function menuAssetIcon(path, alt = "") {
  return `<img src="${path}" alt="${alt}" draggable="false" />`;
}

const ICONS = {
  home: menuAssetIcon("./assets/menu/home.svg"),
  extensions: menuAssetIcon("./assets/menu/extensions.svg"),
  network: menuAssetIcon("./assets/menu/network.svg"),
  traffic: menuAssetIcon("./assets/menu/traffic.svg"),
  dns: menuAssetIcon("./assets/menu/dns.svg"),
  sync: "<svg viewBox='0 0 24 24' fill='none' stroke='currentColor' stroke-width='2'><path d='M3 12a9 9 0 0 1 15-6'/><path d='M21 12a9 9 0 0 1-15 6'/><path d='M18 3v4h-4'/><path d='M6 21v-4h4'/></svg>",
  security: menuAssetIcon("./assets/menu/security.svg"),
  logs: "<svg viewBox='0 0 24 24' fill='none' stroke='currentColor' stroke-width='2'><rect x='4' y='3' width='16' height='18' rx='2'/><path d='M8 8h8M8 12h8M8 16h5'/></svg>",
  settings: menuAssetIcon("./assets/menu/settings.svg"),
  identity: menuAssetIcon("./assets/menu/identity.svg"),
  diagnostics: "<svg viewBox='0 0 24 24' fill='none' stroke='currentColor' stroke-width='2'><path d='M3 3v18h18'/><path d='M7 14l3-3 3 2 4-5'/></svg>"
};

const viewMap = {
  home: (t, model) => renderHome(t, model),
  extensions: (t, model) => renderExtensions(t, model),
  network: (t, model) => renderNetwork(t, model),
  traffic: (t, model) => renderTraffic(t, model),
  dns: (t, model) => renderDns(t, model),
  security: (t, model) => renderSecurity(t, model),
  settings: (t, model) => renderSettings(t, model),
  identity: (t, model) => renderIdentity(t, model)
};

function pickLocale() {
  return localStorage.getItem("launcher.locale") ?? "ru";
}

function navItem(entry, isActive, t) {
  const className = isActive ? "sidebar-link active" : "sidebar-link";
  const icon = ICONS[entry.key] ?? ICONS.home;
  return `<button class='${className}' data-feature='${entry.key}' title='${t(entry.labelKey)}'><span class='sidebar-link-icon'>${icon}</span><span class='sidebar-link-label'>${t(entry.labelKey)}</span></button>`;
}

function selectedProfile(model) {
  return (model.profiles ?? []).find((item) => item.id === model.selectedProfileId) ?? null;
}

function shouldRenderPanicControls(model) {
  const profile = selectedProfile(model);
  return Boolean(profile && profile.state === "running" && profile.panic_frame_enabled);
}

function fireIcon() {
  return "<img src='./assets/icons/fire-64.png' alt='' draggable='false' />";
}

function fireHeroIcon() {
  return "<img src='./assets/icons/fire-128.png' alt='' draggable='false' />";
}

function normalizePanicColor(value) {
  const raw = String(value ?? "").trim();
  return /^#[0-9a-f]{6}$/i.test(raw) ? raw.toLowerCase() : DEFAULT_PANIC_FRAME_COLOR;
}

function hexToRgbTriplet(value) {
  const color = normalizePanicColor(value);
  const r = Number.parseInt(color.slice(1, 3), 16);
  const g = Number.parseInt(color.slice(3, 5), 16);
  const b = Number.parseInt(color.slice(5, 7), 16);
  return `${r}, ${g}, ${b}`;
}

function panicFrameStyleAttr(profile) {
  const color = normalizePanicColor(profile?.panic_frame_color);
  const rgb = hexToRgbTriplet(color);
  return `style="--panic-accent:${color};--panic-accent-rgb:${rgb};"`;
}

function isPanicFrameOverlay() {
  return typeof window !== "undefined" && Boolean(window.__PANIC_FRAME_OVERLAY);
}

function panicFrameProfileId() {
  return typeof window !== "undefined" ? String(window.__PANIC_FRAME_PROFILE_ID ?? "").trim() : "";
}

function sleep(ms) {
  return new Promise((resolve) => window.setTimeout(resolve, ms));
}

function homeMetricValue(model, key) {
  return model.homeDashboard?.metrics?.find((item) => item.key === key)?.value ?? 0;
}

function setHomeMetricValue(model, key, nextValue) {
  if (!model.homeDashboard?.metrics) {
    model.homeDashboard = {
      metrics: [
        { key: "home.metric.dns_blocked", value: 0 },
        { key: "home.metric.tracker_blocked", value: 0 },
        { key: "home.metric.service_blocked", value: 0 }
      ],
      quick_actions: []
    };
  }
  const metric = model.homeDashboard.metrics.find((item) => item.key === key);
  if (metric) {
    metric.value = nextValue;
    return;
  }
  model.homeDashboard.metrics.push({ key, value: nextValue });
}

function applyHomeMetricEntry(model, entry) {
  if (!entry || entry.status !== "blocked") return false;
  const reason = String(entry.reason ?? "").toLowerCase();
  const blockedByRule = Boolean(entry.blocked_globally) || Boolean(entry.blocked_for_profile);
  let changed = false;
  if (reason.includes("dns")) {
    setHomeMetricValue(model, "home.metric.dns_blocked", homeMetricValue(model, "home.metric.dns_blocked") + 1);
    changed = true;
  }
  if (reason.includes("service")) {
    setHomeMetricValue(model, "home.metric.service_blocked", homeMetricValue(model, "home.metric.service_blocked") + 1);
    changed = true;
  }
  if (blockedByRule || reason.includes("tracker")) {
    setHomeMetricValue(model, "home.metric.tracker_blocked", homeMetricValue(model, "home.metric.tracker_blocked") + 1);
    changed = true;
  }
  return changed;
}

function panicFrameMode() {
  return typeof window !== "undefined" ? String(window.__PANIC_FRAME_MODE ?? "controls").trim() : "controls";
}

function renderPanicFloatingButton(i18n, model) {
  if (!shouldRenderPanicControls(model)) return "";
  return `
    <button id="panic-frame-trigger" class="panic-frame-trigger" title="${i18n.t("panicFrame.button")}">
      <span class="panic-frame-trigger-icon">${fireIcon()}</span>
    </button>
  `;
}

function renderPanicModal(i18n, model) {
  const profile = selectedProfile(model);
  if (!profile || !model.panicUi?.open) return "";
  const sites = [...(model.panicUi.sites ?? profile.panic_protected_sites ?? [])];
  const query = String(model.panicUi.search ?? "").trim().toLowerCase();
  const filteredSites = sites.filter((item) => !query || item.toLowerCase().includes(query));
  if (model.panicUi.mode === "config") {
    return `
      <div class="profiles-modal-overlay" id="panic-config-overlay">
        <div class="profiles-modal-window panic-config-window">
          <div class="action-modal">
            <div class="row-between">
              <h3>${i18n.t("panicFrame.sitesTitle")}</h3>
              <input id="panic-sites-search" value="${String(model.panicUi.search ?? "").replaceAll("\"", "&quot;")}" placeholder="${i18n.t("panicFrame.searchPlaceholder")}" />
            </div>
            <div class="panic-sites-list" id="panic-sites-list">
              ${filteredSites.length ? filteredSites.map((site) => `
                <div class="panic-site-row">
                  <span>${site}</span>
                  <button type="button" data-panic-site-remove="${site}">${i18n.t("action.delete")}</button>
                </div>
              `).join("") : `<p class="meta">${i18n.t("panicFrame.sitesEmpty")}</p>`}
            </div>
            <div class="grid-two">
              <input id="panic-sites-add-input" placeholder="example.com" />
              <button type="button" id="panic-sites-add">${i18n.t("panicFrame.addSite")}</button>
            </div>
            <div class="modal-actions">
              <button type="button" id="panic-sites-clear">${i18n.t("panicFrame.clearSites")}</button>
              <button type="button" id="panic-sites-cancel">${i18n.t("action.cancel")}</button>
              <button type="button" id="panic-sites-save">${i18n.t("action.save")}</button>
            </div>
          </div>
        </div>
      </div>
    `;
  }
  return `
    <div class="profiles-modal-overlay" id="panic-modal-overlay">
      <div class="profiles-modal-window panic-modal-window">
        <div class="action-modal">
          <h3>${i18n.t("panicFrame.menuTitle")}</h3>
          <p class="meta">${i18n.t("panicFrame.subtitle")}</p>
          <div class="panic-fire-hero">${fireHeroIcon()}</div>
          <h4>${i18n.t("panicFrame.cleanupTitle")}</h4>
          <p>${i18n.t("panicFrame.cleanupBody")}</p>
          <div class="panic-protected-summary">
            <div>
              <strong>${i18n.t("panicFrame.protectedSummary")}</strong>
              <p class="meta">${sites.length ? i18n.t("panicFrame.protectedSitesCount").replace("{count}", String(sites.length)) : i18n.t("panicFrame.protectedSitesNone")}</p>
            </div>
            <button type="button" id="panic-config-open">${i18n.t("panicFrame.configure")}</button>
          </div>
          ${model.panicUi.notice ? `<p class="notice ${model.panicUi.notice.type}">${model.panicUi.notice.text}</p>` : ""}
          <div class="modal-actions">
            <button type="button" id="panic-modal-cancel">${i18n.t("action.cancel")}</button>
            <button type="button" class="danger" id="panic-modal-clean">${i18n.t("panicFrame.clean")}</button>
          </div>
        </div>
      </div>
    </div>
  `;
}

function panicMenuState(model, profile) {
  return model.panicUi ?? {
    open: true,
    mode: "panel",
    sites: [...(profile?.panic_protected_sites ?? [])],
    search: "",
    notice: null
  };
}

function renderPanicMenu(i18n, model) {
  const profile = selectedProfile(model);
  if (!profile) return "";
  const panicUi = panicMenuState(model, profile);
  const sites = [...(panicUi.sites ?? profile.panic_protected_sites ?? [])];
  const query = String(panicUi.search ?? "").trim().toLowerCase();
  const filteredSites = sites.filter((item) => !query || item.toLowerCase().includes(query));
  if (panicUi.mode === "config") {
    return `
      <div class="panic-context-menu-shell">
        <div class="panic-context-menu panic-context-menu--config">
          <div class="panic-context-menu-header">
            <h3>${i18n.t("panicFrame.sitesTitle")}</h3>
            <button type="button" class="panic-context-close" id="panic-menu-close" aria-label="${i18n.t("action.cancel")}">×</button>
          </div>
          <div class="panic-context-search-row">
            <input id="panic-sites-search" value="${String(panicUi.search ?? "").replaceAll("\"", "&quot;")}" placeholder="${i18n.t("panicFrame.searchPlaceholder")}" />
          </div>
          <div class="panic-sites-list" id="panic-sites-list">
            ${filteredSites.length ? filteredSites.map((site) => `
              <div class="panic-site-row">
                <span>${site}</span>
                <button type="button" data-panic-site-remove="${site}">${i18n.t("action.delete")}</button>
              </div>
            `).join("") : `<p class="meta">${i18n.t("panicFrame.sitesEmpty")}</p>`}
          </div>
          <div class="grid-two">
            <input id="panic-sites-add-input" placeholder="example.com" />
            <button type="button" id="panic-sites-add">${i18n.t("panicFrame.addSite")}</button>
          </div>
          ${panicUi.notice ? `<p class="notice ${panicUi.notice.type}">${panicUi.notice.text}</p>` : ""}
          <div class="modal-actions panic-context-actions">
            <button type="button" id="panic-sites-clear">${i18n.t("panicFrame.clearSites")}</button>
            <button type="button" id="panic-sites-cancel">${i18n.t("action.cancel")}</button>
            <button type="button" id="panic-sites-save">${i18n.t("action.save")}</button>
          </div>
        </div>
      </div>
    `;
  }
  return `
      <div class="panic-context-menu-shell">
      <div class="panic-context-menu">
        <div class="panic-context-menu-header">
          <h3>${i18n.t("panicFrame.menuTitle")}</h3>
          <button type="button" class="panic-context-close" id="panic-menu-close" aria-label="${i18n.t("action.cancel")}">×</button>
        </div>
        <div class="panic-fire-hero">${fireHeroIcon()}</div>
        <h4>${i18n.t("panicFrame.cleanupTitle")}</h4>
        <p>${i18n.t("panicFrame.cleanupBody")}</p>
        <div class="panic-protected-summary">
          <div>
            <strong>${i18n.t("panicFrame.protectedSummary")}</strong>
            <p class="meta">${sites.length ? i18n.t("panicFrame.protectedSitesCount").replace("{count}", String(sites.length)) : i18n.t("panicFrame.protectedSitesNone")}</p>
          </div>
          <button type="button" id="panic-config-open">${i18n.t("panicFrame.configure")}</button>
        </div>
        ${panicUi.notice ? `<p class="notice ${panicUi.notice.type}">${panicUi.notice.text}</p>` : ""}
        <div class="modal-actions panic-context-actions">
          <button type="button" id="panic-modal-cancel">${i18n.t("action.cancel")}</button>
          <button type="button" class="danger" id="panic-modal-clean">${i18n.t("panicFrame.clean")}</button>
        </div>
      </div>
    </div>
  `;
}

function renderPanicOverlay(i18n, model) {
  const profile = selectedProfile(model);
  const mode = panicFrameMode();
  if (mode === "border") {
    return `
      <div class="panic-overlay-shell ${profile ? "" : "panic-overlay-shell--empty"}" ${panicFrameStyleAttr(profile)}>
        <div class="panic-overlay-border"></div>
      </div>
    `;
  }
  if (mode === "label") {
    return `
      <div class="panic-overlay-shell ${profile ? "" : "panic-overlay-shell--empty"} panic-overlay-shell--label" ${panicFrameStyleAttr(profile)}>
        <div class="panic-overlay-label-shell">
          <div class="panic-overlay-titlebar" aria-hidden="true">
            <strong>${profile?.name ?? "Cerbena"}</strong>
          </div>
        </div>
      </div>
    `;
  }
  if (mode === "menu") {
    return `
      <div class="panic-overlay-shell ${profile ? "" : "panic-overlay-shell--empty"} panic-overlay-shell--menu" ${panicFrameStyleAttr(profile)}>
        ${renderPanicMenu(i18n, model)}
      </div>
    `;
  }
  return `
    <div class="panic-overlay-shell ${profile ? "" : "panic-overlay-shell--empty"} panic-overlay-shell--controls" ${panicFrameStyleAttr(profile)}>
      <div class="panic-overlay-actions">
        <button id="panic-frame-trigger" class="panic-frame-trigger panic-frame-trigger--overlay" title="${i18n.t("panicFrame.button")}">
          <span class="panic-frame-trigger-icon">${fireIcon()}</span>
        </button>
      </div>
    </div>
  `;
}

function renderProfileLaunchOverlay(i18n, model) {
  const overlay = model.profileLaunchOverlay;
  if (!overlay) return "";
  const profile = (model.profiles ?? []).find((item) => item.id === overlay.profileId);
  const title = i18n.t("profile.launchProgress.title");
  const profileName = profile?.name ?? i18n.t("profile.launchProgress.profileFallback");
  const stageText = overlay.messageKey ? i18n.t(overlay.messageKey) : i18n.t("profile.launchProgress.starting");
  return `
    <div class="profile-launch-overlay" aria-live="polite">
      <div class="profile-launch-card">
        <div class="profile-launch-spinner" aria-hidden="true"></div>
        <div class="profile-launch-copy">
          <strong>${title}</strong>
          <span>${profileName}</span>
          <p>${stageText}</p>
        </div>
      </div>
    </div>
  `;
}

function renderApp(root, state, i18n, model) {
  if (isPanicFrameOverlay()) {
    document.body.classList.add("panic-overlay-mode");
    root.innerHTML = renderPanicOverlay(i18n, model);
    return;
  }
  document.body.classList.remove("panic-overlay-mode");
  const renderFeature = viewMap[state.currentFeature] ?? viewMap.home;
  const nav = featureRegistry.map((entry) => navItem(entry, entry.key === state.currentFeature, i18n.t)).join("");

  root.innerHTML = `
    <div id='top-frame'>
      <div id='window-controls'>
        <button class='control-btn' id='window-min' aria-label='Minimize'><svg viewBox='0 0 16 16'><path d='M3 8.5h10' stroke='currentColor' stroke-width='1.6'/></svg></button>
        <button class='control-btn' id='window-max' aria-label='Maximize'><svg viewBox='0 0 16 16'><rect x='3.5' y='3.5' width='9' height='9' fill='none' stroke='currentColor' stroke-width='1.4'/></svg></button>
        <button class='control-btn close-btn' id='window-close' aria-label='Close'><svg viewBox='0 0 16 16'><path d='M4 4l8 8M12 4l-8 8' stroke='currentColor' stroke-width='1.6'/></svg></button>
      </div>
    </div>

    <aside id='sidebar-frame'>
      <div id='sidebar'>
        <div class='sidebar-header'>
          <div class='brand-block'>
            <span class='sidebar-logo-collapsed brand-mark-shell no-select' aria-hidden='true'>${renderBrandLogo("compact")}</span>
            <span class='brand-logo brand-mark-shell no-select' aria-hidden='true'>${renderBrandLogo("full")}</span>
            <div class='brand-text'>
              <span class='brand-title'>Cerbena</span>
              <span class='brand-subtitle'>Controlled Environments with Routed Browsing &amp; Enforced Network Access</span>
            </div>
          </div>
          <button class='sidebar-collapse-btn' id='sidebar-toggle'><span>❯</span></button>
        </div>
        <div class='sidebar-content'>
          <nav class='sidebar-nav'>${nav}</nav>
          <div class='sidebar-footer'>
            <select id='locale-select' class='sidebar-lang-select'>
              <option value='en' ${i18n.getLocale()==='en'?'selected':''}>EN</option>
              <option value='ru' ${i18n.getLocale()==='ru'?'selected':''}>RU</option>
            </select>
            <div class='app-version'>v${APP_VERSION}</div>
          </div>
        </div>
      </div>
    </aside>

    <main id='content'>
      <div id='content-area'>
        <section class='content-pane'>${renderFeature(i18n.t, model)}</section>
      </div>
    </main>
    ${renderLinkLaunchModal(i18n.t, model)}
    ${renderPanicModal(i18n, model)}
    ${renderProfileLaunchOverlay(i18n, model)}
  `;

  document.body.classList.toggle("sidebar-collapsed", state.sidebarCollapsed);
}

function getListen() {
  return window.__TAURI__?.event?.listen ?? null;
}

function applyResponsiveState(state) {
  state.sidebarCollapsed = window.innerWidth < COLLAPSE_BREAKPOINT;
}

async function hydrateOverlayModel(model) {
  await hydrateProfilesModel(model);
  const profileId = panicFrameProfileId();
  if (profileId) {
    model.selectedProfileId = profileId;
  } else if (!model.selectedProfileId && model.profiles[0]) {
    model.selectedProfileId = model.profiles[0].id;
  }
}

function wirePanicInteractions(root, model, rerender, i18n, state) {
  const mode = panicFrameMode();
  if (mode === "border" || mode === "label") {
    return;
  }
  if (mode === "controls") {
    root.querySelector("#panic-frame-trigger")?.addEventListener("click", async () => {
      const profile = selectedProfile(model);
      if (!profile) return;
      await callCommand("panic_frame_show_menu", {
        request: { profileId: profile.id }
      });
    });
    return;
  }
  const closePanicMenu = async () => {
    const profile = selectedProfile(model);
    if (!profile) return;
    model.panicUi = null;
    await callCommand("panic_frame_hide_menu", {
      request: { profileId: profile.id }
    });
  };
  const applyPanicLaunchNotice = (type, text) => {
    if (state.currentFeature === "home") {
      model.homeNotice = { type, text };
      return;
    }
    model.profileNotice = { type, text };
  };
  if (mode === "menu" && !model.panicUi) {
    const profile = selectedProfile(model);
    model.panicUi = panicMenuState(model, profile);
  }
  if (mode === "menu") {
    root.querySelector("#panic-menu-close")?.addEventListener("click", closePanicMenu);
  }
  root.querySelector("#panic-frame-trigger")?.addEventListener("click", async () => {
    const profile = selectedProfile(model);
    if (!profile) return;
    model.panicUi = {
      open: true,
      mode: "panel",
      sites: [...(profile.panic_protected_sites ?? [])],
      search: "",
      notice: null
    };
    await rerender();
  });

  root.querySelector("#panic-modal-cancel")?.addEventListener("click", async () => {
    if (mode === "menu") {
      await closePanicMenu();
      return;
    }
    model.panicUi = null;
    await rerender();
  });

  root.querySelector("#panic-config-open")?.addEventListener("click", async () => {
    model.panicUi = {
      ...(model.panicUi ?? { sites: [] }),
      open: true,
      mode: "config",
      search: ""
    };
    await rerender();
  });

  root.querySelector("#panic-modal-clean")?.addEventListener("click", async () => {
    const profile = selectedProfile(model);
    if (!profile) return;
    const result = await panicWipeProfile({
      profileId: profile.id,
      mode: "full",
      retainPaths: [],
      confirmPhrase: "ERASE_NOW"
    });
    model.panicUi = null;
    applyPanicLaunchNotice(result.ok ? "success" : "error", result.ok ? i18n.t("home.panicDone") : String(result.data.error));
    if (result.ok) {
      await sleep(1200);
      const relaunchResult = await launchProfile(profile.id);
      if (!relaunchResult.ok) {
        applyPanicLaunchNotice("error", String(relaunchResult.data.error));
      }
    }
    if (mode === "menu") {
      await closePanicMenu();
      await rerender();
      return;
    }
    await rerender();
  });

  root.querySelector("#panic-sites-cancel")?.addEventListener("click", async () => {
    model.panicUi = {
      ...(model.panicUi ?? {}),
      open: true,
      mode: "panel",
      search: "",
      notice: null
    };
    await rerender();
  });

  root.querySelector("#panic-sites-search")?.addEventListener("input", async (event) => {
    if (!model.panicUi) return;
    model.panicUi.search = event.target.value;
    await rerender();
  });

  root.querySelector("#panic-sites-add")?.addEventListener("click", async () => {
    const input = root.querySelector("#panic-sites-add-input");
    const value = String(input?.value ?? "").trim().toLowerCase();
    if (!value || !/^[a-z0-9.-]+$/i.test(value)) {
      return;
    }
    if (!model.panicUi) return;
    const next = new Set(model.panicUi.sites ?? []);
    next.add(value);
    model.panicUi.sites = [...next].sort();
    model.panicUi.search = "";
    await rerender();
  });

  root.querySelector("#panic-sites-clear")?.addEventListener("click", async () => {
    if (!model.panicUi) return;
    model.panicUi.sites = [];
    model.panicUi.search = "";
    await rerender();
  });

  for (const button of root.querySelectorAll("[data-panic-site-remove]")) {
    button.addEventListener("click", async () => {
      if (!model.panicUi) return;
      const target = button.getAttribute("data-panic-site-remove");
      model.panicUi.sites = (model.panicUi.sites ?? []).filter((item) => item !== target);
      await rerender();
    });
  }

  root.querySelector("#panic-sites-save")?.addEventListener("click", async () => {
    const profile = selectedProfile(model);
    if (!profile || !model.panicUi) return;
    const result = await updateProfile({
      profileId: profile.id,
      panicProtectedSites: model.panicUi.sites ?? [],
      expectedUpdatedAt: profile.updated_at
    });
    if (!result.ok) {
      model.panicUi.notice = { type: "error", text: String(result.data.error) };
      await rerender();
      return;
    }
    model.panicUi = {
      open: true,
      mode: "panel",
      sites: [...(model.panicUi.sites ?? [])],
      search: "",
      notice: { type: "success", text: i18n.t("panicFrame.saved") }
    };
    await rerender();
  });
}

async function bootstrap() {
  const root = document.getElementById("app");
  if (!root) throw new Error("Root element #app not found");

  log.info("bootstrap start", `url=${window.location.href}`);

  const dictionaries = await loadDictionaries();
  const i18n = createI18n(dictionaries, pickLocale());
  const teardownEngineDownloads = await initEngineDownloadNotifications(i18n);
  const teardownDnsBlocklists = await initDnsBlocklistNotifications(i18n);
  const state = createUiState();
  const model = {
    profiles: [],
    selectedProfileId: null,
    identityDraft: null,
    identityPreview: null,
    notice: null,
    networkDraft: null,
    networkNotice: null,
    networkTemplates: null,
    networkTemplateDraft: null,
    networkGlobalRoute: null,
    networkNodeTestState: {},
    networkPingState: {},
    networkPingPoller: null,
    networkPingInFlight: false,
    networkLastPingAt: 0,
    networkLoaded: false,
    serviceCatalog: null,
    dnsNotice: null,
    extensionState: null,
    extensionLibraryState: null,
    extensionNotice: null,
    trafficState: null,
    trafficNotice: null,
    trafficPoller: null,
    homeMetricsRenderTimer: null,
    syncOverview: null,
    syncNotice: null,
    homeDashboard: null,
    homeNotice: null,
    profileNotice: null,
    identityNotice: null,
    securityNotice: null,
    settingsNotice: null,
    settingsProvider: "duckduckgo",
    linkRoutingOverview: null,
    linkLaunchModal: null,
    panicUi: null
  };

  state.locale = i18n.getLocale();
  applyResponsiveState(state);
  if (isPanicFrameOverlay()) {
    await hydrateOverlayModel(model);
  } else {
    await hydrateProfilesModel(model);
    if (!model.selectedProfileId && model.profiles[0]) model.selectedProfileId = model.profiles[0].id;
  }

  const bus = createEventBus();
  const rerender = async () => {
    if (isPanicFrameOverlay()) {
      await hydrateOverlayModel(model);
    } else {
      await hydrateProfilesModel(model);
      if (state.currentFeature === "network" || state.currentFeature === "dns") await hydrateNetworkModel(model);
      if (state.currentFeature === "dns") await hydrateDnsModel(model);
      if (state.currentFeature === "extensions") await hydrateExtensionsModel(model);
      if (state.currentFeature === "traffic") await hydrateTrafficModel(model);
      if (state.currentFeature === "home") await hydrateHomeModel(model);
      if (state.currentFeature === "settings") await hydrateSettingsModel(model);
    }
    renderApp(root, state, i18n, model);
    wire(root, bus, state, model, rerender, i18n);
  };

  bus.on("feature:selected", async (featureKey) => {
    if (featureKey !== "network" && model.networkPingPoller) {
      clearInterval(model.networkPingPoller);
      model.networkPingPoller = null;
    }
    if (featureKey !== "traffic" && model.trafficPoller) {
      clearInterval(model.trafficPoller);
      model.trafficPoller = null;
    }
    state.currentFeature = featureKey;
    persistSelectedFeature(featureKey);
    await rerender();
  });

  bus.on("locale:set", async (locale) => {
    i18n.setLocale(locale);
    state.locale = locale;
    await rerender();
  });

  bus.on("sidebar:toggled", async () => {
    state.sidebarCollapsed = !state.sidebarCollapsed;
    await rerender();
  });

  bus.on("window:resized", async () => {
    applyResponsiveState(state);
    await rerender();
  });

  await rerender();
  if (!isPanicFrameOverlay()) {
    await consumePendingLinkLaunch(model, rerender, i18n.t);
  }
  const listen = getListen();
  let unlistenProfileState = null;
  let unlistenProfileLaunchProgress = null;
  if (listen) {
    unlistenProfileState = await listen("profile-state-changed", async (event) => {
      const payload = event.payload ?? {};
      const profile = model.profiles.find((item) => item.id === payload.profileId);
      if (profile) {
        profile.state = payload.state;
      }
      if (payload.state === "running" && model.profileLaunchOverlay?.profileId === payload.profileId) {
        window.setTimeout(async () => {
          if (model.profileLaunchOverlay?.profileId === payload.profileId) {
            model.profileLaunchOverlay = null;
            await rerender();
          }
        }, 900);
      }
      if (isPanicFrameOverlay() || state.currentFeature === "home" || model.profileLaunchOverlay) {
        await rerender();
      }
    });
    unlistenProfileLaunchProgress = await listen("profile-launch-progress", async (event) => {
      if (isPanicFrameOverlay()) return;
      const payload = event.payload ?? {};
      model.profileLaunchOverlay = {
        profileId: payload.profileId,
        stageKey: payload.stageKey ?? null,
        messageKey: payload.messageKey ?? "profile.launchProgress.starting",
        done: Boolean(payload.done)
      };
      if (payload.done) {
        window.setTimeout(async () => {
          if (model.profileLaunchOverlay?.profileId === payload.profileId) {
            model.profileLaunchOverlay = null;
            await rerender();
          }
        }, 900);
      }
      await rerender();
    });
    await listen("traffic-gateway-event", async (event) => {
      if (isPanicFrameOverlay()) return;
      if (!applyHomeMetricEntry(model, event.payload ?? {})) return;
      if (state.currentFeature !== "home") return;
      if (model.homeMetricsRenderTimer) {
        clearTimeout(model.homeMetricsRenderTimer);
      }
      model.homeMetricsRenderTimer = window.setTimeout(async () => {
        model.homeMetricsRenderTimer = null;
        renderApp(root, state, i18n, model);
        wire(root, bus, state, model, rerender, i18n);
      }, HOME_METRICS_RENDER_DEBOUNCE_MS);
    });
  }
  if (!isPanicFrameOverlay() && window.__TAURI__?.core?.invoke) {
    ensureEngineBinaries().catch((error) => {
      log.error("engine bootstrap failed", String(error));
    });
  }
  window.addEventListener("resize", () => bus.emit("window:resized"));
  window.addEventListener("beforeunload", () => {
    teardownEngineDownloads?.();
    teardownDnsBlocklists?.();
    try {
      if (model.networkPingPoller) {
        clearInterval(model.networkPingPoller);
        model.networkPingPoller = null;
      }
      if (model.homeMetricsRenderTimer) {
        clearTimeout(model.homeMetricsRenderTimer);
        model.homeMetricsRenderTimer = null;
      }
      unlistenProfileState?.();
      unlistenProfileLaunchProgress?.();
    } catch {}
  }, { once: true });
  window.addEventListener("contextmenu", (event) => event.preventDefault());
  log.info("bootstrap complete");
}

function wire(root, bus, state, model, rerender, i18n) {
  if (isPanicFrameOverlay()) {
    wirePanicInteractions(root, model, rerender, i18n, state);
    return;
  }
  for (const button of root.querySelectorAll("[data-feature]")) {
    button.addEventListener("click", () => bus.emit("feature:selected", button.getAttribute("data-feature")));
  }

  root.querySelector("#sidebar-toggle")?.addEventListener("click", () => bus.emit("sidebar:toggled"));
  root.querySelector("#locale-select")?.addEventListener("change", (e) => bus.emit("locale:set", e.target.value));

  root.querySelector("#window-min")?.addEventListener("click", async () => {
    try {
      await minimizeWindow();
    } catch (error) {
      log.error("window minimize failed", String(error));
    }
  });
  root.querySelector("#window-max")?.addEventListener("click", async () => {
    try {
      await toggleMaximizeWindow();
    } catch (error) {
      log.error("window maximize toggle failed", String(error));
    }
  });
  root.querySelector("#window-close")?.addEventListener("click", async () => {
    try {
      await closeWindow();
    } catch (error) {
      log.error("window close failed", String(error));
    }
  });
  wirePanicInteractions(root, model, rerender, i18n, state);

  const t = i18n.t;
  if (state.currentFeature === "identity") wireIdentity(root, model, rerender, t);
  if (state.currentFeature === "network") wireNetwork(root, model, rerender, t);
  if (state.currentFeature === "traffic") wireTraffic(root, model, rerender, t);
  if (state.currentFeature === "dns") wireDns(root, model, rerender, t);
  if (state.currentFeature === "extensions") wireExtensions(root, model, rerender, t);
  if (state.currentFeature === "home") {
    wireHome(root, model, rerender, t);
    wireProfiles(root, model, rerender, t);
  }
  if (state.currentFeature === "security") wireSecurity(root, model, rerender, t);
  if (state.currentFeature === "settings") wireSettings(root, model, rerender, t);
  if (model.linkLaunchModal) wireLinkLaunchModal(document.body, model, rerender, t);
}

function renderFatalError(error) {
  const root = document.getElementById("app");
  if (!root) return;
  const message = error instanceof Error ? `${error.name}: ${error.message}` : String(error);
  const stack = error instanceof Error && error.stack ? error.stack : "no stack";
  root.innerHTML = `
    <div style='padding:16px;color:#fff;background:#111;font-family:Consolas,monospace;'>
      <h2 style='margin:0 0 12px;'>Cerbena UI Fatal Error</h2>
      <p style='margin:0 0 12px;white-space:pre-wrap;'>${message}</p>
      <pre style='white-space:pre-wrap;opacity:.9;'>${stack}</pre>
    </div>
  `;
}

window.addEventListener("error", (event) => renderFatalError(event.error ?? event.message));
window.addEventListener("unhandledrejection", (event) => renderFatalError(event.reason));
bootstrap().catch((error) => renderFatalError(error));

