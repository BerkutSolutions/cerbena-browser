import { buildTagPickerMarkup, collectTagOptions, tagSummary, uniqueTags } from "../../core/tag-picker.js";
import { APP_VERSION } from "../../core/app-version.js";

function escapeHtml(value) {
  return String(value ?? "")
    .replaceAll("&", "&amp;")
    .replaceAll("<", "&lt;")
    .replaceAll(">", "&gt;")
    .replaceAll("\"", "&quot;");
}

export function normalizeEngineScope(value) {
  const scope = String(value ?? "chromium/firefox").toLowerCase();
  if (scope === "firefox") return "firefox";
  if (scope === "chromium") return "chromium";
  return "chromium/firefox";
}

function isChromiumFamilyProfile(profile) {
  return profile?.engine === "chromium" || profile?.engine === "ungoogled-chromium";
}

function profileMatchesScope(profile, scope) {
  if (scope === "firefox") return profile.engine === "librewolf";
  if (scope === "chromium") return isChromiumFamilyProfile(profile);
  return true;
}

function compatibleProfiles(item, profiles) {
  const scope = normalizeEngineScope(item.engineScope);
  return (profiles ?? []).filter((profile) => profileMatchesScope(profile, scope));
}

function profileSummary(item, profiles, t) {
  const assigned = (item.assignedProfileIds ?? [])
    .map((id) => profiles.find((profile) => profile.id === id)?.name ?? id)
    .filter(Boolean);
  if (!assigned.length) return t("extensions.assign.none");
  if (assigned.length === 1) return assigned[0];
  return `${assigned[0]} +${assigned.length - 1}`;
}

function sourceLabel(item) {
  return item.storeUrl || item.packageFileName || item.sourceValue || "";
}

export function packageVariants(item) {
  const variants = Array.isArray(item?.packageVariants) && item.packageVariants.length
    ? item.packageVariants
    : [{
        engineScope: item?.engineScope,
        version: item?.version,
        sourceKind: item?.sourceKind,
        sourceValue: item?.sourceValue,
        logoUrl: item?.logoUrl,
        storeUrl: item?.storeUrl,
        packagePath: item?.packagePath,
        packageFileName: item?.packageFileName
      }];
  return variants
    .map((variant) => ({
      ...variant,
      engineScope: normalizeEngineScope(variant.engineScope),
      version: String(variant.version ?? item?.version ?? APP_VERSION).trim() || APP_VERSION
    }))
    .sort((left, right) => left.engineScope.localeCompare(right.engineScope));
}

function variantSourceLabel(variant) {
  return variant.storeUrl || variant.packageFileName || variant.sourceValue || "";
}

function variantSummaryVersion(item, t) {
  const variants = packageVariants(item);
  if (variants.length === 1) return variants[0].version;
  const versions = [...new Set(variants.map((variant) => variant.version).filter(Boolean))];
  return versions.length === 1
    ? versions[0]
    : t("extensions.version.multiple").replace("{count}", String(variants.length));
}

export function engineScopeLabel(scope, t) {
  const normalized = normalizeEngineScope(scope);
  if (normalized === "firefox") return t("extensions.filter.firefox");
  if (normalized === "chromium") return t("extensions.filter.chromium");
  return t("extensions.filter.hybrid");
}

function libraryFilterOptions(t) {
  return [
    { value: "all", label: t("extensions.filter.all") },
    { value: "chromium", label: t("extensions.filter.chromium") },
    { value: "firefox", label: t("extensions.filter.firefox") },
    { value: "chromium/firefox", label: t("extensions.filter.hybrid") }
  ];
}

function filterExtensionItems(items, filterValue) {
  const normalized = normalizeEngineScope(filterValue);
  if (!filterValue || filterValue === "all") return items;
  return items.filter((item) => normalizeEngineScope(item.engineScope) === normalized);
}

function filterExtensionItemsByTags(items, selectedTags) {
  const normalizedSelected = new Set(uniqueTags(selectedTags).map((tag) => tag.toLocaleLowerCase()));
  if (!normalizedSelected.size) return items;
  return items.filter((item) => (item.tags ?? []).some((tag) => normalizedSelected.has(String(tag).trim().toLocaleLowerCase())));
}

