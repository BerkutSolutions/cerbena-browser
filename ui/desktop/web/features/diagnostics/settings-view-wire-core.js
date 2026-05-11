import { registerSettingsLinkSyncHandlers } from "./settings-view-wire-core-links-sync.js";

export function wireSettingsImpl(root, model, rerender, t, deps) {
  const {
    ensureSettingsModel,
    ensureGlobalSecurityState,
    SEARCH_PROVIDER_PRESETS,
    importSearchProviders,
    setDefaultSearchProvider,
    saveGlobalSecuritySettings,
    buildGlobalSecuritySaveRequest,
    getRuntimeToolsStatus,
    installRuntimeTool,
    openExternalUrl,
    showLinuxSandboxGuideModal,
    showLinuxDockerGuideModal,
    refreshDevicePostureReport,
    setLauncherAutoUpdate,
    saveShellPreferences,
    checkLauncherUpdates,
    launchUpdaterPreview,
    buildReleaseUrl,
    openDefaultAppsSettings,
    setDefaultProfileForLinks,
    clearDefaultProfileForLinks,
    handleExternalLinkRequest,
    saveLinkTypeProfileBinding,
    removeLinkTypeProfileBinding,
    syncProfileId,
    saveSyncControls,
    syncHealthPing,
    createBackupSnapshot,
    restoreSnapshot,
    hydrateSettingsModel
  } = deps;


  const state = ensureSettingsModel(model);
  const securityState = ensureGlobalSecurityState(model);

  for (const button of root.querySelectorAll("[data-settings-tab]")) {
    button.addEventListener("click", async () => {
      state.activeTab = button.getAttribute("data-settings-tab");
      await rerender();
    });
  }

  root.querySelector("#settings-links-browser-save")?.addEventListener("click", async () => {
    const providerId = root.querySelector("#settings-search-provider")?.value ?? "duckduckgo";
    const preset = SEARCH_PROVIDER_PRESETS.find((item) => item.id === providerId) ?? SEARCH_PROVIDER_PRESETS[0];
    await importSearchProviders([
      {
        id: preset.id,
        display_name: preset.name,
        query_template: preset.template
      }
    ]);
    const providerResult = await setDefaultSearchProvider(providerId);
    securityState.startupPage = root.querySelector("#settings-start-page")?.value?.trim() ?? "";
    const settingsResult = await saveGlobalSecuritySettings(buildGlobalSecuritySaveRequest(securityState));
    model.settingsProvider = providerId;
    model.settingsNotice = {
      type: providerResult.ok && settingsResult.ok ? "success" : "error",
      text: providerResult.ok && settingsResult.ok ? t("settings.saved") : String((!providerResult.ok ? providerResult.data.error : settingsResult.data.error))
    };
    await rerender();
  });

  root.querySelector("#settings-runtime-tools-refresh")?.addEventListener("click", async () => {
    const runtimeTools = await getRuntimeToolsStatus();
    model.runtimeToolsStatus = runtimeTools.ok ? runtimeTools.data : model.runtimeToolsStatus;
    model.settingsNotice = {
      type: runtimeTools.ok ? "success" : "error",
      text: runtimeTools.ok ? t("settings.tools.refreshed") : String(runtimeTools.data.error)
    };
    await rerender();
  });

  for (const button of root.querySelectorAll("[data-settings-tool-install]")) {
    button.addEventListener("click", async () => {
      const toolId = button.getAttribute("data-settings-tool-install");
      if (!toolId) return;
      const result = await installRuntimeTool(toolId);
      model.settingsNotice = {
        type: result.ok ? "success" : "error",
        text: result.ok ? t("settings.tools.installed") : String(result.data.error)
      };
      await hydrateSettingsModel(model);
      await rerender();
    });
  }

  for (const button of root.querySelectorAll("[data-settings-tool-external]")) {
    button.addEventListener("click", async () => {
      const toolId = button.getAttribute("data-settings-tool-external");
      if (toolId !== "docker") return;
      const result = await openExternalUrl("https://www.docker.com/products/docker-desktop/");
      model.settingsNotice = {
        type: result.ok ? "success" : "error",
        text: result.ok ? t("settings.tools.externalOpened") : String(result.data.error)
      };
      await rerender();
    });
  }

  for (const button of root.querySelectorAll("[data-settings-tool-guide]")) {
    button.addEventListener("click", async () => {
      const toolId = button.getAttribute("data-settings-tool-guide");
      if (toolId === "linux-browser-sandbox") {
        await showLinuxSandboxGuideModal(t);
        return;
      }
      if (toolId === "docker") {
        await showLinuxDockerGuideModal(t);
      }
    });
  }

  root.querySelector("#settings-posture-refresh")?.addEventListener("click", async () => {
    const posture = await refreshDevicePostureReport();
    model.devicePostureReport = posture.ok ? posture.data : model.devicePostureReport;
    model.settingsNotice = {
      type: posture.ok ? "success" : "error",
      text: posture.ok ? t("devicePosture.refreshed") : String(posture.data.error)
    };
    await rerender();
  });

  root.querySelector("#settings-update-auto")?.addEventListener("change", async (event) => {
    const result = await setLauncherAutoUpdate(Boolean(event.target.checked));
    model.launcherUpdateState = result.ok ? result.data : model.launcherUpdateState;
    model.settingsNotice = {
      type: result.ok ? "success" : "error",
      text: result.ok ? t("settings.updates.saved") : String(result.data.error)
    };
    await rerender();
  });

  root.querySelector("#settings-tray-minimize")?.addEventListener("change", async (event) => {
    const enabled = Boolean(event.target.checked);
    const result = await saveShellPreferences({
      minimizeToTrayEnabled: enabled,
      closeToTrayPromptDeclined: enabled ? false : undefined
    });
    model.shellPreferencesState = result.ok ? result.data : model.shellPreferencesState;
    model.settingsNotice = {
      type: result.ok ? "success" : "error",
      text: result.ok ? t("settings.tray.saved") : String(result.data.error)
    };
    await rerender();
  });

  root.querySelector("#settings-autostart-enabled")?.addEventListener("change", async (event) => {
    const enabled = Boolean(event.target.checked);
    const result = await saveShellPreferences({
      launchOnSystemStartup: enabled,
      startupProfileId: enabled ? (state.startupProfileDraft || model.shellPreferencesState?.startupProfileId || null) : null
    });
    model.shellPreferencesState = result.ok ? result.data : model.shellPreferencesState;
    if (!enabled) {
      state.startupProfileDraft = "";
    } else if (!state.startupProfileDraft) {
      state.startupProfileDraft = result.ok ? (result.data?.startupProfileId ?? "") : "";
    }
    model.settingsNotice = {
      type: result.ok ? "success" : "error",
      text: result.ok ? t("settings.autostart.saved") : String(result.data.error)
    };
    await rerender();
  });

  root.querySelector("#settings-startup-profile-toggle")?.addEventListener("click", () => {
    root.querySelector("#settings-startup-profile-menu")?.classList.toggle("hidden");
  });

  for (const checkbox of root.querySelectorAll("[data-settings-startup-profile]")) {
    checkbox.addEventListener("change", async () => {
      const profileId = checkbox.getAttribute("data-settings-startup-profile");
      state.startupProfileDraft = checkbox.checked ? profileId : "";
      for (const other of root.querySelectorAll("[data-settings-startup-profile]")) {
        if (other !== checkbox) {
          other.checked = false;
        }
      }
      const result = await saveShellPreferences({
        startupProfileId: state.startupProfileDraft || null
      });
      model.shellPreferencesState = result.ok ? result.data : model.shellPreferencesState;
      if (result.ok) {
        state.startupProfileDraft = result.data?.startupProfileId ?? "";
      }
      model.settingsNotice = {
        type: result.ok ? "success" : "error",
        text: result.ok ? t("settings.startupProfile.saved") : String(result.data.error)
      };
      await rerender();
    });
  }

  root.querySelector("#settings-update-check")?.addEventListener("click", async () => {
    const result = await checkLauncherUpdates(true);
    model.launcherUpdateState = result.ok ? result.data : model.launcherUpdateState;
    const status = result.ok ? String(result.data?.status ?? "") : "";
    const failed = !result.ok || status === "error";
    model.settingsNotice = {
      type: failed ? "error" : "success",
      text: failed
        ? String(result.ok ? (result.data?.lastError ?? result.data?.status ?? "update check failed") : result.data.error)
        : t("settings.updates.checked")
    };
    await rerender();
  });

  root.querySelector("#settings-update-preview")?.addEventListener("click", async () => {
    const result = await launchUpdaterPreview();
    model.settingsNotice = {
      type: result.ok ? "success" : "error",
      text: result.ok ? t("settings.updates.previewOpened") : String(result.data.error)
    };
    await rerender();
  });

  root.querySelector("#settings-update-open-release")?.addEventListener("click", async (event) => {
    event.preventDefault();
    const result = await openExternalUrl(buildReleaseUrl(model.launcherUpdateState ?? {}));
    model.settingsNotice = {
      type: result.ok ? "success" : "error",
      text: result.ok ? t("settings.updates.releaseOpened") : String(result.data.error)
    };
    await rerender();
  });

  root.querySelector("#settings-links-global-profile")?.addEventListener("change", (event) => {
    state.globalLinkProfileDraft = event.target.value;
  });

  root.querySelector("#settings-default-browser-check")?.addEventListener("change", async (event) => {
    const enabled = Boolean(event.target.checked);
    const result = await saveShellPreferences({
      checkDefaultBrowserOnStartup: enabled,
      defaultBrowserPromptDecided: true
    });
    model.shellPreferencesState = result.ok ? result.data : model.shellPreferencesState;
    model.settingsNotice = {
      type: result.ok ? "success" : "error",
      text: result.ok ? t("links.defaultBrowser.saved") : String(result.data.error)
    };
    await rerender();
  });

  root.querySelector("#settings-default-browser-open")?.addEventListener("click", async () => {
    const result = await openDefaultAppsSettings();
    model.settingsNotice = {
      type: result.ok ? "success" : "error",
      text: result.ok ? t("links.defaultBrowser.opened") : String(result.data.error)
    };
    await hydrateSettingsModel(model);
    await rerender();
  });

  registerSettingsLinkSyncHandlers({
    root, model, rerender, t, state,
    setDefaultProfileForLinks, clearDefaultProfileForLinks,
    handleExternalLinkRequest, saveLinkTypeProfileBinding, removeLinkTypeProfileBinding,
    hydrateSettingsModel, syncProfileId, saveSyncControls, syncHealthPing, createBackupSnapshot, restoreSnapshot
  });

}
