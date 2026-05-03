import { SEARCH_PROVIDER_PRESETS } from "../../core/catalogs.js";
import { askConfirmModal } from "../../core/modal.js";
import { acknowledgeWayfernTos, launchProfile } from "../profiles/api.js";
import { saveGlobalSecuritySettings } from "../security/api.js";
import {
  buildGlobalSecuritySaveRequest,
  ensureGlobalSecurityState,
  hydrateGlobalSecurityState
} from "../security/shared.js";
import {
  checkLauncherUpdates,
  consumePendingExternalLink,
  clearDefaultProfileForLinks,
  dispatchExternalLink,
  getDevicePostureReport,
  getLauncherUpdateState,
  getLinkRoutingOverview,
  getShellPreferencesState,
  importSearchProviders,
  launchUpdaterPreview,
  openDefaultAppsSettings,
  removeLinkTypeProfileBinding,
  refreshDevicePostureReport,
  saveShellPreferences,
  saveLinkTypeProfileBinding,
  setLauncherAutoUpdate,
  setDefaultProfileForLinks,
  setDefaultSearchProvider
} from "../settings/api.js";
import {
  createBackupSnapshot,
  getSyncOverview,
  restoreSnapshot,
  saveSyncControls,
  syncHealthPing
} from "../sync/api.js";

function escapeHtml(value) {
  return String(value ?? "")
    .replaceAll("&", "&amp;")
    .replaceAll("<", "&lt;")
    .replaceAll(">", "&gt;")
    .replaceAll('"', "&quot;");
}

function ensureSettingsModel(model) {
  if (!model.settingsState) {
    model.settingsState = {
      activeTab: "general",
      linkTestUrl: "https://duckduckgo.com",
      syncProfileId: model.selectedProfileId ?? model.profiles?.[0]?.id ?? null,
      globalLinkProfileDraft: "",
      linkProfileDrafts: {}
    };
  }
  if (!model.settingsState.syncProfileId) {
    model.settingsState.syncProfileId = model.selectedProfileId ?? model.profiles?.[0]?.id ?? null;
  }
  return model.settingsState;
}

function profileName(model, profileId) {
  return model.profiles?.find((item) => item.id === profileId)?.name ?? "";
}

function syncProfileId(model) {
  return ensureSettingsModel(model).syncProfileId;
}

function linkBindingLabel(item, model, t) {
  if (item.profileId) return profileName(model, item.profileId) || item.profileId;
  if (item.usesGlobalDefault && model.linkRoutingOverview?.globalProfileId) {
    return `${profileName(model, model.linkRoutingOverview.globalProfileId) || model.linkRoutingOverview.globalProfileId} ${t("links.binding.globalDefault")}`;
  }
  return t("links.binding.none");
}

function profileOptions(model, selectedProfileId, t) {
  return [
    `<option value="">${t("links.binding.none")}</option>`,
    ...(model.profiles ?? []).map((profile) => `<option value="${profile.id}" ${profile.id === selectedProfileId ? "selected" : ""}>${escapeHtml(profile.name)}</option>`)
  ].join("");
}

function syncStatusLabel(info, t) {
  const messageKey = info?.controls?.status?.message_key ?? info?.controls?.status?.messageKey;
  if (!messageKey) return t("sync.status.unknown");
  return t(messageKey);
}

function postureStatusLabel(report, t) {
  return t(`devicePosture.status.${report?.status ?? "healthy"}`);
}

function postureReactionLabel(report, t) {
  return t(`devicePosture.reaction.${report?.reaction ?? "allow"}`);
}

function updateStatusLabel(updateState, t) {
  return t(`settings.updates.status.${updateState?.status ?? "idle"}`);
}

function pendingWayfernProfileIds(model) {
  return new Set(model.wayfernTermsStatus?.pendingProfileIds ?? []);
}

async function ensureWayfernTermsAcceptedForProfile(model, profileId, rerender, t) {
  if (!profileId || !pendingWayfernProfileIds(model).has(profileId)) {
    return true;
  }
  const accepted = await askConfirmModal(t, {
    title: t("profile.wayfernTerms.title"),
    description: t("profile.wayfernTerms.description"),
    submitLabel: t("action.confirm"),
    cancelLabel: t("action.cancel")
  });
  if (!accepted) {
    return false;
  }
  const ackResult = await acknowledgeWayfernTos(profileId);
  if (!ackResult.ok) {
    model.settingsNotice = { type: "error", text: String(ackResult.data.error) };
    await rerender();
    return false;
  }
  model.wayfernTermsStatus = { pendingProfileIds: [] };
  return true;
}

