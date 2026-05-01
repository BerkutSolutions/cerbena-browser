import { getGlobalSecuritySettings } from "./api.js";

export const COMMON_SUFFIXES = [
  "app", "biz", "blog", "cc", "club", "cn", "co", "com", "company", "dev", "de", "edu", "es", "eu", "fr", "fun", "games", "gov", "id", "info", "io", "it", "jp", "kz", "link", "live", "me", "media", "mil", "mobi", "mx", "name", "net", "news", "nl", "online", "org", "page", "plus", "pro", "ru", "shop", "site", "space", "store", "tech", "today", "top", "tv", "uk", "us", "vip", "website", "wiki", "world", "xyz"
];

export function ensureGlobalSecurityState(model) {
  if (!model.securityState) {
    model.securityState = {
      startupPage: "",
      certificates: [],
      blockedDomainSuffixes: [],
      blocklists: [],
      suffixFilter: "",
      suffixMenuOpen: false
    };
  }
  return model.securityState;
}

function parseGlobalSecurityPayload(raw) {
  const payload = JSON.parse(raw || "{}");
  return {
    startupPage: payload.startup_page ?? payload.startupPage ?? "",
    certificates: (payload.certificates ?? []).map((item) => ({
      id: item.id,
      name: item.name,
      path: item.path,
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

export async function hydrateGlobalSecurityState(model) {
  const state = ensureGlobalSecurityState(model);
  if (model.securityLoaded) return state;
  model.securityLoaded = true;
  const result = await getGlobalSecuritySettings();
  if (result.ok) {
    try {
      model.securityState = {
        ...state,
        ...parseGlobalSecurityPayload(result.data)
      };
    } catch {}
  }
  return ensureGlobalSecurityState(model);
}

export function buildGlobalSecuritySaveRequest(state) {
  return {
    startupPage: state.startupPage?.trim() ? state.startupPage.trim() : null,
    certificates: state.certificates ?? [],
    blockedDomainSuffixes: state.blockedDomainSuffixes ?? [],
    blocklists: state.blocklists ?? []
  };
}
