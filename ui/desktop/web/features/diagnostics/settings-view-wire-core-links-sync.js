export function registerSettingsLinkSyncHandlers(ctx){
  const {
    root, model, rerender, t, state,
    setDefaultProfileForLinks, clearDefaultProfileForLinks,
    handleExternalLinkRequest, saveLinkTypeProfileBinding, removeLinkTypeProfileBinding,
    hydrateSettingsModel, syncProfileId, saveSyncControls, syncHealthPing, createBackupSnapshot, restoreSnapshot
  } = ctx;
  root.querySelector("#settings-links-global-apply")?.addEventListener("click", async () => {
    const profileId = state.globalLinkProfileDraft || root.querySelector("#settings-links-global-profile")?.value;
    if (!profileId) {
      model.settingsNotice = { type: "error", text: t("links.notice.profileRequired") };
      await rerender();
      return;
    }
    const result = await setDefaultProfileForLinks({ profileId });
    model.settingsNotice = {
      type: result.ok ? "success" : "error",
      text: result.ok ? t("links.notice.globalSaved") : String(result.data.error)
    };
    await hydrateSettingsModel(model);
    await rerender();
  });

  root.querySelector("#settings-links-global-clear")?.addEventListener("click", async () => {
    const result = await clearDefaultProfileForLinks();
    state.globalLinkProfileDraft = "";
    model.settingsNotice = {
      type: result.ok ? "success" : "error",
      text: result.ok ? t("links.notice.bindingCleared") : String(result.data.error)
    };
    await hydrateSettingsModel(model);
    await rerender();
  });

  root.querySelector("#settings-links-url")?.addEventListener("input", (event) => {
    state.linkTestUrl = event.target.value;
  });

  root.querySelector("#settings-links-open")?.addEventListener("click", async () => {
    const url = (root.querySelector("#settings-links-url")?.value ?? "").trim();
    if (!url) {
      model.settingsNotice = { type: "error", text: t("links.notice.urlRequired") };
      await rerender();
      return;
    }
    await handleExternalLinkRequest(model, url, rerender, t);
  });

  for (const select of root.querySelectorAll("[data-link-type-select]")) {
    select.addEventListener("change", (event) => {
      state.linkProfileDrafts[select.getAttribute("data-link-type-select")] = event.target.value;
    });
  }

  for (const button of root.querySelectorAll("[data-link-type-apply]")) {
    button.addEventListener("click", async () => {
      const linkType = button.getAttribute("data-link-type-apply");
      const select = root.querySelector(`[data-link-type-select="${linkType}"]`);
      const profileId = state.linkProfileDrafts[linkType] ?? select?.value ?? "";
      if (!profileId) {
        model.settingsNotice = { type: "error", text: t("links.notice.profileRequired") };
        await rerender();
        return;
      }
      const result = await saveLinkTypeProfileBinding({ linkType, profileId });
      model.settingsNotice = {
        type: result.ok ? "success" : "error",
        text: result.ok ? t("links.notice.typeSaved") : String(result.data.error)
      };
      await hydrateSettingsModel(model);
      await rerender();
    });
  }

  for (const button of root.querySelectorAll("[data-link-type-clear]")) {
    button.addEventListener("click", async () => {
      const linkType = button.getAttribute("data-link-type-clear");
      const result = await removeLinkTypeProfileBinding(linkType);
      delete state.linkProfileDrafts[linkType];
      model.settingsNotice = {
        type: result.ok ? "success" : "error",
        text: result.ok ? t("links.notice.bindingCleared") : String(result.data.error)
      };
      await hydrateSettingsModel(model);
      await rerender();
    });
  }

  root.querySelector("#settings-sync-profile")?.addEventListener("change", async (event) => {
    state.syncProfileId = event.target.value || null;
    await hydrateSettingsModel(model);
    await rerender();
  });

  root.querySelector("#settings-sync-save")?.addEventListener("click", async () => {
    const profileId = syncProfileId(model);
    if (!profileId) return;
    const modelPayload = {
      server: {
        server_url: root.querySelector("#settings-sync-url")?.value ?? "",
        key_id: root.querySelector("#settings-sync-key")?.value ?? "",
        sync_enabled: Boolean(root.querySelector("#settings-sync-enabled")?.checked)
      },
      status: { level: "healthy", message_key: "sync.status.healthy", last_sync_unix_ms: Date.now() },
      conflicts: model.syncOverview?.conflicts ?? [],
      can_backup: true,
      can_restore: true
    };
    const result = await saveSyncControls(profileId, modelPayload);
    model.settingsNotice = {
      type: result.ok ? "success" : "error",
      text: result.ok ? t("sync.saved") : String(result.data.error)
    };
    await hydrateSettingsModel(model);
    await rerender();
  });

  root.querySelector("#settings-sync-ping")?.addEventListener("click", async () => {
    const profileId = syncProfileId(model);
    const ping = await syncHealthPing(profileId);
    model.settingsNotice = {
      type: ping.ok ? "success" : "error",
      text: ping.ok ? t("sync.healthy") : String(ping.data.error)
    };
    await hydrateSettingsModel(model);
    await rerender();
  });

  root.querySelector("#settings-sync-backup")?.addEventListener("click", async () => {
    const profileId = syncProfileId(model);
    if (!profileId) return;
    const result = await createBackupSnapshot(profileId);
    model.settingsNotice = {
      type: result.ok ? "success" : "error",
      text: result.ok ? t("sync.backupCreated") : String(result.data.error)
    };
    await hydrateSettingsModel(model);
    await rerender();
  });

  root.querySelector("#settings-sync-restore")?.addEventListener("click", async () => {
    const profileId = syncProfileId(model);
    const latest = model.syncOverview?.snapshots?.[model.syncOverview.snapshots.length - 1];
    if (!profileId || !latest) {
      model.settingsNotice = { type: "error", text: t("sync.noSnapshots") };
      await rerender();
      return;
    }
    const request = {
      profile_id: profileId,
      snapshot_id: latest.snapshot_id,
      scope: "full",
      include_prefixes: [],
      expected_schema_version: 1
    };
    const restored = await restoreSnapshot(request);
    model.settingsNotice = {
      type: restored.ok ? "success" : "error",
      text: restored.ok ? t("sync.restored") : String(restored.data.error)
    };
    await hydrateSettingsModel(model);
    await rerender();
  });
}
