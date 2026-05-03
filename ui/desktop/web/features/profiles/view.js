import {
  acknowledgeWayfernTos,
  copyProfileCookies,
  createProfile,
  deleteProfile,
  exportProfile,
  importProfile,
  launchProfile,
  listProfiles,
  pickCertificateFiles,
  setProfilePassword,
  stopProfile,
  updateProfile,
  validateProfileModal
} from "./api.js";
import { SEARCH_PROVIDER_PRESETS } from "../../core/catalogs.js";
import { listExtensionLibrary, setExtensionProfiles } from "../extensions/api.js";
import { generateAutoPreset, getIdentityProfile, saveIdentityProfile } from "../identity/api.js";
import { getDevicePostureReport } from "../settings/api.js";
import {
  buildManualPreset,
  cloneIdentityPreset,
  firstTemplateKeyForTemplatePlatform,
  inferIdentityUiState,
  listIdentityPlatforms,
  listIdentityTemplatePlatforms,
  listIdentityTemplates,
  normalizeTemplatePlatform,
  normalizeAutoPlatform
} from "../identity/shared.js";
import {
  getNetworkState,
  getServiceCatalog,
  previewNetworkSandboxSettings,
  saveDnsPolicy,
  saveNetworkSandboxProfileSettings,
  saveVpnProxyPolicy
} from "../network/api.js";
import { getGlobalSecuritySettings, saveGlobalSecuritySettings } from "../security/api.js";
import { getSyncOverview, saveSyncControls } from "../sync/api.js";
import { blockedServicesToPairs, loadDnsTemplates, loadProfileDnsDraft, saveProfileDnsDraft } from "../dns/store.js";
import { applyPolicyPresetToDraft, loadPolicyPresets, summarizePolicyPreset } from "../dns/policy-store.js";
import { askConfirmModal, askInputModal, closeModalOverlay, showModalOverlay } from "../../core/modal.js";
import { buildTagPickerMarkup, collectTagOptions, uniqueTags, wireTagPicker } from "../../core/tag-picker.js";

const DOMAIN_OPTIONS = [
  "cloudflare.com",
  "docs.rs",
  "example.com",
  "github.com",
  "google.com",
  "mozilla.org",
  "openai.com",
  "reddit.com",
  "wikipedia.org",
  "youtube.com"
];

function option(value, label, selected) {
  return `<option value="${value}" ${selected ? "selected" : ""}>${label}</option>`;
}

function escapeHtml(value) {
  return String(value ?? "")
    .replaceAll("&", "&amp;")
    .replaceAll("<", "&lt;")
    .replaceAll(">", "&gt;")
    .replaceAll("\"", "&quot;");
}

function engineIcon(engine) {
  if (engine === "camoufox") {
    return `<svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="1.8"><path d="M7 4h10l2 4-1 8-4 4H10l-4-4-1-8 2-4z"/><path d="M9 9h.01M15 9h.01"/><path d="M9 14c1 1 2 1.5 3 1.5S14 15 15 14"/></svg>`;
  }
  return `<svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="1.8"><circle cx="12" cy="12" r="9"/><path d="M3 12h18"/><path d="M12 3c3 3 4.5 6 4.5 9S15 18 12 21c-3-3-4.5-6-4.5-9S9 6 12 3z"/></svg>`;
}

function pencilIcon() {
  return `<svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="1.8"><path d="M12 20h9"/><path d="M16.5 3.5a2.1 2.1 0 0 1 3 3L8 18l-4 1 1-4 11.5-11.5z"/></svg>`;
}

function exportIcon() {
  return `<svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="1.8"><path d="M12 3v12"/><path d="m7 8 5-5 5 5"/><path d="M5 21h14"/></svg>`;
}

function trashIcon() {
  return `<svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="1.8"><path d="M3 6h18"/><path d="M8 6V4h8v2"/><path d="M19 6l-1 14H6L5 6"/><path d="M10 11v6M14 11v6"/></svg>`;
}

function closeIcon() {
  return `<svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="1.8"><path d="m6 6 12 12"/><path d="m18 6-12 12"/></svg>`;
}

function playIcon() {
  return `<svg viewBox="0 0 24 24" fill="currentColor"><path d="M8 6.5v11l9-5.5z"/></svg>`;
}

function stopIcon() {
  return `<svg viewBox="0 0 24 24" fill="currentColor"><rect x="7" y="7" width="10" height="10" rx="1.5"/></svg>`;
}

function usersIcon() {
  return `<svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="1.8"><circle cx="9" cy="8" r="3"/><path d="M4 19c0-2.5 2.5-4 5-4"/><circle cx="17" cy="9" r="2.5"/><path d="M13 19c.5-2 2.4-3.5 4.7-3.5 1.5 0 2.8.5 3.8 1.5"/></svg>`;
}

function puzzleIcon() {
  return `<svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="1.8"><path d="M12 3a2 2 0 0 1 2 2v1h2.5A1.5 1.5 0 0 1 18 7.5V10h1a2 2 0 1 1 0 4h-1v2.5a1.5 1.5 0 0 1-1.5 1.5H14v1a2 2 0 1 1-4 0v-1H7.5A1.5 1.5 0 0 1 6 16.5V14H5a2 2 0 1 1 0-4h1V7.5A1.5 1.5 0 0 1 7.5 6H10V5a2 2 0 0 1 2-2z"/></svg>`;
}

function cookieIcon() {
  return `<svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="1.8"><path d="M14 3a3 3 0 0 0 4 4 7 7 0 1 1-4-4z"/><path d="M8.5 9.5h.01M14.5 13h.01M10 15.5h.01M12.5 7.5h.01"/></svg>`;
}

function profileTags(profile) {
  return (profile.tags ?? []).filter((tag) => !tag.startsWith("policy:")
    && !tag.startsWith("dns-template:")
    && !tag.startsWith("locked-app:")
    && !tag.startsWith("ext:")
    && !tag.startsWith("ext-disabled:")
    && !tag.startsWith("cert-id:")
    && !tag.startsWith("cert:")
    && tag !== "ext-system-access"
    && tag !== "ext-keepassxc"
    && tag !== "ext-launch-disabled");
}

function collectProfileTags(profiles) {
  return collectTagOptions(profiles ?? [], (profile) => profileTags(profile));
}

function certificateIds(profile) {
  return (profile?.tags ?? [])
    .filter((tag) => tag.startsWith("cert-id:"))
    .map((tag) => tag.replace("cert-id:", ""));
}

function certificateLegacyPaths(profile) {
  return (profile?.tags ?? [])
    .filter((tag) => tag.startsWith("cert:"))
    .map((tag) => tag.replace("cert:", ""))
    .filter((path) => path !== "global");
}

function hasAssignedProfileCertificates(certificateEntries) {
  return (certificateEntries ?? []).some((entry) => {
    const kind = String(entry?.kind ?? "");
    const value = String(entry?.value ?? "").trim();
    return (kind === "id" || kind === "path") && value.length > 0;
  });
}

function slugId(value) {
  return String(value ?? "")
    .toLowerCase()
    .replace(/[^a-z0-9]+/g, "-")
    .replace(/^-+|-+$/g, "");
}

function makeUniqueId(seed, existingIds) {
  const base = slugId(seed) || "item";
  let candidate = base;
  let suffix = 2;
  while (existingIds.has(candidate)) {
    candidate = `${base}-${suffix}`;
    suffix += 1;
  }
  return candidate;
}

function normalizeGlobalSecuritySettings(raw) {
  const payload = typeof raw === "string" ? JSON.parse(raw || "{}") : (raw ?? {});
  return {
    startupPage: payload.startup_page ?? payload.startupPage ?? "",
    certificates: (payload.certificates ?? []).map((item) => ({
      id: item.id,
      name: item.name,
      path: item.path,
      issuerName: item.issuerName ?? item.issuer_name ?? "",
      subjectName: item.subjectName ?? item.subject_name ?? "",
      applyGlobally: Boolean(item.applyGlobally ?? item.apply_globally),
      profileIds: item.profileIds ?? item.profile_ids ?? []
    })),
    blockedDomainSuffixes: payload.blocked_domain_suffixes ?? payload.blockedDomainSuffixes ?? [],
    blocklists: (payload.blocklists ?? []).map((item) => ({
      id: item.id,
      name: item.name,
      sourceKind: item.sourceKind ?? item.source_kind,
      sourceValue: item.sourceValue ?? item.source_value,
      active: Boolean(item.active),
      domains: item.domains ?? []
    }))
  };
}

function buildGlobalSecuritySaveRequest(state) {
  return {
    startupPage: state.startupPage?.trim() ? state.startupPage.trim() : null,
    certificates: state.certificates ?? [],
    blockedDomainSuffixes: state.blockedDomainSuffixes ?? [],
    blocklists: state.blocklists ?? []
  };
}

function profileExtensions(profile) {
  const enabled = [];
  const disabled = [];
  for (const tag of profile?.tags ?? []) {
    if (tag.startsWith("ext:")) enabled.push(tag.replace("ext:", ""));
    if (tag.startsWith("ext-disabled:")) disabled.push(tag.replace("ext-disabled:", ""));
  }
  return { enabled, disabled };
}

function assignedProfileExtensionIds(model, profile) {
  if (!profile?.id) return [];
  const profileId = String(profile.id);
  return Object.values(model?.extensionLibraryState?.items ?? {})
    .filter((item) => extensionScopeAllowed(item, profile))
    .filter((item) => (item.assignedProfileIds ?? []).includes(profileId))
    .map((item) => item.id);
}

function mergedProfileExtensions(model, profile) {
  const tagged = profileExtensions(profile);
  const disabled = [...new Set(tagged.disabled)];
  const enabled = [...new Set([...tagged.enabled, ...assignedProfileExtensionIds(model, profile)])]
    .filter((id) => !disabled.includes(id));
  return { enabled, disabled };
}

function profileSecurityFlags(profile) {
  const tags = profile?.tags ?? [];
  return {
    allowSystemAccess: tags.includes("ext-system-access"),
    allowKeepassxc: tags.includes("ext-keepassxc"),
    disableExtensionsLaunch: tags.includes("ext-launch-disabled")
  };
}

function selectionState(model) {
  if (!Array.isArray(model.selectedProfileIds)) model.selectedProfileIds = [];
  return model.selectedProfileIds;
}

