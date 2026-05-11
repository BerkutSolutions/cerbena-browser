import {
  ensureGlobalSecurityState,
  hydrateGlobalSecurityState,
  COMMON_SUFFIXES
} from "../security/shared.js";
import { getServiceCatalog } from "../network/api.js";
import { BLOCKLIST_PRESETS, CATEGORY_LABEL_KEYS, serviceLabel } from "./catalog.js";
import {
  applyTemplateToDraft,
  blockedServicesToPairs,
  createTemplateSnapshot,
  loadDnsTemplates,
  loadProfileDnsDraft,
  saveProfileDnsDraft,
  templateMatchesDraft
} from "./store.js";
import {
  defaultPolicyPreset,
  DNS_POLICY_LEVELS,
  loadPolicyPresets,
  summarizePolicyPreset
} from "./policy-store.js";
import { wireDns } from "./view-wire.js";
export { wireDns } from "./view-wire.js";
import {
  accordionSection,
  blocklistCatalogItems,
  categoryCard,
  currentDraft,
  ensureDnsModel,
  escapeHtml,
  makeUniqueId,
  normalizeSuffixInput,
  normalizedSourceKey,
  pencilIcon,
  persistDraft,
  policyModalHtml,
  policySummaryLine,
  policyTitle,
  selectedSuffixes,
  selectedTemplate,
  serviceNamesFromPreset,
  slugId,
  suffixOptions,
  suffixSummary,
  summarizePolicyDetails,
  templateToolbarHtml,
  trashIcon
} from "./view-core-support.js";

