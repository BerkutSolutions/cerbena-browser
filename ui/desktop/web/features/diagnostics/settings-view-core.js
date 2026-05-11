import { SEARCH_PROVIDER_PRESETS } from "../../core/catalogs.js";
import { launchProfile } from "../profiles/api.js";
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
  setDefaultSearchProvider,
  getRuntimeToolsStatus,
  installRuntimeTool
} from "../settings/api.js";
import { openExternalUrl } from "../../core/commands.js";
import {
  createBackupSnapshot,
  getSyncOverview,
  restoreSnapshot,
  saveSyncControls,
  syncHealthPing
} from "../sync/api.js";
import { APP_VERSION } from "../../core/app-version.js";
import { closeModalOverlay, showModalOverlay } from "../../core/modal.js";
import {
  renderRuntimeToolsCard as renderRuntimeToolsCardSlice,
  renderUpdateCard as renderUpdateCardSlice
} from "./settings-view-updates.js";
import { renderGeneralTab, renderLinksTab, renderSyncTab } from "./settings-view-tabs.js";
import { wireSettingsImpl } from "./settings-view-wire.js";
import {
  buildReleaseUrl,
  ensureSettingsModel,
  escapeHtml,
  linuxDockerGuideModalHtml,
  linuxSandboxGuideModalHtml,
  linkBindingLabel,
  postureReactionLabel,
  postureStatusLabel,
  profileName,
  profileOptions,
  runtimeToolStatusLabel,
  startupProfileLabel,
  startupProfileMenu,
  syncProfileId,
  syncStatusLabel,
  updateAssetTypeLabel,
  updateHandoffModeLabel,
  updateSelectionReasonLabel,
  updateStatusLabel
} from "./settings-view-core-support.js";

function renderUpdateCard(t, model) {
  return renderUpdateCardSlice(t, model, {
    ensureSettingsModel,
    startupProfileLabel,
    startupProfileMenu,
    updateStatusLabel,
    updateAssetTypeLabel,
    updateHandoffModeLabel,
    updateSelectionReasonLabel,
    buildReleaseUrl: (updateState) => buildReleaseUrl(updateState, APP_VERSION),
    APP_VERSION,
    escapeHtml
  });
}

function renderRuntimeToolsCard(t, model) {
  return renderRuntimeToolsCardSlice(t, model, {
    escapeHtml,
    runtimeToolStatusLabel
  });
}

async function showLinuxSandboxGuideModal(t) {
  const overlay = showModalOverlay(linuxSandboxGuideModalHtml(t));
  overlay.querySelector("#linux-sandbox-guide-cancel")?.addEventListener("click", () => closeModalOverlay(overlay));
  overlay.querySelector("#linux-sandbox-guide-open")?.addEventListener("click", async () => {
    await openExternalUrl("https://docs.docker.com/engine/security/userns-remap/");
    closeModalOverlay(overlay);
  });
}

async function showLinuxDockerGuideModal(t) {
  const overlay = showModalOverlay(linuxDockerGuideModalHtml(t));
  overlay.querySelector("#linux-docker-guide-cancel")?.addEventListener("click", () => closeModalOverlay(overlay));
  overlay.querySelector("#linux-docker-guide-open")?.addEventListener("click", async () => {
    await openExternalUrl("https://docs.docker.com/engine/install/");
    closeModalOverlay(overlay);
  });
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
      <div class="${state.activeTab === "general" ? "" : "hidden"}" data-settings-pane="general">${renderGeneralTab(t, model, { escapeHtml, postureStatusLabel, postureReactionLabel, renderRuntimeToolsCardSlice, renderUpdateCardSlice, ensureSettingsModel, startupProfileLabel, startupProfileMenu })}</div>
      <div class="${state.activeTab === "links" ? "" : "hidden"}" data-settings-pane="links">${renderLinksTab(t, model, { ensureSettingsModel, ensureGlobalSecurityState, SEARCH_PROVIDER_PRESETS, escapeHtml, profileName, profileOptions, linkBindingLabel })}</div>
      <div class="${state.activeTab === "sync" ? "" : "hidden"}" data-settings-pane="sync">${renderSyncTab(t, model, { ensureSettingsModel, escapeHtml, syncProfileId, syncStatusLabel })}</div>
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
  if (!state.startupProfileDraft) {
    state.startupProfileDraft = model.shellPreferencesState?.startupProfileId ?? "";
  }
  const posture = await getDevicePostureReport();
  model.devicePostureReport = posture.ok ? posture.data : null;
  const updateState = await getLauncherUpdateState();
  model.launcherUpdateState = updateState.ok ? updateState.data : null;
  const runtimeTools = await getRuntimeToolsStatus();
  model.runtimeToolsStatus = runtimeTools.ok ? runtimeTools.data : [];
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
  return wireSettingsImpl(root, model, rerender, t, {
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
