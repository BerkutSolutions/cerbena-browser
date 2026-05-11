import { getNetworkState, pingConnectionTemplate } from "./api.js";
import { defaultTemplateDraft, normalizeTemplateNodes } from "./view-template-editor.js";

const PING_INTERVAL_MS = 30000;

export async function refreshTemplatePings(model, force = false) {
  if (!Array.isArray(model.networkTemplates) || !model.networkTemplates.length) return;
  if (model.networkPingInFlight) return false;
  const now = Date.now();
  if (!force && model.networkLastPingAt && now - model.networkLastPingAt < PING_INTERVAL_MS) return false;
  model.networkPingInFlight = true;
  const updates = {};
  try {
    for (const template of model.networkTemplates) {
      try {
        const result = await pingConnectionTemplate(template.id);
        if (result.ok) updates[template.id] = result.data;
      } catch {}
    }
  } finally {
    model.networkPingInFlight = false;
  }
  model.networkPingState = { ...(model.networkPingState ?? {}), ...updates };
  model.networkLastPingAt = Date.now();
  return Object.keys(updates).length > 0;
}

export async function hydrateNetworkModel(model) {
  if (model.networkLoaded && model.networkTemplates) return;
  const result = await getNetworkState("");
  const state = result.ok ? JSON.parse(result.data) : {
    payload: null,
    selectedTemplateId: null,
    connectionTemplates: [],
    globalRoute: { globalVpnEnabled: false, blockWithoutVpn: true, defaultTemplateId: null }
  };
  model.networkTemplates = (state.connectionTemplates ?? []).map((template) => ({
    ...template,
    nodes: normalizeTemplateNodes(template).map((node) => ({
      id: node.nodeId,
      connectionType: node.connectionType,
      protocol: node.protocol,
      host: node.host || null,
      port: Number(node.port || 0) || null,
      username: node.username || null,
      password: node.password || null,
      bridges: node.bridges || null,
      settings: node.settings ?? {}
    }))
  }));
  model.networkTemplateDraft = model.networkTemplateDraft ?? defaultTemplateDraft();
  model.networkGlobalRoute = state.globalRoute ?? {
    globalVpnEnabled: false,
    blockWithoutVpn: true,
    defaultTemplateId: null
  };
  model.networkPolicyPayload = state.payload ?? null;
  model.networkSelectedTemplateId = state.selectedTemplateId ?? null;
  model.networkSandbox = state.sandbox ?? null;
  model.networkPingState = model.networkPingState ?? {};
  model.networkNodeTestState = model.networkNodeTestState ?? {};
  model.networkLoaded = true;
}
