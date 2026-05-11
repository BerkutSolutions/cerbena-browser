import { templateChainLabel } from "./view-templates.js";
import { escapeHtml } from "./view-template-editor.js";

function sandboxBadge(type, text) {
  return `<span class="badge ${type}">${escapeHtml(text)}</span>`;
}
function sandboxModeLabel(mode, t) {
  return t(`network.sandbox.mode.${mode}`) || mode;
}
function sandboxAdapterLabel(adapterKind, t) {
  return t(`network.sandbox.adapter.${adapterKind}`) || adapterKind;
}
function activeRouteSummary(model, t, scope = "profile") {
  const templates = model.networkTemplates ?? [];
  const globalRoute = model.networkGlobalRoute ?? {};
  const payload = model.networkPolicyPayload ?? null;
  const selectedTemplateId = scope === "global"
    ? (globalRoute.globalVpnEnabled ? (globalRoute.defaultTemplateId ?? null) : null)
    : (model.networkSelectedTemplateId ?? null);
  const template = selectedTemplateId ? templates.find((item) => item.id === selectedTemplateId) : null;
  if (template) return `${template.name} (${templateChainLabel(template, t)})`;
  if (scope === "global") return t("network.sandbox.routeUnknown");
  if ((payload?.routeMode ?? "").toLowerCase() === "direct") return t("network.sandbox.routeDirect");
  return t("network.sandbox.routeUnknown");
}

function formatSandboxReason(reason, sandbox, adapter, activeRoute, t) {
  const value = String(reason || "").trim();
  if (!value) return t("network.sandbox.unknown");
  if (value.startsWith("container runtime probe failed:")) return t("network.sandbox.reason.containerProbeFailed");
  if (value.startsWith("docker runtime is not installed or not reachable:")) return t("network.sandbox.reason.containerRuntimeMissing");
  if (value === "No resolved strategy yet") return t("network.sandbox.unknown");
  if (sandbox?.effectiveMode === "container" && !adapter?.available) return t("network.sandbox.reason.containerProbeFailed");
  return value.replace("{route}", activeRoute).replace("{mode}", sandboxModeLabel(sandbox?.requestedMode || "auto", t));
}

export function renderSandboxFrame(model, t, options = {}) {
  const scope = options.scope || "profile";
  const sandbox = model.networkSandbox ?? null;
  const isGlobal = scope === "global";
  if (!isGlobal && !model.selectedProfileId) return `<div class="panel" style="margin-bottom:12px;"><h4>${t("network.sandbox.title")}</h4><p class="meta">${t("network.sandbox.profileRequired")}</p></div>`;
  if (!sandbox) return `<div class="panel" style="margin-bottom:12px;"><h4>${t("network.sandbox.title")}</h4><p class="meta">${t("network.sandbox.loading")}</p></div>`;
  const adapter = sandbox.adapter ?? { adapterKind: "unknown", runtimeKind: "unknown", available: true, requiresSystemNetworkAccess: false, maxHelperProcesses: 0, estimatedMemoryMb: 0, activeSandboxes: 0, maxActiveSandboxes: 0, supportsNativeIsolation: false, reason: t("network.sandbox.unknown") };
  const routeSummary = activeRouteSummary(model, t, isGlobal ? "global" : "profile");
  const resolutionBadge = adapter.available ? sandboxBadge("success", t("network.sandbox.available")) : sandboxBadge("error", t("network.sandbox.unavailable"));
  const selectedMode = isGlobal ? (sandbox.globalPolicyEnabled ? (["isolated", "compatibility-native", "container"].includes(sandbox.requestedMode) ? sandbox.requestedMode : "isolated") : "isolated") : (sandbox.preferredMode ?? "auto");
  const modeOptions = ["isolated", "compatibility-native", "container"].map((mode) => `<option value="${mode}" ${mode === selectedMode ? "selected" : ""}>${sandboxModeLabel(mode, t)}</option>`).join("");
  return `<div class="panel" style="margin-top:12px; margin-bottom:12px;"><div class="top-actions" style="align-items:flex-start; justify-content:space-between; gap:12px;"><div><h4 style="margin:0 0 6px 0;">${isGlobal ? t("network.sandbox.globalTitle") : t("network.sandbox.title")}</h4>${isGlobal ? "" : `<p class="meta" style="margin:0;">${t("network.sandbox.subtitle")}</p>`}</div>${resolutionBadge}</div><div class="grid-two" style="margin-top:12px;"><div><strong>${t("network.sandbox.effectiveMode")}</strong><p>${escapeHtml(sandboxModeLabel(sandbox.effectiveMode, t))}</p></div><div><strong>${t("network.sandbox.activeRoute")}</strong><p>${escapeHtml(routeSummary)}</p></div><div><strong>${t("network.sandbox.adapterLabel")}</strong><p>${escapeHtml(sandboxAdapterLabel(adapter.adapterKind, t))}</p></div><div><strong>${t("network.sandbox.runtimeLabel")}</strong><p>${escapeHtml(adapter.runtimeKind || "unknown")}</p></div></div>${isGlobal ? `<label class="checkbox-inline" style="margin-top:12px;"><input id="network-global-sandbox-enabled" type="checkbox" ${sandbox.globalPolicyEnabled ? "checked" : ""} /><span>${t("network.sandbox.globalEnable")}</span></label>` : ""}<label style="margin-top:12px;">${isGlobal ? t("network.sandbox.globalChooseMode") : t("network.sandbox.chooseMode")}<select id="${isGlobal ? "network-global-sandbox-mode" : "network-sandbox-mode"}" ${isGlobal && !sandbox.globalPolicyEnabled ? "disabled" : ""}>${modeOptions}</select></label><p class="meta" style="margin-top:8px;">${escapeHtml(formatSandboxReason(adapter.reason || sandbox.lastResolutionReason, sandbox, adapter, routeSummary, t))}</p></div>`;
}
