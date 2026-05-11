import { registerNetworkImportAndTemplateHandlers } from "./view-wire-core-imports.js";

const PING_INTERVAL_MS = 30000;

export function wireNetworkImpl(root, model, rerender, t, deps) {
  const {
    refreshTemplatePings, saveGlobalRouteSettings, saveNetworkSandboxGlobalSettings, syncTemplateDraft, defaultTemplateDraft,
    ensureNodeDefaults, validateDraft, templateRequest, saveConnectionTemplate, formatNetworkError,
    hydrateNetworkModel, askInputModal, networkImportUtils, testConnectionTemplateRequest, eyeIcon, eyeOffIcon,
    pingConnectionTemplate, deleteConnectionTemplate, normalizeTemplateNodes, closeFloatingTemplateMenusGuard
  } = deps;

  document.querySelectorAll(".network-floating-menu").forEach((menu) => menu.remove());
  if (!model.networkPingPoller) {
    model.networkPingPoller = setInterval(async () => {
      const changed = await refreshTemplatePings(model, false).catch(() => false);
      if (changed) {
        await rerender();
      }
    }, PING_INTERVAL_MS);
  }
  refreshTemplatePings(model, false)
    .then(async (changed) => {
      if (changed) {
        await rerender();
      }
    })
    .catch(() => {});

  const saveGlobalSettings = async (next) => {
    const response = await saveGlobalRouteSettings(next);
    model.networkNotice = {
      type: response.ok ? "success" : "error",
      text: response.ok ? t("network.saved") : formatNetworkError(response.data.error, t)
    };
    if (response.ok) {
      model.networkGlobalRoute = {
        globalVpnEnabled: Boolean(next.globalVpnEnabled),
        blockWithoutVpn: Boolean(next.blockWithoutVpn),
        defaultTemplateId: next.defaultTemplateId || null
      };
      model.networkLoaded = false;
      await hydrateNetworkModel(model);
    }
    await rerender();
  };

  root.querySelector("#network-block-without-vpn")?.addEventListener("change", async (event) => {
    const checkbox = event.target;
    const current = model.networkGlobalRoute ?? {};
    await saveGlobalSettings({
      globalVpnEnabled: Boolean(current.globalVpnEnabled),
      blockWithoutVpn: Boolean(checkbox.checked),
      defaultTemplateId: current.defaultTemplateId || null
    });
  });

  root.querySelector("#network-global-vpn-enabled")?.addEventListener("change", async (event) => {
    const checkbox = event.target;
    const current = model.networkGlobalRoute ?? {};
    await saveGlobalSettings({
      globalVpnEnabled: Boolean(checkbox.checked),
      blockWithoutVpn: Boolean(current.blockWithoutVpn),
      defaultTemplateId: current.defaultTemplateId || null
    });
  });

  const saveGlobalSandboxSettings = async (enabled, defaultMode) => {
    const response = await saveNetworkSandboxGlobalSettings(enabled, defaultMode);
    model.networkNotice = {
      type: response.ok ? "success" : "error",
      text: response.ok ? t("network.sandbox.saved") : formatNetworkError(response.data.error, t)
    };
    if (response.ok) {
      model.networkLoaded = false;
      await hydrateNetworkModel(model);
    }
    await rerender();
  };

  root.querySelector("#network-global-sandbox-enabled")?.addEventListener("change", async (event) => {
    const checkbox = event.target;
    const currentMode = root.querySelector("#network-global-sandbox-mode")?.value || "isolated";
    await saveGlobalSandboxSettings(Boolean(checkbox.checked), currentMode);
  });

  root.querySelector("#network-global-sandbox-mode")?.addEventListener("change", async (event) => {
    const select = event.target;
    const enabled = Boolean(model.networkSandbox?.globalPolicyEnabled);
    await saveGlobalSandboxSettings(enabled, select.value);
  });

  root.querySelector("#network-add-node")?.addEventListener("click", async () => {
    syncTemplateDraft(root, model);
    const draft = model.networkTemplateDraft ?? defaultTemplateDraft();
    if (draft.nodes.length >= 3) {
      model.networkNotice = { type: "error", text: t("network.maxNodes") };
      await rerender();
      return;
    }
    draft.nodes.push(ensureNodeDefaults({ connectionType: "proxy", protocol: "socks5" }));
    model.networkTemplateDraft = draft;
    await rerender();
  });

  root.querySelector("#network-template-save")?.addEventListener("click", async () => {
    syncTemplateDraft(root, model);
    const draft = model.networkTemplateDraft ?? defaultTemplateDraft();
    const validationError = validateDraft(draft, t);
    if (validationError) {
      model.networkNotice = { type: "error", text: validationError };
      await rerender();
      return;
    }
    const result = await saveConnectionTemplate(templateRequest(draft));
    model.networkNotice = {
      type: result.ok ? "success" : "error",
      text: result.ok ? t("network.templateSaved") : formatNetworkError(result.data.error, t)
    };
    if (result.ok) {
      model.networkTemplateDraft = defaultTemplateDraft();
      model.networkNodeTestState = {};
      model.networkLoaded = false;
      await hydrateNetworkModel(model);
    }
    await rerender();
  });

  root.querySelector("#network-import-file")?.addEventListener("change", async (event) => {
    const input = event.target;
    const file = input.files?.[0];
    const nodeId = input.getAttribute("data-node-id");
    const importKind = input.getAttribute("data-import-kind");
    if (!file || !nodeId || !importKind) return;
    const raw = await file.text();
    const draft = model.networkTemplateDraft ?? defaultTemplateDraft();
    const current = draft.nodes.find((node) => node.nodeId === nodeId);
    if (!current) return;
    try {
      const nextNode = importKind === "wg"
        ? networkImportUtils.parseWireguardConfig(raw, current)
        : importKind === "ovpn"
          ? networkImportUtils.parseOpenVpnConfig(raw, current)
          : networkImportUtils.parseAmneziaInput(raw, current);
      networkImportUtils.applyImportedNode(model, nodeId, nextNode, file.name.replace(/\.[^.]+$/, ""));
      model.networkNotice = { type: "success", text: t("network.importApplied") };
    } catch (error) {
      model.networkNotice = { type: "error", text: formatNetworkError(error, t) };
    } finally {
      input.value = "";
      input.removeAttribute("data-node-id");
      input.removeAttribute("data-import-kind");
      await rerender();
    }
  });

  for (const control of root.querySelectorAll("[data-node-field='connectionType'], [data-node-field='protocol']")) {
    control.addEventListener("change", async () => {
      syncTemplateDraft(root, model);
      const nodeId = control.getAttribute("data-node-id");
      const draft = model.networkTemplateDraft ?? defaultTemplateDraft();
      draft.nodes = draft.nodes.map((node) => {
        if (node.nodeId !== nodeId) return node;
        const nextType = root.querySelector(`[data-node-id="${nodeId}"][data-node-field="connectionType"]`)?.value ?? node.connectionType;
        const nextProtocol = root.querySelector(`[data-node-id="${nodeId}"][data-node-field="protocol"]`)?.value ?? node.protocol;
        return ensureNodeDefaults({ ...node, connectionType: nextType, protocol: nextProtocol });
      });
      model.networkTemplateDraft = draft;
      await rerender();
    });
  }

  for (const button of root.querySelectorAll("[data-action='remove-node']")) {
    button.addEventListener("click", async () => {
      syncTemplateDraft(root, model);
      const nodeId = button.getAttribute("data-node-id");
      const draft = model.networkTemplateDraft ?? defaultTemplateDraft();
      draft.nodes = draft.nodes.filter((node) => node.nodeId !== nodeId);
      if (!draft.nodes.length) draft.nodes = [ensureNodeDefaults({ connectionType: "vpn", protocol: "wireguard" })];
      model.networkTemplateDraft = draft;
      delete model.networkNodeTestState?.[nodeId];
      await rerender();
    });
  }

  for (const button of root.querySelectorAll("[data-action='test-node']")) {
    button.addEventListener("click", async () => {
      syncTemplateDraft(root, model);
      const nodeId = button.getAttribute("data-node-id");
      const draft = model.networkTemplateDraft ?? defaultTemplateDraft();
      const node = draft.nodes.find((item) => item.nodeId === nodeId);
      if (!node) return;
      const ping = await testConnectionTemplateRequest(templateRequest({
        templateId: null,
        name: draft.name || "Node test",
        nodes: [node]
      }));
      model.networkNodeTestState = model.networkNodeTestState ?? {};
      if (ping.ok) {
        model.networkNodeTestState[nodeId] = ping.data;
        model.networkNotice = {
          type: ping.data.reachable ? "success" : "error",
          text: ping.data.message
        };
      } else {
        model.networkNotice = { type: "error", text: formatNetworkError(ping.data.error, t) };
      }
      await rerender();
    });
  }

  for (const button of root.querySelectorAll("[data-action='import-file']")) {
    button.addEventListener("click", () => {
      const fileInput = root.querySelector("#network-import-file");
      const kind = button.getAttribute("data-import-kind");
      const nodeId = button.getAttribute("data-node-id");
      if (!fileInput || !kind || !nodeId) return;
      fileInput.setAttribute("data-import-kind", kind);
      fileInput.setAttribute("data-node-id", nodeId);
      const accept = kind === "wg"
        ? ".conf,text/plain"
        : kind === "ovpn"
          ? ".ovpn,text/plain"
          : ".vpn,.conf,text/plain";
      fileInput.setAttribute("accept", accept);
      fileInput.click();
    });
  }

  registerNetworkImportAndTemplateHandlers({
    root, model, rerender, t,
    syncTemplateDraft, defaultTemplateDraft, askInputModal, networkImportUtils,
    formatNetworkError, templateRequest, testConnectionTemplateRequest,
    refreshTemplatePings, deleteConnectionTemplate, pingConnectionTemplate,
    normalizeTemplateNodes, closeFloatingTemplateMenusGuard, eyeIcon, eyeOffIcon
  });

}
