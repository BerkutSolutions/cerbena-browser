import { askInputModal } from "../../core/modal.js";
import { saveGlobalSecuritySettings } from "../security/api.js";
import {
  buildGlobalSecuritySaveRequest,
  ensureGlobalSecurityState,
  hydrateGlobalSecurityState,
  COMMON_SUFFIXES
} from "../security/shared.js";
import { getServiceCatalog, saveDnsPolicy } from "../network/api.js";
import { BLOCKLIST_PRESETS, CATEGORY_LABEL_KEYS, serviceLabel } from "./catalog.js";
import { updateProfile } from "../profiles/api.js";
import {
  applyTemplateToDraft,
  blockedServicesToPairs,
  createTemplateSnapshot,
  loadDnsTemplates,
  loadProfileDnsDraft,
  saveDnsTemplates,
  saveProfileDnsDraft,
  templateMatchesDraft
} from "./store.js";
import {
  applyPolicyPresetToDraft,
  createPolicyPresetFromDraft,
  defaultPolicyPreset,
  DNS_POLICY_LEVELS,
  loadPolicyPresets,
  resetPolicyPreset,
  savePolicyPresets,
  summarizePolicyPreset
} from "./policy-store.js";

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

async function persistPolicy(model, root, t) {
  const draft = currentDraft(model);
  ensureCustomDnsServers(draft);
  const profile = model.profiles?.find((item) => item.id === model.selectedProfileId);
  if (!profile?.id) return;
  const selectedBlocklistCatalog = blocklistCatalogItems(ensureGlobalSecurityState(model));
  const payload = {
    profile_id: profile.id,
    dns_config: {
      mode: draft.mode,
      servers: draft.servers.split(",").map((value) => value.trim()).filter(Boolean),
      doh_url: null,
      dot_server_name: null
    },
    selected_blocklists: selectedBlocklistCatalog
      .filter((item) => draft.selectedBlocklists.includes(item.id))
      .map((item) => ({
        list_id: item.id,
        domains: item.domains ?? [],
        updated_at_epoch: Math.floor(Date.now() / 1000)
      })),
    selected_services: blockedServicesToPairs(draft.blockedServices),
    domain_allowlist: draft.allowlist.split(",").map((value) => value.trim()).filter(Boolean),
    domain_denylist: draft.denylist.split(",").map((value) => value.trim()).filter(Boolean),
    domain_exceptions: []
  };
  const result = await saveDnsPolicy(profile.id, payload);
  if (!result.ok) {
    model.dnsNotice = { type: "error", text: String(result.data.error) };
    return;
  }
  const policyLevel = root?.querySelector("#dns-policy-level")?.value ?? "normal";
  const tags = (profile.tags ?? []).filter((tag) => !tag.startsWith("policy:"));
  tags.push(`policy:${policyLevel}`);
  const updateResult = await updateProfile({
    profileId: profile.id,
    tags,
    expectedUpdatedAt: profile.updated_at
  });
  model.dnsNotice = {
    type: updateResult.ok ? "success" : "error",
    text: updateResult.ok ? t("dns.saved") : String(updateResult.data.error)
  };
}

async function persistGlobalDnsState(model, t, rerender) {
  const state = ensureGlobalSecurityState(model);
  const result = await saveGlobalSecuritySettings(buildGlobalSecuritySaveRequest(state));
  model.dnsNotice = {
    type: result.ok ? "success" : "error",
    text: result.ok ? t("action.save") : String(result.data.error)
  };
  await rerender();
}

function syncDraft(root, model) {
  const draft = currentDraft(model);
  draft.mode = root.querySelector("#dns-mode")?.value ?? "system";
  draft.servers = root.querySelector("#dns-servers")?.value ?? "";
  draft.allowlist = root.querySelector("#dns-allow")?.value ?? "";
  draft.denylist = root.querySelector("#dns-deny")?.value ?? "";
  draft.search = root.querySelector("#dns-service-search")?.value ?? "";
  draft.templateName = root.querySelector("#dns-template-name")?.value ?? draft.templateName ?? "";
  persistDraft(model);
}

function ensureCustomDnsServers(draft) {
  if ((draft.mode ?? "system") !== "custom") return;
  if (String(draft.servers ?? "").trim()) return;
  draft.servers = "1.1.7.1,8.8.8.8";
}