function rowHtml(profile, isSelected, t) {
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
          <button class="profiles-icon-btn" data-action="edit" aria-label="${t("profile.action.edit")}">${pencilIcon()}</button>
          <button class="profiles-icon-btn danger" data-action="delete" aria-label="${t("profile.action.delete")}">${trashIcon()}</button>
        </div>
      </td>
    </tr>
  `;
}

function selectionBarHtml(model, t) {
  const selectedIds = selectionState(model);
  const canExport = selectedIds.length === 1;
  if (!selectedIds.length) return "";
  return `
    <div class="profiles-selection-bar">
      <div class="profiles-selection-count">${selectedIds.length} ${t("profile.bulk.selected")}</div>
      <button class="profiles-selection-btn" id="profiles-clear-selection" aria-label="${t("profile.bulk.clear")}">${closeIcon()}</button>
      <button class="profiles-selection-btn" id="profiles-add-group" title="${t("profile.bulk.addGroup")}">${usersIcon()}</button>
      <button class="profiles-selection-btn" id="profiles-add-ext-group" title="${t("profile.bulk.addExtGroup")}">${puzzleIcon()}</button>
      <button class="profiles-selection-btn" id="profiles-export-selection" title="${t("profile.bulk.export")}" ${canExport ? "" : "disabled"}>${exportIcon()}</button>
      <button class="profiles-selection-btn" id="profiles-copy-cookies" title="${t("profile.bulk.copyCookies")}">${cookieIcon()}</button>
    </div>
  `;
}

function copyCookiesModalHtml(t, profiles, selectedProfiles) {
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

function searchOptions(selected) {
  return SEARCH_PROVIDER_PRESETS
    .map((item) => `<option value="${item.id}" ${selected === item.id ? "selected" : ""}>${item.name}</option>`)
    .join("");
}

function extensionLibraryItem(model, extensionId) {
  return model?.extensionLibraryState?.items?.[extensionId] ?? null;
}

function extensionDisplayName(model, extensionId) {
  return extensionLibraryItem(model, extensionId)?.displayName ?? extensionId;
}

function templateSummaryLabel(t, identityTemplates, templateKey) {
  return identityTemplates.find((item) => item.key === templateKey)?.label ?? t("profile.identity.templateNone");
}

function templateDropdownOptionsHtml(t, identityTemplates, selectedKey) {
  return identityTemplates.map((item) => `
    <label class="dns-dropdown-option profile-identity-template-option" data-identity-template-option="${escapeHtml(item.label.toLowerCase())}">
      <input type="checkbox" data-identity-template-key="${item.key}" ${item.key === selectedKey ? "checked" : ""} />
      <span class="profile-identity-template-label">${escapeHtml(item.label)}</span>
    </label>
  `).join("");
}

function templateInputValue(identityState) {
  return identityState.mode === "manual" ? identityState.templateKey : "";
}

function dnsTemplateOptions(profile, t) {
  const templates = loadDnsTemplates();
  const selected = profile?.tags?.find((tag) => tag.startsWith("dns-template:"))?.replace("dns-template:", "") ?? "";
  return [
    `<option value="">${t("dns.template.custom")}</option>`,
    ...templates.map((template) => `<option value="${template.id}" ${template.id === selected ? "selected" : ""}>${escapeHtml(template.name)}</option>`)
  ].join("");
}

function globalBlocklistOptions(globalSecurity) {
  const seen = new Set();
  const options = [];
  for (const item of globalSecurity?.blocklists ?? []) {
    const id = String(item.id ?? "").trim();
    const sourceValue = String(item.sourceValue ?? item.source_value ?? "").trim();
    const uniqueKey = id || `${item.sourceKind ?? item.source_kind}:${sourceValue}`;
    if (!uniqueKey || seen.has(uniqueKey)) continue;
    seen.add(uniqueKey);
    options.push({
      id: id || uniqueKey,
      label: item.name || id || uniqueKey,
      domains: item.domains ?? [],
      active: Boolean(item.active)
    });
  }
  return options;
}

function extensionScopeAllowed(item, profile) {
  const scope = String(item.engineScope ?? "chromium/firefox").toLowerCase();
  if (scope === "firefox") return profile?.engine === "camoufox";
  if (scope === "chromium") return profile?.engine === "wayfern";
  return true;
}

function extensionLibraryOptions(model, ext, profile) {
  const items = Object.values(model?.extensionLibraryState?.items ?? {});
  return items
    .filter((item) => extensionScopeAllowed(item, profile))
    .filter((item) => !ext.enabled.includes(item.id) && !ext.disabled.includes(item.id))
    .map((item) => `<option value="${item.id}">${escapeHtml(item.displayName)} (${escapeHtml(item.engineScope ?? "chromium/firefox")})</option>`)
    .join("");
}

async function syncProfileExtensionAssignments(model, profileId, extensionState) {
  const libraryItems = Object.values(model?.extensionLibraryState?.items ?? {});
  const selectedIds = new Set([...(extensionState.enabled ?? []), ...(extensionState.disabled ?? [])]);
  for (const item of libraryItems) {
    const assigned = new Set(item.assignedProfileIds ?? []);
    const shouldAssign = selectedIds.has(item.id);
    if (shouldAssign) assigned.add(profileId);
    else assigned.delete(profileId);
    const nextAssigned = [...assigned];
    const previousAssigned = [...(item.assignedProfileIds ?? [])];
    if (JSON.stringify(nextAssigned) === JSON.stringify(previousAssigned)) continue;
    const result = await setExtensionProfiles(item.id, nextAssigned);
    if (result.ok) {
      item.assignedProfileIds = nextAssigned;
    }
  }
}

function domainStatusIcon(type) {
  if (type === "allow") {
    return `<svg viewBox="0 0 16 16" aria-hidden="true"><path d="M3.5 8.5 6.5 11.5 12.5 4.5" fill="none" stroke="currentColor" stroke-width="1.7" stroke-linecap="round" stroke-linejoin="round"/></svg>`;
  }
  return `<svg viewBox="0 0 16 16" aria-hidden="true"><path d="M4 4 12 12M12 4 4 12" fill="none" stroke="currentColor" stroke-width="1.7" stroke-linecap="round"/></svg>`;
}

function eyeIcon() {
  return `<svg viewBox="0 0 16 16" aria-hidden="true"><path d="M1.5 8s2.3-4 6.5-4 6.5 4 6.5 4-2.3 4-6.5 4-6.5-4-6.5-4Z" fill="none" stroke="currentColor" stroke-width="1.3"/><circle cx="8" cy="8" r="2.1" fill="none" stroke="currentColor" stroke-width="1.3"/></svg>`;
}

function domainStatusLabel(type, t) {
  return type === "allow" ? t("profile.security.domainAllowed") : t("profile.security.domainBlocked");
}

function buildDomainEntries(allowDomains, denyDomains) {
  return [
    ...allowDomains.map((domain) => ({ domain, type: "allow" })),
    ...denyDomains.map((domain) => ({ domain, type: "deny" }))
  ];
}

function normalizeRouteTemplateNodes(template) {
  if (Array.isArray(template?.nodes) && template.nodes.length) {
    return template.nodes.map((node, index) => ({
      id: node.id ?? node.nodeId ?? `node-${index + 1}`,
      connectionType: (node.connectionType ?? node.connection_type ?? "").toLowerCase(),
      protocol: (node.protocol ?? "").toLowerCase(),
      host: node.host ?? "",
      port: node.port != null ? Number(node.port) : 0,
      username: node.username ?? "",
      password: node.password ?? "",
      bridges: node.bridges ?? "",
      settings: node.settings ?? {}
    }));
  }
  if (template?.connectionType || template?.connection_type) {
    return [{
      id: "node-1",
      connectionType: (template.connectionType ?? template.connection_type ?? "").toLowerCase(),
      protocol: (template.protocol ?? "").toLowerCase(),
      host: template.host ?? "",
      port: template.port != null ? Number(template.port) : 0,
      username: template.username ?? "",
      password: template.password ?? "",
      bridges: template.bridges ?? "",
      settings: {}
    }];
  }
  return [];
}

function templateSupportsRouteMode(template, routeMode) {
  const nodes = normalizeRouteTemplateNodes(template);
  if (routeMode === "direct" || routeMode === "tor") return true;
  if (routeMode === "proxy") return nodes.some((node) => node.connectionType === "proxy");
  if (routeMode === "vpn") return nodes.some((node) => node.connectionType === "vpn" || node.connectionType === "v2ray");
  if (routeMode === "hybrid") {
    const hasProxy = nodes.some((node) => node.connectionType === "proxy");
    const hasVpnLike = nodes.some((node) => node.connectionType === "vpn" || node.connectionType === "v2ray");
    return hasProxy && hasVpnLike;
  }
  return false;
}

function routeTemplateOptions(routeTemplates, selectedTemplateId, routeMode, t) {
  const options = [`<option value="">${t("network.routeTemplate.none")}</option>`];
  for (const template of routeTemplates) {
    if (!templateSupportsRouteMode(template, routeMode)) continue;
    const chain = normalizeRouteTemplateNodes(template)
      .map((node) => `${t(`network.node.${node.connectionType}`)}:${node.protocol}`)
      .join(" -> ");
    options.push(`<option value="${template.id}" ${template.id === selectedTemplateId ? "selected" : ""}>${escapeHtml(template.name)} (${escapeHtml(chain)})</option>`);
  }
  return options.join("");
}

function normalizeProfileRouteMode(routeMode) {
  const normalized = String(routeMode ?? "direct").toLowerCase();
  if (normalized === "proxy" || normalized === "vpn" || normalized === "tor") {
    return normalized;
  }
  return "direct";
}

function buildRoutePolicyPayload(routeMode, selectedTemplate, killSwitchEnabled, t) {
  const base = {
    route_mode: routeMode,
    proxy: null,
    vpn: null,
    kill_switch_enabled: Boolean(killSwitchEnabled)
  };
  if (routeMode === "direct" || routeMode === "tor") return base;
  if (!selectedTemplate) {
    throw new Error(t("network.templateRequired"));
  }
  const nodes = normalizeRouteTemplateNodes(selectedTemplate);
  if (routeMode === "proxy") {
    const node = nodes.find((item) => item.connectionType === "proxy");
    if (!node) throw new Error(t("network.templateTypeMismatch"));
    base.proxy = {
      protocol: node.protocol,
      host: node.host,
      port: Number(node.port ?? 0),
      username: node.username || null,
      password: node.password || null
    };
    return base;
  }
  if (routeMode === "vpn") {
    const node = nodes.find((item) => item.connectionType === "vpn" || item.connectionType === "v2ray");
    if (!node) throw new Error(t("network.templateTypeMismatch"));
    base.vpn = {
      protocol: node.protocol,
      endpoint_host: node.host,
      endpoint_port: Number(node.port ?? 0),
      profile_ref: selectedTemplate.name
    };
    return base;
  }
  if (routeMode === "hybrid") {
    const proxyNode = nodes.find((item) => item.connectionType === "proxy");
    const vpnNode = nodes.find((item) => item.connectionType === "vpn" || item.connectionType === "v2ray");
    if (!proxyNode || !vpnNode) {
      throw new Error(t("network.templateTypeMismatch"));
    }
    base.proxy = {
      protocol: proxyNode.protocol,
      host: proxyNode.host,
      port: Number(proxyNode.port ?? 0),
      username: proxyNode.username || null,
      password: proxyNode.password || null
    };
    base.vpn = {
      protocol: vpnNode.protocol,
      endpoint_host: vpnNode.host,
      endpoint_port: Number(vpnNode.port ?? 0),
      profile_ref: selectedTemplate.name
    };
    return base;
  }
  return base;
}

function sandboxModeLabel(mode, t) {
  return t(`network.sandbox.mode.${mode}`) || mode;
}

function sandboxAdapterLabel(kind, t) {
  return t(`network.sandbox.adapter.${kind}`) || kind;
}

function profileRouteSummary(template, t) {
  if (!template) return t("network.sandbox.routeUnknown");
  const chain = normalizeRouteTemplateNodes(template)
    .map((node) => `${t(`network.node.${node.connectionType}`)}:${node.protocol}`)
    .join(" -> ");
  return `${template.name} (${chain})`;
}

function compatibleSandboxModeOptions(modes, selectedMode, t) {
  return (modes ?? [])
    .map((mode) => `<option value="${mode}" ${mode === selectedMode ? "selected" : ""}>${sandboxModeLabel(mode, t)}</option>`)
    .join("");
}

function formatProfileSandboxReason(reason, sandbox, adapter, selectedTemplate, t) {
  const value = String(reason || "").trim();
  if (!value) {
    return t("network.sandbox.unknown");
  }
  const activeRoute = profileRouteSummary(selectedTemplate, t);
  const exactMap = {
    "Template is compatible with isolated userspace runtime": "network.sandbox.reason.userspaceCompatible",
    "Profile is pinned to compatibility-native mode": "network.sandbox.reason.compatibilityPinned",
    "Legacy profile was auto-adapted to compatibility-native mode": "network.sandbox.reason.legacyMigrated",
    "Global sandbox policy allows compatibility-native fallback": "network.sandbox.reason.globalCompatibilityFallback",
    "This Amnezia profile requires a machine-wide compatibility backend; isolated mode forbids that path": "network.sandbox.reason.isolatedBlockedByNative",
    "Container sandbox mode is selected; launcher will validate the host runtime and per-profile sandbox capacity during launch": "network.sandbox.reason.containerSelected",
    "Docker Desktop container runtime is available and can build a profile-scoped isolated route helper on first launch": "network.sandbox.reason.containerReady",
    "Selected route is not compatible with container isolation yet": "network.sandbox.reason.containerUnsupported"
  };
  if (exactMap[value]) {
    return t(exactMap[value])
      .replace("{route}", activeRoute)
      .replace("{mode}", sandboxModeLabel(sandbox?.requestedMode || "auto", t));
  }
  const capacityMatch = value.match(/Container sandbox capacity is exhausted \((\d+)\/(\d+) active\)/i);
  if (capacityMatch) {
    return t("network.sandbox.reason.containerCapacity")
      .replace("{active}", capacityMatch[1])
      .replace("{max}", capacityMatch[2]);
  }
  if (value.startsWith("container runtime probe failed:")) {
    return t("network.sandbox.reason.containerProbeFailed");
  }
  if (value.startsWith("docker runtime is not installed or not reachable:")) {
    return t("network.sandbox.reason.containerRuntimeMissing");
  }
  if (value === "No resolved strategy yet") {
    return t("network.sandbox.unknown");
  }
  if (sandbox?.effectiveMode === "container" && !adapter?.available) {
    return t("network.sandbox.reason.containerProbeFailed");
  }
  return value;
}

function renderProfileSandboxFrame(preview, selectedTemplate, selectedModeOverride, t) {
  if (!selectedTemplate || !preview?.sandbox || !(preview.compatibleModes ?? []).length) {
    return "";
  }
  const sandbox = preview.sandbox;
  const adapter = sandbox.adapter ?? {};
  const selectedMode = (selectedModeOverride && (preview.compatibleModes ?? []).includes(selectedModeOverride))
    ? selectedModeOverride
    : (sandbox.preferredMode && (preview.compatibleModes ?? []).includes(sandbox.preferredMode))
    ? sandbox.preferredMode
    : (preview.compatibleModes?.includes(sandbox.effectiveMode) ? sandbox.effectiveMode : preview.compatibleModes?.[0] ?? "isolated");
  const sandboxReason = formatProfileSandboxReason(
    adapter.reason || sandbox.lastResolutionReason,
    sandbox,
    adapter,
    selectedTemplate,
    t
  );
  const nativeWarning = sandbox.effectiveMode === "blocked"
    ? ""
    : adapter.requiresSystemNetworkAccess
      ? `<p class="notice error">${t("network.sandbox.nativeWarning")}</p>`
      : sandbox.requiresNativeBackend && sandbox.effectiveMode === "container"
        ? `<p class="notice success">${t("network.sandbox.containerNativeIsolated")}</p>`
        : `<p class="meta">${t("network.sandbox.isolatedHint")}</p>`;
  return `
    <div class="security-frame" id="profile-sandbox-frame">
      <h4>${t("network.sandbox.title")}</h4>
      <div class="grid-two">
        <div>
          <strong>${t("network.sandbox.activeRoute")}</strong>
          <p>${escapeHtml(profileRouteSummary(selectedTemplate, t))}</p>
        </div>
        <div>
          <strong>${t("network.sandbox.adapterLabel")}</strong>
          <p>${escapeHtml(sandboxAdapterLabel(adapter.adapterKind || "unknown", t))}</p>
        </div>
        <div>
          <strong>${t("network.sandbox.runtimeLabel")}</strong>
          <p>${escapeHtml(adapter.runtimeKind || "unknown")}</p>
        </div>
        <div>
          <strong>${t("network.sandbox.effectiveMode")}</strong>
          <p>${escapeHtml(sandboxModeLabel(sandbox.effectiveMode, t))}</p>
        </div>
      </div>
      <label style="margin-top:12px;">${t("network.sandbox.chooseMode")}
        <select id="profile-sandbox-mode">
          ${compatibleSandboxModeOptions(preview.compatibleModes, selectedMode, t)}
        </select>
      </label>
      <p class="meta" style="margin-top:8px;">${escapeHtml(sandboxReason)}</p>
      ${nativeWarning}
      ${adapter.requiresSystemNetworkAccess ? `<p class="notice error">${t("network.sandbox.systemWideWarning")}</p>` : ""}
      ${sandbox.effectiveMode === "container" ? `<p class="notice">${t("network.sandbox.containerMvp")}</p>` : ""}
      ${sandbox.effectiveMode === "blocked" ? `<p class="notice error">${t("network.sandbox.blockedHint").replace("{route}", profileRouteSummary(selectedTemplate, t))}</p>` : ""}
    </div>
  `;
}

function modalHtml(t, profile, dnsDraft, globalSecurity, model, networkState, syncOverview, identityPreset) {
  const isRunning = profile?.state === "running";
  const searchDefault = profile?.default_search_provider ?? "duckduckgo";
  const singlePageMode = Boolean((profile?.tags ?? []).some((tag) => tag === "locked-app:custom"));
  const currentPolicy = profile?.tags?.find((x) => x.startsWith("policy:"))?.replace("policy:", "") ?? "normal";
  const ext = mergedProfileExtensions(model, profile);
  const securityFlags = profileSecurityFlags(profile);
  const selectedCertIds = certificateIds(profile);
  const selectedCertPaths = certificateLegacyPaths(profile);
  const selectedBlocklists = dnsDraft?.selectedBlocklists ?? [];
  const allowDomains = dnsDraft?.allowlist ? dnsDraft.allowlist.split(",").map((v) => v.trim()).filter(Boolean) : [];
  const denyDomains = dnsDraft?.denylist ? dnsDraft.denylist.split(",").map((v) => v.trim()).filter(Boolean) : [];
  const policyPresets = loadPolicyPresets(model.serviceCatalog);
  const policySummary = summarizePolicyPreset(policyPresets[currentPolicy]);
  const certificateOptions = (globalSecurity?.certificates ?? []).map((item) => `<option value="${item.id}">${escapeHtml(item.name)}</option>`).join("");
  const routeTemplates = networkState?.connectionTemplates ?? [];
  const selectedRouteMode = normalizeProfileRouteMode(profile ? (networkState?.payload?.route_mode ?? "direct") : "direct");
  const routeIsDirect = selectedRouteMode === "direct";
  const selectedRouteTemplateId = profile ? (networkState?.selectedTemplateId ?? "") : "";
  const routeTemplateList = routeTemplateOptions(routeTemplates, selectedRouteTemplateId, selectedRouteMode, t);
  const routeKillSwitchEnabled = profile ? (networkState?.payload?.kill_switch_enabled ?? true) : true;
  const syncServerValue = syncOverview?.controls?.server?.server_url ?? "";
  const syncKeyValue = syncOverview?.controls?.server?.key_id ?? "";
  const syncEnabled = Boolean(syncOverview?.controls?.server?.sync_enabled);
  const resolvedIdentityPreset = identityPreset ?? buildManualPreset("win_7_edge_109");
  const identityState = inferIdentityUiState(resolvedIdentityPreset);
  const identityTemplates = listIdentityTemplates(t);
  const identityPlatforms = listIdentityPlatforms(t);
  const identityTemplatePlatforms = listIdentityTemplatePlatforms(t);
  const filteredIdentityTemplates = listIdentityTemplates(t, { platformFamilies: [identityState.templatePlatform] });
  const isIdentityAuto = identityState.mode === "auto";
  return `
  <div class="profiles-modal-overlay" id="profile-modal-overlay">
    <div class="profiles-modal-window profile-modal">
      <div class="profile-modal-layout">
        <div class="tab-header profile-modal-rail">
          <button type="button" data-tab="general" class="active">${t("profile.tab.general")}</button>
          <button type="button" data-tab="identity">${t("profile.tab.identity")}</button>
          <button type="button" data-tab="vpn">${t("profile.tab.vpn")}</button>
          <button type="button" data-tab="dns">${t("profile.tab.dns")}</button>
          <button type="button" data-tab="extensions">${t("profile.tab.extensions")}</button>
          <button type="button" data-tab="security">${t("profile.tab.security")}</button>
          <button type="button" data-tab="sync">${t("profile.tab.sync")}</button>
          <button type="button" data-tab="advanced">${t("profile.tab.advanced")}</button>
        </div>

        <div class="profile-modal-main">
        ${isRunning ? `<p class="warning">${t("profile.runtime.runningWarning")}</p>` : ""}

        <form id="profile-form" data-profile-id="${profile?.id ?? ""}" class="profile-modal-form">
          <div class="tab-pane" data-pane="general">
            <div class="grid-two profile-modal-grid">
              <label>${t("profile.field.name")}<input name="name" value="${profile?.name ?? ""}" required /></label>
              <label>${t("profile.field.engine")}<select name="engine" id="profile-engine">${option("wayfern", "Wayfern Chromium", profile?.engine === "wayfern")}${option("camoufox", "Camoufox Firefox", profile?.engine === "camoufox")}</select></label>
              <label class="profile-modal-span-2 profile-description-field">${t("profile.field.description")}<textarea name="description" rows="4">${escapeHtml(profile?.description ?? "")}</textarea></label>
              <label class="profile-modal-span-2">${t("profile.field.tags")}
                ${buildTagPickerMarkup({
                  id: "profile-tags",
                  selectedTags: profileTags(profile ?? { tags: [] }) ?? [],
                  availableTags: collectProfileTags(model.profiles),
                  emptyLabel: t("profile.tags.empty"),
                  searchPlaceholder: t("profile.tags.search"),
                  createLabel: (value) => t("profile.tags.create").replace("{tag}", value)
                })}
              </label>
              <label>${t("profile.field.defaultStartPage")}<input name="defaultStartPage" value="${profile?.default_start_page ?? "https://duckduckgo.com"}" /></label>
              <label class="checkbox-inline">
                <input type="checkbox" name="singlePageMode" id="profile-single-page-mode" ${singlePageMode ? "checked" : ""} />
                <span>${t("profile.field.singlePage")}</span>
              </label>
              <label id="profile-default-search-row" class="${singlePageMode ? "hidden" : ""}">${t("profile.field.defaultSearch")}<select name="defaultSearchProvider" ${singlePageMode ? "disabled" : ""}>${searchOptions(searchDefault)}</select></label>
              <p class="meta profile-modal-span-2 ${singlePageMode ? "" : "hidden"}" id="profile-single-page-hint">${t("profile.field.singlePageHint")}</p>
              <label class="checkbox-inline profile-modal-span-2">
                <input type="checkbox" name="panicFrameEnabled" ${profile?.panic_frame_enabled ? "checked" : ""} />
                <span>${t("profile.field.panicFrame")}</span>
              </label>
              <label class="profile-modal-span-2 ${profile?.panic_frame_enabled ? "" : "hidden"}" id="profile-panic-color-row">${t("profile.field.panicFrameColor")}<input type="color" name="panicFrameColor" value="${escapeHtml(profile?.panic_frame_color ?? "#ff8652")}" /></label>
            </div>
          </div>

          <div class="tab-pane hidden" data-pane="identity">
            <div class="grid-two">
              <label>${t("profile.field.identityMode")}
                <select name="identityMode" id="profile-identity-mode">
                  <option value="auto" ${isIdentityAuto ? "selected" : ""}>${t("identity.mode.auto")}</option>
                  <option value="manual" ${!isIdentityAuto ? "selected" : ""}>${t("identity.mode.manual")}</option>
                </select>
              </label>
              <label id="profile-identity-platform-row" class="${isIdentityAuto ? "" : "hidden"}">${t("profile.field.platformTarget")}
                <select name="platformTarget" id="profile-platform-target">
                  ${identityPlatforms.map((item) => `<option value="${item.key}" ${item.key === identityState.autoPlatform ? "selected" : ""}>${escapeHtml(item.label)}</option>`).join("")}
                </select>
              </label>
            </div>
            <div class="security-frame ${isIdentityAuto ? "hidden" : ""}" id="profile-identity-template-row">
              <label>${t("identity.field.platformTemplate")}
                <select id="profile-identity-template-platform">
                  ${identityTemplatePlatforms.map((item) => `<option value="${item.key}" ${item.key === identityState.templatePlatform ? "selected" : ""}>${escapeHtml(item.label)}</option>`).join("")}
                </select>
              </label>
              <label>${t("identity.field.displayName")}
                <input type="text" name="identityDisplayName" id="profile-identity-display-name" value="${escapeHtml(resolvedIdentityPreset?.display_name ?? templateSummaryLabel(t, identityTemplates, identityState.templateKey))}" />
              </label>
              <h4>${t("profile.identity.template")}</h4>
              <div class="dns-dropdown profile-identity-template-dropdown">
                <button type="button" class="dns-dropdown-toggle" id="profile-identity-template-toggle">
                  <span id="profile-identity-template-summary">${escapeHtml(templateSummaryLabel(t, identityTemplates, identityState.templateKey))}</span>
                </button>
                <input type="hidden" name="identityTemplate" value="${escapeHtml(templateInputValue(identityState))}" />
                <div class="dns-dropdown-menu hidden" id="profile-identity-template-menu">
                  <input id="profile-identity-template-search" placeholder="${t("profile.identity.templateSearch")}" />
                  <div id="profile-identity-template-options">
                    ${templateDropdownOptionsHtml(t, filteredIdentityTemplates, identityState.templateKey)}
                  </div>
                </div>
              </div>
            </div>
            <p class="meta ${isIdentityAuto ? "" : "hidden"}" id="profile-identity-auto-hint">${t("identity.autoHint")}</p>
            <div id="profile-identity-state" data-preset="${escapeHtml(JSON.stringify(resolvedIdentityPreset))}" data-ui="${escapeHtml(JSON.stringify(identityState))}"></div>
            <div id="profile-identity-templates" data-templates="${escapeHtml(JSON.stringify(identityTemplates.map((item) => ({ key: item.key, label: item.label, autoPlatform: item.autoPlatform, platformFamily: item.platformFamily }))))}"></div>
            <div id="profile-identity-platforms" data-platforms="${escapeHtml(JSON.stringify(identityPlatforms))}"></div>
          </div>

          <div class="tab-pane hidden" data-pane="vpn">
            <div class="grid-two profile-modal-grid">
              <label>${t("profile.field.routeMode")}
                <select name="profileRouteMode" id="profile-route-mode">
                  <option value="direct" ${selectedRouteMode === "direct" ? "selected" : ""}>${t("network.mode.direct")}</option>
                  <option value="proxy" ${selectedRouteMode === "proxy" ? "selected" : ""}>${t("network.mode.proxy")}</option>
                  <option value="vpn" ${selectedRouteMode === "vpn" ? "selected" : ""}>${t("network.mode.vpn")}</option>
                  <option value="tor" ${selectedRouteMode === "tor" ? "selected" : ""}>${t("network.mode.tor")}</option>
                </select>
              </label>
              <label id="profile-route-template-row" class="${routeIsDirect ? "hidden" : ""}">${t("network.routeTemplate")}
                <select name="profileRouteTemplateId" id="profile-route-template" ${selectedRouteMode === "direct" ? "disabled" : ""}>
                  ${routeTemplateList}
                </select>
              </label>
              <label class="checkbox-inline ${routeIsDirect ? "hidden" : ""}" id="profile-kill-switch-row">
                <input type="checkbox" name="profileKillSwitch" ${routeKillSwitchEnabled ? "checked" : ""} ${routeIsDirect ? "disabled" : ""}/>
                <span>${t("network.killSwitch")}</span>
              </label>
            </div>
            <div id="profile-sandbox-frame-slot"></div>
          </div>

          <div class="tab-pane hidden profile-pane-plain" data-pane="dns">
            <div class="security-frame">
              <div class="grid-two">
                <label>${t("profile.field.dnsMode")}<select name="dnsMode" id="profile-dns-mode"><option value="system" ${(dnsDraft?.mode ?? "system") === "system" ? "selected" : ""}>${t("dns.system")}</option><option value="custom" ${(dnsDraft?.mode ?? "system") === "custom" ? "selected" : ""}>${t("dns.custom")}</option></select></label>
                <label id="profile-dns-servers-row">${t("profile.field.dnsServers")}<input name="dnsServers" placeholder="1.1.1.1,8.8.8.8" value="${escapeHtml(dnsDraft?.servers ?? "")}" /></label>
                <label id="profile-dns-template-row">${t("dns.template.current")}<select name="dnsTemplateId">${dnsTemplateOptions(profile, t)}</select></label>
              </div>
            </div>
            <div class="security-frame">
              <h4>${t("profile.policy")}</h4>
              <label><select name="policyLevel"><option value="light" ${currentPolicy === "light" ? "selected" : ""}>light</option><option value="normal" ${currentPolicy === "normal" ? "selected" : ""}>normal</option><option value="high" ${currentPolicy === "high" ? "selected" : ""}>high</option><option value="maximum" ${currentPolicy === "maximum" ? "selected" : ""}>maximum</option></select></label>
              <p class="meta" id="profile-policy-summary">${escapeHtml(`${policySummary.blocklists} ${t("dns.policy.summary.blocklists")} • ${policySummary.blockedServices} ${t("dns.policy.summary.services")} • ${policySummary.allowDomains} ${t("dns.policy.summary.allow")} • ${policySummary.denyDomains} ${t("dns.policy.summary.deny")}`)}</p>
              <div class="top-actions"><button type="button" id="profile-policy-load">${t("dns.policy.load")}</button></div>
            </div>
            <div class="security-frame">
              <h4>${t("profile.dns.blocklists")}</h4>
              <div class="dns-dropdown profile-blocklist-dropdown">
                <button type="button" class="dns-dropdown-toggle" id="profile-blocklists-toggle">
                  <span id="profile-blocklists-summary">${t("profile.dns.selectBlocklists")}</span>
                </button>
                <div class="dns-dropdown-menu hidden profile-blocklists-menu" id="profile-blocklists-menu">
                  <input id="profile-blocklists-search" placeholder="${t("dns.searchPlaceholder")}" />
                  <div class="top-actions">
                    <button type="button" id="profile-blocklists-select-all">${t("security.all")}</button>
                  </div>
                  <div id="profile-blocklists-options">
                    ${globalBlocklistOptions(globalSecurity).map((item) => `
                      <label class="dns-blocklist-option" data-profile-blocklist-option="${escapeHtml((item.label ?? item.id).toLowerCase())}">
                        <input type="checkbox" data-profile-blocklist-id="${item.id}" ${selectedBlocklists.includes(item.id) || item.active ? "checked" : ""} ${item.active ? "disabled" : ""} />
                        <span>${escapeHtml(item.label)}</span>
                        ${item.active ? `<span class="meta">${t("security.active")}</span>` : ""}
                      </label>
                    `).join("")}
                  </div>
                </div>
              </div>
            </div>
            <div class="security-frame">
              <div class="row-between">
                <div>
                  <h4>${t("profile.security.domains")}</h4>
                  <p class="meta">${t("profile.security.domainsHint")}</p>
                </div>
              </div>
              <div class="grid-two profile-modal-grid">
                <label>${t("profile.security.domainInput")}<input name="domainEntry" id="profile-domain-input" list="profile-domain-suggestions" placeholder="example.com" /></label>
                <label>${t("profile.security.domainStatus")}<select name="domainEntryType" id="profile-domain-type"><option value="deny">${t("profile.security.domainBlocked")}</option><option value="allow">${t("profile.security.domainAllowed")}</option></select></label>
                <label>${t("profile.security.domainSearch")}<input name="domainSearch" id="profile-domain-search" placeholder="${t("profile.security.domainSearch")}" /></label>
                <label>${t("profile.security.domainFilter")}<select name="domainFilter" id="profile-domain-filter"><option value="all">${t("profile.security.domainFilterAll")}</option><option value="deny">${t("profile.security.domainBlocked")}</option><option value="allow">${t("profile.security.domainAllowed")}</option></select></label>
              </div>
              <div class="top-actions">
                <button type="button" id="profile-domain-add">${t("profile.security.domainAdd")}</button>
              </div>
              <datalist id="profile-domain-suggestions">
                ${DOMAIN_OPTIONS.map((value) => `<option value="${value}"></option>`).join("")}
              </datalist>
              <table class="extensions-table">
                <thead><tr><th>${t("profile.security.domainStatus")}</th><th>${t("security.domain")}</th><th>${t("extensions.actions")}</th></tr></thead>
                <tbody id="profile-domain-table" data-domains="${escapeHtml(JSON.stringify(buildDomainEntries(allowDomains, denyDomains)))}"></tbody>
              </table>
            </div>
          </div>

          <div class="tab-pane hidden" data-pane="extensions">
            <div class="grid-two profile-extension-toolbar">
              <label>${t("profile.field.extensionIds")}
                <select name="extensionSelect">
                  <option value="">${t("security.selectExtension")}</option>
                  ${extensionLibraryOptions(model, ext, profile)}
                </select>
              </label>
              <label class="profile-toolbar-action">&nbsp;<button type="button" class="profile-toolbar-button" id="profile-extension-add">${t("extensions.add")}</button></label>
            </div>
            <table class="extensions-table">
              <thead><tr><th>${t("extensions.name")}</th><th>${t("extensions.status")}</th><th>${t("extensions.actions")}</th></tr></thead>
              <tbody id="profile-extensions-table" data-enabled="${escapeHtml(JSON.stringify(ext.enabled))}" data-disabled="${escapeHtml(JSON.stringify(ext.disabled))}"></tbody>
            </table>
          </div>

          <div class="tab-pane hidden" data-pane="security">
            <div class="security-toggle-list">
              <label class="checkbox-inline security-toggle-row"><input type="checkbox" name="passwordLock" ${profile?.password_lock_enabled ? "checked" : ""}/> <span>${t("profile.field.passwordLock")}</span></label>
              <div class="grid-two profile-modal-grid hidden" id="profile-password-fields">
                <label>${t("profile.security.password")} 
                  <span class="profile-password-field">
                    <input type="password" name="profilePassword" autocomplete="new-password" />
                    <button type="button" class="profile-password-toggle" id="profile-password-toggle" aria-label="${t("profile.security.showPassword")}">${eyeIcon()}</button>
                  </span>
                </label>
                <label>${t("profile.security.passwordConfirm")}<input type="password" name="profilePasswordConfirm" autocomplete="new-password" /></label>
              </div>
              <label class="checkbox-inline security-toggle-row"><input type="checkbox" name="ephemeral" ${profile?.ephemeral_mode ? "checked" : ""}/> <span>${t("profile.field.ephemeral")}</span></label>
              <label class="checkbox-inline security-toggle-row"><input type="checkbox" name="disableExtensionsLaunch" ${securityFlags.disableExtensionsLaunch ? "checked" : ""}/> <span>${t("profile.security.disableExtensionsLaunch")}</span></label>
              <label class="checkbox-inline security-toggle-row"><input type="checkbox" name="allowSystemAccess" ${securityFlags.allowSystemAccess ? "checked" : ""}/> <span>${t("profile.security.allowSystemAccess")}</span></label>
              <label class="checkbox-inline security-toggle-row"><input type="checkbox" name="allowKeepassxc" ${securityFlags.allowKeepassxc ? "checked" : ""}/> <span>${t("profile.security.allowKeepassxc")}</span></label>
            </div>
            <div class="security-frame">
              <h4>${t("security.certificates.customTitle")}</h4>
              <p class="meta">${t("security.certificates.hint")}</p>
              <div id="profile-certificate-engine-guard"></div>
              <div class="grid-two profile-certificates-toolbar">
                <label>${t("security.certificates.profile")}<select name="profileCertificateSelect"><option value="">${t("security.selectCertificate")}</option>${certificateOptions}</select></label>
                <label class="profile-toolbar-action">&nbsp;<button type="button" class="profile-toolbar-button" id="profile-certificate-add">${t("security.certificates.add")}</button></label>
                <label class="profile-modal-span-2 profile-toolbar-action">&nbsp;<button type="button" class="profile-toolbar-button profile-toolbar-button-wide" id="profile-certificate-pick">${t("security.certificates.pickFiles")}</button></label>
              </div>
              <table class="extensions-table">
                <thead><tr><th>${t("extensions.name")}</th><th>${t("extensions.actions")}</th></tr></thead>
                <tbody
                  id="profile-certificates-table"
                  data-certificate-ids="${escapeHtml(JSON.stringify(selectedCertIds))}"
                  data-certificate-paths="${escapeHtml(JSON.stringify(selectedCertPaths))}"
                ></tbody>
              </table>
            </div>
          </div>

          <div class="tab-pane hidden" data-pane="sync">
            <div class="grid-two">
              <label>${t("profile.sync.server")}<input name="syncServer" value="${escapeHtml(syncServerValue)}" placeholder="https://sync.example" /></label>
              <label>${t("profile.sync.key")}<input name="syncKey" value="${escapeHtml(syncKeyValue)}" placeholder="generated-key-id" /></label>
              <label class="checkbox-inline"><input name="syncEnabled" type="checkbox" ${syncEnabled ? "checked" : ""}/> ${t("sync.enabled")}</label>
            </div>
          </div>

          <div class="tab-pane hidden" data-pane="advanced">
            <label>${t("profile.advanced.launchHook")}<input name="launchHook" placeholder="https://hook.example/start" /></label>
          </div>

          <footer class="modal-actions">
            <button type="button" id="profile-cancel">${t("action.cancel")}</button>
            <button type="submit">${t("action.save")}</button>
          </footer>
        </form>
        </div>
      </div>
    </div>
  </div>`;
}

async function askInput(root, t, title, label, defaultValue = "") {
  return askInputModal(t, {
    title,
    label,
    defaultValue
  });
}

async function askConfirm(root, t, title, description) {
  return askConfirmModal(t, {
    title,
    description
  });
}

function setNotice(model, type, text) {
  model.profileNotice = { type, text, at: Date.now() };
}

function resolveProfileErrorMessage(t, errorText) {
  const text = String(errorText ?? "");
  const keyMap = {
    "profile_protection.locked_profile_requires_unlock": "profile.security.lockedLaunchBlocked",
    "profile_protection.system_access_forbidden": "profile.security.systemAccessBlocked",
    "profile_protection.keepassxc_forbidden": "profile.security.keepassxcBlocked",
    "profile_protection.maximum_policy_extensions_forbidden": "profile.security.maximumPolicyExtensionsBlocked",
    "profile_protection.cookies_copy_blocked": "profile.security.cookiesCopyBlocked",
    "profile.security.wayfern_certificates_not_supported": "profile.security.wayfernCertificatesBlocked"
  };
  for (const [marker, key] of Object.entries(keyMap)) {
    if (text.includes(marker)) {
      return t(key);
    }
  }
  return text;
}

function postureFindingLines(t, report) {
  const findings = Array.isArray(report?.findings) ? report.findings : [];
  return findings.map((item) => `- ${t(item.labelKey)}${item.detail ? `: ${item.detail}` : ""}`).join("\n");
}

function resolveDevicePostureAction(errorText) {
  const text = String(errorText ?? "");
  if (text.startsWith("device_posture.confirm_required:")) {
    return {
      kind: "confirm",
      reportId: text.split(":").slice(1).join(":")
    };
  }
  if (text.startsWith("device_posture.refused:")) {
    return {
      kind: "refused",
      reportId: text.split(":").slice(1).join(":")
    };
  }
  return null;
}

export function renderProfilesSection(t, model) {
  const selectedIds = selectionState(model);
  const rows = model.profiles.map((profile) => rowHtml(profile, selectedIds.includes(profile.id), t)).join("");
  const notice = model.profileNotice ? `<p class="notice ${model.profileNotice.type}">${model.profileNotice.text}</p>` : "";
  const allSelected = model.profiles.length > 0 && selectedIds.length === model.profiles.length;
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
      ${notice}
      <div class="profiles-table-shell">
        <table class="profiles-table">
          <thead>
            <tr>
              <th class="profiles-col-check"><input type="checkbox" id="profiles-select-all" ${allSelected ? "checked" : ""} /></th>
              <th class="profiles-col-engine"></th>
              <th>${t("profile.field.name")}</th>
              <th>${t("profile.tags")}</th>
              <th>${t("profile.table.note")}</th>
              <th class="profiles-col-actions"></th>
            </tr>
          </thead>
          <tbody>
            ${rows || `<tr><td colspan="6" class="profiles-empty">${t("profile.empty")}</td></tr>`}
          </tbody>
        </table>
      </div>
      ${selectionBarHtml(model, t)}
    </div>
  `;
}

