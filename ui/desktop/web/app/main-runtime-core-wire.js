export function wireMainRuntime(ctx) {
  const {
    root, bus, state, model, rerender, i18n,
    isPanicFrameOverlay, panicUi, minimizeWindow, toggleMaximizeWindow, closeWindow,
    wireIdentity, wireNetwork, wireTraffic, wireDns, wireExtensions, wireHome, wireProfiles, wireLogs, wireSecurity, wireSettings,
    wireLinkLaunchModal, wireShellModals,
    saveShellPreferences, setDefaultProfileForLinks, hideWindowToTray, confirmAppExit, hydrateSettingsModel,
    renderApp, log, refreshBoundaries
  } = ctx;

  if (isPanicFrameOverlay()) {
    panicUi.wirePanicInteractions(root, model, refreshBoundaries?.panic ?? rerender, i18n, state);
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
      model.appLifecycleOverlay = {
        phase: "shutdown",
        titleKey: "app.lifecycle.shutdown.title",
        subtitleKey: "app.lifecycle.shutdown.subtitle",
        messageKey: "app.lifecycle.shutdown.handoff"
      };
      renderApp(root, state, i18n, model);
      wireMainRuntime(ctx);
      await closeWindow();
    } catch (error) {
      log.error("window close failed", String(error));
    }
  });

  panicUi.wirePanicInteractions(root, model, refreshBoundaries?.panic ?? rerender, i18n, state);

  const t = i18n.t;
  if (state.currentFeature === "identity") wireIdentity(root, model, rerender, t);
  if (state.currentFeature === "network") wireNetwork(root, model, refreshBoundaries?.network ?? rerender, t);
  if (state.currentFeature === "traffic") wireTraffic(root, model, rerender, t);
  if (state.currentFeature === "dns") wireDns(root, model, refreshBoundaries?.network ?? rerender, t);
  if (state.currentFeature === "extensions") wireExtensions(root, model, rerender, t);
  if (state.currentFeature === "home") {
    wireHome(root, model, rerender, t);
    wireProfiles(root, model, refreshBoundaries?.profiles ?? rerender, t);
  }
  if (state.currentFeature === "logs") wireLogs(root, model, rerender, t);
  if (state.currentFeature === "security") wireSecurity(root, model, rerender, t);
  if (state.currentFeature === "settings") wireSettings(root, model, refreshBoundaries?.settings ?? rerender, t);
  if (model.linkLaunchModal) wireLinkLaunchModal(document.body, model, refreshBoundaries?.settings ?? rerender, t);

  wireShellModals(root, state, model, refreshBoundaries?.settings ?? rerender, i18n.t, {
    saveShellPreferences,
    setDefaultProfileForLinks,
    hideWindowToTray,
    confirmAppExit,
    hydrateSettingsModel
  });
}

export async function hydrateShellExperienceCore(model, getShellPreferencesState) {
  const shellState = await getShellPreferencesState();
  if (!shellState.ok) return;
  model.shellPreferencesState = shellState.data;
  if (shellState.data.shouldPromptDefaultBrowserPreference) {
    model.defaultBrowserStartupModal = { open: true };
    model.defaultLinkProfileModal = null;
    return;
  }
  if (shellState.data.shouldPromptDefaultLinkProfile) {
    model.defaultLinkProfileModal = {
      selectedProfileId: model.linkRoutingOverview?.globalProfileId ?? model.selectedProfileId ?? model.profiles?.[0]?.id ?? ""
    };
  }
}

export async function maybeLaunchSystemStartupProfileCore(model, _rerender, t, launchProfile) {
  if (model.systemStartupProfileLaunchHandled) return;
  const shellState = model.shellPreferencesState;
  const profileId = shellState?.startupProfileId ?? "";
  model.systemStartupProfileLaunchHandled = true;
  if (!shellState?.launchedFromSystemStartup || !shellState?.launchOnSystemStartup || !profileId) return;
  const result = await launchProfile(profileId);
  model.settingsNotice = {
    type: result.ok ? "success" : "error",
    text: result.ok ? t("links.notice.launched") : String(result.data.error)
  };
}
