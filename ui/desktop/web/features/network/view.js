import {
  deleteConnectionTemplate,
  pingConnectionTemplate,
  saveConnectionTemplate,
  saveGlobalRouteSettings,
  saveNetworkSandboxGlobalSettings,
  testConnectionTemplateRequest
} from "./api.js";
import { askInputModal } from "../../core/modal.js";
import { wireNetworkImpl } from "./view-wire.js";
import { eyeIcon, eyeOffIcon, templateRow } from "./view-templates.js";
import {
  defaultTemplateDraft,
  ensureNodeDefaults,
  escapeHtml,
  networkDraftUtils,
  networkImportUtils,
  normalizeTemplateNodes,
  renderTemplateFrame
} from "./view-template-editor.js";
import { hydrateNetworkModel, refreshTemplatePings } from "./view-model.js";
import { renderSandboxFrame } from "./view-sandbox.js";

const {
  formatNetworkError,
  syncTemplateDraft,
  templateRequest,
  validateDraft
} = networkDraftUtils;

function listFrame(model, t) {
  const templates = model.networkTemplates ?? [];
  const globalRoute = model.networkGlobalRoute ?? {};
  const globalVpnEnabled = Boolean(globalRoute.globalVpnEnabled);
  const blockWithoutVpn = Boolean(globalRoute.blockWithoutVpn);
  const defaultTemplateId = globalRoute.defaultTemplateId ?? "";
  const defaultTemplate = templates.find((item) => item.id === defaultTemplateId);
  const defaultTemplateName = defaultTemplate ? defaultTemplate.name : t("network.defaultTemplateNone");
  return `
    <div class="panel network-list-frame">
      <h3>${t("network.chainTitle")}</h3>
      <div class="network-global-controls">
        <label class="checkbox-inline">
          <input id="network-block-without-vpn" type="checkbox" ${blockWithoutVpn ? "checked" : ""} />
          <span>${t("network.blockWithoutVpn")}</span>
        </label>
        <label class="checkbox-inline">
          <input id="network-global-vpn-enabled" type="checkbox" ${globalVpnEnabled ? "checked" : ""} />
          <span>${t("network.globalVpn")}</span>
        </label>
        <p class="meta">${t("network.defaultTemplateLabel")}: ${escapeHtml(defaultTemplateName)}</p>
      </div>
      ${globalVpnEnabled ? renderSandboxFrame(model, t, { scope: "global" }) : ""}
      <div class="network-table-shell">
        <table class="extensions-table">
          <thead>
            <tr>
              <th>${t("network.templateName")}</th>
              <th>${t("network.protocol")}</th>
              <th>${t("network.ping")}</th>
              <th>${t("extensions.actions")}</th>
            </tr>
          </thead>
          <tbody>
            ${templates.length
              ? templates.map((template) => templateRow(template, model, t, normalizeTemplateNodes)).join("")
              : `<tr><td colspan="4" class="meta">${t("network.templatesEmpty")}</td></tr>`}
          </tbody>
        </table>
      </div>
    </div>
  `;
}

export function renderNetwork(t, model) {
  const notice = model.networkNotice ? `<p class="notice ${model.networkNotice.type}">${model.networkNotice.text}</p>` : "";
  return `
    <div class="feature-page">
      <div class="feature-page-head">
        <h2>${t("nav.network")}</h2>
      </div>
      ${notice}
      <div class="grid-two network-layout-grid">
        ${listFrame(model, t)}
        ${renderTemplateFrame(model, t, eyeIcon, eyeOffIcon)}
      </div>
    </div>
  `;
}

export { hydrateNetworkModel };

export function wireNetwork(root, model, rerender, t) {
  return wireNetworkImpl(root, model, rerender, t, {
    askInputModal,
    closeFloatingTemplateMenusGuard: null,
    defaultTemplateDraft,
    deleteConnectionTemplate,
    ensureNodeDefaults,
    eyeIcon,
    eyeOffIcon,
    formatNetworkError,
    hydrateNetworkModel,
    networkImportUtils,
    normalizeTemplateNodes,
    pingConnectionTemplate,
    refreshTemplatePings,
    saveConnectionTemplate,
    saveGlobalRouteSettings,
    saveNetworkSandboxGlobalSettings,
    syncTemplateDraft,
    templateRequest,
    testConnectionTemplateRequest,
    validateDraft
  });
}