function renderUpdateCard(t, model) {
  const updateState = model.launcherUpdateState ?? {};
  const shellState = model.shellPreferencesState ?? {};
  return `
    <div class="security-frame">
      <div class="row-between">
        <div>
          <h4>${t("settings.updates.title")}</h4>
          <p class="meta">${t("settings.updates.hint")}</p>
        </div>
        <div class="top-actions">
          <button id="settings-update-preview">${t("settings.updates.preview")}</button>
          <button id="settings-update-check">${t("settings.updates.checkNow")}</button>
        </div>
      </div>
      <div class="grid-two">
        <label>${t("settings.updates.currentVersion")}
          <input value="${escapeHtml(updateState.currentVersion ?? "1.0.11")}" disabled />
        </label>
        <label>${t("settings.updates.latestVersion")}
          <input value="${escapeHtml(updateState.latestVersion ?? t("settings.updates.notChecked"))}" disabled />
        </label>
      </div>
      <label class="checkbox-inline">
        <input id="settings-update-auto" type="checkbox" ${updateState.autoUpdateEnabled ? "checked" : ""} />
        <span>${t("settings.updates.enabled")}</span>
      </label>
      <label class="checkbox-inline">
        <input id="settings-tray-minimize" type="checkbox" ${shellState.minimizeToTrayEnabled ? "checked" : ""} />
        <span>${t("settings.tray.enabled")}</span>
      </label>
      <p class="meta">${t("settings.updates.lastChecked")}: ${escapeHtml(updateState.lastCheckedAt ?? t("settings.updates.notChecked"))}</p>
      <p class="meta">${t("settings.updates.statusLabel")}: ${escapeHtml(updateStatusLabel(updateState, t))}</p>
      ${updateState.stagedVersion ? `<p class="meta">${t("settings.updates.staged")}: ${escapeHtml(updateState.stagedVersion)}</p>` : ""}
      ${updateState.releaseUrl ? `<p class="meta"><a href="${escapeHtml(updateState.releaseUrl)}" target="_blank" rel="noreferrer">${t("settings.updates.openRelease")}</a></p>` : ""}
      ${updateState.lastError ? `<p class="notice error">${escapeHtml(updateState.lastError)}</p>` : ""}
    </div>
  `;
}

function renderGeneralTab(t, model) {
  const state = ensureSettingsModel(model);
  const securityState = ensureGlobalSecurityState(model);
  const selectedProvider = model.settingsProvider ?? "duckduckgo";
  const posture = model.devicePostureReport;
  return `
    <section class="settings-tab-panel">
      <div class="settings-panel-grid">
        <div class="panel settings-card">
          <h4>${t("settings.searchProvider")}</h4>
          <p class="meta">${t("settings.searchProviderHint")}</p>
          <label>${t("settings.searchProvider")}
            <select id="settings-search-provider">
              ${SEARCH_PROVIDER_PRESETS.map((item) => `<option value="${item.id}" ${item.id === selectedProvider ? "selected" : ""}>${escapeHtml(item.name)}</option>`).join("")}
            </select>
          </label>
        </div>
        <div class="panel settings-card">
          <h4>${t("security.startPage")}</h4>
          <p class="meta">${t("settings.startPageHint")}</p>
          <label>${t("security.startPage")}
            <input id="settings-start-page" value="${escapeHtml(securityState.startupPage ?? "")}" placeholder="https://duckduckgo.com" />
          </label>
        </div>
        <div class="panel settings-card">
          <div class="row-between">
            <div>
              <h4>${t("devicePosture.title")}</h4>
              <p class="meta">${t("devicePosture.subtitle")}</p>
            </div>
            <button id="settings-posture-refresh">${t("action.refresh")}</button>
          </div>
          <div class="grid-two">
            <label>${t("devicePosture.statusLabel")}<input value="${escapeHtml(postureStatusLabel(posture, t))}" disabled /></label>
            <label>${t("devicePosture.reactionLabel")}<input value="${escapeHtml(postureReactionLabel(posture, t))}" disabled /></label>
          </div>
          <p class="meta">${posture?.checkedAtEpochMs ? `${t("devicePosture.checkedAt")}: ${new Date(Number(posture.checkedAtEpochMs)).toLocaleString()}` : t("devicePosture.notChecked")}</p>
          <ul class="settings-posture-list">
            ${(posture?.findings?.length ? posture.findings : [{ labelKey: "devicePosture.finding.none", detail: "" }]).map((item) => `<li>${escapeHtml(t(item.labelKey))}${item.detail ? `: ${escapeHtml(item.detail)}` : ""}</li>`).join("")}
          </ul>
        </div>
        ${renderUpdateCard(t, model)}
      </div>
      <div class="top-actions settings-save-row">
        <button id="settings-apply-general">${t("action.save")}</button>
      </div>
    </section>
  `;
}

