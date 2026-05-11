import {
  closeIcon,
  cookieIcon,
  engineIcon,
  escapeHtml,
  exportIcon,
  pencilIcon,
  playIcon,
  profileTags,
  puzzleIcon,
  stopIcon,
  terminalIcon,
  trashIcon,
  usersIcon
} from "./view-helpers.js";

export function rowHtml(profile, isSelected, t) {
  const tags = profileTags(profile);
  const firstTag = tags[0] ?? null;
  const extraTags = tags.slice(1);
  const running = profile.state === "running";
  return `
    <tr class="profiles-row ${running ? "is-running" : ""}" data-profile-id="${profile.id}">
      <td class="profiles-cell profiles-cell-check">
        <input type="checkbox" class="profile-select" data-select-id="${profile.id}" ${isSelected ? "checked" : ""} />
      </td>
      <td class="profiles-cell profiles-cell-engine">
        <div class="engine-mark engine-${profile.engine}">
          ${engineIcon(profile.engine)}
        </div>
      </td>
      <td class="profiles-cell">
        <div class="profiles-name">${escapeHtml(profile.name)}</div>
      </td>
      <td class="profiles-cell">
        <div class="profiles-tags">
          ${firstTag ? `<span class="profiles-tag">${escapeHtml(firstTag)}</span>` : `<span class="profiles-muted">${t("profile.emptyTags")}</span>`}
          ${extraTags.length ? `
            <span class="profiles-tag-overflow" data-tag-overflow>
              <button
                type="button"
                class="profiles-tag profiles-tag-more"
                data-tag-tooltip-trigger
                data-tag-tooltip-tags="${escapeHtml(extraTags.join("\n"))}"
                aria-label="${escapeHtml(extraTags.join(", "))}"
              >+${extraTags.length}</button>
            </span>
          ` : ""}
        </div>
      </td>
      <td class="profiles-cell">
        <div class="profiles-note">${escapeHtml(profile.description ?? t("profile.emptyNote"))}</div>
      </td>
      <td class="profiles-cell profiles-cell-actions">
        <div class="profiles-actions-row">
          <button
            class="profiles-launch-btn ${running ? "stop" : "launch"}"
            data-action="${running ? "stop" : "launch"}"
            aria-label="${running ? t("profile.action.stop") : t("profile.action.launch")}"
            title="${running ? t("profile.action.stop") : t("profile.action.launch")}"
          >${running ? stopIcon() : playIcon()}</button>
          <button class="profiles-icon-btn" data-action="logs" aria-label="${t("profile.action.logs")}" title="${t("profile.action.logs")}">${terminalIcon()}</button>
          <button class="profiles-icon-btn" data-action="edit" aria-label="${t("profile.action.edit")}">${pencilIcon()}</button>
          <button class="profiles-icon-btn danger" data-action="delete" aria-label="${t("profile.action.delete")}">${trashIcon()}</button>
        </div>
      </td>
    </tr>
  `;
}

export function selectionBarHtml(selectedCount, t) {
  const canExport = selectedCount === 1;
  if (!selectedCount) return "";
  return `
    <div class="profiles-selection-bar">
      <div class="profiles-selection-count">${selectedCount} ${t("profile.bulk.selected")}</div>
      <button class="profiles-selection-btn" id="profiles-clear-selection" aria-label="${t("profile.bulk.clear")}">${closeIcon()}</button>
      <button class="profiles-selection-btn" id="profiles-add-group" title="${t("profile.bulk.addGroup")}">${usersIcon()}</button>
      <button class="profiles-selection-btn" id="profiles-add-ext-group" title="${t("profile.bulk.addExtGroup")}">${puzzleIcon()}</button>
      <button class="profiles-selection-btn" id="profiles-export-selection" title="${t("profile.bulk.export")}" ${canExport ? "" : "disabled"}>${exportIcon()}</button>
      <button class="profiles-selection-btn" id="profiles-copy-cookies" title="${t("profile.bulk.copyCookies")}">${cookieIcon()}</button>
    </div>
  `;
}

export function copyCookiesModalHtml(t, profiles, selectedProfiles) {
  const selectedNames = selectedProfiles.map((profile) => `<span class="profiles-target-pill">${escapeHtml(profile.name)}</span>`).join("");
  const engines = [...new Set(selectedProfiles.map((profile) => profile.engine))];
  const sourceProfiles = profiles.filter((profile) => !selectedProfiles.some((item) => item.id === profile.id) && engines.length === 1 && profile.engine === engines[0]);
  const sourceOptions = sourceProfiles.map((profile) => `<option value="${profile.id}">${escapeHtml(profile.name)}</option>`).join("");
  return `
    <div class="profiles-modal-overlay" id="profile-cookie-overlay">
      <div class="profiles-modal-window profiles-modal-window-md profiles-cookie-modal">
        <div class="profiles-cookie-head">
          <h3>${t("profile.cookies.title")}</h3>
          <button type="button" class="profiles-icon-btn" id="profile-cookie-close" aria-label="${t("action.cancel")}">${closeIcon()}</button>
        </div>
        <p class="meta">${t("profile.cookies.description")}</p>
        ${engines.length > 1 ? `<p class="notice error">${t("profile.bulk.mixedEngines")}</p>` : ""}
        <label>${t("profile.cookies.source")}
          <select id="profile-cookie-source" ${engines.length > 1 ? "disabled" : ""}>
            <option value="">${t("profile.cookies.sourcePlaceholder")}</option>
            ${sourceOptions}
          </select>
        </label>
        <label>${t("profile.cookies.targets")}
          <div class="profiles-target-box">${selectedNames}</div>
        </label>
        <footer class="modal-actions">
          <button type="button" id="profile-cookie-cancel">${t("action.cancel")}</button>
          <button type="button" id="profile-cookie-submit" ${engines.length > 1 ? "disabled" : ""}>${t("profile.bulk.copyCookies")}</button>
        </footer>
      </div>
    </div>
  `;
}

export function renderProfilesSectionHtml(t, model, rowsHtml, sortAriaByKey, allSelected, noticeHtml, selectedCount) {
  return `
    <div class="profiles-panel">
      <div class="profiles-header">
        <div>
          <h2>${t("nav.profiles")}</h2>
        </div>
        <div class="top-actions">
          <button id="profile-create">${t("profile.action.create")}</button>
          <button id="profile-import">${t("profile.action.import")}</button>
        </div>
      </div>
      ${noticeHtml}
      <div class="profiles-table-shell">
        <table class="profiles-table">
          <thead>
            <tr>
              <th class="profiles-col-check"><input type="checkbox" id="profiles-select-all" ${allSelected ? "checked" : ""} /></th>
              <th class="profiles-col-engine"></th>
              <th class="is-sortable" data-profile-sort="name" aria-sort="${sortAriaByKey("name")}">${t("profile.field.name")}</th>
              <th class="is-sortable" data-profile-sort="tags" aria-sort="${sortAriaByKey("tags")}">${t("profile.tags")}</th>
              <th class="is-sortable" data-profile-sort="note" aria-sort="${sortAriaByKey("note")}">${t("profile.table.note")}</th>
              <th class="profiles-col-actions"></th>
            </tr>
          </thead>
          <tbody>
            ${rowsHtml || `<tr><td colspan="6" class="profiles-empty">${t("profile.empty")}</td></tr>`}
          </tbody>
        </table>
      </div>
      ${selectionBarHtml(selectedCount, t)}
    </div>
  `;
}