function collectExtensionTags(state) {
  return collectTagOptions(Object.values(state?.items ?? {}), (item) => item.tags ?? []);
}

function engineIcon(engineScope) {
  const scope = normalizeEngineScope(engineScope);
  if (scope === "firefox") {
    return `<svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="1.8"><path d="M7 4h10l2 4-1 8-4 4H10l-4-4-1-8 2-4z"/><path d="M9 9h.01M15 9h.01"/><path d="M9 14c1 1 2 1.5 3 1.5S14 15 15 14"/></svg>`;
  }
  if (scope === "chromium") {
    return `<svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="1.8"><circle cx="12" cy="12" r="9"/><path d="M3 12h18"/><path d="M12 3c3 3 4.5 6 4.5 9S15 18 12 21c-3-3-4.5-6-4.5-9S9 6 12 3z"/></svg>`;
  }
  return `<svg viewBox="0 0 24 24" fill="none" stroke-width="1.8"><defs><clipPath id="ext-split-left"><rect x="0" y="0" width="12" height="24" /></clipPath><clipPath id="ext-split-right"><rect x="12" y="0" width="12" height="24" /></clipPath></defs><g clip-path="url(#ext-split-left)" stroke="#8ec5ff"><circle cx="12" cy="12" r="9"/><path d="M3 12h18"/><path d="M12 3c3 3 4.5 6 4.5 9S15 18 12 21c-3-3-4.5-6-4.5-9S9 6 12 3z"/></g><g clip-path="url(#ext-split-right)" stroke="#ffb37a"><path d="M7 4h10l2 4-1 8-4 4H10l-4-4-1-8 2-4z"/><path d="M9 9h.01M15 9h.01"/><path d="M9 14c1 1 2 1.5 3 1.5S14 15 15 14"/></g></svg>`;
}

function trashIcon() {
  return `<svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="1.8"><path d="M3 6h18"/><path d="M8 6V4h8v2"/><path d="M19 6l-1 14H6L5 6"/><path d="M10 11v6M14 11v6"/></svg>`;
}

function extensionPlaceholderIcon() {
  return `<svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="1.8"><path d="M12 3a2 2 0 0 1 2 2v1h2.5A1.5 1.5 0 0 1 18 7.5V10h1a2 2 0 1 1 0 4h-1v2.5a1.5 1.5 0 0 1-1.5 1.5H14v1a2 2 0 1 1-4 0v-1H7.5A1.5 1.5 0 0 1 6 16.5V14H5a2 2 0 1 1 0-4h1V7.5A1.5 1.5 0 0 1 7.5 6H10V5a2 2 0 0 1 2-2z"/></svg>`;
}

function extensionLogo(item) {
  if (item.logoUrl) return `<img src="${escapeHtml(item.logoUrl)}" alt="${escapeHtml(item.displayName ?? "Extension")}" class="extension-library-logo-image" loading="lazy" />`;
  return `<span class="extension-library-logo-fallback">${extensionPlaceholderIcon()}</span>`;
}

function extensionCard(item, profiles, t) {
  const name = item.displayName ?? "Extension";
  const version = variantSummaryVersion(item, t);
  const assigned = profileSummary(item, profiles, t);
  const primaryTag = (item.tags ?? [])[0] ?? "";
  const scope = normalizeEngineScope(item.engineScope);
  return `<article class="extension-library-card" data-extension-id="${item.id}" tabindex="0" role="button" aria-label="${escapeHtml(name)}"><div class="extension-library-logo">${extensionLogo(item)}</div><div class="extension-library-body"><h3>${escapeHtml(name)}</h3><div class="extension-library-version">${escapeHtml(version)}</div>${primaryTag ? `<div class="extension-library-tag">${escapeHtml(primaryTag)}</div>` : ""}<div class="extension-library-assignment" title="${escapeHtml(assigned)}">${escapeHtml(assigned)}</div></div><div class="extension-library-engine-badge engine-${scope === "firefox" ? "librewolf" : scope === "chromium" ? "chromium" : "hybrid"}" title="${escapeHtml(engineScopeLabel(item.engineScope, t))}">${engineIcon(item.engineScope)}</div></article>`;
}