export function renderProfiles(t, model) {
  return `
    <div class="feature-page">
      ${renderProfilesSection(t, model)}
    </div>
  `;
}

export async function hydrateProfilesModel(model) {
  const res = await listProfiles();
  model.profiles = res.ok ? res.data : [];
  const selected = new Set(selectionState(model));
  model.selectedProfileIds = model.profiles.map((profile) => profile.id).filter((id) => selected.has(id));
}

async function exportProfileArchive(root, model, rerender, t, profileId) {
  const passphrase = await askInput(root, t, t("profile.export.title"), t("profile.export.passphrase"));
  if (!passphrase) return;
  const data = await exportProfile(profileId, passphrase);
  if (data.ok) {
    await navigator.clipboard?.writeText(data.data.archive_json);
    setNotice(model, "success", t("profile.export.copied"));
  } else {
    setNotice(model, "error", String(data.data.error));
  }
  await hydrateProfilesModel(model);
  rerender();
}

export function wireProfiles(root, model, rerender, t) {
  if (!(model.profileActionPendingIds instanceof Set)) {
    model.profileActionPendingIds = new Set();
  }
  let floatingTagTooltip = document.body.querySelector("#profiles-floating-tag-tooltip");
  if (!floatingTagTooltip) {
    floatingTagTooltip = document.createElement("div");
    floatingTagTooltip.id = "profiles-floating-tag-tooltip";
    floatingTagTooltip.className = "profiles-floating-tooltip hidden";
    document.body.appendChild(floatingTagTooltip);
  }
  const hideFloatingTagTooltip = () => {
    floatingTagTooltip._activeTrigger = null;
    floatingTagTooltip.classList.add("hidden");
    floatingTagTooltip.innerHTML = "";
  };
  const positionFloatingTagTooltip = (trigger) => {
    const rect = trigger.getBoundingClientRect();
    const margin = 8;
    floatingTagTooltip.style.left = `${Math.max(12, Math.min(rect.left, window.innerWidth - floatingTagTooltip.offsetWidth - 12))}px`;
    let top = rect.bottom + margin;
    if (top + floatingTagTooltip.offsetHeight > window.innerHeight - 12) {
      top = Math.max(12, rect.top - floatingTagTooltip.offsetHeight - margin);
    }
    floatingTagTooltip.style.top = `${top}px`;
  };
  const showFloatingTagTooltip = (trigger) => {
    const raw = trigger.getAttribute("data-tag-tooltip-tags") || "";
    const tags = raw.split("\n").map((value) => value.trim()).filter(Boolean);
    if (!tags.length) {
      hideFloatingTagTooltip();
      return;
    }
    floatingTagTooltip.innerHTML = tags
      .map((tag) => `<span class="profiles-tag">${escapeHtml(tag)}</span>`)
      .join("");
    floatingTagTooltip.classList.remove("hidden");
    floatingTagTooltip._activeTrigger = trigger;
    positionFloatingTagTooltip(trigger);
  };
  if (!floatingTagTooltip.dataset.bound) {
    window.addEventListener("scroll", () => {
      if (floatingTagTooltip._activeTrigger) positionFloatingTagTooltip(floatingTagTooltip._activeTrigger);
    }, { passive: true });
    window.addEventListener("resize", () => {
      if (floatingTagTooltip._activeTrigger) positionFloatingTagTooltip(floatingTagTooltip._activeTrigger);
    });
    document.addEventListener("pointerdown", (event) => {
      if (
        floatingTagTooltip._activeTrigger
        && !event.target?.closest?.("[data-tag-tooltip-trigger]")
        && !event.target?.closest?.("#profiles-floating-tag-tooltip")
      ) {
        hideFloatingTagTooltip();
      }
    });
    floatingTagTooltip.dataset.bound = "true";
  }
  root.querySelector("#profile-create")?.addEventListener("click", () => openProfileModal(root, model, rerender, t, null));
  root.querySelector("#profile-import")?.addEventListener("click", async () => {
    const archiveJson = await askInput(root, t, t("profile.import.title"), t("profile.import.archive"));
    if (!archiveJson) return;
    const expectedProfileId = await askInput(root, t, t("profile.import.title"), t("profile.import.profileId"));
    if (!expectedProfileId) return;
    const passphrase = await askInput(root, t, t("profile.import.title"), t("profile.import.passphrase"));
    if (!passphrase) return;
    const result = await importProfile(archiveJson, expectedProfileId, passphrase);
    setNotice(model, result.ok ? "success" : "error", result.ok ? t("profile.import.success") : String(result.data.error));
    await hydrateProfilesModel(model);
    rerender();
  });

  root.querySelector("#profiles-select-all")?.addEventListener("change", (event) => {
    model.selectedProfileIds = event.target.checked ? model.profiles.map((profile) => profile.id) : [];
    rerender();
  });

  for (const checkbox of root.querySelectorAll(".profile-select")) {
    checkbox.addEventListener("change", (event) => {
      const profileId = checkbox.getAttribute("data-select-id");
      const selectedIds = new Set(selectionState(model));
      if (event.target.checked) selectedIds.add(profileId);
      else selectedIds.delete(profileId);
      model.selectedProfileIds = [...selectedIds];
      rerender();
    });
  }

  for (const trigger of root.querySelectorAll("[data-tag-tooltip-trigger]")) {
    trigger.addEventListener("mouseenter", () => showFloatingTagTooltip(trigger));
    trigger.addEventListener("focus", () => showFloatingTagTooltip(trigger));
    trigger.addEventListener("mouseleave", () => {
      if (floatingTagTooltip._activeTrigger === trigger) hideFloatingTagTooltip();
    });
    trigger.addEventListener("blur", () => {
      if (floatingTagTooltip._activeTrigger === trigger) hideFloatingTagTooltip();
    });
  }

  for (const row of root.querySelectorAll(".profiles-row")) {
    row.addEventListener("click", async (event) => {
      const action = event.target?.closest?.("[data-action]")?.getAttribute?.("data-action");
      if (!action) return;
      const profileId = row.getAttribute("data-profile-id");
      const profile = model.profiles.find((item) => item.id === profileId);
      if (!profile) return;
      if ((action === "launch" || action === "stop") && model.profileActionPendingIds.has(profileId)) return;

      if (action === "launch") {
        model.profileActionPendingIds.add(profileId);
        model.profileLaunchOverlay = {
          profileId,
          stageKey: "starting",
          messageKey: "profile.launchProgress.starting",
          done: false
        };
        rerender();
        try {
          const launchResult = await launchProfile(profileId);
          if (!launchResult.ok) {
            model.profileLaunchOverlay = null;
            const errorText = String(launchResult.data.error);
            const postureAction = resolveDevicePostureAction(errorText);
            if (postureAction) {
              const postureResult = await getDevicePostureReport();
              const report = postureResult.ok ? postureResult.data : null;
              const detail = report ? postureFindingLines(t, report) : "";
              if (postureAction.kind === "confirm") {
                const accepted = await askConfirm(
                  root,
                  t,
                  t("devicePosture.confirmTitle"),
                  `${t("devicePosture.confirmDescription")}${detail ? `\n\n${detail}` : ""}`
                );
                if (accepted) {
                  const confirmedLaunch = await launchProfile(profileId, null, postureAction.reportId);
                  setNotice(
                    model,
                    confirmedLaunch.ok ? "success" : "error",
                    confirmedLaunch.ok ? t("profile.notice.launched") : resolveProfileErrorMessage(t, confirmedLaunch.data.error)
                  );
                }
              } else {
                setNotice(
                  model,
                  "error",
                  `${t("devicePosture.refusedDescription")}${detail ? ` ${detail}` : ""}`
                );
              }
            } else if (errorText.includes("wayfern_terms_not_acknowledged") || errorText.includes("wayfern_terms_ack_stale")) {
              const accepted = await askConfirm(root, t, t("profile.wayfernTerms.title"), t("profile.wayfernTerms.description"));
              if (accepted) {
                const ackResult = await acknowledgeWayfernTos(profileId);
                if (!ackResult.ok) {
                  setNotice(model, "error", resolveProfileErrorMessage(t, ackResult.data.error));
                } else {
                  model.wayfernTermsStatus = { pendingProfileIds: [] };
                  const relaunched = await launchProfile(profileId);
                  setNotice(model, relaunched.ok ? "success" : "error", relaunched.ok ? t("profile.notice.launched") : resolveProfileErrorMessage(t, relaunched.data.error));
                }
              }
            } else if (errorText.includes("profile.security.wayfern_certificates_not_supported")) {
              const accepted = await askConfirm(
                root,
                t,
                t("profile.security.wayfernCertificatesBlockedTitle"),
                t("profile.security.wayfernCertificatesBlockedDescription")
              );
              if (accepted) {
                await openProfileModal(root, model, rerender, t, profile);
              } else {
                setNotice(model, "error", resolveProfileErrorMessage(t, errorText));
              }
            } else {
              setNotice(model, "error", resolveProfileErrorMessage(t, errorText));
            }
          } else {
            setNotice(model, "success", t("profile.notice.launched"));
          }
        } finally {
          model.profileActionPendingIds.delete(profileId);
        }
      }

      if (action === "stop") {
        model.profileActionPendingIds.add(profileId);
        try {
          const stopResult = await stopProfile(profileId);
          setNotice(model, stopResult.ok ? "success" : "error", stopResult.ok ? t("profile.notice.stopped") : String(stopResult.data.error));
        } finally {
          model.profileActionPendingIds.delete(profileId);
        }
      }

      if (action === "edit") {
        return openProfileModal(root, model, rerender, t, profile);
      }

      if (action === "delete") {
        const confirmed = await askConfirm(root, t, t("profile.delete.title"), t("profile.delete.confirm"));
        if (confirmed) {
          const result = await deleteProfile(profileId);
          setNotice(model, result.ok ? "success" : "error", result.ok ? t("profile.notice.deleted") : String(result.data.error));
        }
      }

      await hydrateProfilesModel(model);
      rerender();
    });
  }

  root.querySelector("#profiles-clear-selection")?.addEventListener("click", () => {
    model.selectedProfileIds = [];
    rerender();
  });

  root.querySelector("#profiles-add-group")?.addEventListener("click", async () => {
    const groupName = await askInput(root, t, t("profile.bulk.addGroup"), t("profile.bulk.groupName"));
    if (!groupName) return;
    await applyBulkTag(model, "group", groupName);
    setNotice(model, "success", t("profile.bulk.groupSaved"));
    await hydrateProfilesModel(model);
    rerender();
  });

  root.querySelector("#profiles-add-ext-group")?.addEventListener("click", async () => {
    const groupName = await askInput(root, t, t("profile.bulk.addExtGroup"), t("profile.bulk.extGroupName"));
    if (!groupName) return;
    await applyBulkTag(model, "ext-group", groupName);
    setNotice(model, "success", t("profile.bulk.extGroupSaved"));
    await hydrateProfilesModel(model);
    rerender();
  });

  root.querySelector("#profiles-export-selection")?.addEventListener("click", async () => {
    const selectedIds = selectionState(model);
    if (selectedIds.length !== 1) return;
    await exportProfileArchive(root, model, rerender, t, selectedIds[0]);
  });

  root.querySelector("#profiles-copy-cookies")?.addEventListener("click", () => openCopyCookiesModal(root, model, rerender, t));
}

