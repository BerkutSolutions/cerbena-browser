export function renderGeneralTab(t, model, deps) {
  const {
    escapeHtml,
    postureStatusLabel,
    postureReactionLabel,
    renderRuntimeToolsCardSlice,
    renderUpdateCardSlice,
    ensureSettingsModel,
    startupProfileLabel,
    startupProfileMenu
  } = deps;

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
        ${renderRuntimeToolsCardSlice(t, model, escapeHtml)}
        <div class="settings-grid-spacer" aria-hidden="true"></div>
        ${renderUpdateCardSlice(t, model, {
          escapeHtml,
          ensureSettingsModel,
          startupProfileLabel,
          startupProfileMenu
        })}
      </div>
    </section>
  `;
}

export function renderLinksTab(t, model, deps) {
  const {
    ensureSettingsModel,
    ensureGlobalSecurityState,
    SEARCH_PROVIDER_PRESETS,
    escapeHtml,
    profileName,
    profileOptions,
    linkBindingLabel
  } = deps;

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

export function renderSyncTab(t, model, deps) {
  const { ensureSettingsModel, escapeHtml, syncProfileId, syncStatusLabel } = deps;

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