function profileDropdownMarkup(item, profiles, t, mode = "modal") {
  const scopedProfiles = compatibleProfiles(item, profiles);
  if (!scopedProfiles.length) return `<div class="meta">${t("extensions.noCompatibleProfiles")}</div>`;
  const summary = profileSummary(item, profiles, t);
  const toggleAttr = mode === "modal" ? "data-modal-profile-menu-toggle" : "data-profile-menu-toggle";
  const menuAttr = mode === "modal" ? "data-modal-profile-menu" : "data-profile-menu";
  const checkboxAttr = mode === "modal" ? "data-modal-profile-assign" : "data-profile-assign";
  return `<div class="dns-dropdown extension-library-profile-picker"><button type="button" class="dns-dropdown-toggle extension-library-profile-toggle" ${toggleAttr}="${item.id}">${escapeHtml(summary)}</button><div class="dns-dropdown-menu hidden extension-library-profile-menu" ${menuAttr}="${item.id}">${scopedProfiles.map((profile) => `<label class="dns-dropdown-option"><input type="checkbox" ${checkboxAttr}="${item.id}:${profile.id}" ${(item.assignedProfileIds ?? []).includes(profile.id) ? "checked" : ""}/><span>${escapeHtml(profile.name)}</span></label>`).join("")}</div></div>`;
}

export function variantDetailCard(item, variant, t) {
  const source = variantSourceLabel(variant);
  const link = variant.storeUrl?.trim()
    ? `<a href="${escapeHtml(variant.storeUrl)}" target="_blank" rel="noreferrer" class="extension-library-store-link">${escapeHtml(variant.storeUrl)}</a>`
    : `<span class="meta">${source ? escapeHtml(source) : t("extensions.noStoreUrl")}</span>`;
  return `<div class="extension-library-variant-card"><div class="extension-library-variant-copy">${link}<div class="extension-library-variant-meta"><span class="extension-library-variant-chip">${escapeHtml(engineScopeLabel(variant.engineScope, t))}</span><span class="extension-library-variant-chip">${escapeHtml(t("extensions.version"))}: ${escapeHtml(variant.version)}</span></div></div><button type="button" class="profiles-icon-btn danger extension-library-variant-remove" data-action="remove-variant" data-engine-scope="${escapeHtml(normalizeEngineScope(variant.engineScope))}" aria-label="${t("extensions.remove")}" title="${t("extensions.remove")}">${trashIcon()}</button></div>`;
}

export function extensionModalHtml(t, profiles, item) {
  const scopeLabel = engineScopeLabel(item.engineScope, t);
  return `<div class="profiles-modal-overlay" id="extension-library-overlay"><div class="profiles-modal-window extension-library-modal extension-library-modal-window"><div class="profiles-cookie-head"><h3>${escapeHtml(item.displayName ?? t("nav.extensions"))}</h3><button type="button" class="profiles-icon-btn" id="extension-library-close" aria-label="${t("action.cancel")}"><span class="extension-library-close-glyph">x</span></button></div><div class="extension-library-modal-layout"><aside class="extension-library-modal-rail"><div class="extension-library-modal-identity"><div class="extension-library-logo extension-library-logo-lg">${extensionLogo(item)}</div><div class="extension-library-modal-name">${escapeHtml(item.displayName ?? t("nav.extensions"))}</div></div><div class="extension-library-modal-engine-chip">${escapeHtml(scopeLabel)}</div><div class="extension-library-modal-settings"><label class="checkbox-inline"><input type="checkbox" id="extension-auto-update" ${item.autoUpdateEnabled ? "checked" : ""} /><span>${t("extensions.autoUpdate")}</span></label><label class="checkbox-inline"><input type="checkbox" id="extension-preserve-on-panic" ${item.preserveOnPanicWipe ? "checked" : ""} /><span>${t("extensions.preserveOnPanicWipe")}</span></label><label class="checkbox-inline"><input type="checkbox" id="extension-protect-data-on-panic" ${item.protectDataFromPanicWipe ? "checked" : ""} /><span>${t("extensions.protectDataFromPanicWipe")}</span></label></div></aside><div class="extension-library-modal-main"><div class="security-frame extension-library-modal-frame"><h4>${t("extensions.sources")}</h4><div class="extension-library-variant-list">${packageVariants(item).map((variant) => variantDetailCard(item, variant, t)).join("")}</div></div><div class="security-frame extension-library-modal-frame"><h4>${t("extensions.profiles")}</h4>${profileDropdownMarkup(item, profiles, t, "modal")}</div><div class="security-frame extension-library-modal-frame"><h4>${t("extensions.tags")}</h4>${buildTagPickerMarkup({ id: "extension-tags", selectedTags: item.tags ?? [], availableTags: [], emptyLabel: t("extensions.tags.empty"), searchPlaceholder: t("extensions.tags.search"), createLabel: (value) => t("extensions.tags.create").replace("{tag}", value) })}</div></div></div><footer class="modal-actions"><button type="button" id="extension-library-cancel">${t("action.cancel")}</button><button type="button" id="extension-library-save">${t("action.save")}</button></footer></div></div>`;
}