function renderLinksTab(t, model) {
  const state = ensureSettingsModel(model);
  const overview = model.linkRoutingOverview ?? { globalProfileId: null, supportedTypes: [] };
  const shellState = model.shellPreferencesState ?? {};
  const globalDraft = state.globalLinkProfileDraft || overview.globalProfileId || "";
  return `
    <section class="settings-tab-panel">
      <div class="panel settings-card">
        <div class="row-between">
          <div>
            <label class="checkbox-inline">
              <input id="settings-default-browser-check" type="checkbox" ${shellState.checkDefaultBrowserOnStartup ? "checked" : ""} />
              <span>${t("links.defaultBrowser.check")}</span>
            </label>
            <p class="meta">${t(shellState.isDefaultBrowser ? "links.defaultBrowser.status.enabled" : "links.defaultBrowser.status.disabled")}</p>
          </div>
          <div class="top-actions">
            <button id="settings-default-browser-open">${t("links.defaultBrowser.open")}</button>
          </div>
        </div>
      </div>

      <div class="panel settings-card">
        <div class="row-between">
          <div>
            <h4>${t("links.global.title")}</h4>
            <p class="meta">${t("links.global.hint")}</p>
          </div>
          <div class="top-actions">
            <button id="settings-links-global-apply">${t("links.action.applyDefault")}</button>
            <button id="settings-links-global-clear">${t("links.action.clearBinding")}</button>
          </div>
        </div>
        <div class="grid-two">
          <label>${t("links.global.current")}
            <input value="${escapeHtml(overview.globalProfileId ? profileName(model, overview.globalProfileId) || overview.globalProfileId : t("links.binding.none"))}" disabled />
          </label>
          <label>${t("links.global.assign")}
            <select id="settings-links-global-profile">
              ${profileOptions(model, globalDraft, t)}
            </select>
          </label>
        </div>
      </div>

      <div class="panel settings-card">
        <div class="row-between">
          <div>
            <h4>${t("links.test.title")}</h4>
            <p class="meta">${t("links.test.hint")}</p>
          </div>
          <button id="settings-links-open">${t("links.action.open")}</button>
        </div>
        <label>${t("links.test.url")}
          <input id="settings-links-url" value="${escapeHtml(state.linkTestUrl)}" placeholder="https://example.com" />
        </label>
      </div>

      <div class="panel settings-card">
        <div class="row-between">
          <div>
            <h4>${t("links.table.title")}</h4>
            <p class="meta">${t("links.table.hint")}</p>
          </div>
        </div>
        <table class="extensions-table settings-links-table">
          <thead><tr><th>${t("links.table.type")}</th><th>${t("links.table.current")}</th><th>${t("links.table.assign")}</th><th>${t("extensions.actions")}</th></tr></thead>
          <tbody>
            ${overview.supportedTypes.map((item) => {
              const draftValue = state.linkProfileDrafts[item.linkType] ?? item.profileId ?? "";
              return `
                <tr>
                  <td>${t(item.labelKey)}</td>
                  <td>${escapeHtml(linkBindingLabel(item, model, t))}</td>
                  <td>
                    <select data-link-type-select="${item.linkType}">
                      ${profileOptions(model, draftValue, t)}
                    </select>
                  </td>
                  <td class="actions">
                    <button type="button" data-link-type-apply="${item.linkType}">${t("links.action.applyType")}</button>
                    <button type="button" data-link-type-clear="${item.linkType}">${t("links.action.clearBinding")}</button>
                  </td>
                </tr>
              `;
            }).join("")}
          </tbody>
        </table>
      </div>
    </section>
  `;
}