function setCategoryBlocked(draft, categoryKey, blocked) {
  for (const serviceKey of Object.keys(draft.blockedServices?.[categoryKey] ?? {})) {
    draft.blockedServices[categoryKey][serviceKey] = blocked;
  }
}

function saveTemplatesAndDraft(model) {
  saveDnsTemplates(model.dnsTemplates);
  persistDraft(model);
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
            <input id="dns-servers" value="${draft.servers}" placeholder="1.1.7.1,8.8.8.8" />
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

export function wireDns(root, model, rerender, t) {
  const draft = currentDraft(model);
  const globalSecurity = ensureGlobalSecurityState(model);
  const policyPresets = model.dnsPolicyPresets ?? loadPolicyPresets(model.serviceCatalog);
  model.dnsPolicyPresets = policyPresets;

  for (const input of root.querySelectorAll("#dns-servers, #dns-allow, #dns-deny, #dns-template-name")) {
    input.addEventListener("change", async () => {
      syncDraft(root, model);
      await persistPolicy(model, root, t);
      await rerender();
    });
    input.addEventListener("input", () => syncDraft(root, model));
  }

  root.querySelector("#dns-mode")?.addEventListener("change", async () => {
    syncDraft(root, model);
    ensureCustomDnsServers(draft);
    if (draft.mode === "custom") {
      const serversInput = root.querySelector("#dns-servers");
      if (serversInput && !serversInput.value.trim()) {
        serversInput.value = draft.servers;
      }
    }
    await persistPolicy(model, root, t);
    await rerender();
  });

  root.querySelector("#dns-policy-level")?.addEventListener("change", async () => {
    await persistPolicy(model, root, t);
    await rerender();
  });

  const applyPolicyPreset = async (level) => {
    const preset = policyPresets[level];
    if (!preset) return;
    model.dnsUiState.editingPolicyLevel = level;
    applyPolicyPresetToDraft(draft, preset, model.serviceCatalog);
    ensureCustomDnsServers(draft);
    persistDraft(model);
    model.dnsNotice = { type: "success", text: t("dns.policy.loaded") };
    await persistPolicy(model, root, t);
    await rerender();
  };

  const resetPolicyLevel = async (level) => {
    policyPresets[level] = resetPolicyPreset(level, model.serviceCatalog);
    savePolicyPresets(policyPresets);
    if (model.dnsUiState.editingPolicyLevel === level) {
      model.dnsUiState.editingPolicyLevel = "";
    }
    model.dnsNotice = { type: "success", text: t("dns.policy.deleted") };
    await rerender();
  };

  root.querySelector("#dns-policy-commit")?.addEventListener("click", async () => {
    syncDraft(root, model);
    ensureCustomDnsServers(draft);
    const level = model.dnsUiState.editingPolicyLevel;
    if (!level) return;
    policyPresets[level] = createPolicyPresetFromDraft(level, draft, model.serviceCatalog);
    savePolicyPresets(policyPresets);
    model.dnsNotice = { type: "success", text: t("dns.policy.levelSaved") };
    model.dnsUiState.editingPolicyLevel = "";
    await rerender();
  });

  root.querySelector("#dns-policy-cancel-edit")?.addEventListener("click", async () => {
    model.dnsUiState.editingPolicyLevel = "";
    model.dnsNotice = null;
    await rerender();
  });

  for (const row of root.querySelectorAll("[data-policy-row]")) {
    row.addEventListener("click", async (event) => {
      if (event.target.closest("[data-policy-edit]") || event.target.closest("[data-policy-delete]")) return;
      model.dnsUiState.policyModal = { level: row.getAttribute("data-policy-row") };
      await rerender();
    });
  }

  root.querySelector("#dns-policy-modal-close")?.addEventListener("click", async () => {
    model.dnsUiState.policyModal = null;
    await rerender();
  });

  root.querySelector("#dns-policy-modal-overlay")?.addEventListener("click", async (event) => {
    if (event.target.id !== "dns-policy-modal-overlay") return;
    model.dnsUiState.policyModal = null;
    await rerender();
  });

  for (const button of root.querySelectorAll("[data-policy-edit]")) {
    button.addEventListener("click", async (event) => {
      event.stopPropagation();
      await applyPolicyPreset(button.getAttribute("data-policy-edit"));
    });
  }

  for (const button of root.querySelectorAll("[data-policy-delete]")) {
    button.addEventListener("click", async (event) => {
      event.stopPropagation();
      await resetPolicyLevel(button.getAttribute("data-policy-delete"));
    });
  }

  root.querySelector("#dns-service-search")?.addEventListener("input", async () => {
    syncDraft(root, model);
    await rerender();
  });

  for (const checkbox of root.querySelectorAll("[data-action='toggle-service']")) {
    checkbox.addEventListener("change", async () => {
      const categoryKey = checkbox.getAttribute("data-category");
      const serviceKey = checkbox.getAttribute("data-service");
      draft.blockedServices[categoryKey][serviceKey] = checkbox.checked;
      persistDraft(model);
      await persistPolicy(model, root, t);
      await rerender();
    });
  }

  for (const button of root.querySelectorAll("[data-action='toggle-category']")) {
    button.addEventListener("click", async () => {
      const categoryKey = button.getAttribute("data-category");
      const services = Object.values(draft.blockedServices?.[categoryKey] ?? {});
      const shouldBlock = !services.every(Boolean);
      setCategoryBlocked(draft, categoryKey, shouldBlock);
      persistDraft(model);
      await persistPolicy(model, root, t);
      await rerender();
    });
  }

  root.querySelector("#dns-template-select")?.addEventListener("change", async (event) => {
    const templateId = event.target.value;
    const template = model.dnsTemplates.find((item) => item.id === templateId) ?? null;
    applyTemplateToDraft(draft, template, model.serviceCatalog);
    saveTemplatesAndDraft(model);
    await persistPolicy(model, root, t);
    await rerender();
  });

  root.querySelector("#dns-template-save-new")?.addEventListener("click", async () => {
    syncDraft(root, model);
    const name = draft.templateName?.trim();
    if (!name) {
      model.dnsNotice = { type: "error", text: t("dns.template.nameRequired") };
      await rerender();
      return;
    }
    const template = createTemplateSnapshot(name, draft);
    model.dnsTemplates.push(template);
    draft.activeTemplateId = template.id;
    draft.templateName = template.name;
    saveTemplatesAndDraft(model);
    model.dnsNotice = { type: "success", text: t("dns.template.saved") };
    await rerender();
  });

  root.querySelector("#dns-template-save-current")?.addEventListener("click", async () => {
    syncDraft(root, model);
    const template = selectedTemplate(model);
    if (!template) return;
    template.name = draft.templateName?.trim() || template.name;
    template.selectedBlocklists = [...draft.selectedBlocklists];
    template.blockedServices = structuredClone(draft.blockedServices);
    template.updatedAt = Date.now();
    draft.templateName = template.name;
    saveTemplatesAndDraft(model);
    model.dnsNotice = { type: "success", text: t("dns.template.updated") };
    await rerender();
  });

  root.querySelector("#dns-template-delete")?.addEventListener("click", async () => {
    const template = selectedTemplate(model);
    if (!template) return;
    model.dnsTemplates = model.dnsTemplates.filter((item) => item.id !== template.id);
    draft.activeTemplateId = "";
    draft.templateName = "";
    saveTemplatesAndDraft(model);
    model.dnsNotice = { type: "success", text: t("dns.template.deleted") };
    await rerender();
  });

  for (const toggle of root.querySelectorAll("[data-dns-accordion]")) {
    toggle.addEventListener("click", async () => {
      const id = toggle.getAttribute("data-dns-accordion");
      if (id === "blocklists") model.dnsUiState.blocklistsOpen = !(model.dnsUiState.blocklistsOpen !== false);
      if (id === "catalog") model.dnsUiState.serviceCatalogOpen = !(model.dnsUiState.serviceCatalogOpen !== false);
      await rerender();
    });
  }

  root.querySelector("#dns-blocklist-add")?.addEventListener("click", async () => {
    const url = await askInputModal(t, {
      title: t("security.blocklists.addUrl"),
      label: t("security.blocklists.prompt"),
      defaultValue: "https://adguardteam.github.io/HostlistsRegistry/assets/filter_1.txt"
    });
    if (!url?.trim()) return;
    const clean = url.trim();
    const sourceKey = normalizedSourceKey("url", clean);
    const duplicate = (globalSecurity.blocklists ?? [])
      .some((item) => normalizedSourceKey(item.sourceKind, item.sourceValue) === sourceKey);
    if (duplicate) {
      model.dnsNotice = {
        type: "error",
        text: `${t("profile.modal.validationError")}: ${t("security.source")}`
      };
      await rerender();
      return;
    }
    const existingIds = new Set((globalSecurity.blocklists ?? []).map((item) => String(item.id ?? "")));
    globalSecurity.blocklists.push({
      id: makeUniqueId(clean, existingIds),
      name: clean.split("/").pop() || clean,
      sourceKind: "url",
      sourceValue: clean,
      active: true,
      domains: []
    });
    await rerender();
  });

  root.querySelector("#dns-blocklist-file")?.addEventListener("click", async () => {
    const path = await askInputModal(t, {
      title: t("security.blocklists.addFile"),
      label: t("security.blocklists.filePrompt"),
      defaultValue: ""
    });
    if (!path?.trim()) return;
    const clean = path.trim();
    const sourceKey = normalizedSourceKey("file", clean);
    const duplicate = (globalSecurity.blocklists ?? [])
      .some((item) => normalizedSourceKey(item.sourceKind, item.sourceValue) === sourceKey);
    if (duplicate) {
      model.dnsNotice = {
        type: "error",
        text: `${t("profile.modal.validationError")}: ${t("security.source")}`
      };
      await rerender();
      return;
    }
    const existingIds = new Set((globalSecurity.blocklists ?? []).map((item) => String(item.id ?? "")));
    globalSecurity.blocklists.push({
      id: makeUniqueId(clean, existingIds),
      name: clean.split(/[/\\]/).pop() || clean,
      sourceKind: "file",
      sourceValue: clean,
      active: true,
      domains: []
    });
    await rerender();
  });

  root.querySelector("#dns-global-save")?.addEventListener("click", async () => {
    await persistGlobalDnsState(model, t, rerender);
  });

  for (const checkbox of root.querySelectorAll("[data-blocklist-active]")) {
    checkbox.addEventListener("change", async () => {
      const id = checkbox.getAttribute("data-blocklist-active");
      globalSecurity.blocklists = (globalSecurity.blocklists ?? []).map((item) => item.id === id ? { ...item, active: checkbox.checked } : item);
      await rerender();
    });
  }

  for (const button of root.querySelectorAll("[data-blocklist-remove]")) {
    button.addEventListener("click", async () => {
      const id = button.getAttribute("data-blocklist-remove");
      globalSecurity.blocklists = (globalSecurity.blocklists ?? []).filter((item) => item.id !== id);
      await rerender();
    });
  }

  root.querySelector("#dns-suffix-toggle")?.addEventListener("click", async () => {
    globalSecurity.suffixMenuOpen = !globalSecurity.suffixMenuOpen;
    await rerender();
  });

  root.querySelector("#dns-suffix-search")?.addEventListener("input", async (event) => {
    globalSecurity.suffixFilter = event.target.value;
    await rerender();
  });

  root.querySelector("#dns-suffix-search")?.addEventListener("keydown", async (event) => {
    if (event.key !== "Enter") return;
    const suffix = normalizeSuffixInput(event.target.value);
    if (!suffix) return;
    event.preventDefault();
    const next = selectedSuffixes(globalSecurity);
    next.add(suffix);
    globalSecurity.blockedDomainSuffixes = [...next].sort((left, right) => left.localeCompare(right));
    globalSecurity.suffixFilter = "";
    await rerender();
  });

  root.querySelector("#dns-suffix-toggle-all")?.addEventListener("click", async () => {
    globalSecurity.blockedDomainSuffixes = (globalSecurity.blockedDomainSuffixes ?? []).length === COMMON_SUFFIXES.length ? [] : [...COMMON_SUFFIXES];
    await rerender();
  });

  root.querySelector("#dns-suffix-create")?.addEventListener("click", async () => {
    const suffix = normalizeSuffixInput(globalSecurity.suffixFilter);
    if (!suffix) return;
    const next = selectedSuffixes(globalSecurity);
    next.add(suffix);
    globalSecurity.blockedDomainSuffixes = [...next].sort((left, right) => left.localeCompare(right));
    globalSecurity.suffixFilter = "";
    await rerender();
  });

  for (const checkbox of root.querySelectorAll("[data-suffix]")) {
    checkbox.addEventListener("change", async () => {
      const suffix = checkbox.getAttribute("data-suffix");
      const next = selectedSuffixes(globalSecurity);
      if (checkbox.checked) next.add(suffix);
      else next.delete(suffix);
      globalSecurity.blockedDomainSuffixes = [...next];
      await rerender();
    });
  }

  root.querySelector("#dns-suffix-save")?.addEventListener("click", async () => {
    await persistGlobalDnsState(model, t, rerender);
  });
}