export function renderDns(t, model) {
  ensureDnsModel(model);
  const draft = currentDraft(model);
  const globalSecurity = ensureGlobalSecurityState(model);
  const notice = model.dnsNotice ? `<p class="notice ${model.dnsNotice.type}">${model.dnsNotice.text}</p>` : "";
  const categories = Object.values(model.serviceCatalog?.categories ?? {});
  const profile = model.profiles?.find((item) => item.id === model.selectedProfileId);
  const policyLevel = profile?.tags?.find((tag) => tag.startsWith("policy:"))?.replace("policy:", "") ?? "normal";
  const policyRows = DNS_POLICY_LEVELS.map((level) => {
    const preset = model.dnsPolicyPresets?.[level] ?? defaultPolicyPreset(model.serviceCatalog, level);
    const summary = summarizePolicyPreset(preset);
    return `
      <tr data-policy-row="${level}" class="${level === policyLevel ? "is-running" : ""}">
        <td>${escapeHtml(policyTitle(level, t))}</td>
        <td>${summary.blocklists}</td>
        <td>${summary.blockedServices}</td>
        <td>${summary.denyDomains}</td>
        <td class="profiles-cell-actions">
          <div class="profiles-actions-row">
            <button type="button" class="profiles-icon-btn" data-policy-edit="${level}" aria-label="${t("profile.action.edit")}" title="${t("profile.action.edit")}">${pencilIcon()}</button>
            <button type="button" class="profiles-icon-btn danger" data-policy-delete="${level}" aria-label="${t("extensions.remove")}" title="${t("extensions.remove")}">${trashIcon()}</button>
          </div>
        </td>
      </tr>
    `;
  }).join("");
  const suffixes = suffixOptions(globalSecurity);
  const suffixQuery = String(globalSecurity.suffixFilter ?? "");
  const suffixCreateValue = normalizeSuffixInput(suffixQuery);
  const dnsMode = draft.mode ?? "system";
  const isManual = dnsMode === "custom";
  const editingPolicyLevel = model.dnsUiState.editingPolicyLevel ?? "";

  const blocklistsBody = `
    <table class="extensions-table">
      <thead><tr><th>${t("extensions.name")}</th><th>${t("security.source")}</th><th>${t("security.status")}</th><th>${t("extensions.actions")}</th></tr></thead>
      <tbody>
        ${(globalSecurity.blocklists ?? []).map((item) => `
          <tr>
            <td>${escapeHtml(item.name)}</td>
            <td>${escapeHtml(item.sourceValue)}</td>
            <td><label class="checkbox-inline"><input type="checkbox" data-blocklist-active="${item.id}" ${item.active ? "checked" : ""}/> <span>${item.active ? t("security.active") : t("security.inactive")}</span></label></td>
            <td class="actions"><button type="button" data-blocklist-remove="${item.id}">${t("extensions.remove")}</button></td>
          </tr>
        `).join("") || `<tr><td colspan="4" class="meta">${t("extensions.empty")}</td></tr>`}
      </tbody>
    </table>
  `;

  const serviceCatalogBody = `
    <input id="dns-service-search" value="${draft.search}" placeholder="${t("dns.searchPlaceholder")}" />
    ${templateToolbarHtml(draft, model.dnsTemplates, t, model)}
    <div class="dns-categories">
      ${categories.map((category) => categoryCard(category, draft, draft.search, t)).join("") || `<p class="meta">${t("dns.catalogEmpty")}</p>`}
    </div>
  `;

  const blocklistsOpen = model.dnsUiState.blocklistsOpen !== false;
  const serviceCatalogOpen = model.dnsUiState.serviceCatalogOpen !== false;
  return `
    <div class="dns-page settings-tab-panel">
      <div class="dns-header">
        <div>
          <h2>${t("nav.dns")}</h2>
        </div>
      </div>
      ${notice}
      ${editingPolicyLevel ? `
        <section class="panel dns-policy-edit-banner">
          <strong>${t("dns.policy.editing").replace("{level}", policyTitle(editingPolicyLevel, t))}</strong>
          <div class="top-actions">
            <button type="button" id="dns-policy-commit">${t("dns.policy.saveLevel")}</button>
            <button type="button" id="dns-policy-cancel-edit">${t("action.cancel")}</button>
          </div>
        </section>
      ` : ""}

      <section class="panel dns-config-card">
        <div class="dns-config-grid">
          <label>${t("dns.mode")}
            <select id="dns-mode">
              <option value="system" ${dnsMode === "system" ? "selected" : ""}>${t("dns.system")}</option>
              <option value="custom" ${dnsMode === "custom" ? "selected" : ""}>${t("dns.custom")}</option>
            </select>
          </label>
          <label class="${isManual ? "" : "hidden"}" id="dns-servers-row">${t("dns.servers")}
            <input id="dns-servers" value="${draft.servers}" placeholder="1.2.1.1,8.8.8.8" />
          </label>
          <label>${t("dns.allowlist")}
            <input id="dns-allow" value="${draft.allowlist}" placeholder="example.com,github.com" />
          </label>
          <label>${t("dns.denylist")}
            <input id="dns-deny" value="${draft.denylist}" placeholder="ads.com,tracker.com" />
          </label>
        </div>
      </section>

      <section class="panel dns-suffix-section">
        <div class="dns-section-head">
          <div>
            <h3>${t("profile.policy")}</h3>
          </div>
        </div>
        <div class="profiles-table-shell">
          <table class="profiles-table dns-policy-table">
            <thead>
              <tr>
                <th>${t("dns.policy.table.level")}</th>
                <th>${t("dns.policy.table.blocklists")}</th>
                <th>${t("dns.policy.table.services")}</th>
                <th>${t("dns.policy.table.deny")}</th>
                <th class="profiles-col-actions"></th>
              </tr>
            </thead>
            <tbody>
              ${policyRows}
            </tbody>
          </table>
        </div>
      </section>

      ${accordionSection(
        "blocklists",
        t("profile.dns.blocklists"),
        "",
        blocklistsOpen,
        blocklistsBody,
        `<div class="top-actions"><button type="button" id="dns-blocklist-add">${t("security.blocklists.addUrl")}</button><button type="button" id="dns-blocklist-file">${t("security.blocklists.addFile")}</button><button type="button" id="dns-global-save">${t("action.save")}</button></div>`
      )}

      <section class="panel dns-suffix-section">
        <div class="dns-section-head">
          <div>
            <h3>${t("security.domainSuffixBlacklist")}</h3>
          </div>
          <button type="button" id="dns-suffix-save">${t("action.save")}</button>
        </div>
        <div class="dns-dropdown">
          <button type="button" class="dns-dropdown-toggle" id="dns-suffix-toggle">
            <input type="checkbox" tabindex="-1" ${(globalSecurity.blockedDomainSuffixes ?? []).length ? "checked" : ""} />
            <span>${escapeHtml(suffixSummary(globalSecurity, t))}</span>
          </button>
          <div class="dns-dropdown-menu dns-suffix-menu ${globalSecurity.suffixMenuOpen ? "" : "hidden"}" id="dns-suffix-menu">
            <input id="dns-suffix-search" value="${escapeHtml(globalSecurity.suffixFilter ?? "")}" placeholder="${t("security.suffix.search")}" />
            <div class="top-actions">
              <button type="button" id="dns-suffix-toggle-all">${(globalSecurity.blockedDomainSuffixes ?? []).length === COMMON_SUFFIXES.length ? t("security.clear") : t("security.all")}</button>
            </div>
            <div class="dns-suffix-chip-grid">
              ${suffixes.map((suffix) => `
                <label class="dns-suffix-chip">
                  <input type="checkbox" data-suffix="${suffix}" ${selectedSuffixes(globalSecurity).has(suffix) ? "checked" : ""} />
                  <span>.${suffix}</span>
                </label>
              `).join("")}
              ${!suffixes.length && suffixCreateValue ? `<button type="button" class="dns-suffix-create" id="dns-suffix-create">${t("security.suffix.create").replace("{suffix}", `.${suffixCreateValue}`)}</button>` : ""}
              ${!suffixes.length && !suffixCreateValue ? `<p class="meta">${t("dns.policy.empty")}</p>` : ""}
            </div>
          </div>
        </div>
      </section>

      ${accordionSection(
        "catalog",
        t("dns.serviceCatalog"),
        "",
        serviceCatalogOpen,
        serviceCatalogBody
      )}
      ${policyModalHtml(model.dnsUiState.policyModal ?? null, t, model)}
    </div>
  `;
}

export async function hydrateDnsModel(model) {
  ensureDnsModel(model);
  await hydrateGlobalSecurityState(model);
  if (!model.serviceCatalog) {
    const response = await getServiceCatalog();
    if (response.ok) {
      model.serviceCatalog = JSON.parse(response.data);
    }
  }
  model.dnsPolicyPresets = loadPolicyPresets(model.serviceCatalog);
  const draft = currentDraft(model);
  const profile = model.profiles?.find((item) => item.id === model.selectedProfileId);
  const taggedTemplateId = profile?.tags?.find((tag) => tag.startsWith("dns-template:"))?.replace("dns-template:", "");
  if (taggedTemplateId && !draft.activeTemplateId) {
    const template = model.dnsTemplates.find((item) => item.id === taggedTemplateId);
    if (template) {
      applyTemplateToDraft(draft, template, model.serviceCatalog);
      persistDraft(model);
    }
  }
}
