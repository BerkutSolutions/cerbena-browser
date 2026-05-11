export function escapeHtml(value) {
  return String(value ?? "")
    .replaceAll("&", "&amp;")
    .replaceAll("<", "&lt;")
    .replaceAll(">", "&gt;")
    .replaceAll('"', "&quot;");
}

export function ensureSettingsModel(model) {
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

export function profileName(model, profileId) {
  return model.profiles?.find((item) => item.id === profileId)?.name ?? "";
}

export function startupProfileLabel(model, profileId, t) {
  if (!profileId) return t("settings.startupProfile.none");
  return profileName(model, profileId) || t("settings.startupProfile.none");
}

export function startupProfileMenu(model, selectedProfileId) {
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

export function syncProfileId(model) {
  return ensureSettingsModel(model).syncProfileId;
}

export function linkBindingLabel(item, model, t) {
  if (item.profileId) return profileName(model, item.profileId) || item.profileId;
  if (item.usesGlobalDefault && model.linkRoutingOverview?.globalProfileId) {
    return `${profileName(model, model.linkRoutingOverview.globalProfileId) || model.linkRoutingOverview.globalProfileId} ${t("links.binding.globalDefault")}`;
  }
  return t("links.binding.none");
}

export function profileOptions(model, selectedProfileId, t) {
  return [
    `<option value="">${t("links.binding.none")}</option>`,
    ...(model.profiles ?? []).map((profile) => `<option value="${profile.id}" ${profile.id === selectedProfileId ? "selected" : ""}>${escapeHtml(profile.name)}</option>`)
  ].join("");
}

export function syncStatusLabel(info, t) {
  const messageKey = info?.controls?.status?.message_key ?? info?.controls?.status?.messageKey;
  if (!messageKey) return t("sync.status.unknown");
  return t(messageKey);
}

export function postureStatusLabel(report, t) {
  return t(`devicePosture.status.${report?.status ?? "healthy"}`);
}

export function postureReactionLabel(report, t) {
  return t(`devicePosture.reaction.${report?.reaction ?? "allow"}`);
}

export function updateStatusLabel(updateState, t) {
  return t(`settings.updates.status.${updateState?.status ?? "idle"}`);
}

export function updateAssetTypeLabel(updateState, t) {
  return t(`settings.updates.assetType.${updateState?.selectedAssetType ?? "unknown"}`);
}

export function updateHandoffModeLabel(updateState, t) {
  return t(`settings.updates.handoffMode.${updateState?.installHandoffMode ?? "unknown"}`);
}

export function updateSelectionReasonLabel(updateState, t) {
  return t(`settings.updates.reason.${updateState?.selectedAssetReason ?? "unknown"}`);
}

export function runtimeToolStatusLabel(tool, t) {
  if (tool.version) return tool.version;
  if (tool.status === "docker") return t("settings.tools.inDocker");
  return "";
}

export function releaseVersionForLink(updateState, appVersion) {
  const candidate =
    updateState?.latestVersion || updateState?.stagedVersion || updateState?.currentVersion || appVersion;
  return String(candidate).trim().replace(/^v/i, "");
}

export function buildReleaseUrl(updateState, appVersion) {
  const provided = String(updateState?.releaseUrl ?? "").trim();
  if (/^https?:\/\//i.test(provided)) {
    return provided;
  }
  const version = releaseVersionForLink(updateState, appVersion);
  return `https://github.com/BerkutSolutions/cerbena-browser/releases/tag/v${encodeURIComponent(version)}`;
}

export function linuxSandboxGuideModalHtml(t) {
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

export function linuxDockerGuideModalHtml(t) {
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
