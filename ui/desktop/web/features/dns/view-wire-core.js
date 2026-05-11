import { askInputModal } from "../../core/modal.js";
import { saveGlobalSecuritySettings } from "../security/api.js";
import {
  buildGlobalSecuritySaveRequest,
  ensureGlobalSecurityState,
  hydrateGlobalSecurityState,
  COMMON_SUFFIXES
} from "../security/shared.js";
import { saveDnsPolicy } from "../network/api.js";
import { updateProfile } from "../profiles/api.js";
import {
  applyTemplateToDraft,
  createTemplateSnapshot,
  loadDnsTemplates,
  loadProfileDnsDraft,
  saveDnsTemplates,
  saveProfileDnsDraft
} from "./store.js";
import {
  applyPolicyPresetToDraft,
  createPolicyPresetFromDraft,
  loadPolicyPresets,
  resetPolicyPreset,
  savePolicyPresets
} from "./policy-store.js";

function ensureDnsModel(model) {
  if (!model.dnsTemplates) model.dnsTemplates = loadDnsTemplates();
  if (!model.dnsDrafts) model.dnsDrafts = {};
  if (!model.dnsNotice) model.dnsNotice = null;
  if (!model.dnsUiState) model.dnsUiState = {};
  if (!model.dnsPolicyPresets) model.dnsPolicyPresets = null;
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
  return new Set((state.blockedDomainSuffixes ?? []).map((suffix) => String(suffix).trim().toLowerCase()).filter(Boolean));
}

function normalizeSuffixInput(value) {
  return String(value ?? "")
    .trim()
    .replace(/^\./, "")
    .toLowerCase()
    .replace(/[^a-z0-9.-]/g, "");
}