function renderSyncTab(t, model) {
  const state = ensureSettingsModel(model);
  const info = model.syncOverview;
  return `
    <section class="settings-tab-panel">
      <div class="panel settings-card">
        <div class="row-between">
          <div>
            <h4>${t("sync.title")}</h4>
            <p class="meta">${t("sync.subtitle")}</p>
          </div>
        </div>
        <div class="grid-two">
          <label>${t("sync.profile")}
            <select id="settings-sync-profile">
              ${(model.profiles ?? []).map((profile) => `<option value="${profile.id}" ${profile.id === state.syncProfileId ? "selected" : ""}>${escapeHtml(profile.name)}</option>`).join("")}
            </select>
          </label>
          <label>${t("sync.statusLabel")}
            <input value="${escapeHtml(syncStatusLabel(info, t))}" disabled />
          </label>
          <label>${t("sync.serverUrl")}<input id="settings-sync-url" value="${escapeHtml(info?.controls?.server?.server_url ?? "")}" /></label>
          <label>${t("sync.keyId")}<input id="settings-sync-key" value="${escapeHtml(info?.controls?.server?.key_id ?? "")}" /></label>
          <label class="checkbox-inline"><input id="settings-sync-enabled" type="checkbox" ${info?.controls?.server?.sync_enabled ? "checked" : ""}/> <span>${t("sync.enabled")}</span></label>
        </div>
      </div>

      <div class="settings-sync-stats">
        <div class="panel settings-card">
          <h4>${t("sync.conflicts")}</h4>
          <strong>${(info?.conflicts ?? []).length}</strong>
          <p class="meta">${t("sync.conflictsHint")}</p>
        </div>
        <div class="panel settings-card">
          <h4>${t("sync.snapshots")}</h4>
          <strong>${(info?.snapshots ?? []).length}</strong>
          <p class="meta">${t("sync.snapshotsHint")}</p>
        </div>
      </div>

      <div class="top-actions settings-save-row">
        <button id="settings-sync-save">${t("sync.saveConfig")}</button>
        <button id="settings-sync-ping">${t("sync.healthPing")}</button>
        <button id="settings-sync-backup">${t("sync.createBackup")}</button>
        <button id="settings-sync-restore">${t("sync.restoreLatest")}</button>
      </div>
    </section>
  `;
}

export function renderSettings(t, model) {
  const state = ensureSettingsModel(model);
  const notice = model.settingsNotice ? `<p class="notice ${model.settingsNotice.type}">${model.settingsNotice.text}</p>` : "";
  const tabs = [
    ["general", t("settings.tab.general")],
    ["links", t("settings.tab.links")],
    ["sync", t("settings.tab.sync")]
  ];
  return `
  <div id="settings-page" class="feature-page">
    <div class="settings-page-head">
      <div>
        <h2>${t("nav.settings")}</h2>
      </div>
    </div>
    ${notice}
    <div class="tabs browser-tabs">
      ${tabs.map(([key, label]) => `<button type="button" class="tab-btn ${state.activeTab === key ? "active" : ""}" data-settings-tab="${key}">${escapeHtml(label)}</button>`).join("")}
    </div>
    <div class="settings-page-body">
      <div class="${state.activeTab === "general" ? "" : "hidden"}" data-settings-pane="general">${renderGeneralTab(t, model)}</div>
      <div class="${state.activeTab === "links" ? "" : "hidden"}" data-settings-pane="links">${renderLinksTab(t, model)}</div>
      <div class="${state.activeTab === "sync" ? "" : "hidden"}" data-settings-pane="sync">${renderSyncTab(t, model)}</div>
    </div>
  </div>`;
}

export function renderLinkLaunchModal(t, model) {
  const modal = model.linkLaunchModal;
  if (!modal) return "";
  return `
    <div class="profiles-modal-overlay" id="link-launch-modal-overlay">
      <div class="profiles-modal-window profiles-modal-window-sm">
        <div class="action-modal">
          <h3>${t("links.modal.title")}</h3>
          <p class="meta">${t("links.modal.description")}</p>
          <label>${t("links.modal.type")}
            <input value="${escapeHtml(t(`links.type.${modal.linkType}`))}" disabled />
          </label>
          <label>${t("links.modal.url")}
            <input value="${escapeHtml(modal.url)}" disabled />
          </label>
          <label>${t("links.modal.profile")}
            <select id="link-launch-profile">
              ${(model.profiles ?? []).map((profile) => `<option value="${profile.id}" ${profile.id === modal.selectedProfileId ? "selected" : ""}>${escapeHtml(profile.name)}</option>`).join("")}
            </select>
          </label>
          <div class="modal-actions link-launch-modal-actions">
            <button type="button" id="link-launch-cancel">${t("action.cancel")}</button>
            <button type="button" id="link-launch-choose">${t("links.action.choose")}</button>
            <button type="button" id="link-launch-choose-global">${t("links.action.chooseDefault")}</button>
            <button type="button" id="link-launch-choose-type">${t("links.action.chooseTypeDefault")}</button>
          </div>
        </div>
      </div>
    </div>
  `;
}