export function renderExtensions(t, model) {
  const state = model.extensionLibraryState ?? { autoUpdateEnabled: false, items: {} };
  const items = Object.values(state.items ?? {});
  const filterValue = model.extensionLibraryFilter ?? "all";
  const selectedTags = uniqueTags(model.extensionLibraryTagFilter ?? []);
  const visibleItems = filterExtensionItemsByTags(filterExtensionItems(items, filterValue), selectedTags);
  const filterOptions = libraryFilterOptions(t);
  const availableTags = collectExtensionTags(state);
  const notice = model.extensionNotice ? `<p class="notice ${model.extensionNotice.type}">${model.extensionNotice.text}</p>` : "";
  return `<div class="feature-page"><div class="feature-page-head row-between"><div><h2>${t("nav.extensions")}</h2><p class="meta">${t("extensions.subtitle")}</p></div><div class="top-actions"><button id="extension-add-url">${t("extensions.addStoreUrl")}</button><div class="dns-dropdown extension-actions-dropdown"><button type="button" class="dns-dropdown-toggle extension-actions-toggle" id="extension-import-toggle">${t("extensions.import")}</button><div class="dns-dropdown-menu hidden extension-actions-menu" id="extension-import-menu"><button type="button" class="dns-dropdown-option" data-extension-import-mode="file">${t("extensions.transfer.file")}</button><button type="button" class="dns-dropdown-option" data-extension-import-mode="local-folder">${t("extensions.source.localFolder")}</button><button type="button" class="dns-dropdown-option" data-extension-import-mode="archive">${t("extensions.transfer.archive")}</button></div></div><div class="dns-dropdown extension-actions-dropdown"><button type="button" class="dns-dropdown-toggle extension-actions-toggle" id="extension-export-toggle">${t("extensions.export")}</button><div class="dns-dropdown-menu hidden extension-actions-menu" id="extension-export-menu"><button type="button" class="dns-dropdown-option" data-extension-export-mode="file">${t("extensions.transfer.file")}</button><button type="button" class="dns-dropdown-option" data-extension-export-mode="archive">${t("extensions.transfer.archive")}</button></div></div></div></div>${notice}<div class="panel extension-library-toolbar"><label><span>${t("extensions.filter.label")}</span><select id="extension-library-filter">${filterOptions.map((option) => `<option value="${option.value}" ${option.value === filterValue ? "selected" : ""}>${escapeHtml(option.label)}</option>`).join("")}</select></label><label class="checkbox-inline"><input type="checkbox" id="extension-auto-update-all" ${state.autoUpdateEnabled ? "checked" : ""} /><span>${t("extensions.autoUpdateAll")}</span></label>${buildTagPickerMarkup({ id: "extension-filter-tags", selectedTags, availableTags, emptyLabel: t("extensions.tags.filterAll"), searchPlaceholder: t("extensions.tags.search"), createLabel: null, allowCreate: false, toggleLabel: tagSummary(selectedTags, t("extensions.tags.filterAll")) })}</div><input id="extension-local-picker" type="file" accept=".zip,.xpi,.crx,application/zip,application/x-xpinstall" style="display:none;" /><div id="extension-dropzone" class="profiles-target-box extension-library-dropzone"><div class="meta">${t("extensions.dropHint")}</div></div><div class="extension-library-grid">${visibleItems.length ? visibleItems.map((item) => extensionCard(item, model.profiles ?? [], t)).join("") : `<div class="panel"><div class="meta">${t("extensions.empty")}</div></div>`}</div></div>`;
}
