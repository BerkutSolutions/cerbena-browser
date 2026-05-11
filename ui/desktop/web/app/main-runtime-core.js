import { createEventBus } from "../core/event-bus.js";
import { createUiState, persistSelectedFeature } from "../core/state.js";
import { featureRegistry } from "../core/feature-registry.js";
import { loadDictionaries, createI18n } from "../i18n/runtime.js";
import { createDebugLogger } from "../core/debug.js";
import { initEngineDownloadNotifications } from "../core/engine-downloads.js";
import { initDnsBlocklistNotifications } from "../core/dns-blocklist-downloads.js";
import { minimizeWindow, toggleMaximizeWindow, closeWindow } from "../core/window-controls.js";
import { renderHome, hydrateHomeModel, wireHome } from "../features/home/view.js";
import {
  launchProfile,
  updateProfile
} from "../features/profiles/api.js";
import { hydrateProfilesModel, wireProfiles } from "../features/profiles/view.js";
import { panicWipeProfile } from "../features/home/api.js";
import { callCommand } from "../core/commands.js";
import { renderIdentity, wireIdentity } from "../features/identity/view.js";
import { renderNetwork, hydrateNetworkModel, wireNetwork } from "../features/network/view.js";
import { renderDns, hydrateDnsModel, wireDns } from "../features/dns/view.js";
import { renderExtensions, hydrateExtensionsModel, wireExtensions } from "../features/extensions/view.js";
import { renderTraffic, hydrateTrafficModel, wireTraffic } from "../features/traffic/view.js";
import { renderSecurity, wireSecurity } from "../features/security/view.js";
import { renderLogs, hydrateLogsModel, wireLogs } from "../features/diagnostics/logs-view.js";
import {
  renderSettings,
  hydrateSettingsModel,
  wireSettings,
  renderLinkLaunchModal,
  wireLinkLaunchModal,
  handleExternalLinkRequest,
  consumePendingLinkLaunch
} from "../features/settings/view.js";
import {
  checkLauncherUpdates,
  confirmAppExit,
  getLauncherUpdateState,
  getShellPreferencesState,
  hideWindowToTray,
  setDefaultProfileForLinks,
  saveShellPreferences
} from "../features/settings/api.js";
import { appendRuntimeLog } from "../features/diagnostics/api.js";
import { APP_VERSION } from "../core/app-version.js";
import { registerShellBusHandlers, registerShellRuntimeListeners } from "./shell-events.js";
import { createShellModel } from "./feature-state.js";
import { homeState, networkState } from "./feature-state-access.js";
import { wireShellModals } from "./main-shell-modals.js";
import { createPanicUi } from "./main-panic.js";
import { hydrateShellExperienceCore, maybeLaunchSystemStartupProfileCore, wireMainRuntime } from "./main-runtime-core-wire.js";
import { createRefreshBoundaries, hydrateCurrentFeatureModel as hydrateCurrentFeatureModelByPolicy } from "./main-runtime-refresh.js";
import {
  applyHomeMetricEntry,
  fireHeroIcon,
  fireIcon,
  isPanicFrameOverlay,
  navItem,
  panicFrameProfileId,
  panicFrameStyleAttr,
  pickLocale,
  renderAppLifecycleOverlay,
  renderDefaultBrowserStartupModal,
  renderDefaultLinkProfileModal,
  renderProfileLaunchOverlay,
  renderTrayCloseModal,
  selectedProfile,
  shouldRenderPanicControls,
  sleep
} from "./main-runtime-core-view.js";

const log = createDebugLogger("app");
const COLLAPSE_BREAKPOINT = 1200;
const DEFAULT_PANIC_FRAME_COLOR = "#ff8652";
const HOME_METRICS_RENDER_DEBOUNCE_MS = 900;
const panicFrameStyleWithDefault = (profile) => panicFrameStyleAttr(profile, DEFAULT_PANIC_FRAME_COLOR);

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
  logs: (t, model) => renderLogs(t, model),
  settings: (t, model) => renderSettings(t, model),
  identity: (t, model) => renderIdentity(t, model)
};

