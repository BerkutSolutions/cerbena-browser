import { settingsState } from "./feature-state-access.js";

export function wireShellModals(root, state, model, rerender, t, deps) {
  const { saveShellPreferences, setDefaultProfileForLinks, hideWindowToTray, confirmAppExit, hydrateSettingsModel } = deps;
  const settings = settingsState(model);

  root.querySelector("#default-browser-startup-no")?.addEventListener("click", async () => {
    const result = await saveShellPreferences({
      checkDefaultBrowserOnStartup: false,
      defaultBrowserPromptDecided: true
    });
    if (result.ok) {
      settings.shellPreferencesState = result.data;
      settings.defaultBrowserStartupModal = null;
      settings.defaultLinkProfileModal = null;
    }
    await rerender({ refreshProfiles: false, refreshFeature: true });
  });

  root.querySelector("#default-browser-startup-yes")?.addEventListener("click", async () => {
    const result = await saveShellPreferences({
      checkDefaultBrowserOnStartup: true,
      defaultBrowserPromptDecided: true
    });
    if (result.ok) {
      settings.shellPreferencesState = result.data;
      settings.defaultBrowserStartupModal = null;
      settings.defaultLinkProfileModal = result.data.isDefaultBrowser
        ? {
            selectedProfileId:
              model.linkRoutingOverview?.globalProfileId ??
              model.selectedProfileId ??
              model.profiles?.[0]?.id ??
              ""
          }
        : null;
    }
    await rerender({ refreshProfiles: false, refreshFeature: true });
  });

  root.querySelector("#default-link-profile-cancel")?.addEventListener("click", async () => {
    settings.defaultLinkProfileModal = null;
    await rerender({ refreshProfiles: false, refreshFeature: false });
  });

  root.querySelector("#default-link-profile-save")?.addEventListener("click", async () => {
    const profileId = root.querySelector("#default-link-profile-select")?.value ?? "";
    if (!profileId) return;
    const result = await setDefaultProfileForLinks({ profileId });
    if (result.ok) {
      settings.defaultLinkProfileModal = null;
      if (state.currentFeature === "settings") {
        await hydrateSettingsModel(model);
      }
    }
    await rerender({ refreshProfiles: false, refreshFeature: false });
  });

  root.querySelector("#tray-close-prompt-cancel")?.addEventListener("click", async () => {
    settings.trayClosePromptModal = null;
    model.appLifecycleOverlay = null;
    await rerender({ refreshProfiles: false, refreshFeature: false });
  });

  root.querySelector("#tray-close-prompt-yes")?.addEventListener("click", async () => {
    const result = await saveShellPreferences({
      minimizeToTrayEnabled: true,
      closeToTrayPromptDeclined: false
    });
    if (result.ok) {
      settings.shellPreferencesState = result.data;
    }
    settings.trayClosePromptModal = null;
    model.appLifecycleOverlay = null;
    await hideWindowToTray();
    await rerender({ refreshProfiles: false, refreshFeature: false });
  });

  root.querySelector("#tray-close-prompt-no")?.addEventListener("click", async () => {
    const result = await saveShellPreferences({
      closeToTrayPromptDeclined: true
    });
    if (result.ok) {
      settings.shellPreferencesState = result.data;
    }
    settings.trayClosePromptModal = null;
    await confirmAppExit();
  });
}
