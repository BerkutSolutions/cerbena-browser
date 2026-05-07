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
      linkProfileDrafts: {},
      startupProfileDraft: ""
    };
  }
  if (!model.settingsState.syncProfileId) {
    model.settingsState.syncProfileId = model.selectedProfileId ?? model.profiles?.[0]?.id ?? null;
  }
  return model.settingsState;
}

function startupProfileLabel(model, profileId, t) {
  if (!profileId) return t("settings.startupProfile.none");
  return profileName(model, profileId) || t("settings.startupProfile.none");
}

function startupProfileMenu(model, selectedProfileId) {
  return `
    <div class="dns-dropdown-menu hidden" id="settings-startup-profile-menu">
      ${(model.profiles ?? []).map((profile) => `
        <label class="dns-blocklist-option">
          <input
            type="checkbox"
            data-settings-startup-profile="${profile.id}"
            ${profile.id === selectedProfileId ? "checked" : ""}
          />
          <span>${escapeHtml(profile.name)}</span>
        </label>
      `).join("")}
    </div>
  `;
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

function updateAssetTypeLabel(updateState, t) {
  return t(`settings.updates.assetType.${updateState?.selectedAssetType ?? "unknown"}`);
}

function updateHandoffModeLabel(updateState, t) {
  return t(`settings.updates.handoffMode.${updateState?.installHandoffMode ?? "unknown"}`);
}

function updateSelectionReasonLabel(updateState, t) {
  return t(`settings.updates.reason.${updateState?.selectedAssetReason ?? "unknown"}`);
}

function runtimeToolStatusLabel(tool, t) {
  if (tool.version) return tool.version;
  if (tool.status === "docker") return t("settings.tools.inDocker");
  return "";
}

function releaseVersionForLink(updateState) {
  const candidate =
    updateState?.latestVersion || updateState?.stagedVersion || updateState?.currentVersion || APP_VERSION;
  return String(candidate).trim().replace(/^v/i, "");
}

function buildReleaseUrl(updateState) {
  const provided = String(updateState?.releaseUrl ?? "").trim();
  if (/^https?:\/\//i.test(provided)) {
    return provided;
  }
  const version = releaseVersionForLink(updateState);
  return `https://github.com/BerkutSolutions/cerbena-browser/releases/tag/v${encodeURIComponent(version)}`;
}

function renderUpdateCard(t, model) {
  const updateState = model.launcherUpdateState ?? {};
  const shellState = model.shellPreferencesState ?? {};
  const settingsState = ensureSettingsModel(model);
  const startupProfileId = settingsState.startupProfileDraft || shellState.startupProfileId || "";
  const releaseUrl = buildReleaseUrl(updateState);
  return `
    <div class="panel settings-card">
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
          <input value="${escapeHtml(updateState.currentVersion ?? APP_VERSION)}" disabled />
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
      <label class="checkbox-inline">
        <input id="settings-autostart-enabled" type="checkbox" ${shellState.launchOnSystemStartup ? "checked" : ""} />
        <span>${t("settings.autostart.enabled")}</span>
      </label>
      ${shellState.launchOnSystemStartup ? `
        <div class="dns-dropdown">
          <label>${t("settings.startupProfile.label")}</label>
          <button
            type="button"
            class="dns-dropdown-toggle"
            id="settings-startup-profile-toggle"
          >${escapeHtml(startupProfileLabel(model, startupProfileId, t))}</button>
          ${startupProfileMenu(model, startupProfileId)}
        </div>
        <p class="meta">${t("settings.startupProfile.hint")}</p>
      ` : ""}
      <p class="meta">${t("settings.updates.lastChecked")}: ${escapeHtml(updateState.lastCheckedAt ?? t("settings.updates.notChecked"))}</p>
      <p class="meta">${t("settings.updates.statusLabel")}: ${escapeHtml(updateStatusLabel(updateState, t))}</p>
      ${updateState.stagedVersion ? `<p class="meta">${t("settings.updates.staged")}: ${escapeHtml(updateState.stagedVersion)}</p>` : ""}
      ${updateState.selectedAssetType ? `<p class="meta">${t("settings.updates.assetTypeLabel")}: ${escapeHtml(updateAssetTypeLabel(updateState, t))}</p>` : ""}
      ${updateState.installHandoffMode ? `<p class="meta">${t("settings.updates.handoffModeLabel")}: ${escapeHtml(updateHandoffModeLabel(updateState, t))}</p>` : ""}
      ${updateState.selectedAssetReason ? `<p class="meta">${t("settings.updates.assetReasonLabel")}: ${escapeHtml(updateSelectionReasonLabel(updateState, t))}</p>` : ""}
      <p class="meta"><a href="${escapeHtml(releaseUrl)}" id="settings-update-open-release" target="_blank" rel="noreferrer">${t("settings.updates.openRelease")}</a></p>
      ${updateState.lastError ? `<p class="notice error">${escapeHtml(updateState.lastError)}</p>` : ""}
    </div>
  `;
}

function renderRuntimeToolsCard(t, model) {
  const tools = model.runtimeToolsStatus ?? [];
  return `
    <div class="panel settings-card">
      <div class="row-between">
        <div>
          <h4>${t("settings.tools.title")}</h4>
          <p class="meta">${t("settings.tools.hint")}</p>
        </div>
        <button id="settings-runtime-tools-refresh">${t("action.refresh")}</button>
      </div>
      <ul class="settings-runtime-list">
        ${tools.map((tool) => `
          <li class="settings-runtime-item">
            <div class="settings-runtime-main">
              <span class="settings-runtime-dot ${tool.status === "missing" ? "is-missing" : "is-ready"}" aria-hidden="true"></span>
              <div class="settings-runtime-copy">
                <strong>${t(tool.nameKey)}</strong>
                ${tool.detailKey ? `<p class="meta">${t(tool.detailKey)}</p>` : ""}
              </div>
            </div>
            <div class="settings-runtime-side">
              ${tool.action === "internal"
                ? `<button type="button" data-settings-tool-install="${escapeHtml(tool.id)}">${t("settings.tools.download")}</button>`
                : tool.action === "external"
                  ? `<button type="button" data-settings-tool-external="${escapeHtml(tool.id)}">${t("settings.tools.download")}</button>`
                  : tool.action === "guide"
                    ? `<button type="button" data-settings-tool-guide="${escapeHtml(tool.id)}">${t("settings.tools.configure")}</button>`
                  : `<span class="settings-runtime-version">${escapeHtml(runtimeToolStatusLabel(tool, t))}</span>`}
            </div>
          </li>
        `).join("")}
      </ul>
    </div>
  `;
}

function linuxSandboxGuideModalHtml(t) {
  return `
    <div class="profiles-modal-overlay" id="linux-sandbox-guide-overlay">
      <div class="profiles-modal-window profiles-modal-window-sm">
        <div class="action-modal">
          <h3>${escapeHtml(t("linuxSandbox.modal.title"))}</h3>
          <p class="meta">${escapeHtml(t("linuxSandbox.modal.body"))}</p>
          <pre class="preview-box"># 1) Validate current state
cat /proc/sys/kernel/unprivileged_userns_clone
cat /proc/sys/kernel/apparmor_restrict_unprivileged_userns

# 2) Keep userns enabled persistently
sudo sysctl -w kernel.unprivileged_userns_clone=1
echo "kernel.unprivileged_userns_clone=1" | sudo tee /etc/sysctl.d/99-cerbena-userns.conf
sudo sysctl --system</pre>
          <pre class="preview-box"># 3) Safer AppArmor allowlist for Cerbena Chromium runtime (recommended)
cat <<'EOF' | sudo tee /etc/apparmor.d/cerbena-chromium
abi &lt;abi/4.0&gt;,
include &lt;tunables/global&gt;

profile cerbena-chromium-dev @{HOME}/.local/share/dev.cerbena.app/engine-runtime/engines/chromium/*/chrome-linux/chrome flags=(unconfined) {
  userns,
  include if exists &lt;local/cerbena-chromium&gt;
}

profile cerbena-chromium-prod @{HOME}/.local/share/cerbena.app/engine-runtime/engines/chromium/*/chrome-linux/chrome flags=(unconfined) {
  userns,
  include if exists &lt;local/cerbena-chromium&gt;
}
EOF
sudo apparmor_parser -r /etc/apparmor.d/cerbena-chromium
sudo systemctl reload apparmor</pre>
          <p class="meta">${escapeHtml(t("linuxSandbox.modal.apparmorHint"))}</p>
          <pre class="preview-box"># 4) Last-resort fallback (weakens host security globally; avoid if possible)
# echo 0 | sudo tee /proc/sys/kernel/apparmor_restrict_unprivileged_userns
# echo "kernel.apparmor_restrict_unprivileged_userns=0" | sudo tee /etc/sysctl.d/60-apparmor-userns.conf</pre>
          <footer class="modal-actions">
            <button type="button" id="linux-sandbox-guide-cancel">${t("action.cancel")}</button>
            <button type="button" id="linux-sandbox-guide-open">${t("linuxSandbox.modal.openDocs")}</button>
          </footer>
        </div>
      </div>
    </div>
  `;
}

function linuxDockerGuideModalHtml(t) {
  return `
    <div class="profiles-modal-overlay" id="linux-docker-guide-overlay">
      <div class="profiles-modal-window profiles-modal-window-sm">
        <div class="action-modal">
          <h3>${escapeHtml(t("linuxDocker.modal.title"))}</h3>
          <p class="meta">${escapeHtml(t("linuxDocker.modal.body"))}</p>
          <pre class="preview-box">sudo apt-get update
sudo apt-get install -y ca-certificates curl gnupg
sudo install -m 0755 -d /etc/apt/keyrings
curl -fsSL https://download.docker.com/linux/ubuntu/gpg | sudo gpg --dearmor -o /etc/apt/keyrings/docker.gpg
sudo chmod a+r /etc/apt/keyrings/docker.gpg
echo "deb [arch=$(dpkg --print-architecture) signed-by=/etc/apt/keyrings/docker.gpg] https://download.docker.com/linux/ubuntu $(. /etc/os-release && echo $VERSION_CODENAME) stable" | sudo tee /etc/apt/sources.list.d/docker.list > /dev/null
sudo apt-get update
sudo apt-get install -y docker-ce docker-ce-cli containerd.io docker-buildx-plugin docker-compose-plugin
sudo usermod -aG docker $USER
newgrp docker
docker version</pre>
          <p class="meta">${escapeHtml(t("linuxDocker.modal.hint"))}</p>
          <footer class="modal-actions">
            <button type="button" id="linux-docker-guide-cancel">${t("action.cancel")}</button>
            <button type="button" id="linux-docker-guide-open">${t("linuxDocker.modal.openDocs")}</button>
          </footer>
        </div>
      </div>
    </div>
  `;
}

async function showLinuxSandboxGuideModal(t) {
  const existing = document.body.querySelector("#linux-sandbox-guide-overlay");
  if (existing) {
    closeModalOverlay(existing);
  }
  document.body.insertAdjacentHTML("beforeend", linuxSandboxGuideModalHtml(t));
  const overlay = document.body.querySelector("#linux-sandbox-guide-overlay");
  if (!overlay) return;
  showModalOverlay(overlay);
  const close = () => closeModalOverlay(overlay);
  overlay.querySelector("#linux-sandbox-guide-cancel")?.addEventListener("click", close);
  overlay.querySelector("#linux-sandbox-guide-open")?.addEventListener("click", async () => {
    await openExternalUrl("https://chromium.googlesource.com/chromium/src/+/main/docs/security/apparmor-userns-restrictions.md");
    close();
  });
  overlay.addEventListener("click", (event) => {
    if (event.target === overlay) close();
  });
}

async function showLinuxDockerGuideModal(t) {
  const existing = document.body.querySelector("#linux-docker-guide-overlay");
  if (existing) {
    closeModalOverlay(existing);
  }
  document.body.insertAdjacentHTML("beforeend", linuxDockerGuideModalHtml(t));
  const overlay = document.body.querySelector("#linux-docker-guide-overlay");
  if (!overlay) return;
  showModalOverlay(overlay);
  const close = () => closeModalOverlay(overlay);
  overlay.querySelector("#linux-docker-guide-cancel")?.addEventListener("click", close);
  overlay.querySelector("#linux-docker-guide-open")?.addEventListener("click", async () => {
    await openExternalUrl("https://docs.docker.com/engine/install/ubuntu/");
    close();
  });
  overlay.addEventListener("click", (event) => {
    if (event.target === overlay) close();
  });
}

function renderGeneralTab(t, model) {
  const posture = model.devicePostureReport;
  return `
    <section class="settings-tab-panel">
      <div class="settings-panel-grid">
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
        ${renderRuntimeToolsCard(t, model)}
        <div class="settings-grid-spacer" aria-hidden="true"></div>
        ${renderUpdateCard(t, model)}
      </div>
    </section>
  `;
}

function renderLinksTab(t, model) {
  const state = ensureSettingsModel(model);
  const overview = model.linkRoutingOverview ?? { globalProfileId: null, supportedTypes: [] };
  const shellState = model.shellPreferencesState ?? {};
  const globalDraft = state.globalLinkProfileDraft || overview.globalProfileId || "";
  const securityState = ensureGlobalSecurityState(model);
  const selectedProvider = model.settingsProvider ?? "duckduckgo";
  return `
    <section class="settings-tab-panel">
      <div class="panel settings-card">
        <div class="row-between">
          <div>
            <h4>${t("settings.browserDefaults.title")}</h4>
            <p class="meta">${t("settings.browserDefaults.hint")}</p>
          </div>
          <button id="settings-links-browser-save">${t("action.save")}</button>
        </div>
        <div class="grid-two">
          <label>${t("settings.searchProvider")}
            <select id="settings-search-provider">
              ${SEARCH_PROVIDER_PRESETS.map((item) => `<option value="${item.id}" ${item.id === selectedProvider ? "selected" : ""}>${escapeHtml(item.name)}</option>`).join("")}
            </select>
          </label>
          <label>${t("security.startPage")}
            <input id="settings-start-page" value="${escapeHtml(securityState.startupPage ?? "")}" placeholder="https://duckduckgo.com" />
          </label>
        </div>
        <p class="meta">${t("settings.startPageHint")}</p>
      </div>

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
