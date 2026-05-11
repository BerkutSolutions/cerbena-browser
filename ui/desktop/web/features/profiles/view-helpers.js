import { collectTagOptions } from "../../core/tag-picker.js";

export const DOMAIN_OPTIONS = [
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

export function option(value, label, selected) {
  return `<option value="${value}" ${selected ? "selected" : ""}>${label}</option>`;
}

export function escapeHtml(value) {
  return String(value ?? "")
    .replaceAll("&", "&amp;")
    .replaceAll("<", "&lt;")
    .replaceAll(">", "&gt;")
    .replaceAll("\"", "&quot;");
}

export function engineIcon(engine) {
  if (engine === "librewolf" || engine === "firefox-esr") {
    return `<svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="1.8"><path d="M7 4h10l2 4-1 8-4 4H10l-4-4-1-8 2-4z"/><path d="M9 9h.01M15 9h.01"/><path d="M9 14c1 1 2 1.5 3 1.5S14 15 15 14"/></svg>`;
  }
  return `<svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="1.8"><circle cx="12" cy="12" r="9"/><path d="M3 12h18"/><path d="M12 3c3 3 4.5 6 4.5 9S15 18 12 21c-3-3-4.5-6-4.5-9S9 6 12 3z"/></svg>`;
}

export function pencilIcon() {
  return `<svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="1.8"><path d="M12 20h9"/><path d="M16.5 3.5a2.1 2.1 0 0 1 3 3L8 18l-4 1 1-4 11.5-11.5z"/></svg>`;
}

export function exportIcon() {
  return `<svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="1.8"><path d="M12 3v12"/><path d="m7 8 5-5 5 5"/><path d="M5 21h14"/></svg>`;
}

export function trashIcon() {
  return `<svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="1.8"><path d="M3 6h18"/><path d="M8 6V4h8v2"/><path d="M19 6l-1 14H6L5 6"/><path d="M10 11v6M14 11v6"/></svg>`;
}

export function closeIcon() {
  return `<svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="1.8"><path d="m6 6 12 12"/><path d="m18 6-12 12"/></svg>`;
}

export function playIcon() {
  return `<svg viewBox="0 0 24 24" fill="currentColor"><path d="M8 6.5v11l9-5.5z"/></svg>`;
}

export function stopIcon() {
  return `<svg viewBox="0 0 24 24" fill="currentColor"><rect x="7" y="7" width="10" height="10" rx="1.5"/></svg>`;
}

export function terminalIcon() {
  return `<svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="1.8"><path d="M4 6h16v12H4z"/><path d="m7 10 3 2-3 2"/><path d="M12 14h5"/></svg>`;
}

export function usersIcon() {
  return `<svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="1.8"><circle cx="9" cy="8" r="3"/><path d="M4 19c0-2.5 2.5-4 5-4"/><circle cx="17" cy="9" r="2.5"/><path d="M13 19c.5-2 2.4-3.5 4.7-3.5 1.5 0 2.8.5 3.8 1.5"/></svg>`;
}

export function puzzleIcon() {
  return `<svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="1.8"><path d="M12 3a2 2 0 0 1 2 2v1h2.5A1.5 1.5 0 0 1 18 7.5V10h1a2 2 0 1 1 0 4h-1v2.5a1.5 1.5 0 0 1-1.5 1.5H14v1a2 2 0 1 1-4 0v-1H7.5A1.5 1.5 0 0 1 6 16.5V14H5a2 2 0 1 1 0-4h1V7.5A1.5 1.5 0 0 1 7.5 6H10V5a2 2 0 0 1 2-2z"/></svg>`;
}

export function cookieIcon() {
  return `<svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="1.8"><path d="M14 3a3 3 0 0 0 4 4 7 7 0 1 1-4-4z"/><path d="M8.5 9.5h.01M14.5 13h.01M10 15.5h.01M12.5 7.5h.01"/></svg>`;
}

export function profileTags(profile) {
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

export function collectProfileTags(profiles) {
  return collectTagOptions(profiles ?? [], (profile) => profileTags(profile));
}

export function certificateIds(profile) {
  return (profile?.tags ?? [])
    .filter((tag) => tag.startsWith("cert-id:"))
    .map((tag) => tag.replace("cert-id:", ""));
}

export function certificateLegacyPaths(profile) {
  return (profile?.tags ?? [])
    .filter((tag) => tag.startsWith("cert:"))
    .map((tag) => tag.replace("cert:", ""))
    .filter((path) => path !== "global");
}

export function certificateEntriesForProfile(profile, globalSecurity) {
  const entries = [];
  const seen = new Set();
  for (const id of certificateIds(profile)) {
    const key = `id:${id}`;
    if (!seen.has(key)) {
      entries.push({ kind: "id", value: id });
      seen.add(key);
    }
  }
  for (const item of globalSecurity?.certificates ?? []) {
    const assigned = (item.profileIds ?? []).includes(profile?.id);
    const key = `id:${item.id}`;
    if (assigned && !seen.has(key)) {
      entries.push({ kind: "id", value: item.id });
      seen.add(key);
    }
  }
  for (const path of certificateLegacyPaths(profile)) {
    const key = `path:${path}`;
    if (!seen.has(key)) {
      entries.push({ kind: "path", value: path });
      seen.add(key);
    }
  }
  return entries;
}

export function hasAssignedProfileCertificates(certificateEntries) {
  return (certificateEntries ?? []).some((entry) => {
    const kind = String(entry?.kind ?? "");
    const value = String(entry?.value ?? "").trim();
    return (kind === "id" || kind === "path") && value.length > 0;
  });
}

export function slugId(value) {
  return String(value ?? "")
    .toLowerCase()
    .replace(/[^a-z0-9]+/g, "-")
    .replace(/^-+|-+$/g, "");
}

export function makeUniqueId(seed, existingIds) {
  const base = slugId(seed) || "item";
  let candidate = base;
  let suffix = 2;
  while (existingIds.has(candidate)) {
    candidate = `${base}-${suffix}`;
    suffix += 1;
  }
  return candidate;
}

export function normalizeGlobalSecuritySettings(raw) {
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

export function buildGlobalSecuritySaveRequest(state) {
  return {
    startupPage: state.startupPage?.trim() ? state.startupPage.trim() : null,
    certificates: state.certificates ?? [],
    blockedDomainSuffixes: state.blockedDomainSuffixes ?? [],
    blocklists: state.blocklists ?? []
  };
}

export function syncManagedCertificateAssignments(globalSecurity, profileId, certificateState) {
  const selectedIds = new Set(
    (certificateState ?? [])
      .filter((item) => item.kind === "id")
      .map((item) => String(item.value ?? "").trim())
      .filter(Boolean)
  );
  const nextCertificates = (globalSecurity?.certificates ?? []).map((item) => {
    const nextProfileIds = new Set((item.profileIds ?? []).filter((value) => value !== profileId));
    if (selectedIds.has(item.id)) {
      nextProfileIds.add(profileId);
    }
    return { ...item, profileIds: [...nextProfileIds] };
  });
  return {
    ...(globalSecurity ?? {}),
    certificates: nextCertificates
  };
}
