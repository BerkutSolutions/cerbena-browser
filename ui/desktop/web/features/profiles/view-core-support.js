import { SEARCH_PROVIDER_PRESETS } from "../../core/catalogs.js";
import { loadDnsTemplates } from "../dns/store.js";
import { setExtensionProfiles } from "../extensions/api.js";
import { classifyDockerRuntimeIssue } from "./view-launch-overlays.js";
import { escapeHtml, profileTags } from "./view-helpers.js";

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
  if (profile?.id && model?.profileExtensionStateMap?.[profile.id]) {
    const entries = model.profileExtensionStateMap[profile.id];
    return {
      enabled: entries.filter((item) => item.enabled).map((item) => item.libraryItemId),
      disabled: entries.filter((item) => !item.enabled).map((item) => item.libraryItemId)
    };
  }
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

function ensureProfilesViewState(model) {
  if (!model.profilesViewState) {
    model.profilesViewState = {
      sortKey: "name",
      sortDirection: "asc"
    };
  }
  return model.profilesViewState;
}

function profileSortAria(sortState, key) {
  if (sortState.sortKey !== key) return "none";
  return sortState.sortDirection === "desc" ? "descending" : "ascending";
}

function sortedProfiles(model) {
  const sortState = ensureProfilesViewState(model);
  const direction = sortState.sortDirection === "desc" ? -1 : 1;
  const collator = new Intl.Collator(undefined, { sensitivity: "base", numeric: true });
  const valueFor = (profile, key) => {
    if (key === "tags") {
      return profileTags(profile).join(", ");
    }
    if (key === "note") {
      return profile.description ?? "";
    }
    return profile[key] ?? "";
  };
  return [...(model.profiles ?? [])].sort((left, right) => {
    const result = collator.compare(
      String(valueFor(left, sortState.sortKey)),
      String(valueFor(right, sortState.sortKey))
    );
    if (result !== 0) return result * direction;
    return collator.compare(String(left.name ?? ""), String(right.name ?? ""));
  });
}

function isChromiumFamilyEngine(engine) {
  return engine === "chromium" || engine === "ungoogled-chromium";
}

function isFirefoxFamilyEngine(engine) {
  return engine === "firefox-esr" || engine === "librewolf";
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
  if (scope === "firefox") return isFirefoxFamilyEngine(profile?.engine);
  if (scope === "chromium") return isChromiumFamilyEngine(profile?.engine);
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


function setNotice(model, type, text) {
  model.profileNotice = { type, text, at: Date.now() };
}

function resolveProfileErrorMessage(t, errorText) {
  const text = String(errorText ?? "");
  const dockerIssue = classifyDockerRuntimeIssue(text);
  if (dockerIssue === "missing") {
    return t("network.sandbox.reason.containerRuntimeMissing");
  }
  if (dockerIssue === "stopped") {
    return t("network.sandbox.reason.containerProbeFailed");
  }
  const keyMap = {
    "profile_protection.locked_profile_requires_unlock": "profile.security.lockedLaunchBlocked",
    "profile_protection.system_access_forbidden": "profile.security.systemAccessBlocked",
    "profile_protection.keepassxc_forbidden": "profile.security.keepassxcBlocked",
    "profile_protection.maximum_policy_extensions_forbidden": "profile.security.maximumPolicyExtensionsBlocked",
    "profile_protection.cookies_copy_blocked": "profile.security.cookiesCopyBlocked",
    "profile.security.chromium_certificates_not_supported": "profile.security.chromiumCertificatesBlocked"
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


export {
  domainStatusIcon, eyeIcon, domainStatusLabel, buildDomainEntries, profileExtensions,
  assignedProfileExtensionIds, mergedProfileExtensions, profileSecurityFlags, selectionState,
  ensureProfilesViewState, profileSortAria, sortedProfiles, isChromiumFamilyEngine, isFirefoxFamilyEngine,
  searchOptions, extensionLibraryItem, extensionDisplayName, templateSummaryLabel,
  templateDropdownOptionsHtml, templateInputValue, dnsTemplateOptions, globalBlocklistOptions,
  extensionScopeAllowed, extensionLibraryOptions, setNotice, resolveProfileErrorMessage,
  postureFindingLines, resolveDevicePostureAction
};
