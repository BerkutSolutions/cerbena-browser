const DEFAULT_PANIC_FRAME_COLOR = "#ff8652";

function pickLocale() {
  return localStorage.getItem("launcher.locale") ?? "ru";
}

function navItem(entry, isActive, t, icons) {
  const className = isActive ? "sidebar-link active" : "sidebar-link";
  const icon = icons[entry.key] ?? icons.home;
  return `<button class='${className}' data-feature='${entry.key}' title='${t(entry.labelKey)}'><span class='sidebar-link-icon'>${icon}</span><span class='sidebar-link-label'>${t(entry.labelKey)}</span></button>`;
}

function selectedProfile(model) {
  return (model.profiles ?? []).find((item) => item.id === model.selectedProfileId) ?? null;
}

function shouldRenderPanicControls(model) {
  const profile = selectedProfile(model);
  return Boolean(profile && profile.state === "running" && profile.panic_frame_enabled);
}

function fireIcon() {
  return "<img src='./assets/icons/fire-64.png' alt='' draggable='false' />";
}

function fireHeroIcon() {
  return "<img src='./assets/icons/fire-128.png' alt='' draggable='false' />";
}

function normalizePanicColor(value, defaultPanicFrameColor) {
  const raw = String(value ?? "").trim();
  return /^#[0-9a-f]{6}$/i.test(raw) ? raw.toLowerCase() : defaultPanicFrameColor;
}

function hexToRgbTriplet(value) {
  const color = normalizePanicColor(value, DEFAULT_PANIC_FRAME_COLOR);
  const r = Number.parseInt(color.slice(1, 3), 16);
  const g = Number.parseInt(color.slice(3, 5), 16);
  const b = Number.parseInt(color.slice(5, 7), 16);
  return `${r}, ${g}, ${b}`;
}

function panicFrameStyleAttr(profile) {
  const color = normalizePanicColor(profile?.panic_frame_color, DEFAULT_PANIC_FRAME_COLOR);
  const rgb = hexToRgbTriplet(color);
  return `style="--panic-accent:${color};--panic-accent-rgb:${rgb};"`;
}

function isPanicFrameOverlay() {
  return typeof window !== "undefined" && Boolean(window.__PANIC_FRAME_OVERLAY);
}

function panicFrameProfileId() {
  return typeof window !== "undefined" ? String(window.__PANIC_FRAME_PROFILE_ID ?? "").trim() : "";
}

function sleep(ms) {
  return new Promise((resolve) => window.setTimeout(resolve, ms));
}

function homeMetricValue(model, key) {
  return model.homeDashboard?.metrics?.find((item) => item.key === key)?.value ?? 0;
}

function setHomeMetricValue(model, key, nextValue) {
  if (!model.homeDashboard?.metrics) {
    model.homeDashboard = {
      metrics: [
        { key: "home.metric.dns_blocked", value: 0 },
        { key: "home.metric.tracker_blocked", value: 0 },
        { key: "home.metric.service_blocked", value: 0 }
      ],
      quick_actions: []
    };
  }
  const metric = model.homeDashboard.metrics.find((item) => item.key === key);
  if (metric) {
    metric.value = nextValue;
    return;
  }
  model.homeDashboard.metrics.push({ key, value: nextValue });
}

function applyHomeMetricEntry(model, entry) {
  if (!entry || entry.status !== "blocked") return false;
  const reason = String(entry.reason ?? "").toLowerCase();
  const blockedByRule = Boolean(entry.blocked_globally) || Boolean(entry.blocked_for_profile);
  let changed = false;
  if (reason.includes("dns")) {
    setHomeMetricValue(model, "home.metric.dns_blocked", homeMetricValue(model, "home.metric.dns_blocked") + 1);
    changed = true;
  }
  if (reason.includes("service")) {
    setHomeMetricValue(model, "home.metric.service_blocked", homeMetricValue(model, "home.metric.service_blocked") + 1);
    changed = true;
  }
  if (blockedByRule || reason.includes("tracker")) {
    setHomeMetricValue(model, "home.metric.tracker_blocked", homeMetricValue(model, "home.metric.tracker_blocked") + 1);
    changed = true;
  }
  return changed;
}

function renderProfileLaunchOverlay(i18n, model) {
  const overlay = model.profileLaunchOverlay;
  if (!overlay) return "";
  const profile = (model.profiles ?? []).find((item) => item.id === overlay.profileId);
  const title = i18n.t("profile.launchProgress.title");
  const profileName = profile?.name ?? i18n.t("profile.launchProgress.profileFallback");
  const stageText = overlay.messageKey ? i18n.t(overlay.messageKey) : i18n.t("profile.launchProgress.starting");
  return `
    <div class="profile-launch-overlay" aria-live="polite">
      <div class="profile-launch-card">
        <div class="profile-launch-spinner" aria-hidden="true"></div>
        <div class="profile-launch-copy">
          <strong>${title}</strong>
          <span>${profileName}</span>
          <p>${stageText}</p>
        </div>
      </div>
    </div>
  `;
}