export async function hydrateSettingsModel(model) {
  const state = ensureSettingsModel(model);
  await hydrateGlobalSecurityState(model);
  const links = await getLinkRoutingOverview();
  model.linkRoutingOverview = links.ok ? links.data : { globalProfileId: null, supportedTypes: [] };
  const shellState = await getShellPreferencesState();
  model.shellPreferencesState = shellState.ok ? shellState.data : null;
  const posture = await getDevicePostureReport();
  model.devicePostureReport = posture.ok ? posture.data : null;
  const updateState = await getLauncherUpdateState();
  model.launcherUpdateState = updateState.ok ? updateState.data : null;
  if (!state.globalLinkProfileDraft) {
    state.globalLinkProfileDraft = model.linkRoutingOverview.globalProfileId ?? "";
  }
  if (state.syncProfileId) {
    const syncResult = await getSyncOverview(state.syncProfileId);
    model.syncOverview = syncResult.ok ? syncResult.data : null;
  } else {
    model.syncOverview = null;
  }
}

async function launchResolvedLink(model, url, profileId, rerender, t) {
  if (!(await ensureWayfernTermsAcceptedForProfile(model, profileId, rerender, t))) {
    return;
  }
  const result = await launchProfile(profileId, url);
  model.settingsNotice = {
    type: result.ok ? "success" : "error",
    text: result.ok ? t("links.notice.launched") : String(result.data.error)
  };
}

export async function handleExternalLinkRequest(model, url, rerender, t) {
  const resolution = await dispatchExternalLink(url);
  if (!resolution.ok) {
    model.settingsNotice = { type: "error", text: String(resolution.data.error) };
    await rerender();
    return;
  }
  if (resolution.data.status === "resolved" && resolution.data.targetProfileId) {
    await launchResolvedLink(model, resolution.data.url, resolution.data.targetProfileId, rerender, t);
    await rerender();
    return;
  }
  model.linkLaunchModal = {
    url: resolution.data.url,
    linkType: resolution.data.linkType,
    selectedProfileId: resolution.data.targetProfileId ?? model.selectedProfileId ?? model.profiles?.[0]?.id ?? ""
  };
  await rerender();
}

export function wireSettings(root, model, rerender, t) {
  const state = ensureSettingsModel(model);
  const securityState = ensureGlobalSecurityState(model);

  for (const button of root.querySelectorAll("[data-settings-tab]")) {
    button.addEventListener("click", async () => {
      state.activeTab = button.getAttribute("data-settings-tab");
      await rerender();
    });
  }

  root.querySelector("#settings-apply-general")?.addEventListener("click", async () => {
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

export async function consumePendingLinkLaunch(model, rerender, t) {
  const pending = await consumePendingExternalLink();
  if (pending.ok && pending.data) {
    await handleExternalLinkRequest(model, pending.data, rerender, t);
  }
}

export function wireLinkLaunchModal(root, model, rerender, t) {
  const overlay = root.querySelector("#link-launch-modal-overlay");
  if (!overlay || !model.linkLaunchModal) return;

  const close = async () => {
    model.linkLaunchModal = null;
    await rerender();
  };

  const resolveSelection = async (mode) => {
    const modal = model.linkLaunchModal;
    const profileId = overlay.querySelector("#link-launch-profile")?.value ?? "";
    if (!profileId) {
      model.settingsNotice = { type: "error", text: t("links.notice.profileRequired") };
      await rerender();
      return;
    }
    if (!(await ensureWayfernTermsAcceptedForProfile(model, profileId, rerender, t))) {
      return;
    }
    if (mode === "global") {
      await setDefaultProfileForLinks({ profileId });
    }
    if (mode === "type") {
      await saveLinkTypeProfileBinding({ linkType: modal.linkType, profileId });
    }
    await launchResolvedLink(model, modal.url, profileId, rerender, t);
    model.linkLaunchModal = null;
    await hydrateSettingsModel(model);
    await rerender();
  };

  overlay.querySelector("#link-launch-cancel")?.addEventListener("click", close);
  overlay.addEventListener("click", async (event) => {
    if (event.target === overlay) {
      await close();
    }
  });
  overlay.querySelector("#link-launch-choose")?.addEventListener("click", async () => resolveSelection("once"));
  overlay.querySelector("#link-launch-choose-global")?.addEventListener("click", async () => resolveSelection("global"));
  overlay.querySelector("#link-launch-choose-type")?.addEventListener("click", async () => resolveSelection("type"));
}
