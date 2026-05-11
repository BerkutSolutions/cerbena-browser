export function createNetworkDraftUtils(deps) {
  const { ensureNodeDefaults, defaultTemplateDraft, normalizeConnectionType, normalizeProtocol, normalizeOpenVpnTransport, PROTOCOLS } = deps;
  function collectNodeFromDom(root, node) {
  const nodeId = node.nodeId;
  const connectionType = root.querySelector(`[data-node-id="${nodeId}"][data-node-field="connectionType"]`)?.value ?? node.connectionType;
  const protocolRaw = root.querySelector(`[data-node-id="${nodeId}"][data-node-field="protocol"]`)?.value ?? node.protocol;
  const protocol = normalizeProtocol(protocolRaw);
  const next = ensureNodeDefaults({
    ...node,
    connectionType,
    protocol,
    host: root.querySelector(`[data-node-id="${nodeId}"][data-node-field="host"]`)?.value ?? "",
    port: root.querySelector(`[data-node-id="${nodeId}"][data-node-field="port"]`)?.value ?? "",
    username: root.querySelector(`[data-node-id="${nodeId}"][data-node-field="username"]`)?.value ?? "",
    password: root.querySelector(`[data-node-id="${nodeId}"][data-node-field="password"]`)?.value ?? "",
    bridges: root.querySelector(`[data-node-id="${nodeId}"][data-node-field="bridges"]`)?.value ?? ""
  });
  const settings = {};
  for (const field of root.querySelectorAll(`[data-node-id="${nodeId}"][data-node-setting]`)) {
    const key = field.getAttribute("data-node-setting");
    if (!key) continue;
    settings[key] = field.value ?? "";
  }
  next.settings = settings;
  return next;
}

  function syncTemplateDraft(root, model) {
  const current = model.networkTemplateDraft ?? defaultTemplateDraft();
  model.networkTemplateDraft = {
    ...current,
    name: root.querySelector("#network-template-name")?.value ?? current.name,
    nodes: (current.nodes ?? []).map((node) => collectNodeFromDom(root, node))
  };
}

  function looksLikeHost(value) {
  if (!value || /\s/.test(value)) return false;
  return value.length >= 2;
}

  function looksLikeUuid(value) {
  return /^[0-9a-f]{8}-[0-9a-f]{4}-[1-5][0-9a-f]{3}-[89ab][0-9a-f]{3}-[0-9a-f]{12}$/i.test(value ?? "");
}

  function formatNetworkError(error, t) {
  const raw = String(error?.message ?? error ?? "").trim();
  const normalized = raw.toLowerCase();
  if (!raw) return t("network.error.importFailed");
  if (normalized.includes("empty amnezia input") || normalized.includes("empty amnezia config")) {
    return t("network.error.amneziaInputEmpty");
  }
  if (normalized.includes("amnezia config does not contain endpoint")) {
    return t("network.error.amneziaEndpointMissing");
  }
  if (normalized.includes("amnezia input must be vpn:// key or awg .conf")) {
    return t("network.error.amneziaKeyInvalid");
  }
  if (normalized.includes("vless reality requires pbk/public key")) {
    return t("network.error.vlessRealityPbkRequired");
  }
  if (raw.startsWith("Error:")) {
    return raw.slice("Error:".length).trim() || t("network.error.importFailed");
  }
  return raw;
}

  function validateDraft(draft, t) {
  if (!draft.name.trim()) return t("network.error.templateNameRequired");
  if (!Array.isArray(draft.nodes) || !draft.nodes.length) return t("network.error.nodesRequired");
  if (draft.nodes.length > 3) return t("network.maxNodes");
  for (const node of draft.nodes) {
    const type = normalizeConnectionType(node.connectionType);
    const protocol = normalizeProtocol(node.protocol);
    if (!(PROTOCOLS[type] ?? []).includes(protocol)) {
      return t("network.error.protocolUnsupported");
    }
    if (type === "vpn") {
      if (protocol === "amnezia") {
        const key = String(node.settings?.amneziaKey ?? "").trim();
        const firstLine = key
          .split(/\r?\n/)
          .map((line) => line.trim())
          .find((line) => line.length > 0) ?? "";
        const looksLikeConf = (key.includes("[Interface]") || key.includes("[interface]"))
          && (key.includes("[Peer]") || key.includes("[peer]"));
        if (!firstLine.toLowerCase().startsWith("vpn://") && !looksLikeConf) {
          return t("network.error.amneziaKeyInvalid");
        }
      } else {
        if (!looksLikeHost(node.host)) return t("network.error.hostInvalid");
        const port = Number(node.port ?? 0);
        if (!Number.isInteger(port) || port <= 0 || port > 65535) return t("network.error.portInvalid");
      }
    }
    if (type === "proxy" || type === "v2ray") {
      if (!looksLikeHost(node.host)) return t("network.error.hostInvalid");
      const port = Number(node.port ?? 0);
      if (!Number.isInteger(port) || port <= 0 || port > 65535) return t("network.error.portInvalid");
    }
    if (type === "tor" && protocol === "obfs4" && !(node.bridges ?? "").trim()) {
      return t("network.error.torBridgesRequired");
    }
    if (type === "v2ray" && (protocol === "vmess" || protocol === "vless")) {
      if (!looksLikeUuid(node.settings.uuid ?? "")) {
        return t("network.error.uuidInvalid");
      }
    }
    if (type === "v2ray" && (protocol === "trojan" || protocol === "shadowsocks")) {
      if (!(node.password ?? "").trim()) return t("network.error.passwordRequired");
    }
  }
  return "";
}

  function templateRequest(draft) {
  return {
    templateId: draft.templateId || null,
    name: draft.name.trim(),
    nodes: draft.nodes.map((node) => {
      const settings = {};
      for (const [key, value] of Object.entries(node.settings ?? {})) {
        const normalized = String(value ?? "").trim();
        if (!normalized && normalized !== "0") continue;
        settings[key] = normalized;
      }
      return {
        nodeId: node.nodeId,
        connectionType: normalizeConnectionType(node.connectionType),
        protocol: normalizeProtocol(node.protocol),
        host: node.host?.trim() || null,
        port: Number(node.port || 0) || null,
        username: node.username?.trim() || null,
        password: node.password?.trim() || null,
        bridges: node.bridges?.trim() || null,
        settings
      };
    }),
    connectionType: normalizeConnectionType(draft.nodes[0]?.connectionType || "vpn"),
    protocol: normalizeProtocol(draft.nodes[0]?.protocol || "wireguard"),
    host: draft.nodes[0]?.host?.trim() || null,
    port: Number(draft.nodes[0]?.port || 0) || null,
    username: draft.nodes[0]?.username?.trim() || null,
    password: draft.nodes[0]?.password?.trim() || null,
    bridges: draft.nodes[0]?.bridges?.trim() || null
  };
}


  return { collectNodeFromDom, syncTemplateDraft, looksLikeHost, looksLikeUuid, formatNetworkError, validateDraft, templateRequest };
}