function renderAppLifecycleOverlay(i18n, model) {
  const overlay = model.appLifecycleOverlay;
  if (!overlay) return "";
  const title = i18n.t(overlay.titleKey);
  const subtitle = overlay.subtitleKey ? i18n.t(overlay.subtitleKey) : "";
  const stageText = overlay.messageKey ? i18n.t(overlay.messageKey) : "";
  return `
    <div class="profile-launch-overlay app-lifecycle-overlay" aria-live="polite">
      <div class="profile-launch-card app-lifecycle-card">
        <div class="profile-launch-spinner" aria-hidden="true"></div>
        <div class="profile-launch-copy">
          <strong>${title}</strong>
          <span>${subtitle}</span>
          <p>${stageText}</p>
        </div>
      </div>
    </div>
  `;
}

function renderDefaultBrowserStartupModal(i18n, model) {
  if (!model.defaultBrowserStartupModal) return "";
  return `
    <div class="profiles-modal-overlay" id="default-browser-startup-overlay">
      <div class="profiles-modal-window profiles-modal-window-sm">
        <div class="action-modal">
          <h3>${i18n.t("links.defaultBrowser.modal.title")}</h3>
          <p class="meta">${i18n.t("links.defaultBrowser.modal.body")}</p>
          <div class="modal-actions">
            <button type="button" id="default-browser-startup-no">${i18n.t("action.no")}</button>
            <button type="button" id="default-browser-startup-yes">${i18n.t("action.yes")}</button>
          </div>
        </div>
      </div>
    </div>
  `;
}

function renderDefaultLinkProfileModal(i18n, model) {
  if (!model.defaultLinkProfileModal) return "";
  return `
    <div class="profiles-modal-overlay" id="default-link-profile-overlay">
      <div class="profiles-modal-window profiles-modal-window-sm">
        <div class="action-modal">
          <h3>${i18n.t("links.defaultProfile.modal.title")}</h3>
          <p class="meta">${i18n.t("links.defaultProfile.modal.body")}</p>
          <label>${i18n.t("links.defaultProfile.modal.profile")}
            <select id="default-link-profile-select">
              ${(model.profiles ?? []).map((profile) => `<option value="${profile.id}" ${profile.id === model.defaultLinkProfileModal.selectedProfileId ? "selected" : ""}>${profile.name}</option>`).join("")}
            </select>
          </label>
          <div class="modal-actions">
            <button type="button" id="default-link-profile-cancel">${i18n.t("action.cancel")}</button>
            <button type="button" id="default-link-profile-save">${i18n.t("action.save")}</button>
          </div>
        </div>
      </div>
    </div>
  `;
}

function renderTrayCloseModal(i18n, model) {
  if (!model.trayClosePromptModal) return "";
  return `
    <div class="profiles-modal-overlay" id="tray-close-prompt-overlay">
      <div class="profiles-modal-window profiles-modal-window-sm">
        <div class="action-modal">
          <h3>${i18n.t("settings.tray.modal.title")}</h3>
          <p class="meta">${i18n.t("settings.tray.modal.body")}</p>
          <div class="modal-actions">
            <button type="button" id="tray-close-prompt-yes">${i18n.t("action.yes")}</button>
            <button type="button" id="tray-close-prompt-no">${i18n.t("action.no")}</button>
            <button type="button" id="tray-close-prompt-cancel">${i18n.t("action.cancel")}</button>
          </div>
        </div>
      </div>
    </div>
  `;
}

function renderStandaloneLifecycleOverlay(i18n, overlay) {
  const model = { appLifecycleOverlay: overlay };
  return `
    <div class="app-lifecycle-shell">
      ${renderAppLifecycleOverlay(i18n, model)}
    </div>
  `;
}


export {
  pickLocale, navItem, selectedProfile, shouldRenderPanicControls, fireIcon, fireHeroIcon,
  hexToRgbTriplet, panicFrameStyleAttr, isPanicFrameOverlay, panicFrameProfileId, sleep,
  homeMetricValue, setHomeMetricValue, applyHomeMetricEntry, renderProfileLaunchOverlay,
  renderAppLifecycleOverlay, renderDefaultBrowserStartupModal, renderDefaultLinkProfileModal,
  renderTrayCloseModal, renderStandaloneLifecycleOverlay
};
