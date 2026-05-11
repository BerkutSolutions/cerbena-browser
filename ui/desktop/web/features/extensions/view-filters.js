import { collectTagOptions, uniqueTags } from "../../core/tag-picker.js";

export function normalizeEngineScope(value) {
  const scope = String(value ?? "chromium/firefox").toLowerCase();
  if (scope === "firefox") return "firefox";
  if (scope === "chromium") return "chromium";
  return "chromium/firefox";
}

export function profileMatchesScope(profile, scope) {
  if (scope === "firefox") return profile.engine === "librewolf" || profile.engine === "firefox-esr";
  if (scope === "chromium") return profile?.engine === "chromium" || profile?.engine === "ungoogled-chromium";
  return true;
}

export function compatibleProfiles(item, profiles) {
  const scope = normalizeEngineScope(item.engineScope);
  return (profiles ?? []).filter((profile) => profileMatchesScope(profile, scope));
}

export function profileSummary(item, profiles, t) {
  const assigned = (item.assignedProfileIds ?? [])
    .map((id) => profiles.find((profile) => profile.id === id)?.name ?? id)
    .filter(Boolean);
  if (!assigned.length) return t("extensions.assign.none");
  if (assigned.length === 1) return assigned[0];
  return `${assigned[0]} +${assigned.length - 1}`;
}

export function libraryFilterOptions(t) {
  return [
    { value: "all", label: t("extensions.filter.all") },
    { value: "chromium", label: t("extensions.filter.chromium") },
    { value: "firefox", label: t("extensions.filter.firefox") },
    { value: "chromium/firefox", label: t("extensions.filter.hybrid") }
  ];
}

export function filterExtensionItems(items, filterValue) {
  const normalized = normalizeEngineScope(filterValue);
  if (!filterValue || filterValue === "all") return items;
  return items.filter((item) => normalizeEngineScope(item.engineScope) === normalized);
}

export function filterExtensionItemsByTags(items, selectedTags) {
  const normalizedSelected = new Set(uniqueTags(selectedTags).map((tag) => tag.toLocaleLowerCase()));
  if (!normalizedSelected.size) return items;
  return items.filter((item) => (item.tags ?? []).some((tag) => normalizedSelected.has(String(tag).trim().toLocaleLowerCase())));
}

export function collectExtensionTags(state) {
  return collectTagOptions(Object.values(state?.items ?? {}), (item) => item.tags ?? []);
}
