import { homeState, logsState, networkState, profilesState, settingsState, trafficState } from "./feature-state-access.js";

export function registerShellBusHandlers({
  bus,
  state,
  model,
  i18n,
  persistSelectedFeature,
  applyResponsiveState,
  refreshBoundaries
}) {
  bus.on("feature:selected", async (featureKey) => {
    const network = networkState(model);
    const traffic = trafficState(model);
    if (featureKey !== "network" && network.networkPingPoller) {
      clearInterval(network.networkPingPoller);
      network.networkPingPoller = null;
    }
    if (featureKey !== "traffic" && traffic.trafficPoller) {
      clearInterval(traffic.trafficPoller);
      traffic.trafficPoller = null;
    }
    state.currentFeature = featureKey;
    persistSelectedFeature(featureKey);
    await refreshBoundaries.featureSelected();
  });

  bus.on("locale:set", async (locale) => {
    i18n.setLocale(locale);
    state.locale = locale;
    await refreshBoundaries.shell();
  });

  bus.on("sidebar:toggled", async () => {
    state.sidebarCollapsed = !state.sidebarCollapsed;
    await refreshBoundaries.shell();
  });

  bus.on("window:resized", async () => {
    applyResponsiveState(state);
    await refreshBoundaries.shell();
  });
}

export async function registerShellRuntimeListeners({
  listen,
  state,
  model,
  rerender,
  isPanicFrameOverlay,
  applyHomeMetricEntry,
  renderApp,
  rewire,
  root,
  i18n,
  handleExternalLinkRequest,
  HOME_METRICS_RENDER_DEBOUNCE_MS,
  refreshBoundaries
}) {
  if (!listen) return {};
  const unlistenProfileState = await listen("profile-state-changed", async (event) => {
    const profiles = profilesState(model);
    const payload = event.payload ?? {};
    const profile = profiles.profiles.find((item) => item.id === payload.profileId);
    if (profile) profile.state = payload.state;
    if (payload.state === "running" && profiles.profileLaunchOverlay?.profileId === payload.profileId) {
      window.setTimeout(async () => {
        if (profiles.profileLaunchOverlay?.profileId === payload.profileId) {
          profiles.profileLaunchOverlay = null;
          await refreshBoundaries.profiles();
        }
      }, 900);
    }
    if (isPanicFrameOverlay() || state.currentFeature === "home" || profiles.profileLaunchOverlay) {
      await refreshBoundaries.panic();
    }
  });
  const unlistenProfileLaunchProgress = await listen("profile-launch-progress", async (event) => {
    const profiles = profilesState(model);
    if (isPanicFrameOverlay()) return;
    const payload = event.payload ?? {};
    profiles.profileLaunchOverlay = {
      profileId: payload.profileId,
      stageKey: payload.stageKey ?? null,
      messageKey: payload.messageKey ?? "profile.launchProgress.starting",
      done: Boolean(payload.done)
    };
    if (payload.done) {
      window.setTimeout(async () => {
        if (profiles.profileLaunchOverlay?.profileId === payload.profileId) {
          profiles.profileLaunchOverlay = null;
          await refreshBoundaries.profiles();
        }
      }, 900);
    }
    await refreshBoundaries.panic();
  });
  const unlistenAppLifecycleProgress = await listen("app-lifecycle-progress", async (event) => {
    if (isPanicFrameOverlay()) return;
    const payload = event.payload ?? {};
    if (payload.phase !== "shutdown") return;
    model.appLifecycleOverlay = {
      phase: "shutdown",
      titleKey: "app.lifecycle.shutdown.title",
      subtitleKey: "app.lifecycle.shutdown.subtitle",
      messageKey: payload.messageKey ?? "app.lifecycle.shutdown.handoff"
    };
    await refreshBoundaries.shell();
  });
  const unlistenCloseRequested = await listen("app-close-requested", async () => {
    settingsState(model).trayClosePromptModal = { open: true };
    await refreshBoundaries.shell();
  });
  await listen("external-link-received", async (event) => {
    if (isPanicFrameOverlay()) return;
    const payload = event.payload ?? {};
    const url = typeof payload === "string" ? payload : String(payload.url ?? "").trim();
    if (!url) return;
    await handleExternalLinkRequest(model, url, rerender, i18n.t);
  });
  await listen("traffic-gateway-event", async (event) => {
    const home = homeState(model);
    if (isPanicFrameOverlay()) return;
    if (!applyHomeMetricEntry(model, event.payload ?? {})) return;
    if (state.currentFeature !== "home") return;
    if (home.homeMetricsRenderTimer) clearTimeout(home.homeMetricsRenderTimer);
    home.homeMetricsRenderTimer = window.setTimeout(async () => {
      home.homeMetricsRenderTimer = null;
      renderApp(root, state, i18n, model);
      rewire();
    }, HOME_METRICS_RENDER_DEBOUNCE_MS);
  });
  const unlistenRuntimeLogs = await listen("runtime-log-appended", async (event) => {
    const logs = logsState(model);
    if (isPanicFrameOverlay()) return;
    const line = typeof event.payload === "string" ? event.payload : String(event.payload ?? "").trim();
    if (!line) return;
    logs.runtimeLogs = [...(logs.runtimeLogs ?? []), line].slice(-1000);
    if (state.currentFeature !== "logs") return;
    renderApp(root, state, i18n, model);
    rewire();
  });
  return {
    unlistenProfileState,
    unlistenProfileLaunchProgress,
    unlistenAppLifecycleProgress,
    unlistenCloseRequested,
    unlistenRuntimeLogs
  };
}