async function applyBulkTag(model, prefix, value) {
  const normalized = `${prefix}:${value.trim()}`;
  for (const profileId of selectionState(model)) {
    const profile = model.profiles.find((item) => item.id === profileId);
    if (!profile) continue;
    const baseTags = (profile.tags ?? []).filter((tag) => !tag.startsWith(`${prefix}:`));
    baseTags.push(normalized);
    await updateProfile({
      profileId: profile.id,
      tags: baseTags,
      expectedUpdatedAt: profile.updated_at
    });
  }
}

function openCopyCookiesModal(root, model, rerender, t) {
  const selectedProfiles = model.profiles.filter((profile) => selectionState(model).includes(profile.id));
  document.body.insertAdjacentHTML("beforeend", copyCookiesModalHtml(t, model.profiles, selectedProfiles));
  const overlay = document.body.querySelector("#profile-cookie-overlay");
  const close = () => overlay.remove();
  overlay.querySelector("#profile-cookie-close")?.addEventListener("click", close);
  overlay.querySelector("#profile-cookie-cancel")?.addEventListener("click", close);
  overlay.addEventListener("click", (event) => {
    if (event.target === overlay) close();
  });
  overlay.querySelector("#profile-cookie-submit")?.addEventListener("click", async () => {
    const sourceProfileId = overlay.querySelector("#profile-cookie-source")?.value?.trim();
    if (!sourceProfileId) return;
    const result = await copyProfileCookies(sourceProfileId, selectedProfiles.map((profile) => profile.id));
    if (result.ok) {
      const skipped = result.data.skipped_targets?.length ? ` ${t("profile.cookies.skipped")}: ${result.data.skipped_targets.length}.` : "";
      setNotice(model, "success", `${t("profile.cookies.copied")} ${result.data.copied_targets}.${skipped}`);
      close();
      await hydrateProfilesModel(model);
      rerender();
      return;
    }
    setNotice(model, "error", resolveProfileErrorMessage(t, result.data.error));
    close();
    rerender();
  });
}

