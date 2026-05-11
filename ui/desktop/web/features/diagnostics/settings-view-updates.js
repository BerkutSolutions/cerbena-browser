import { APP_VERSION } from "../../core/app-version.js";

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

export function renderUpdateCard(t, model, helpers) {
  const { escapeHtml, ensureSettingsModel, startupProfileLabel, startupProfileMenu } = helpers;
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

export function renderRuntimeToolsCard(t, model, escapeHtml) {
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
