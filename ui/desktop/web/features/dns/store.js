import { BLOCKLIST_PRESETS } from "./catalog.js";

const TEMPLATES_KEY = "browser.launcher.dns.templates.v1";
const PROFILES_KEY = "browser.launcher.dns.profiles.v1";

function readJson(key, fallback) {
  try {
    const raw = window.localStorage.getItem(key);
    return raw ? JSON.parse(raw) : fallback;
  } catch {
    return fallback;
  }
}

function writeJson(key, value) {
  window.localStorage.setItem(key, JSON.stringify(value));
}

function defaultBlockedServices(catalog) {
  const blocked = {};
  for (const [categoryKey, category] of Object.entries(catalog?.categories ?? {})) {
    blocked[categoryKey] = {};
    for (const serviceKey of Object.keys(category.services ?? {})) {
      blocked[categoryKey][serviceKey] = false;
    }
  }
  return blocked;
}

function normalizeBlockedServices(catalog, raw) {
  const blocked = defaultBlockedServices(catalog);
  for (const [categoryKey, services] of Object.entries(raw ?? {})) {
    if (!blocked[categoryKey]) continue;
    for (const [serviceKey, value] of Object.entries(services ?? {})) {
      if (serviceKey in blocked[categoryKey]) {
        blocked[categoryKey][serviceKey] = Boolean(value);
      }
    }
  }
  return blocked;
}

export function loadDnsTemplates() {
  return readJson(TEMPLATES_KEY, []);
}

export function saveDnsTemplates(templates) {
  writeJson(TEMPLATES_KEY, templates);
}

export function createTemplateSnapshot(name, draft, id = crypto.randomUUID()) {
  return {
    id,
    name: name.trim(),
    selectedBlocklists: [...(draft.selectedBlocklists ?? [])],
    blockedServices: structuredClone(draft.blockedServices ?? {}),
    updatedAt: Date.now()
  };
}

export function loadProfileDnsDraft(profileId, catalog) {
  const profiles = readJson(PROFILES_KEY, {});
  const saved = profiles[profileId] ?? {};
  return {
    mode: saved.mode ?? "system",
    servers: saved.servers ?? "",
    allowlist: saved.allowlist ?? "",
    denylist: saved.denylist ?? "",
    search: saved.search ?? "",
    activeTemplateId: saved.activeTemplateId ?? "",
    templateName: saved.templateName ?? "",
    selectedBlocklists: Array.isArray(saved.selectedBlocklists) ? saved.selectedBlocklists : [],
    blockedServices: normalizeBlockedServices(catalog, saved.blockedServices),
    blocklistMenuOpen: false
  };
}

export function saveProfileDnsDraft(profileId, draft) {
  const profiles = readJson(PROFILES_KEY, {});
  profiles[profileId] = {
    mode: draft.mode,
    servers: draft.servers,
    allowlist: draft.allowlist,
    denylist: draft.denylist,
    search: draft.search,
    activeTemplateId: draft.activeTemplateId ?? "",
    templateName: draft.templateName ?? "",
    selectedBlocklists: [...(draft.selectedBlocklists ?? [])],
    blockedServices: structuredClone(draft.blockedServices ?? {})
  };
  writeJson(PROFILES_KEY, profiles);
}

export function blockedServicesToPairs(blockedServices) {
  const selected = [];
  for (const [categoryKey, services] of Object.entries(blockedServices ?? {})) {
    for (const [serviceKey, blocked] of Object.entries(services ?? {})) {
      if (blocked) selected.push([categoryKey, serviceKey]);
    }
  }
  return selected;
}

export function applyTemplateToDraft(draft, template, catalog) {
  draft.activeTemplateId = template?.id ?? "";
  draft.templateName = template?.name ?? "";
  draft.selectedBlocklists = [...(template?.selectedBlocklists ?? [])];
  draft.blockedServices = normalizeBlockedServices(catalog, template?.blockedServices);
}

export function templateMatchesDraft(template, draft, catalog) {
  if (!template) return false;
  const left = JSON.stringify({
    selectedBlocklists: [...(template.selectedBlocklists ?? [])].sort(),
    blockedServices: normalizeBlockedServices(catalog, template.blockedServices)
  });
  const right = JSON.stringify({
    selectedBlocklists: [...(draft.selectedBlocklists ?? [])].sort(),
    blockedServices: normalizeBlockedServices(catalog, draft.blockedServices)
  });
  return left === right;
}

export function presetSummary(ids) {
  if (!ids?.length) return "0";
  return ids
    .map((id) => BLOCKLIST_PRESETS.find((preset) => preset.id === id)?.label)
    .filter(Boolean)
    .join(", ");
}