async function openProfileModal(root, model, rerender, t, existing) {
  if (!model.extensionLibraryState) {
    const libraryResult = await listExtensionLibrary();
    if (libraryResult.ok) {
      try {
        model.extensionLibraryState = JSON.parse(libraryResult.data || "{}");
      } catch {}
    }
  }
  if (!model.serviceCatalog) {
    const catalogResult = await getServiceCatalog();
    if (catalogResult.ok) {
      try {
        model.serviceCatalog = JSON.parse(catalogResult.data);
      } catch {}
    }
  }
  let globalSecurity = { certificates: [], blocklists: [] };
  const globalSecurityResult = await getGlobalSecuritySettings();
  if (globalSecurityResult.ok) {
    try {
      globalSecurity = normalizeGlobalSecuritySettings(globalSecurityResult.data);
    } catch {}
  }
  const dnsDraftKey = existing?.id ?? "create-profile";
  const dnsDraft = loadProfileDnsDraft(dnsDraftKey, model.serviceCatalog);
  let profileNetworkState = { payload: null, selectedTemplateId: null, connectionTemplates: [] };
  const networkStateResult = await getNetworkState(existing?.id ?? "");
  if (networkStateResult.ok) {
    try {
      profileNetworkState = JSON.parse(networkStateResult.data || "{}");
    } catch {}
  }
  let syncOverview = null;
  if (existing?.id) {
    const syncResult = await getSyncOverview(existing.id);
    if (syncResult.ok) {
      syncOverview = syncResult.data;
    }
  }
  let identityPreset = null;
  if (existing?.id) {
    const identityResult = await getIdentityProfile(existing.id);
    if (identityResult.ok) {
      identityPreset = identityResult.data ?? null;
    }
  }
  if (!identityPreset) {
    identityPreset = buildManualPreset("win_7_edge_109");
  }
  document.body.insertAdjacentHTML(
    "beforeend",
    modalHtml(t, existing, dnsDraft, globalSecurity, model, profileNetworkState, syncOverview, identityPreset)
  );
  const overlay = document.body.querySelector("#profile-modal-overlay");
  showModalOverlay(overlay);
  const form = document.body.querySelector("#profile-form");
  let dirty = false;
  const profileRouteMode = overlay.querySelector("#profile-route-mode");
  const profileRouteTemplate = overlay.querySelector("#profile-route-template");
  const profileRouteTemplateRow = overlay.querySelector("#profile-route-template-row");
  const profileKillSwitchRow = overlay.querySelector("#profile-kill-switch-row");
  const profileKillSwitchInput = overlay.querySelector("[name='profileKillSwitch']");
  const profileSandboxSlot = overlay.querySelector("#profile-sandbox-frame-slot");
  let profileSandboxPreview = null;
  let initialSandboxMode = null;
  let draftSandboxMode = null;
  const selectedRouteTemplateFromForm = () => {
    const routeTemplateId = profileRouteTemplate?.value || "";
    return (profileNetworkState.connectionTemplates ?? []).find((item) => item.id === routeTemplateId) ?? null;
  };
  const bindProfileSandboxSelect = () => {
    const select = overlay.querySelector("#profile-sandbox-mode");
    select?.addEventListener("change", async () => {
      dirty = true;
      draftSandboxMode = select.value || null;
      await refreshProfileSandboxFrame();
    });
  };
  const refreshProfileSandboxFrame = async () => {
    if (!profileSandboxSlot || !profileRouteMode || !profileRouteTemplate) return;
    const routeMode = normalizeProfileRouteMode(profileRouteMode.value || "direct");
    const selectedTemplate = selectedRouteTemplateFromForm();
    if (routeMode === "direct" || !selectedTemplate) {
      profileSandboxPreview = null;
      initialSandboxMode = null;
      profileSandboxSlot.innerHTML = "";
      return;
    }
    const preferredMode =
      draftSandboxMode
      || overlay.querySelector("#profile-sandbox-mode")?.value
      || profileNetworkState.sandbox?.preferredMode
      || null;
    const previewResult = await previewNetworkSandboxSettings({
      profileId: existing?.id ?? null,
      routeMode,
      templateId: selectedTemplate.id,
      preferredMode
    });
    if (!previewResult.ok) {
      profileSandboxPreview = null;
      profileSandboxSlot.innerHTML = "";
      return;
    }
    profileSandboxPreview = previewResult.data;
    if (!initialSandboxMode) {
      initialSandboxMode = profileSandboxPreview.sandbox.preferredMode
        || profileSandboxPreview.sandbox.effectiveMode
        || profileSandboxPreview.compatibleModes?.[0]
        || null;
    }
    if (!draftSandboxMode) {
      draftSandboxMode = preferredMode || initialSandboxMode;
    }
    profileSandboxSlot.innerHTML = renderProfileSandboxFrame(
      profileSandboxPreview,
      selectedTemplate,
      draftSandboxMode,
      t
    );
    bindProfileSandboxSelect();
  };
  const refreshRouteTemplateOptions = () => {
    if (!profileRouteTemplate || !profileRouteMode) return;
    const routeMode = normalizeProfileRouteMode(profileRouteMode.value || "direct");
    profileRouteMode.value = routeMode;
    const routeIsDirect = routeMode === "direct";
    profileRouteTemplateRow?.classList.toggle("hidden", routeIsDirect);
    if (profileKillSwitchRow) {
      profileKillSwitchRow.classList.toggle("hidden", routeIsDirect);
    }
    if (profileKillSwitchInput) {
      profileKillSwitchInput.disabled = routeIsDirect;
    }
    if (routeIsDirect) {
      profileRouteTemplate.disabled = true;
      profileRouteTemplate.value = "";
      return;
    }
    const currentValue = profileRouteTemplate.value || "";
    profileRouteTemplate.disabled = false;
    profileRouteTemplate.innerHTML = routeTemplateOptions(
      profileNetworkState.connectionTemplates ?? [],
      currentValue,
      routeMode,
      t
    );
    if (![...profileRouteTemplate.options].some((option) => option.value === currentValue)) {
      profileRouteTemplate.value = "";
    }
  };
  refreshRouteTemplateOptions();
  refreshProfileSandboxFrame().catch(() => {});
  profileRouteMode?.addEventListener("change", async () => {
    dirty = true;
    initialSandboxMode = null;
    draftSandboxMode = null;
    refreshRouteTemplateOptions();
    await refreshProfileSandboxFrame();
  });
  profileRouteTemplate?.addEventListener("change", async () => {
    dirty = true;
    initialSandboxMode = null;
    draftSandboxMode = null;
    await refreshProfileSandboxFrame();
  });
  const profileDnsModeField = overlay.querySelector("#profile-dns-mode");
  const profileDnsServersRow = overlay.querySelector("#profile-dns-servers-row");
  const profileDnsTemplateRow = overlay.querySelector("#profile-dns-template-row");
  const renderDnsControls = () => {
    const isManual = (profileDnsModeField?.value ?? "system") === "custom";
    if (isManual) {
      const dnsServersField = overlay.querySelector("[name='dnsServers']");
      if (dnsServersField && !String(dnsServersField.value ?? "").trim()) {
        dnsServersField.value = "1.1.1.1,8.8.8.8";
      }
    }
    profileDnsServersRow?.classList.toggle("hidden", !isManual);
    profileDnsTemplateRow?.classList.toggle("hidden", !isManual);
  };
  renderDnsControls();
  profileDnsModeField?.addEventListener("change", () => {
    dirty = true;
    renderDnsControls();
  });
  overlay.querySelector("[name='policyLevel']")?.addEventListener("change", () => {
    dirty = true;
    renderPolicySummary();
  });
  overlay.querySelector("#profile-policy-load")?.addEventListener("click", () => {
    applyPolicyLevelToModal();
  });
  const identityStateNode = overlay.querySelector("#profile-identity-state");
  const identityTemplatesNode = overlay.querySelector("#profile-identity-templates");
  const identityModeField = overlay.querySelector("#profile-identity-mode");
  const identityPlatformField = overlay.querySelector("#profile-platform-target");
  const identityPlatformRow = overlay.querySelector("#profile-identity-platform-row");
  const identityTemplateRow = overlay.querySelector("#profile-identity-template-row");
  const identityTemplatePlatformField = overlay.querySelector("#profile-identity-template-platform");
  const identityDisplayNameField = overlay.querySelector("#profile-identity-display-name");
  const identityAutoHint = overlay.querySelector("#profile-identity-auto-hint");
  const identityTemplateField = overlay.querySelector("[name='identityTemplate']");
  const identityTemplateToggle = overlay.querySelector("#profile-identity-template-toggle");
  const identityTemplateMenu = overlay.querySelector("#profile-identity-template-menu");
  const identityTemplateSummary = overlay.querySelector("#profile-identity-template-summary");
  const identityTemplateSearch = overlay.querySelector("#profile-identity-template-search");
  const identityTemplateOptions = overlay.querySelector("#profile-identity-template-options");
  let identityPresetState = (() => {
    try {
      return JSON.parse(identityStateNode?.dataset?.preset ?? "{}");
    } catch {
      return buildManualPreset("win_7_edge_109");
    }
  })();
  let identityUiState = (() => {
    try {
      return JSON.parse(identityStateNode?.dataset?.ui ?? "{}");
    } catch {
      return inferIdentityUiState(identityPresetState);
    }
  })();
  const identityTemplates = (() => {
    try {
      return JSON.parse(identityTemplatesNode?.dataset?.templates ?? "[]");
    } catch {
      return listIdentityTemplates(t);
    }
  })();
  const filteredIdentityTemplates = () => identityTemplates.filter((item) =>
    normalizeTemplatePlatform(item.platformFamily) === normalizeTemplatePlatform(identityUiState.templatePlatform)
  );
  const renderIdentityTemplateOptions = () => {
    if (!identityTemplateOptions) return;
    identityTemplateOptions.innerHTML = templateDropdownOptionsHtml(
      t,
      filteredIdentityTemplates(),
      identityUiState.templateKey
    );
    for (const checkbox of identityTemplateOptions.querySelectorAll("[data-identity-template-key]")) {
      checkbox.addEventListener("change", () => {
        selectIdentityTemplate(checkbox.getAttribute("data-identity-template-key"));
        identityTemplateMenu?.classList.add("hidden");
      });
    }
    applyIdentityTemplateSearch();
  };
  const applyIdentityTemplateSearch = () => {
    const query = String(identityTemplateSearch?.value ?? "").trim().toLowerCase();
    for (const optionEl of overlay.querySelectorAll("[data-identity-template-option]")) {
      const haystack = optionEl.getAttribute("data-identity-template-option") || "";
      optionEl.classList.toggle("hidden", Boolean(query) && !haystack.includes(query));
    }
  };
  const renderIdentityControls = () => {
    const isAuto = identityUiState.mode === "auto";
    if (identityModeField) {
      identityModeField.value = isAuto ? "auto" : "manual";
    }
    if (identityPlatformField) {
      identityPlatformField.value = normalizeAutoPlatform(identityUiState.autoPlatform);
    }
    if (identityTemplatePlatformField) {
      identityTemplatePlatformField.value = normalizeTemplatePlatform(identityUiState.templatePlatform);
    }
    if (identityPlatformRow) {
      identityPlatformRow.classList.toggle("hidden", !isAuto);
    }
    if (identityTemplateRow) {
      identityTemplateRow.classList.toggle("hidden", isAuto);
    }
    if (identityAutoHint) {
      identityAutoHint.classList.toggle("hidden", !isAuto);
    }
    if (identityTemplateField) {
      identityTemplateField.value = isAuto ? "" : identityUiState.templateKey;
    }
    renderIdentityTemplateOptions();
    if (identityTemplateSummary) {
      identityTemplateSummary.textContent = templateSummaryLabel(t, identityTemplates, identityUiState.templateKey);
    }
  };
  const selectIdentityTemplate = (templateKey) => {
    identityUiState.templateKey = templateKey || firstTemplateKeyForTemplatePlatform(identityUiState.templatePlatform);
    identityPresetState = buildManualPreset(identityUiState.templateKey, Date.now());
    identityUiState.autoPlatform = normalizeAutoPlatform(identityPresetState.auto_platform);
    identityUiState.templatePlatform = normalizeTemplatePlatform(
      identityTemplates.find((item) => item.key === identityUiState.templateKey)?.platformFamily ?? identityUiState.templatePlatform
    );
    if (identityDisplayNameField) {
      identityDisplayNameField.value = identityTemplates.find((item) => item.key === identityUiState.templateKey)?.label ?? identityDisplayNameField.value;
    }
    dirty = true;
    renderIdentityControls();
  };
  identityModeField?.addEventListener("change", () => {
    identityUiState.mode = identityModeField.value === "auto" ? "auto" : "manual";
    if (identityUiState.mode === "manual") {
      if (!identityUiState.templateKey) {
        identityUiState.templateKey = firstTemplateKeyForTemplatePlatform(identityUiState.templatePlatform);
      }
      if (identityPresetState?.mode === "auto") {
        identityPresetState = buildManualPreset(identityUiState.templateKey, Date.now());
      }
    }
    dirty = true;
    renderIdentityControls();
  });
  identityPlatformField?.addEventListener("change", () => {
    identityUiState.autoPlatform = normalizeAutoPlatform(identityPlatformField.value);
    dirty = true;
  });
  identityTemplatePlatformField?.addEventListener("change", () => {
    identityUiState.templatePlatform = normalizeTemplatePlatform(identityTemplatePlatformField.value);
    identityUiState.templateKey = firstTemplateKeyForTemplatePlatform(identityUiState.templatePlatform);
    identityPresetState = buildManualPreset(identityUiState.templateKey, Date.now());
    identityUiState.autoPlatform = normalizeAutoPlatform(identityPresetState.auto_platform);
    if (identityDisplayNameField) {
      identityDisplayNameField.value = identityTemplates.find((item) => item.key === identityUiState.templateKey)?.label ?? identityDisplayNameField.value;
    }
    dirty = true;
    renderIdentityControls();
  });
  identityTemplateToggle?.addEventListener("click", (event) => {
    event.preventDefault();
    event.stopPropagation();
    identityTemplateMenu?.classList.toggle("hidden");
    if (!identityTemplateMenu?.classList.contains("hidden")) {
      setTimeout(() => identityTemplateSearch?.focus(), 0);
    }
  });
  identityTemplateMenu?.addEventListener("click", (event) => {
    event.stopPropagation();
  });
  identityTemplateSearch?.addEventListener("input", applyIdentityTemplateSearch);
  renderIdentityControls();
  const tagsState = (() => {
    return uniqueTags(profileTags(existing ?? { tags: [] }) ?? []);
  })();
  const extensionsTable = overlay.querySelector("#profile-extensions-table");
  const extensionSelect = overlay.querySelector("[name='extensionSelect']");
  const passwordLockField = overlay.querySelector("[name='passwordLock']");
  const panicFrameEnabledField = overlay.querySelector("[name='panicFrameEnabled']");
  const panicColorRow = overlay.querySelector("#profile-panic-color-row");
  const profileEngineField = overlay.querySelector("#profile-engine");
  const singlePageModeField = overlay.querySelector("#profile-single-page-mode");
  const defaultSearchRow = overlay.querySelector("#profile-default-search-row");
  const singlePageHint = overlay.querySelector("#profile-single-page-hint");
  const passwordFields = overlay.querySelector("#profile-password-fields");
  const passwordValueField = overlay.querySelector("[name='profilePassword']");
  const passwordConfirmField = overlay.querySelector("[name='profilePasswordConfirm']");
  const passwordToggleButton = overlay.querySelector("#profile-password-toggle");
  const domainTable = overlay.querySelector("#profile-domain-table");
  const domainInput = overlay.querySelector("#profile-domain-input");
  const domainTypeField = overlay.querySelector("#profile-domain-type");
  const domainSearchField = overlay.querySelector("#profile-domain-search");
  const domainFilterField = overlay.querySelector("#profile-domain-filter");
  const certificateTable = overlay.querySelector("#profile-certificates-table");
  const profileCertificateSelectField = overlay.querySelector("[name='profileCertificateSelect']");
  const profileCertificateEngineGuard = overlay.querySelector("#profile-certificate-engine-guard");
  const extensionState = (() => {
    try {
      return {
        enabled: JSON.parse(extensionsTable?.dataset?.enabled ?? "[]"),
        disabled: JSON.parse(extensionsTable?.dataset?.disabled ?? "[]")
      };
    } catch {
      return { enabled: [], disabled: [] };
    }
  })();
  const initialDomainEntries = (() => {
    try {
      return JSON.parse(domainTable?.dataset?.domains ?? "[]");
    } catch {
      return [];
    }
  })();
  const allowState = initialDomainEntries
    .filter((item) => item?.type === "allow" && item?.domain)
    .map((item) => item.domain);
  const denyState = initialDomainEntries
    .filter((item) => item?.type === "deny" && item?.domain)
    .map((item) => item.domain);
  const certificateState = (() => {
    try {
      const byId = JSON.parse(certificateTable?.dataset?.certificateIds ?? "[]").map((value) => ({ kind: "id", value }));
      const byPath = JSON.parse(certificateTable?.dataset?.certificatePaths ?? "[]").map((value) => ({ kind: "path", value }));
      return [...byId, ...byPath];
    } catch {
      return [];
    }
  })();
  const policyPresets = loadPolicyPresets(model.serviceCatalog);
  const blocklistItems = globalBlocklistOptions(globalSecurity);
  const globalActiveBlocklistIds = new Set(
    blocklistItems.filter((item) => item.active).map((item) => item.id)
  );
  const blocklistState = new Set(dnsDraft.selectedBlocklists ?? []);
  for (const id of globalActiveBlocklistIds) {
    blocklistState.add(id);
  }
  const domainTableState = (() => {
    try {
      return JSON.parse(domainTable?.dataset?.domains ?? "[]");
    } catch {
      return buildDomainEntries(allowState, denyState);
    }
  })();
  const domainUiState = {
    search: "",
    filter: "all"
  };

  const profileTagState = {
    selected: [...tagsState],
    available: collectProfileTags(model.profiles)
  };
  const profileTagPicker = wireTagPicker(overlay, {
    id: "profile-tags",
    state: profileTagState,
    emptyLabel: t("profile.tags.empty"),
    searchPlaceholder: t("profile.tags.search"),
    createLabel: (value) => t("profile.tags.create").replace("{tag}", value),
    onChange(selected) {
      dirty = true;
      tagsState.splice(0, tagsState.length, ...uniqueTags(selected ?? []));
    }
  });
  profileTagPicker?.rerender(profileTagState.available, profileTagState.selected);
  const renderSinglePageControls = () => {
    const engine = profileEngineField?.value ?? "wayfern";
    const supported = engine === "wayfern";
    if (singlePageModeField) {
      if (!supported) {
        singlePageModeField.checked = false;
      }
      singlePageModeField.disabled = !supported;
    }
    const active = Boolean(supported && singlePageModeField?.checked);
    if (defaultSearchRow) {
      defaultSearchRow.classList.toggle("hidden", active);
      const select = defaultSearchRow.querySelector("[name='defaultSearchProvider']");
      if (select) {
        select.disabled = active;
      }
    }
    if (singlePageHint) {
      singlePageHint.classList.toggle("hidden", !active);
    }
  };
  const renderCertificateEngineGuard = () => {
    const engine = profileEngineField?.value ?? "wayfern";
    const hasCertificates = hasAssignedProfileCertificates(certificateState);
    const certificatesSupported = engine === "camoufox";
    if (!profileCertificateEngineGuard) return;
    let message = "";
    if (!certificatesSupported && hasCertificates) {
      message = t("profile.security.certificateIsolationWarning");
    } else if (!certificatesSupported) {
      message = t("profile.security.certificateIsolationHint");
    } else if (hasCertificates) {
      message = t("profile.security.certificateIsolationCamoufox");
    }
    profileCertificateEngineGuard.innerHTML = message
      ? `
        <div class="notice ${certificatesSupported ? "" : "error"}">
          ${escapeHtml(message)}
        </div>
      `
      : "";
  };
  renderSinglePageControls();
  renderCertificateEngineGuard();
  profileEngineField?.addEventListener("change", () => {
    dirty = true;
    renderSinglePageControls();
    renderCertificateEngineGuard();
  });
  singlePageModeField?.addEventListener("change", () => {
    dirty = true;
    renderSinglePageControls();
  });
  const renderExtensions = () => {
    if (!extensionsTable) return;
    const rows = [];
    for (const id of extensionState.enabled) {
      rows.push(`
        <tr>
          <td>${escapeHtml(extensionDisplayName(model, id))}</td>
          <td>${t("extensions.status.enabled")}</td>
          <td class="actions">
            <button type="button" data-ext-toggle="${escapeHtml(id)}">${t("extensions.disable")}</button>
            <button type="button" data-ext-remove="${escapeHtml(id)}">${t("extensions.remove")}</button>
          </td>
        </tr>
      `);
    }
    for (const id of extensionState.disabled) {
      rows.push(`
        <tr>
          <td>${escapeHtml(extensionDisplayName(model, id))}</td>
          <td>${t("extensions.status.disabled")}</td>
          <td class="actions">
            <button type="button" data-ext-toggle="${escapeHtml(id)}">${t("extensions.enable")}</button>
            <button type="button" data-ext-remove="${escapeHtml(id)}">${t("extensions.remove")}</button>
          </td>
        </tr>
      `);
    }
    extensionsTable.innerHTML = rows.join("") || `<tr><td colspan="3" class="meta">${t("extensions.empty")}</td></tr>`;
    for (const btn of extensionsTable.querySelectorAll("[data-ext-toggle]")) {
      btn.addEventListener("click", () => {
        const id = btn.getAttribute("data-ext-toggle");
        if (extensionState.enabled.includes(id)) {
          extensionState.enabled = extensionState.enabled.filter((x) => x !== id);
          if (!extensionState.disabled.includes(id)) extensionState.disabled.push(id);
        } else {
          extensionState.disabled = extensionState.disabled.filter((x) => x !== id);
          if (!extensionState.enabled.includes(id)) extensionState.enabled.push(id);
        }
        renderExtensions();
      });
    }
    for (const btn of extensionsTable.querySelectorAll("[data-ext-remove]")) {
      btn.addEventListener("click", () => {
        const id = btn.getAttribute("data-ext-remove");
        extensionState.enabled = extensionState.enabled.filter((x) => x !== id);
        extensionState.disabled = extensionState.disabled.filter((x) => x !== id);
        renderExtensions();
      });
    }
  };
  renderExtensions();
  const syncDomainArrays = () => {
    allowState.length = 0;
    denyState.length = 0;
    for (const item of domainTableState) {
      if (item.type === "allow") {
        allowState.push(item.domain);
      } else {
        denyState.push(item.domain);
      }
    }
  };
  const renderDomainTable = () => {
    if (!domainTable) return;
    const query = domainUiState.search.trim().toLowerCase();
    const filter = domainUiState.filter;
    const rows = domainTableState
      .filter((item) => (filter === "all" ? true : item.type === filter))
      .filter((item) => (!query ? true : item.domain.toLowerCase().includes(query)))
      .sort((left, right) => {
        if (left.type !== right.type) {
          return left.type === "deny" ? -1 : 1;
        }
        return left.domain.localeCompare(right.domain);
      });
    domainTable.innerHTML = rows.map((item) => `
      <tr>
        <td class="profile-domain-status"><span class="profile-domain-status-badge profile-domain-status-${item.type}">${domainStatusIcon(item.type)} ${escapeHtml(domainStatusLabel(item.type, t))}</span></td>
        <td>${escapeHtml(item.domain)}</td>
        <td class="actions"><button type="button" data-domain-remove="${item.type}:${escapeHtml(item.domain)}">${t("extensions.remove")}</button></td>
      </tr>
    `).join("") || `<tr><td colspan="3" class="meta">${t("extensions.empty")}</td></tr>`;
    for (const btn of domainTable.querySelectorAll("[data-domain-remove]")) {
      btn.addEventListener("click", () => {
        const [type, domain] = btn.getAttribute("data-domain-remove").split(":");
        const next = domainTableState.filter((item) => !(item.type === type && item.domain === domain));
        domainTableState.length = 0;
        domainTableState.push(...next);
        syncDomainArrays();
        dirty = true;
        renderDomainTable();
      });
    }
  };
  const renderCertificates = () => {
    if (!certificateTable) return;
    certificateTable.innerHTML = certificateState.map((entry) => {
      if (entry.kind === "id") {
        const cert = (globalSecurity.certificates ?? []).find((item) => item.id === entry.value);
        if (!cert) return "";
        return `
          <tr>
            <td>${escapeHtml(cert.name)}</td>
            <td class="actions"><button type="button" data-cert-remove="id:${cert.id}">${t("extensions.remove")}</button></td>
          </tr>
        `;
      }
      const path = String(entry.value ?? "").trim();
      if (!path) return "";
      const name = path.split(/[/\\\\]/).pop()?.replace(/\.(pem|crt|cer)$/i, "") || path;
      return `
        <tr>
          <td>${escapeHtml(name)}</td>
          <td class="actions"><button type="button" data-cert-remove="path:${escapeHtml(path)}">${t("extensions.remove")}</button></td>
        </tr>
      `;
    }).join("") || `<tr><td colspan="2" class="meta">${t("security.certificates.empty")}</td></tr>`;
    for (const btn of certificateTable.querySelectorAll("[data-cert-remove]")) {
      btn.addEventListener("click", () => {
        const [kind, ...valueParts] = String(btn.getAttribute("data-cert-remove") ?? "").split(":");
        const value = valueParts.join(":");
        const next = certificateState.filter((item) => !(item.kind === kind && item.value === value));
        certificateState.length = 0;
        certificateState.push(...next);
        dirty = true;
        renderCertificates();
        renderCertificateEngineGuard();
      });
    }
  };
  const renderBlocklistSummary = () => {
    for (const checkbox of overlay.querySelectorAll("[data-profile-blocklist-id]")) {
      const id = checkbox.getAttribute("data-profile-blocklist-id");
      checkbox.checked = blocklistState.has(id) || globalActiveBlocklistIds.has(id);
    }
  };
  const renderPolicySummary = () => {
    const summaryEl = overlay.querySelector("#profile-policy-summary");
    if (!summaryEl) return;
    const level = overlay.querySelector("[name='policyLevel']")?.value ?? "normal";
    const summary = summarizePolicyPreset(policyPresets[level]);
    summaryEl.textContent = `${summary.blocklists} ${t("dns.policy.summary.blocklists")} • ${summary.blockedServices} ${t("dns.policy.summary.services")} • ${summary.allowDomains} ${t("dns.policy.summary.allow")} • ${summary.denyDomains} ${t("dns.policy.summary.deny")}`;
  };
  const applyPolicyLevelToModal = () => {
    const level = overlay.querySelector("[name='policyLevel']")?.value ?? "normal";
    const preset = policyPresets[level];
    if (!preset) return;
    applyPolicyPresetToDraft(dnsDraft, preset, model.serviceCatalog);
    blocklistState.clear();
    for (const id of dnsDraft.selectedBlocklists ?? []) {
      blocklistState.add(id);
    }
    for (const id of globalActiveBlocklistIds) {
      blocklistState.add(id);
    }
    if (profileDnsModeField) {
      profileDnsModeField.value = dnsDraft.mode ?? "system";
    }
    if (overlay.querySelector("[name='dnsServers']")) {
      overlay.querySelector("[name='dnsServers']").value = dnsDraft.servers ?? "";
    }
    if (overlay.querySelector("#profile-domain-search")) {
      overlay.querySelector("#profile-domain-search").value = "";
    }
    if (overlay.querySelector("#profile-domain-input")) {
      overlay.querySelector("#profile-domain-input").value = "";
    }
    allowState.length = 0;
    allowState.push(...String(dnsDraft.allowlist ?? "").split(",").map((item) => item.trim()).filter(Boolean));
    denyState.length = 0;
    denyState.push(...String(dnsDraft.denylist ?? "").split(",").map((item) => item.trim()).filter(Boolean));
    domainTableState.length = 0;
    domainTableState.push(...buildDomainEntries(allowState, denyState));
    domainUiState.search = "";
    domainUiState.filter = "all";
    renderDnsControls();
    renderBlocklistSummary();
    renderDomainTable();
    renderPolicySummary();
    dirty = true;
  };
  const renderPasswordControls = () => {
    const enabled = Boolean(passwordLockField?.checked);
    passwordFields?.classList.toggle("hidden", !enabled);
    if (passwordValueField) {
      passwordValueField.required = enabled;
    }
    if (passwordConfirmField) {
      passwordConfirmField.required = enabled;
    }
  };
  const renderPanicColorControls = () => {
    panicColorRow?.classList.toggle("hidden", !panicFrameEnabledField?.checked);
  };
  renderPasswordControls();
  renderPanicColorControls();
  renderDomainTable();
  renderCertificates();
  renderBlocklistSummary();
  renderPolicySummary();
  overlay.querySelector("#profile-extension-add")?.addEventListener("click", () => {
    const id = extensionSelect?.value?.trim();
    if (!id) return;
    if (!extensionState.enabled.includes(id) && !extensionState.disabled.includes(id)) {
      extensionState.enabled.push(id);
      renderExtensions();
    }
  });
  passwordLockField?.addEventListener("change", () => {
    dirty = true;
    renderPasswordControls();
  });
  panicFrameEnabledField?.addEventListener("change", () => {
    dirty = true;
    renderPanicColorControls();
  });
  passwordToggleButton?.addEventListener("click", () => {
    const reveal = (passwordValueField?.type ?? "password") === "password";
    if (passwordValueField) passwordValueField.type = reveal ? "text" : "password";
    if (passwordConfirmField) passwordConfirmField.type = reveal ? "text" : "password";
  });
  overlay.querySelector("#profile-domain-add")?.addEventListener("click", () => {
    const domain = String(domainInput?.value ?? "").trim().toLowerCase();
    if (!domain) return;
    const type = domainTypeField?.value === "allow" ? "allow" : "deny";
    if (!/^[a-z0-9.-]+$/i.test(domain)) {
      return;
    }
    const existingIndex = domainTableState.findIndex((item) => item.domain === domain);
    if (existingIndex >= 0) {
      domainTableState[existingIndex] = { domain, type };
    } else {
      domainTableState.push({ domain, type });
    }
    syncDomainArrays();
    dirty = true;
    if (domainInput) domainInput.value = "";
    renderDomainTable();
  });
  domainSearchField?.addEventListener("input", () => {
    domainUiState.search = domainSearchField.value;
    renderDomainTable();
  });
  domainFilterField?.addEventListener("change", () => {
    domainUiState.filter = domainFilterField.value || "all";
    renderDomainTable();
  });
  overlay.querySelector("#profile-certificate-add")?.addEventListener("click", () => {
    const value = form.profileCertificateSelect.value;
    if (value && !certificateState.some((item) => item.kind === "id" && item.value === value)) {
      certificateState.push({ kind: "id", value });
      dirty = true;
      renderCertificates();
      renderCertificateEngineGuard();
    }
  });
  overlay.querySelector("#profile-certificate-pick")?.addEventListener("click", async () => {
    const result = await pickCertificateFiles();
    if (!result.ok) {
      setNotice(model, "error", String(result.data?.error ?? "pick_certificate_files failed"));
      await rerender();
      return;
    }
    const files = Array.isArray(result.data) ? result.data : [];
    if (!files.length) return;
    const existingIds = new Set((globalSecurity.certificates ?? []).map((item) => String(item.id ?? "")));
    const existingPaths = new Set((globalSecurity.certificates ?? []).map((item) => String(item.path ?? "").trim().toLowerCase()));
    const addedIds = [];
    for (const filePath of files) {
      const clean = String(filePath ?? "").trim();
      if (!clean) continue;
      if (existingPaths.has(clean.toLowerCase()) || existingIds.has(slugId(clean))) continue;
      const id = makeUniqueId(clean, existingIds);
      existingIds.add(id);
      existingPaths.add(clean.toLowerCase());
      globalSecurity.certificates.push({
        id,
        name: clean.split(/[/\\]/).pop()?.replace(/\.(pem|crt|cer)$/i, "") || clean,
        path: clean,
        issuerName: "",
        subjectName: "",
        applyGlobally: false,
        profileIds: []
      });
      addedIds.push(id);
    }
    if (addedIds.length) {
      const saveResult = await saveGlobalSecuritySettings(buildGlobalSecuritySaveRequest(globalSecurity));
      if (!saveResult.ok) {
        setNotice(model, "error", String(saveResult.data?.error ?? "save_global_security_settings failed"));
        await rerender();
        return;
      }
      const refreshedSecurity = await getGlobalSecuritySettings();
      if (refreshedSecurity.ok) {
        try {
          globalSecurity = normalizeGlobalSecuritySettings(refreshedSecurity.data);
          if (profileCertificateSelectField) {
            profileCertificateSelectField.innerHTML = `<option value="">${t("security.selectCertificate")}</option>${(globalSecurity.certificates ?? []).map((item) => `<option value="${item.id}">${escapeHtml(item.name)}</option>`).join("")}`;
          }
        } catch {}
      }
      for (const id of addedIds) {
        if (!certificateState.some((item) => item.kind === "id" && item.value === id)) {
          certificateState.push({ kind: "id", value: id });
        }
      }
    }
    dirty = true;
    renderCertificates();
    renderCertificateEngineGuard();
  });
  const blocklistDropdown = overlay.querySelector(".profile-blocklist-dropdown");
  const blocklistMenu = overlay.querySelector("#profile-blocklists-menu");
  const blocklistSearch = overlay.querySelector("#profile-blocklists-search");
  const blocklistSelectAll = overlay.querySelector("#profile-blocklists-select-all");
  const updateBlocklistSelectAllLabel = () => {
    if (!blocklistSelectAll) return;
    const selectable = [...overlay.querySelectorAll("[data-profile-blocklist-id]")].filter((node) => !node.disabled);
    const allSelected = selectable.length > 0 && selectable.every((node) => node.checked);
    blocklistSelectAll.textContent = allSelected ? t("security.clear") : t("security.all");
  };
  const applyBlocklistSearch = () => {
    const query = String(blocklistSearch?.value ?? "").trim().toLowerCase();
    for (const option of overlay.querySelectorAll("[data-profile-blocklist-option]")) {
      const haystack = option.getAttribute("data-profile-blocklist-option") || "";
      option.classList.toggle("hidden", Boolean(query) && !haystack.includes(query));
    }
  };
  overlay.querySelector("#profile-blocklists-toggle")?.addEventListener("click", (event) => {
    event.preventDefault();
    event.stopPropagation();
    blocklistMenu?.classList.toggle("hidden");
    if (!blocklistMenu?.classList.contains("hidden")) {
      setTimeout(() => blocklistSearch?.focus(), 0);
    }
  });
  blocklistMenu?.addEventListener("click", (event) => {
    event.stopPropagation();
  });
  blocklistSearch?.addEventListener("input", () => {
    applyBlocklistSearch();
  });
  blocklistSelectAll?.addEventListener("click", () => {
    const selectable = [...overlay.querySelectorAll("[data-profile-blocklist-id]")].filter((node) => !node.disabled);
    const allSelected = selectable.length > 0 && selectable.every((node) => node.checked);
    for (const checkbox of selectable) {
      checkbox.checked = !allSelected;
      const id = checkbox.getAttribute("data-profile-blocklist-id");
      if (checkbox.checked) blocklistState.add(id);
      else blocklistState.delete(id);
    }
    for (const id of globalActiveBlocklistIds) {
      blocklistState.add(id);
    }
    dirty = true;
    updateBlocklistSelectAllLabel();
    renderBlocklistSummary();
  });
  for (const checkbox of overlay.querySelectorAll("[data-profile-blocklist-id]")) {
    checkbox.addEventListener("change", () => {
      const id = checkbox.getAttribute("data-profile-blocklist-id");
      if (checkbox.checked) blocklistState.add(id);
      else blocklistState.delete(id);
      for (const globalId of globalActiveBlocklistIds) {
        blocklistState.add(globalId);
      }
      dirty = true;
      updateBlocklistSelectAllLabel();
      renderBlocklistSummary();
    });
  }
  applyBlocklistSearch();
  updateBlocklistSelectAllLabel();
  for (const field of overlay.querySelectorAll("input,select,textarea")) {
    field.addEventListener("change", () => {
      dirty = true;
    });
  }

  for (const button of overlay.querySelectorAll("[data-tab]")) {
    button.addEventListener("click", () => {
      const tab = button.getAttribute("data-tab");
      for (const b of overlay.querySelectorAll("[data-tab]")) b.classList.remove("active");
      button.classList.add("active");
      for (const pane of overlay.querySelectorAll(".tab-pane")) {
        pane.classList.toggle("hidden", pane.getAttribute("data-pane") !== tab);
      }
    });
  }

  const closeModal = async () => {
    if (!dirty) {
      closeModalOverlay(overlay);
      return;
    }
    const leave = await askConfirm(root, t, t("profile.modal.closeTitle"), t("profile.modal.closeDirty"));
    if (leave) closeModalOverlay(overlay);
  };

  overlay.querySelector("#profile-cancel")?.addEventListener("click", closeModal);
  overlay.addEventListener("click", (event) => {
    if (blocklistDropdown && !blocklistDropdown.contains(event.target)) {
      blocklistMenu?.classList.add("hidden");
    }
    if (identityTemplateToggle && !identityTemplateToggle.contains(event.target) && identityTemplateMenu && !identityTemplateMenu.contains(event.target)) {
      identityTemplateMenu.classList.add("hidden");
    }
    if (!event.target.closest("[data-tag-picker='profile-tags']")) {
      profileTagPicker?.close();
    }
    if (event.target === overlay) closeModal();
  });

  form.addEventListener("submit", async (event) => {
    event.preventDefault();
    const baseTags = tagsState.slice();
    const tags = baseTags.filter((x) => !x.startsWith("policy:")
      && !x.startsWith("dns-template:")
      && !x.startsWith("ext:")
      && !x.startsWith("ext-disabled:")
      && !x.startsWith("cert-id:")
      && !x.startsWith("cert:")
      && x !== "ext-system-access"
      && x !== "ext-keepassxc"
      && x !== "ext-launch-disabled");
    tags.push(`policy:${form.policyLevel.value}`);
    if (form.dnsMode.value === "custom" && form.dnsTemplateId.value) {
      tags.push(`dns-template:${form.dnsTemplateId.value}`);
    }
    tags.push(...certificateState.filter((item) => item.kind === "id").map((item) => `cert-id:${item.value}`));
    tags.push(...certificateState.filter((item) => item.kind === "path").map((item) => `cert:${item.value}`));
    tags.push(...extensionState.disabled.map((id) => `ext-disabled:${id}`));
    if (form.disableExtensionsLaunch.checked && extensionState.enabled.length && !form.allowKeepassxc.checked) {
      setNotice(model, "error", t("profile.security.disableExtensionsLaunchBlocked"));
      rerender();
      return;
    }
    if (form.disableExtensionsLaunch.checked) {
      tags.push("ext-launch-disabled");
    }
    if (form.allowSystemAccess.checked) {
      const accepted = await askConfirm(root, t, t("profile.security.allowSystemAccess"), t("profile.security.systemAccessWarning"));
      if (!accepted) return;
      tags.push("ext-system-access");
    }
    if (form.allowKeepassxc.checked) {
      tags.push("ext-keepassxc");
    }
    const preservedLockedAppTags = (existing?.tags ?? []).filter((tag) =>
      tag.startsWith("locked-app:") && tag !== "locked-app:custom"
    );
    tags.push(...preservedLockedAppTags);
    if (form.engine.value === "wayfern" && form.singlePageMode?.checked) {
      tags.push("locked-app:custom");
    }
    const defaultStartPageValue = String(form.defaultStartPage.value ?? "").trim();
    if (form.engine.value === "wayfern" && form.singlePageMode?.checked) {
      const normalizedStartPage = /^[a-zA-Z][a-zA-Z0-9+.-]*:/.test(defaultStartPageValue)
        ? defaultStartPageValue
        : `https://${defaultStartPageValue}`;
      let startUrl = null;
      try {
        startUrl = new URL(normalizedStartPage);
      } catch {
        startUrl = null;
      }
      if (!startUrl?.host) {
        setNotice(model, "error", t("profile.field.singlePageInvalidUrl"));
        rerender();
        return;
      }
    }
    const payload = {
      name: form.name.value,
      description: form.description.value || null,
      tags,
      engine: form.engine.value,
      defaultStartPage: defaultStartPageValue || null,
      defaultSearchProvider: form.singlePageMode?.checked ? null : (form.defaultSearchProvider.value || null),
      ephemeralMode: form.ephemeral.checked,
      passwordLockEnabled: form.passwordLock.checked,
      panicFrameEnabled: form.panicFrameEnabled.checked,
      panicFrameColor: form.panicFrameEnabled.checked ? (form.panicFrameColor.value || "#ff8652") : null,
      panicProtectedSites: existing?.panic_protected_sites ?? [],
      ephemeralRetainPaths: []
    };
    if (payload.passwordLockEnabled) {
      const passwordValue = String(form.profilePassword.value ?? "");
      const passwordConfirm = String(form.profilePasswordConfirm.value ?? "");
      if (!passwordValue || !passwordConfirm) {
        setNotice(model, "error", t("profile.security.passwordRequired"));
        rerender();
        return;
      }
      if (passwordValue !== passwordConfirm) {
        setNotice(model, "error", t("profile.security.passwordMismatch"));
        rerender();
        return;
      }
    }
    const identityModeValue = form.identityMode.value === "auto" ? "auto" : "manual";
    const identityPlatformTarget = identityModeValue === "auto"
      ? normalizeAutoPlatform(form.platformTarget.value || identityUiState.autoPlatform)
      : null;
    const identityTemplateKey = identityModeValue === "manual"
      ? (form.identityTemplate.value || identityUiState.templateKey || firstTemplateKeyForTemplatePlatform(identityUiState.templatePlatform))
      : null;

    const validate = await validateProfileModal({
      general: {
        name: payload.name,
        description: payload.description,
        tags: payload.tags,
        default_start_page: payload.defaultStartPage,
        default_search_provider: payload.defaultSearchProvider
      },
      identity: {
        mode: identityModeValue,
        platform_target: identityPlatformTarget,
        template_key: identityTemplateKey
      },
      vpn_proxy: {
        route_mode: form.profileRouteMode.value,
        proxy_url: null,
        vpn_profile_ref: form.profileRouteTemplateId.value || null
      },
      dns: {
        resolver_mode: form.dnsMode.value,
        servers: form.dnsServers.value.split(",").map((v) => v.trim()).filter(Boolean),
        blocklists: [...blocklistState],
        allow_domains: allowState
      },
      extensions: {
        enabled_extension_ids: extensionState.enabled
      },
      security: {
        password_lock_enabled: form.passwordLock.checked,
        ephemeral_mode: form.ephemeral.checked,
        ephemeral_retain_paths: []
      },
      sync: {
        server: form.syncServer.value || null,
        key_id: form.syncKey.value || null
      },
      advanced: {
        launch_hook: form.launchHook.value || null
      }
    });

    if (!validate.ok) {
      setNotice(model, "error", `${t("profile.modal.validationError")}: ${validate.data.error}`);
      rerender();
      return;
    }
    let identityPresetToSave = null;
    try {
      if (identityModeValue === "auto") {
        const generatedPreset = await generateAutoPreset(identityPlatformTarget, Date.now());
        if (!generatedPreset.ok) {
          throw new Error(String(generatedPreset.data.error));
        }
        identityPresetToSave = generatedPreset.data;
      } else {
        if (identityTemplateKey && identityTemplateKey !== identityUiState.templateKey) {
          identityPresetState = buildManualPreset(identityTemplateKey, Date.now());
        }
        identityUiState.mode = "manual";
        identityUiState.templateKey = identityTemplateKey || identityUiState.templateKey;
        identityUiState.templatePlatform = normalizeTemplatePlatform(
          identityTemplates.find((item) => item.key === identityUiState.templateKey)?.platformFamily ?? identityUiState.templatePlatform
        );
        identityPresetToSave = cloneIdentityPreset(identityPresetState ?? buildManualPreset(identityUiState.templateKey, Date.now()));
        identityPresetToSave.mode = "manual";
        identityPresetToSave.display_name = String(form.identityDisplayName?.value ?? "").trim() || null;
      }
    } catch (error) {
      setNotice(model, "error", String(error));
      rerender();
      return;
    }

    const dnsPayload = {
      profile_id: existing?.id ?? "",
      dns_config: {
        mode: form.dnsMode.value,
        servers: form.dnsServers.value.split(",").map((v) => v.trim()).filter(Boolean),
        doh_url: null,
        dot_server_name: null
      },
      selected_blocklists: blocklistItems
        .filter((item) => blocklistState.has(item.id))
        .map((item) => ({
          list_id: item.id,
          domains: item.domains ?? [],
          updated_at_epoch: Math.floor(Date.now() / 1000)
        })),
      selected_services: blockedServicesToPairs(dnsDraft.blockedServices ?? {}),
      domain_allowlist: allowState,
      domain_denylist: denyState,
      domain_exceptions: []
    };
    const routeMode = form.profileRouteMode.value;
    const routeTemplateId = form.profileRouteTemplateId.value || null;
    const selectedRouteTemplate = (profileNetworkState.connectionTemplates ?? []).find((item) => item.id === routeTemplateId) ?? null;
    let routePayload = null;
    try {
      const killSwitchEnabled = routeMode === "direct" ? false : Boolean(form.profileKillSwitch?.checked);
      routePayload = buildRoutePolicyPayload(routeMode, selectedRouteTemplate, killSwitchEnabled, t);
    } catch (error) {
      setNotice(model, "error", String(error));
      rerender();
      return;
    }
    const saveRoutePolicy = async (profileId) => {
      return saveVpnProxyPolicy(profileId, routePayload, routeMode === "direct" ? null : routeTemplateId);
    };
    const saveSandboxPolicy = async (profileId) => {
      if (routeMode === "direct" || !routeTemplateId) {
        return { ok: true };
      }
      const selectedMode = overlay.querySelector("#profile-sandbox-mode")?.value || null;
      if (!selectedMode || selectedMode === initialSandboxMode) {
        return { ok: true };
      }
      return saveNetworkSandboxProfileSettings(profileId, selectedMode);
    };
    const saveSyncPolicy = async (profileId) => {
      const serverUrl = form.syncServer.value.trim();
      const keyId = form.syncKey.value.trim();
      const enabled = Boolean(form.syncEnabled?.checked);
      const syncModel = {
        server: {
          server_url: serverUrl,
          key_id: keyId,
          sync_enabled: enabled
        },
        status: {
          level: enabled ? "healthy" : "warning",
          message_key: enabled ? "sync.healthy" : "sync.disabled",
          last_sync_unix_ms: syncOverview?.controls?.status?.last_sync_unix_ms ?? null
        },
        conflicts: syncOverview?.conflicts ?? [],
        can_backup: true,
        can_restore: true
      };
      return saveSyncControls(profileId, syncModel);
    };
    const saveIdentityPolicy = async (profileId) => {
      return saveIdentityProfile(profileId, identityPresetToSave);
    };
    const saveProfilePassword = async (profileId) => {
      if (!payload.passwordLockEnabled) {
        return { ok: true };
      }
      return setProfilePassword(profileId, form.profilePassword.value);
    };
    const resolveSaveError = (dnsResult, routeResult, sandboxResult, syncResult, identityResult) => {
      if (!dnsResult.ok) return String(dnsResult.data.error);
      if (!routeResult.ok) return String(routeResult.data.error);
      if (!sandboxResult.ok) return String(sandboxResult.data.error);
      if (!syncResult.ok) return String(syncResult.data.error);
      if (!identityResult.ok) return String(identityResult.data.error);
      return t("profile.modal.validationError");
    };

    if (existing) {
      const engineChanged = String(existing.engine ?? "wayfern") !== String(form.engine.value ?? "wayfern");
      const updateResult = await updateProfile({
        profileId: existing.id,
        name: payload.name,
        description: payload.description,
        tags: payload.tags,
        engine: form.engine.value,
        defaultStartPage: payload.defaultStartPage,
        defaultSearchProvider: payload.defaultSearchProvider,
        ephemeralMode: payload.ephemeralMode,
        passwordLockEnabled: payload.passwordLockEnabled,
        panicFrameEnabled: payload.panicFrameEnabled,
        panicFrameColor: payload.panicFrameColor,
        panicProtectedSites: payload.panicProtectedSites,
        ephemeralRetainPaths: payload.ephemeralRetainPaths,
        expectedUpdatedAt: existing.updated_at
      });
      if (updateResult.ok) {
        await syncProfileExtensionAssignments(model, existing.id, extensionState);
        dnsPayload.profile_id = existing.id;
        saveProfileDnsDraft(existing.id, {
          ...dnsDraft,
          mode: dnsPayload.dns_config.mode,
          servers: dnsPayload.dns_config.servers.join(","),
          allowlist: allowState.join(","),
          denylist: denyState.join(","),
          selectedBlocklists: [...blocklistState]
        });
        const dnsResult = await saveDnsPolicy(existing.id, dnsPayload);
        const routeResult = await saveRoutePolicy(existing.id);
        const sandboxResult = await saveSandboxPolicy(existing.id);
        const syncResult = await saveSyncPolicy(existing.id);
        const identityResult = await saveIdentityPolicy(existing.id);
        const passwordResult = await saveProfilePassword(existing.id);
        if (dnsResult.ok && routeResult.ok && sandboxResult.ok && syncResult.ok && identityResult.ok && passwordResult.ok) {
          setNotice(model, "success", engineChanged ? t("profile.runtime.engineChangedReset") : t("profile.runtime.appliedNow"));
        } else {
          setNotice(model, "error", !passwordResult.ok ? String(passwordResult.data.error) : resolveSaveError(dnsResult, routeResult, sandboxResult, syncResult, identityResult));
        }
      } else {
        setNotice(model, "error", String(updateResult.data.error));
      }
    } else {
      const createResult = await createProfile(payload);
      if (createResult.ok) {
        await syncProfileExtensionAssignments(model, createResult.data.id, extensionState);
        dnsPayload.profile_id = createResult.data.id;
        saveProfileDnsDraft(createResult.data.id, {
          ...dnsDraft,
          mode: dnsPayload.dns_config.mode,
          servers: dnsPayload.dns_config.servers.join(","),
          allowlist: allowState.join(","),
          denylist: denyState.join(","),
          selectedBlocklists: [...blocklistState]
        });
        const dnsResult = await saveDnsPolicy(createResult.data.id, dnsPayload);
        const routeResult = await saveRoutePolicy(createResult.data.id);
        const sandboxResult = await saveSandboxPolicy(createResult.data.id);
        const syncResult = await saveSyncPolicy(createResult.data.id);
        const identityResult = await saveIdentityPolicy(createResult.data.id);
        const passwordResult = await saveProfilePassword(createResult.data.id);
        if (dnsResult.ok && routeResult.ok && sandboxResult.ok && syncResult.ok && identityResult.ok && passwordResult.ok) {
          setNotice(model, "success", t("profile.create.success"));
        } else {
          setNotice(model, "error", !passwordResult.ok ? String(passwordResult.data.error) : resolveSaveError(dnsResult, routeResult, sandboxResult, syncResult, identityResult));
        }
      } else {
        setNotice(model, "error", String(createResult.data.error));
      }
    }

    closeModalOverlay(overlay, async () => {
      await hydrateProfilesModel(model);
      rerender();
    });
  });
}