function normalizedSourceKey(sourceKind, sourceValue) {
  return `${String(sourceKind ?? "").trim().toLowerCase()}:${String(sourceValue ?? "").trim().toLowerCase()}`;
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

async function persistPolicy(model, root, t) {
  const draft = currentDraft(model);
  const mode = root.querySelector("#dns-mode")?.value ?? draft.mode;
  const serversRaw = root.querySelector("#dns-servers")?.value ?? draft.servers;
  const servers = serversRaw.split(/\s+/).map((entry) => entry.trim()).filter(Boolean).join("\n");
  const allowlist = (root.querySelector("#dns-allow")?.value ?? draft.allowlist)
    .split(/[\n,]/)
    .map((entry) => entry.trim())
    .filter(Boolean);
  const denylist = (root.querySelector("#dns-deny")?.value ?? draft.denylist)
    .split(/[\n,]/)
    .map((entry) => entry.trim())
    .filter(Boolean);
  const policyLevel = root.querySelector("#dns-policy-level")?.value ?? draft.policyLevel ?? "off";
  const payload = { mode, servers, allowlist, denylist, selectedBlocklists: [...(draft.selectedBlocklists ?? [])], blockedServices: structuredClone(draft.blockedServices ?? {}), policyLevel };
  const result = await saveDnsPolicy(model.selectedProfileId ?? "default", payload);
  model.dnsNotice = {
    type: result.ok ? "success" : "error",
    text: result.ok ? t("dns.saved") : String(result.data.error)
  };
  if (!result.ok || !model.selectedProfileId) return;
  const profile = model.profiles?.find((item) => item.id === model.selectedProfileId);
  if (!profile) return;
  const nextTags = (profile.tags ?? []).filter((tag) => !tag.startsWith("dns-template:"));
  if (draft.activeTemplateId) nextTags.push(`dns-template:${draft.activeTemplateId}`);
  await updateProfile(model.selectedProfileId, { ...profile, tags: nextTags });
}

async function persistGlobalDnsState(model, t, rerender) {
  const state = ensureGlobalSecurityState(model);
  const request = buildGlobalSecuritySaveRequest(state);
  const result = await saveGlobalSecuritySettings(request);
  model.dnsNotice = {
    type: result.ok ? "success" : "error",
    text: result.ok ? t("action.save") : String(result.data.error)
  };
  if (result.ok) {
    await hydrateGlobalSecurityState(model);
  }
  await rerender();
}

function syncDraft(root, model) {
  const draft = currentDraft(model);
  draft.mode = root.querySelector("#dns-mode")?.value ?? draft.mode;
  draft.servers = root.querySelector("#dns-servers")?.value ?? draft.servers;
  draft.allowlist = root.querySelector("#dns-allow")?.value ?? draft.allowlist;
  draft.denylist = root.querySelector("#dns-deny")?.value ?? draft.denylist;
  draft.templateName = root.querySelector("#dns-template-name")?.value ?? draft.templateName;
  draft.policyLevel = root.querySelector("#dns-policy-level")?.value ?? draft.policyLevel ?? "off";
  persistDraft(model);
}

function ensureCustomDnsServers(draft) {
  if (draft.mode !== "custom") return;
  if (draft.servers?.trim()) return;
  draft.servers = "1.1.1.1\n9.9.9.9";
}

function setCategoryBlocked(draft, categoryKey, blocked) {
  const category = draft.blockedServices?.[categoryKey];
  if (!category) return;
  Object.keys(category).forEach((serviceKey) => { category[serviceKey] = blocked; });
}

function saveTemplatesAndDraft(model) {
  saveDnsTemplates(model.dnsTemplates);
  persistDraft(model);
}

function selectedTemplate(model) {
  const draft = currentDraft(model);
  return model.dnsTemplates.find((item) => item.id === draft.activeTemplateId) ?? null;
}

export function wireDns(root, model, rerender, t) {
  const draft = currentDraft(model);
  const globalSecurity = ensureGlobalSecurityState(model);
  const policyPresets = model.dnsPolicyPresets ?? loadPolicyPresets(model.serviceCatalog);
  model.dnsPolicyPresets = policyPresets;

  for (const input of root.querySelectorAll("#dns-servers, #dns-allow, #dns-deny, #dns-template-name")) {
    input.addEventListener("change", async () => { syncDraft(root, model); await persistPolicy(model, root, t); await rerender(); });
    input.addEventListener("input", () => syncDraft(root, model));
  }
  root.querySelector("#dns-mode")?.addEventListener("change", async () => { syncDraft(root, model); ensureCustomDnsServers(draft); await persistPolicy(model, root, t); await rerender(); });
  root.querySelector("#dns-policy-level")?.addEventListener("change", async () => { await persistPolicy(model, root, t); await rerender(); });

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
    if (model.dnsUiState.editingPolicyLevel === level) model.dnsUiState.editingPolicyLevel = "";
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
  root.querySelector("#dns-policy-cancel-edit")?.addEventListener("click", async () => { model.dnsUiState.editingPolicyLevel = ""; model.dnsNotice = null; await rerender(); });
  for (const row of root.querySelectorAll("[data-policy-row]")) {
    row.addEventListener("click", async (event) => {
      if (event.target.closest("[data-policy-edit]") || event.target.closest("[data-policy-delete]")) return;
      model.dnsUiState.policyModal = { level: row.getAttribute("data-policy-row") };
      await rerender();
    });
  }
  root.querySelector("#dns-policy-modal-close")?.addEventListener("click", async () => { model.dnsUiState.policyModal = null; await rerender(); });
  root.querySelector("#dns-policy-modal-overlay")?.addEventListener("click", async (event) => { if (event.target.id !== "dns-policy-modal-overlay") return; model.dnsUiState.policyModal = null; await rerender(); });
  for (const button of root.querySelectorAll("[data-policy-edit]")) {
    button.addEventListener("click", async (event) => { event.stopPropagation(); await applyPolicyPreset(button.getAttribute("data-policy-edit")); });
  }
  for (const button of root.querySelectorAll("[data-policy-delete]")) {
    button.addEventListener("click", async (event) => { event.stopPropagation(); await resetPolicyLevel(button.getAttribute("data-policy-delete")); });
  }

  root.querySelector("#dns-service-search")?.addEventListener("input", async () => { syncDraft(root, model); await rerender(); });
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
      setCategoryBlocked(draft, categoryKey, !services.every(Boolean));
      persistDraft(model);
      await persistPolicy(model, root, t);
      await rerender();
    });
  }

  root.querySelector("#dns-template-select")?.addEventListener("change", async (event) => {
    const template = model.dnsTemplates.find((item) => item.id === event.target.value) ?? null;
    applyTemplateToDraft(draft, template, model.serviceCatalog);
    saveTemplatesAndDraft(model);
    await persistPolicy(model, root, t);
    await rerender();
  });
  root.querySelector("#dns-template-save-new")?.addEventListener("click", async () => {
    syncDraft(root, model);
    const name = draft.templateName?.trim();
    if (!name) { model.dnsNotice = { type: "error", text: t("dns.template.nameRequired") }; await rerender(); return; }
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
    const url = await askInputModal(t, { title: t("security.blocklists.addUrl"), label: t("security.blocklists.prompt"), defaultValue: "https://adguardteam.github.io/HostlistsRegistry/assets/filter_1.txt" });
    if (!url?.trim()) return;
    const clean = url.trim();
    const sourceKey = normalizedSourceKey("url", clean);
    if ((globalSecurity.blocklists ?? []).some((item) => normalizedSourceKey(item.sourceKind, item.sourceValue) === sourceKey)) {
      model.dnsNotice = { type: "error", text: `${t("profile.modal.validationError")}: ${t("security.source")}` };
      await rerender();
      return;
    }
    const existingIds = new Set((globalSecurity.blocklists ?? []).map((item) => String(item.id ?? "")));
    globalSecurity.blocklists.push({ id: makeUniqueId(clean, existingIds), name: clean.split("/").pop() || clean, sourceKind: "url", sourceValue: clean, active: true, domains: [] });
    await rerender();
  });
  root.querySelector("#dns-blocklist-file")?.addEventListener("click", async () => {
    const path = await askInputModal(t, { title: t("security.blocklists.addFile"), label: t("security.blocklists.filePrompt"), defaultValue: "" });
    if (!path?.trim()) return;
    const clean = path.trim();
    const sourceKey = normalizedSourceKey("file", clean);
    if ((globalSecurity.blocklists ?? []).some((item) => normalizedSourceKey(item.sourceKind, item.sourceValue) === sourceKey)) {
      model.dnsNotice = { type: "error", text: `${t("profile.modal.validationError")}: ${t("security.source")}` };
      await rerender();
      return;
    }
    const existingIds = new Set((globalSecurity.blocklists ?? []).map((item) => String(item.id ?? "")));
    globalSecurity.blocklists.push({ id: makeUniqueId(clean, existingIds), name: clean.split(/[/\\]/).pop() || clean, sourceKind: "file", sourceValue: clean, active: true, domains: [] });
    await rerender();
  });
  root.querySelector("#dns-global-save")?.addEventListener("click", async () => { await persistGlobalDnsState(model, t, rerender); });
  for (const checkbox of root.querySelectorAll("[data-blocklist-active]")) {
    checkbox.addEventListener("change", async () => {
      const id = checkbox.getAttribute("data-blocklist-active");
      globalSecurity.blocklists = (globalSecurity.blocklists ?? []).map((item) => item.id === id ? { ...item, active: checkbox.checked } : item);
      await rerender();
    });
  }
  for (const button of root.querySelectorAll("[data-blocklist-remove]")) {
    button.addEventListener("click", async () => { globalSecurity.blocklists = (globalSecurity.blocklists ?? []).filter((item) => item.id !== button.getAttribute("data-blocklist-remove")); await rerender(); });
  }
  root.querySelector("#dns-suffix-toggle")?.addEventListener("click", async () => { globalSecurity.suffixMenuOpen = !globalSecurity.suffixMenuOpen; await rerender(); });
  root.querySelector("#dns-suffix-search")?.addEventListener("input", async (event) => { globalSecurity.suffixFilter = event.target.value; await rerender(); });
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
      if (checkbox.checked) next.add(suffix); else next.delete(suffix);
      globalSecurity.blockedDomainSuffixes = [...next];
      await rerender();
    });
  }
  root.querySelector("#dns-suffix-save")?.addEventListener("click", async () => { await persistGlobalDnsState(model, t, rerender); });
}
