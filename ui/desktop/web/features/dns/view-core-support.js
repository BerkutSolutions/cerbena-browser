import { ensureGlobalSecurityState, COMMON_SUFFIXES } from "../security/shared.js";
import { BLOCKLIST_PRESETS, CATEGORY_LABEL_KEYS, serviceLabel } from "./catalog.js";
import { blockedServicesToPairs, loadDnsTemplates, loadProfileDnsDraft, saveProfileDnsDraft, templateMatchesDraft } from "./store.js";
import { summarizePolicyPreset } from "./policy-store.js";

function ensureDnsModel(model) {
  if (!model.dnsTemplates) model.dnsTemplates = loadDnsTemplates();
  if (!model.dnsDrafts) model.dnsDrafts = {};
  if (!model.dnsNotice) model.dnsNotice = null;
  if (!model.dnsUiState) model.dnsUiState = {};
  if (!model.dnsPolicyPresets) model.dnsPolicyPresets = null;
}

function escapeHtml(value) {
  return String(value ?? "")
    .replaceAll("&", "&amp;")
    .replaceAll("<", "&lt;")
    .replaceAll(">", "&gt;")
    .replaceAll('"', "&quot;");
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

function normalizedSourceKey(sourceKind, sourceValue) {
  return `${String(sourceKind ?? "").trim().toLowerCase()}:${String(sourceValue ?? "").trim().toLowerCase()}`;
}

function currentDraft(model) {
  ensureDnsModel(model);
  const profileId = model.selectedProfileId ?? "default";
  if (!model.dnsDrafts[profileId]) {
    model.dnsDrafts[profileId] = loadProfileDnsDraft(profileId, model.serviceCatalog);
  }
  return model.dnsDrafts[profileId];
}

function persistDraft(model) {
  const profileId = model.selectedProfileId ?? "default";
  saveProfileDnsDraft(profileId, currentDraft(model));
}

function selectedSuffixes(state) {
  return new Set(state.blockedDomainSuffixes ?? []);
}

function normalizeSuffixInput(value) {
  const raw = String(value ?? "").trim().toLowerCase();
  const withoutPrefix = raw.startsWith(".") ? raw.slice(1) : raw;
  if (!withoutPrefix) return null;
  return /^[a-z0-9-]+(?:\.[a-z0-9-]+)*$/i.test(withoutPrefix) ? withoutPrefix : null;
}

function suffixOptions(state) {
  const filter = String(state.suffixFilter ?? "").trim().toLowerCase().replace(/^\./, "");
  const selected = [...selectedSuffixes(state)];
  const pool = [...new Set([...COMMON_SUFFIXES, ...selected])].sort((left, right) => left.localeCompare(right));
  return pool.filter((item) => !filter || item.includes(filter));
}

function suffixSummary(state, t) {
  const total = state.blockedDomainSuffixes?.length ?? 0;
  return total ? `${total} ${t("security.selectionSuffix")}` : t("security.suffix.none");
}

function policyTitle(level, t) {
  return level === "disabled" ? t("dns.policy.disabled") : level;
}

function pencilIcon() {
  return `<svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="1.8"><path d="M12 20h9"/><path d="M16.5 3.5a2.1 2.1 0 0 1 3 3L8 18l-4 1 1-4 11.5-11.5z"/></svg>`;
}

function trashIcon() {
  return `<svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="1.8"><path d="M3 6h18"/><path d="M8 6V4h8v2"/><path d="M19 6l-1 14H6L5 6"/><path d="M10 11v6M14 11v6"/></svg>`;
}

function summarizePolicyDetails(preset, t) {
  const summary = summarizePolicyPreset(preset);
  return `${summary.blocklists} ${t("dns.policy.summary.blocklists")} • ${summary.blockedServices} ${t("dns.policy.summary.services")} • ${summary.denyDomains} ${t("dns.policy.summary.deny")}`;
}

function serviceNamesFromPreset(preset) {
  const names = [];
  for (const [categoryKey, services] of Object.entries(preset?.blockedServices ?? {})) {
    for (const [serviceKey, blocked] of Object.entries(services ?? {})) {
      if (blocked) {
        names.push({
          categoryKey,
          label: serviceLabel(serviceKey)
        });
      }
    }
  }
  return names.sort((left, right) => left.label.localeCompare(right.label));
}

function blocklistCatalogItems(globalSecurity) {
  const globalItems = (globalSecurity?.blocklists ?? []).map((item) => ({
    id: item.id,
    label: item.name ?? item.id,
    domains: item.domains ?? []
  }));
  const knownIds = new Set(globalItems.map((item) => item.id));
  for (const preset of BLOCKLIST_PRESETS) {
    if (!knownIds.has(preset.id)) {
      globalItems.push({
        id: preset.id,
        label: preset.label,
        domains: preset.domains ?? []
      });
    }
  }
  return globalItems;
}

function serviceRow(categoryKey, serviceKey, blocked) {
  return `
    <label class="dns-service-row">
      <input
        type="checkbox"
        data-category="${categoryKey}"
        data-service="${serviceKey}"
        data-action="toggle-service"
        ${blocked ? "checked" : ""}
      />
      <span>${serviceLabel(serviceKey)}</span>
    </label>
  `;
}

function categoryCard(category, draft, search, t) {
  const services = Object.keys(category.services ?? {}).filter((serviceKey) => {
    const needle = search.trim().toLowerCase();
    return !needle || serviceLabel(serviceKey).toLowerCase().includes(needle);
  });
  if (!services.length) return "";

  const allBlocked = services.every((serviceKey) => draft.blockedServices?.[category.category]?.[serviceKey]);
  return `
    <section class="dns-category-card">
      <div class="dns-category-head">
        <div>
          <h3>${t(CATEGORY_LABEL_KEYS[category.category] ?? "dns.serviceCatalog")}</h3>
          <p class="meta">${services.length} ${t("dns.servicesCount")}</p>
        </div>
        <button data-category="${category.category}" data-action="toggle-category">
          ${allBlocked ? t("dns.unblockAll") : t("dns.blockAll")}
        </button>
      </div>
      <div class="dns-services-grid">
        ${services.map((serviceKey) => serviceRow(category.category, serviceKey, Boolean(draft.blockedServices?.[category.category]?.[serviceKey]))).join("")}
      </div>
    </section>
  `;
}

function templateToolbarHtml(draft, templates, t, model) {
  const selectedTemplate = templates.find((item) => item.id === draft.activeTemplateId) ?? null;
  const isDirty = selectedTemplate ? !templateMatchesDraft(selectedTemplate, draft, model.serviceCatalog) : blockedServicesToPairs(draft.blockedServices).length > 0 || draft.selectedBlocklists.length > 0;
  return `
    <div class="dns-template-toolbar">
      <label class="dns-template-select">
        <span>${t("dns.template.current")}</span>
        <select id="dns-template-select">
          <option value="">${t("dns.template.custom")}</option>
          ${templates.map((template) => `<option value="${template.id}" ${template.id === draft.activeTemplateId ? "selected" : ""}>${template.name}</option>`).join("")}
        </select>
      </label>
      <label class="dns-template-name">
        <span>${t("dns.template.name")}</span>
        <input id="dns-template-name" value="${draft.templateName ?? ""}" placeholder="${t("dns.template.placeholder")}" />
      </label>
      <button type="button" id="dns-template-save-new">${t("dns.template.saveNew")}</button>
      <button type="button" id="dns-template-save-current" ${selectedTemplate ? "" : "disabled"}>${t("dns.template.update")}</button>
      <button type="button" id="dns-template-delete" ${selectedTemplate ? "" : "disabled"}>${t("dns.template.delete")}</button>
      <span class="badge">${isDirty ? t("dns.template.dirty") : t("dns.template.synced")}</span>
    </div>
  `;
}

function accordionSection(id, title, subtitle, open, body, actions = "") {
  return `
    <section class="panel dns-accordion-section">
      <div class="dns-accordion-head">
        <button type="button" class="dns-accordion-toggle" data-dns-accordion="${id}" aria-expanded="${open ? "true" : "false"}">
          <span class="dns-accordion-title">${escapeHtml(title)}</span>
          ${subtitle ? `<span class="meta">${escapeHtml(subtitle)}</span>` : ""}
        </button>
      </div>
      <div class="dns-accordion-body ${open ? "" : "hidden"}" id="dns-accordion-${id}">
        ${actions}
        ${body}
      </div>
    </section>
  `;
}

function selectedTemplate(model) {
  const draft = currentDraft(model);
  return model.dnsTemplates.find((item) => item.id === draft.activeTemplateId) ?? null;
}

function policySummaryLine(summary, t) {
  return [
    `${summary.blocklists} ${t("dns.policy.summary.blocklists")}`,
    `${summary.blockedServices} ${t("dns.policy.summary.services")}`,
    `${summary.allowDomains} ${t("dns.policy.summary.allow")}`,
    `${summary.denyDomains} ${t("dns.policy.summary.deny")}`
  ].join(" • ");
}

function policyModalHtml(policyModal, t, model) {
  if (!policyModal) return "";
  const preset = model.dnsPolicyPresets?.[policyModal.level];
  if (!preset) return "";
  const services = serviceNamesFromPreset(preset);
  const blocklists = [...(preset.selectedBlocklists ?? [])]
    .map((id) => blocklistCatalogItems(ensureGlobalSecurityState(model)).find((item) => item.id === id)?.label ?? id);
  const denyDomains = String(preset.denylist ?? "").split(",").map((item) => item.trim()).filter(Boolean);
  return `
    <div class="profiles-modal-overlay" id="dns-policy-modal-overlay">
      <div class="profiles-modal-window dns-policy-modal-window">
        <div class="action-modal">
          <div class="row-between">
            <h3>${escapeHtml(policyTitle(policyModal.level, t))}</h3>
            <button type="button" id="dns-policy-modal-close">${t("action.cancel")}</button>
          </div>
          <p class="meta">${escapeHtml(summarizePolicyDetails(preset, t))}</p>
          <div class="dns-policy-preview-grid">
            <section class="panel">
              <h4>${t("profile.dns.blocklists")}</h4>
              <div class="dns-policy-chip-list">
                ${blocklists.length ? blocklists.map((label) => `<span class="profiles-tag">${escapeHtml(label)}</span>`).join("") : `<span class="meta">${t("dns.policy.empty")}</span>`}
              </div>
            </section>
            <section class="panel">
              <h4>${t("dns.denylist")}</h4>
              <div class="dns-policy-chip-list">
                ${denyDomains.length ? denyDomains.map((domain) => `<span class="profiles-tag">${escapeHtml(domain)}</span>`).join("") : `<span class="meta">${t("dns.policy.empty")}</span>`}
              </div>
            </section>
            <section class="panel">
              <h4>${t("dns.serviceCatalog")}</h4>
              <div class="dns-policy-chip-list">
                ${services.length ? services.map((item) => `<span class="profiles-tag">${escapeHtml(item.label)}</span>`).join("") : `<span class="meta">${t("dns.policy.empty")}</span>`}
              </div>
            </section>
          </div>
        </div>
      </div>
    </div>
  `;
}


export {
  ensureDnsModel, escapeHtml, slugId, makeUniqueId, normalizedSourceKey, currentDraft, persistDraft,
  selectedSuffixes, normalizeSuffixInput, suffixOptions, suffixSummary, policyTitle, pencilIcon, trashIcon,
  summarizePolicyDetails, serviceNamesFromPreset, blocklistCatalogItems, serviceRow, categoryCard,
  templateToolbarHtml, accordionSection, selectedTemplate, policySummaryLine, policyModalHtml
};