const panicUi = createPanicUi({
  selectedProfile,
  shouldRenderPanicControls,
  fireIcon,
  fireHeroIcon,
  panicFrameStyleAttr: panicFrameStyleWithDefault,
  callCommand,
  panicWipeProfile,
  launchProfile,
  updateProfile,
  sleep
});


function renderApp(root, state, i18n, model) {
  if (isPanicFrameOverlay()) {
    document.body.classList.add("panic-overlay-mode");
    root.innerHTML = panicUi.renderPanicOverlay(i18n, model);
    return;
  }
  document.body.classList.remove("panic-overlay-mode");
  const renderFeature = viewMap[state.currentFeature] ?? viewMap.home;
  const nav = featureRegistry.map((entry) => navItem(entry, entry.key === state.currentFeature, i18n.t, ICONS)).join("");

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
    ${renderDefaultBrowserStartupModal(i18n, model)}
    ${renderDefaultLinkProfileModal(i18n, model)}
    ${renderTrayCloseModal(i18n, model)}
    ${panicUi.renderPanicModal(i18n, model)}
    ${renderProfileLaunchOverlay(i18n, model)}
    ${renderAppLifecycleOverlay(i18n, model)}
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

function createRuntimeWire(root, bus, state, model, rerender, i18n, refreshBoundaries) {
  return wireMainRuntime({
    root,
    bus,
    state,
    model,
    rerender,
    i18n,
    isPanicFrameOverlay,
    panicUi,
    minimizeWindow,
    toggleMaximizeWindow,
    closeWindow,
    wireIdentity,
    wireNetwork,
    wireTraffic,
    wireDns,
    wireExtensions,
    wireHome,
    wireProfiles,
    wireLogs,
    wireSecurity,
    wireSettings,
    wireLinkLaunchModal,
    wireShellModals,
    saveShellPreferences,
    setDefaultProfileForLinks,
    hideWindowToTray,
    confirmAppExit,
    hydrateSettingsModel,
    renderApp,
    log,
    refreshBoundaries
  });
}

export async function bootstrap() {
  const root = document.getElementById("app");
  if (!root) throw new Error("Root element #app not found");

  log.info("bootstrap start", `url=${window.location.href}`);

  const dictionaries = await loadDictionaries();
  const i18n = createI18n(dictionaries, pickLocale());
  const teardownEngineDownloads = await initEngineDownloadNotifications(i18n);
  const teardownDnsBlocklists = await initDnsBlocklistNotifications(i18n);
  const state = createUiState();
  const model = createShellModel();

  state.locale = i18n.getLocale();
  applyResponsiveState(state);
  if (isPanicFrameOverlay()) {
    await hydrateOverlayModel(model);
  } else {
    await hydrateProfilesModel(model);
    if (!model.selectedProfileId && model.profiles[0]) model.selectedProfileId = model.profiles[0].id;
    await hydrateCurrentFeatureModel(state.currentFeature, model);
  }

  const bus = createEventBus();
  let refreshBoundaries = null;
  const rewire = () => createRuntimeWire(root, bus, state, model, rerender, i18n, refreshBoundaries);
  const rerender = async ({ refreshProfiles = true, refreshFeature = true, refreshOverlay = true } = {}) => {
    if (isPanicFrameOverlay()) {
      if (refreshOverlay) {
        await hydrateOverlayModel(model);
      }
    } else {
      if (refreshProfiles) {
        await hydrateProfilesModel(model);
      }
      if (refreshFeature) {
        await hydrateCurrentFeatureModel(state.currentFeature, model);
      }
    }
    renderApp(root, state, i18n, model);
    rewire();
  };
  refreshBoundaries = createRefreshBoundaries({ rerender, state });

  registerShellBusHandlers({
    bus,
    state,
    model,
    rerender,
    i18n,
    persistSelectedFeature,
    applyResponsiveState,
    refreshBoundaries
  });

  await rerender({ refreshProfiles: false, refreshFeature: false, refreshOverlay: false });
  if (!isPanicFrameOverlay()) {
    await hydrateShellExperienceCore(model, getShellPreferencesState);
    await maybeLaunchSystemStartupProfileCore(model, rerender, i18n.t, launchProfile);
    await consumePendingLinkLaunch(model, rerender, i18n.t);
    await rerender({ refreshProfiles: false, refreshFeature: false });
  }
  const listen = getListen();
  let unlistenProfileState = null;
  let unlistenProfileLaunchProgress = null;
  let unlistenAppLifecycleProgress = null;
  let unlistenCloseRequested = null;
  let unlistenRuntimeLogs = null;
  const runtimeListeners = await registerShellRuntimeListeners({
    listen,
    state,
    model,
    rerender,
    isPanicFrameOverlay,
    applyHomeMetricEntry,
    renderApp,
    rewire,
    root,
    bus,
    i18n,
    handleExternalLinkRequest,
    HOME_METRICS_RENDER_DEBOUNCE_MS,
    refreshBoundaries
  });
  unlistenProfileState = runtimeListeners.unlistenProfileState ?? null;
  unlistenProfileLaunchProgress = runtimeListeners.unlistenProfileLaunchProgress ?? null;
  unlistenAppLifecycleProgress = runtimeListeners.unlistenAppLifecycleProgress ?? null;
  unlistenCloseRequested = runtimeListeners.unlistenCloseRequested ?? null;
  unlistenRuntimeLogs = runtimeListeners.unlistenRuntimeLogs ?? null;
  if (!isPanicFrameOverlay() && window.__TAURI__?.core?.invoke) {
    window.setTimeout(async () => {
      try {
        const updateState = await getLauncherUpdateState();
        if (!updateState?.ok) {
          log.error("update state load failed", String(updateState?.data?.error ?? "unknown error"));
          return;
        }
        if (!updateState.data?.autoUpdateEnabled) {
          return;
        }
        await checkLauncherUpdates(false);
      } catch (error) {
        log.error("post-bootstrap update check failed", String(error));
      }
    }, 0);
  }
  window.addEventListener("resize", () => bus.emit("window:resized"));
  window.addEventListener("beforeunload", () => {
    teardownEngineDownloads?.();
      teardownDnsBlocklists?.();
      unlistenAppLifecycleProgress?.();
      unlistenCloseRequested?.();
      unlistenRuntimeLogs?.();
      try {
        const network = networkState(model);
        const home = homeState(model);
        if (network.networkPingPoller) {
        clearInterval(network.networkPingPoller);
        network.networkPingPoller = null;
      }
      if (home.homeMetricsRenderTimer) {
        clearTimeout(home.homeMetricsRenderTimer);
        home.homeMetricsRenderTimer = null;
      }
      unlistenProfileState?.();
      unlistenProfileLaunchProgress?.();
    } catch {}
  }, { once: true });
  window.addEventListener("contextmenu", (event) => event.preventDefault());
  window.addEventListener("error", async (event) => {
    const message = String(event.message ?? "unknown frontend error");
    const source = String(event.filename ?? "").trim();
    const line = Number.isFinite(event.lineno) ? `:${event.lineno}` : "";
    const column = Number.isFinite(event.colno) ? `:${event.colno}` : "";
    const location = source ? ` ${source}${line}${column}` : "";
    try {
      await appendRuntimeLog(`[frontend][error] ${message}${location}`);
    } catch {}
  });
  window.addEventListener("unhandledrejection", async (event) => {
    const reason = event.reason instanceof Error
      ? event.reason.stack || event.reason.message
      : String(event.reason ?? "unknown rejection");
    try {
      await appendRuntimeLog(`[frontend][unhandledrejection] ${reason}`);
    } catch {}
  });
  log.info("bootstrap complete");
}

async function hydrateCurrentFeatureModel(featureKey, model) {
  return hydrateCurrentFeatureModelByPolicy(featureKey, model, {
    hydrateNetworkModel,
    hydrateDnsModel,
    hydrateExtensionsModel,
    hydrateTrafficModel,
    hydrateHomeModel,
    hydrateLogsModel,
    hydrateSettingsModel
  });
}

export function renderFatalError(error) {
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
