const STORAGE_KEY = "browser.launcher.dns.policy-presets.v1";

export const DNS_POLICY_LEVELS = ["disabled", "light", "normal", "high", "maximum"];

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

function emptyBlockedServices(catalog) {
  const blocked = {};
  for (const [categoryKey, category] of Object.entries(catalog?.categories ?? {})) {
    blocked[categoryKey] = {};
    for (const serviceKey of Object.keys(category.services ?? {})) {
      blocked[categoryKey][serviceKey] = false;
    }
  }
  return blocked;
}

function blockedServicesFromCategories(catalog, categories) {
  const blocked = emptyBlockedServices(catalog);
  for (const categoryKey of categories) {
    if (!blocked[categoryKey]) continue;
    for (const serviceKey of Object.keys(blocked[categoryKey])) {
      blocked[categoryKey][serviceKey] = true;
    }
  }
  return blocked;
}

function normalizeBlockedServices(catalog, raw) {
  const blocked = emptyBlockedServices(catalog);
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

function defaultPolicyPresets(catalog) {
  return {
    disabled: {
      level: "disabled",
      selectedBlocklists: [],
      blockedServices: blockedServicesFromCategories(catalog, []),
      allowlist: "",
      denylist: ""
    },
    light: {
      level: "light",
      selectedBlocklists: ["adguard_dns"],
      blockedServices: blockedServicesFromCategories(catalog, []),
      allowlist: "",
      denylist: ""
    },
    normal: {
      level: "normal",
      selectedBlocklists: ["adguard_dns", "adguard_tracking"],
      blockedServices: blockedServicesFromCategories(catalog, ["artificial_intelligence"]),
      allowlist: "",
      denylist: ""
    },
    high: {
      level: "high",
      selectedBlocklists: ["adguard_dns", "adguard_tracking", "adguard_malware"],
      blockedServices: blockedServicesFromCategories(catalog, ["artificial_intelligence", "social_networks_and_communities"]),
      allowlist: "",
      denylist: ""
    },
    maximum: {
      level: "maximum",
      selectedBlocklists: ["adguard_dns", "adguard_mobile", "adguard_tracking", "adguard_social", "adguard_annoyances", "adguard_malware"],
      blockedServices: blockedServicesFromCategories(catalog, [
        "artificial_intelligence",
        "social_networks_and_communities",
        "media_and_streaming",
        "shopping",
        "dating_services",
        "gambling_and_betting",
        "messaging_services"
      ]),
      allowlist: "",
      denylist: ""
    }
  };
}

export function defaultPolicyPreset(catalog, level) {
  return defaultPolicyPresets(catalog)[level];
}

function normalizePolicyPreset(catalog, level, raw, fallback) {
  return {
    level,
    selectedBlocklists: Array.isArray(raw?.selectedBlocklists) ? [...raw.selectedBlocklists] : [...fallback.selectedBlocklists],
    blockedServices: normalizeBlockedServices(catalog, raw?.blockedServices ?? fallback.blockedServices),
    allowlist: typeof raw?.allowlist === "string" ? raw.allowlist : fallback.allowlist,
    denylist: typeof raw?.denylist === "string" ? raw.denylist : fallback.denylist
  };
}

export function loadPolicyPresets(catalog) {
  const defaults = defaultPolicyPresets(catalog);
  const saved = readJson(STORAGE_KEY, {});
  const presets = {};
  for (const level of DNS_POLICY_LEVELS) {
    presets[level] = normalizePolicyPreset(catalog, level, saved[level], defaults[level]);
  }
  return presets;
}

export function savePolicyPresets(presets) {
  writeJson(STORAGE_KEY, presets);
}

export function createPolicyPresetFromDraft(level, draft, catalog) {
  return normalizePolicyPreset(catalog, level, {
    level,
    selectedBlocklists: [...(draft.selectedBlocklists ?? [])],
    blockedServices: structuredClone(draft.blockedServices ?? {}),
    allowlist: draft.allowlist ?? "",
    denylist: draft.denylist ?? ""
  }, {
    selectedBlocklists: [],
    blockedServices: emptyBlockedServices(catalog),
    allowlist: "",
    denylist: ""
  });
}

export function resetPolicyPreset(level, catalog) {
  return normalizePolicyPreset(catalog, level, defaultPolicyPreset(catalog, level), defaultPolicyPreset(catalog, level));
}

export function applyPolicyPresetToDraft(draft, preset, catalog) {
  if (!draft || !preset) return;
  draft.activeTemplateId = "";
  draft.templateName = "";
  draft.selectedBlocklists = [...(preset.selectedBlocklists ?? [])];
  draft.blockedServices = normalizeBlockedServices(catalog, preset.blockedServices);
  draft.allowlist = preset.allowlist ?? "";
  draft.denylist = preset.denylist ?? "";
}

export function summarizePolicyPreset(preset) {
  let blockedServices = 0;
  for (const services of Object.values(preset?.blockedServices ?? {})) {
    for (const blocked of Object.values(services ?? {})) {
      if (blocked) blockedServices += 1;
    }
  }
  return {
    blocklists: preset?.selectedBlocklists?.length ?? 0,
    blockedServices,
    allowDomains: String(preset?.allowlist ?? "").split(",").map((item) => item.trim()).filter(Boolean).length,
    denyDomains: String(preset?.denylist ?? "").split(",").map((item) => item.trim()).filter(Boolean).length
  };
}
