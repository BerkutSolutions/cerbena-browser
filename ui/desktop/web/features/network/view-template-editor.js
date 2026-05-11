import { createNetworkImportUtils } from "./view-import.js";
import { createNetworkDraftUtils } from "./view-draft.js";

const CONNECTION_TYPES = ["vpn", "proxy", "v2ray", "tor"];
const PROTOCOLS = {
  vpn: ["wireguard", "openvpn", "amnezia"],
  proxy: ["http", "socks4", "socks5"],
  v2ray: ["vmess", "vless", "trojan", "shadowsocks"],
  tor: ["obfs4", "snowflake", "meek", "none"]
};

const PORT_DEFAULTS = {
  wireguard: "51820",
  openvpn: "1194",
  amnezia: "",
  http: "8080",
  socks4: "1080",
  socks5: "1080",
  vmess: "443",
  vless: "443",
  trojan: "443",
  shadowsocks: "8388",
  obfs4: ""
};

export function escapeHtml(value) {
  return String(value ?? "")
    .replaceAll("&", "&amp;")
    .replaceAll("<", "&lt;")
    .replaceAll(">", "&gt;")
    .replaceAll("\"", "&quot;");
}

function makeNodeId() {
  return `node-${Date.now().toString(36)}-${Math.random().toString(36).slice(2, 8)}`;
}

function normalizeProtocol(protocol) {
  if ((protocol ?? "").toLowerCase() === "ss") return "shadowsocks";
  return (protocol ?? "").toLowerCase();
}

function normalizeOpenVpnTransport(value) {
  const normalized = String(value ?? "").trim().toLowerCase();
  if (normalized.startsWith("udp")) return "udp";
  if (normalized.startsWith("tcp")) return "tcp";
  return "";
}

function normalizeConnectionType(type) {
  const value = (type ?? "").toLowerCase();
  if (value === "xray") return "v2ray";
  return value;
}

export function ensureNodeDefaults(node) {
  const connectionType = normalizeConnectionType(node?.connectionType || "vpn");
  const supported = PROTOCOLS[connectionType] ?? PROTOCOLS.vpn;
  const protocol = supported.includes(normalizeProtocol(node?.protocol))
    ? normalizeProtocol(node.protocol)
    : supported[0];
  const settings = { ...(node?.settings ?? {}) };
  if (connectionType === "v2ray" && !settings.network) settings.network = "tcp";
  if (connectionType === "v2ray" && !settings.tls) settings.tls = "off";
  if (connectionType === "tor" && protocol !== "obfs4") {
    delete settings.bridges;
  }
  return {
    nodeId: node?.nodeId || makeNodeId(),
    connectionType,
    protocol,
    host: node?.host ?? "",
    port: node?.port ?? PORT_DEFAULTS[protocol] ?? "",
    username: node?.username ?? "",
    password: node?.password ?? "",
    bridges: node?.bridges ?? "",
    settings
  };
}

export function normalizeTemplateNodes(template) {
  if (Array.isArray(template?.nodes) && template.nodes.length) {
    return template.nodes.map((node) =>
      ensureNodeDefaults({
        nodeId: node.id ?? node.nodeId ?? makeNodeId(),
        connectionType: node.connectionType ?? node.connection_type ?? template.connectionType,
        protocol: node.protocol,
        host: node.host ?? "",
        port: node.port != null ? String(node.port) : "",
        username: node.username ?? "",
        password: node.password ?? "",
        bridges: node.bridges ?? "",
        settings: node.settings ?? {}
      })
    );
  }
  if (template?.connectionType || template?.connection_type) {
    return [ensureNodeDefaults({
      nodeId: makeNodeId(),
      connectionType: template.connectionType ?? template.connection_type,
      protocol: template.protocol,
      host: template.host ?? "",
      port: template.port != null ? String(template.port) : "",
      username: template.username ?? "",
      password: template.password ?? "",
      bridges: template.bridges ?? "",
      settings: {}
    })];
  }
  return [ensureNodeDefaults({})];
}

export function defaultTemplateDraft() {
  return {
    templateId: "",
    name: "",
    nodes: [ensureNodeDefaults({ connectionType: "vpn", protocol: "wireguard" })]
  };
}

function protocolOptions(type, selected) {
  return (PROTOCOLS[type] ?? [])
    .map((protocol) => `<option value="${protocol}" ${protocol === selected ? "selected" : ""}>${protocol}</option>`)
    .join("");
}

function settingsInput(nodeId, key, value, label, type = "text", attrs = "") {
  if (type === "textarea") {
    return `<label>${label}<textarea data-node-id="${nodeId}" data-node-setting="${key}" ${attrs}>${escapeHtml(value ?? "")}</textarea></label>`;
  }
  return `<label>${label}<input data-node-id="${nodeId}" data-node-setting="${key}" type="${type}" value="${escapeHtml(value ?? "")}" ${attrs}/></label>`;
}

function selectSetting(nodeId, key, value, label, options) {
  return `<label>${label}<select data-node-id="${nodeId}" data-node-setting="${key}">${options.map((option) => `<option value="${option.value}" ${option.value === value ? "selected" : ""}>${option.label}</option>`).join("")}</select></label>`;
}

function passwordField(nodeId, value, t, eyeIcon, eyeOffIcon) {
  return `<label>${t("network.password")}<span class="input-icon-field"><input data-node-id="${nodeId}" data-node-field="password" type="password" autocomplete="off" value="${escapeHtml(value ?? "")}" /><button type="button" class="input-icon-btn" data-action="toggle-password-visibility" data-password-visible="false" aria-label="${escapeHtml(t("profile.security.showPassword"))}" title="${escapeHtml(t("profile.security.showPassword"))}">${eyeOffIcon()}</button></span></label>`;
}

function commonEndpointFields(node, t, eyeIcon, eyeOffIcon, includeAuth = false) {
  return `<label>${t("network.host")}<input data-node-id="${node.nodeId}" data-node-field="host" value="${escapeHtml(node.host)}" /></label><label>${t("network.port")}<input data-node-id="${node.nodeId}" data-node-field="port" type="number" min="1" max="65535" value="${escapeHtml(node.port)}" /></label>${includeAuth ? `<label>${t("network.login")}<input data-node-id="${node.nodeId}" data-node-field="username" value="${escapeHtml(node.username)}" /></label>${passwordField(node.nodeId, node.password, t, eyeIcon, eyeOffIcon)}` : ""}`;
}

function wireguardFields(node, t) {
  return `
    ${commonEndpointFields(node, t)}
    ${settingsInput(node.nodeId, "publicKey", node.settings.publicKey, t("network.wgPublicKey"))}
    ${settingsInput(node.nodeId, "privateKey", node.settings.privateKey, t("network.wgPrivateKey"))}
    ${settingsInput(node.nodeId, "allowedIps", node.settings.allowedIps, t("network.wgAllowedIps"))}
    ${settingsInput(node.nodeId, "address", node.settings.address, t("network.wgAddress"))}
    ${settingsInput(node.nodeId, "dns", node.settings.dns, t("network.wgDns"))}
    ${settingsInput(node.nodeId, "persistentKeepalive", node.settings.persistentKeepalive, t("network.wgKeepalive"), "number", "min='0'")}
  `;
}

function openvpnFields(node, t, eyeIcon, eyeOffIcon) {
  return `
    ${commonEndpointFields(node, t, eyeIcon, eyeOffIcon, true)}
    <textarea data-node-id="${node.nodeId}" data-node-setting="ovpnRaw" class="hidden">${escapeHtml(node.settings.ovpnRaw ?? "")}</textarea>
    ${selectSetting(node.nodeId, "transport", node.settings.transport || "udp", t("network.ovpnTransport"), [
      { value: "udp", label: "udp" },
      { value: "tcp", label: "tcp" }
    ])}
    <details>
      <summary>${t("network.advanced")}</summary>
      <div class="grid-two" style="margin-top:8px;">
        ${settingsInput(node.nodeId, "caCert", node.settings.caCert, t("network.ovpnCa"), "textarea", "rows='4'")}
        ${settingsInput(node.nodeId, "clientCert", node.settings.clientCert, t("network.ovpnClientCert"), "textarea", "rows='4'")}
        ${settingsInput(node.nodeId, "clientKey", node.settings.clientKey, t("network.ovpnClientKey"), "textarea", "rows='4'")}
      </div>
    </details>
  `;
}

function amneziaFields(node, t) {
  return `
    <label class="grid-span-2">${t("network.amneziaKey")}
      <textarea data-node-id="${node.nodeId}" data-node-setting="amneziaKey" rows="4" placeholder="vpn://... or [Interface]/[Peer] .conf">${escapeHtml(node.settings.amneziaKey ?? "")}</textarea>
    </label>
  `;
}

function proxyFields(node, t, eyeIcon, eyeOffIcon) {
  return commonEndpointFields(node, t, eyeIcon, eyeOffIcon, true);
}

function vmessFields(node, t) {
  return `
    ${commonEndpointFields(node, t)}
    ${settingsInput(node.nodeId, "uuid", node.settings.uuid, t("network.v2rayUuid"))}
    ${settingsInput(node.nodeId, "alterId", node.settings.alterId, t("network.vmessAlterId"), "number", "min='0'")}
    ${selectSetting(node.nodeId, "security", node.settings.security || "auto", t("network.vmessSecurity"), [
      { value: "auto", label: "auto" },
      { value: "aes-128-gcm", label: "aes-128-gcm" },
      { value: "chacha20", label: "chacha20" }
    ])}
    ${selectSetting(node.nodeId, "network", node.settings.network || "tcp", t("network.v2rayNetwork"), [
      { value: "tcp", label: "tcp" },
      { value: "ws", label: "ws" },
      { value: "grpc", label: "grpc" }
    ])}
    ${node.settings.network === "ws" ? `
      ${settingsInput(node.nodeId, "wsPath", node.settings.wsPath, t("network.v2rayWsPath"))}
      ${settingsInput(node.nodeId, "wsHost", node.settings.wsHost, t("network.v2rayWsHost"))}
    ` : ""}
    ${selectSetting(node.nodeId, "tls", node.settings.tls || "off", t("network.v2rayTls"), [
      { value: "off", label: "off" },
      { value: "on", label: "on" }
    ])}
    ${settingsInput(node.nodeId, "sni", node.settings.sni, t("network.v2raySni"))}
  `;
}

function vlessFields(node, t) {
  const securityMode = (node.settings.securityMode || (node.settings.tls === "on" ? "tls" : "none")).toLowerCase();
  return `
    ${commonEndpointFields(node, t)}
    ${settingsInput(node.nodeId, "uuid", node.settings.uuid, t("network.v2rayUuid"))}
    ${settingsInput(node.nodeId, "flow", node.settings.flow, t("network.vlessFlow"))}
    ${selectSetting(node.nodeId, "network", node.settings.network || "tcp", t("network.v2rayNetwork"), [
      { value: "tcp", label: "tcp" },
      { value: "ws", label: "ws" },
      { value: "grpc", label: "grpc" }
    ])}
    ${selectSetting(node.nodeId, "tls", node.settings.tls || "off", t("network.v2rayTls"), [
      { value: "off", label: "off" },
      { value: "on", label: "on" }
    ])}
    ${settingsInput(node.nodeId, "sni", node.settings.sni, t("network.v2raySni"))}
    <input type="hidden" data-node-id="${node.nodeId}" data-node-setting="securityMode" value="${escapeHtml(securityMode)}" />
    <input type="hidden" data-node-id="${node.nodeId}" data-node-setting="realityPublicKey" value="${escapeHtml(node.settings.realityPublicKey || "")}" />
    <input type="hidden" data-node-id="${node.nodeId}" data-node-setting="realityShortId" value="${escapeHtml(node.settings.realityShortId || "")}" />
    <input type="hidden" data-node-id="${node.nodeId}" data-node-setting="realityFingerprint" value="${escapeHtml(node.settings.realityFingerprint || "")}" />
    <input type="hidden" data-node-id="${node.nodeId}" data-node-setting="realitySpiderX" value="${escapeHtml(node.settings.realitySpiderX || "")}" />
  `;
}

function trojanFields(node, t) {
  return `
    ${commonEndpointFields(node, t)}
    <label>${t("network.password")}
      <input data-node-id="${node.nodeId}" data-node-field="password" value="${escapeHtml(node.password)}" />
    </label>
    ${selectSetting(node.nodeId, "tls", node.settings.tls || "on", t("network.v2rayTls"), [
      { value: "on", label: "on" },
      { value: "off", label: "off" }
    ])}
    ${settingsInput(node.nodeId, "sni", node.settings.sni, t("network.v2raySni"))}
    ${settingsInput(node.nodeId, "alpn", node.settings.alpn, t("network.trojanAlpn"))}
    ${selectSetting(node.nodeId, "network", node.settings.network || "tcp", t("network.v2rayNetwork"), [
      { value: "tcp", label: "tcp" },
      { value: "ws", label: "ws" }
    ])}
  `;
}

function shadowsocksFields(node, t) {
  return `
    ${commonEndpointFields(node, t)}
    <label>${t("network.password")}
      <input data-node-id="${node.nodeId}" data-node-field="password" value="${escapeHtml(node.password)}" />
    </label>
    ${selectSetting(node.nodeId, "method", node.settings.method || "aes-256-gcm", t("network.shadowsocksMethod"), [
      { value: "aes-256-gcm", label: "aes-256-gcm" },
      { value: "aes-128-gcm", label: "aes-128-gcm" },
      { value: "chacha20-ietf-poly1305", label: "chacha20-ietf-poly1305" }
    ])}
  `;
}

function torFields(node, t) {
  if (node.protocol !== "obfs4") return `<p class="meta">${t("network.torSimpleMode")}</p>`;
  return `<label class="grid-span-2">${t("network.torBridges")}<textarea data-node-id="${node.nodeId}" data-node-field="bridges" rows="5" placeholder="obfs4 IP:PORT fingerprint cert=... iat-mode=0">${escapeHtml(node.bridges)}</textarea></label>`;
}

function nodeFields(node, t, eyeIcon, eyeOffIcon) {
  if (node.connectionType === "vpn") {
    if (node.protocol === "openvpn") return openvpnFields(node, t, eyeIcon, eyeOffIcon);
    if (node.protocol === "amnezia") return amneziaFields(node, t);
    return wireguardFields(node, t);
  }
  if (node.connectionType === "proxy") {
    return proxyFields(node, t, eyeIcon, eyeOffIcon);
  }
  if (node.connectionType === "v2ray") {
    if (node.protocol === "vmess") return vmessFields(node, t);
    if (node.protocol === "vless") return vlessFields(node, t);
    if (node.protocol === "trojan") return trojanFields(node, t);
    return shadowsocksFields(node, t);
  }
  return torFields(node, t);
}

function nodeImportActions(node, t) {
  if (node.connectionType === "vpn" && node.protocol === "wireguard") return `<button data-action="import-file" data-node-id="${node.nodeId}" data-import-kind="wg">${t("network.importWg")}</button>`;
  if (node.connectionType === "vpn" && node.protocol === "openvpn") return `<button data-action="import-file" data-node-id="${node.nodeId}" data-import-kind="ovpn">${t("network.importOvpn")}</button>`;
  if (node.connectionType === "vpn" && node.protocol === "amnezia") return `<button data-action="import-file" data-node-id="${node.nodeId}" data-import-kind="amnezia">${t("network.importAmnezia")}</button><button data-action="import-amnezia-key" data-node-id="${node.nodeId}">${t("network.importVpnKey")}</button>`;
  if (node.connectionType === "v2ray") return `<button data-action="import-link" data-node-id="${node.nodeId}">${t("network.importLink")}</button>`;
  return "";
}

function renderNode(node, index, model, t, eyeIcon, eyeOffIcon) {
  const ping = model.networkNodeTestState?.[node.nodeId] ?? null;
  const pingText = ping ? `${ping.reachable ? "OK" : "ERR"}${ping.latencyMs ? ` ${ping.latencyMs}ms` : ""}` : t("network.status.unknown");
  const canRemove = (model.networkTemplateDraft?.nodes?.length ?? 0) > 1;
  return `<div class="network-node" data-node-id="${node.nodeId}"><div class="network-node-head"><strong>${t("network.chainNode")} ${index + 1}</strong><div class="top-actions"><span class="badge ${ping?.reachable ? "success" : ping ? "error" : ""}">${escapeHtml(pingText)}</span>${nodeImportActions(node, t)}<button data-action="test-node" data-node-id="${node.nodeId}">${t("network.testConnection")}</button>${canRemove ? `<button data-action="remove-node" data-node-id="${node.nodeId}" class="danger">${t("extensions.remove")}</button>` : ""}</div></div><div class="grid-two"><label>${t("network.nodeType")}<select data-node-id="${node.nodeId}" data-node-field="connectionType">${CONNECTION_TYPES.map((type) => `<option value="${type}" ${type === node.connectionType ? "selected" : ""}>${t(`network.node.${type}`)}</option>`).join("")}</select></label><label>${t("network.protocol")}<select data-node-id="${node.nodeId}" data-node-field="protocol">${protocolOptions(node.connectionType, node.protocol)}</select></label>${nodeFields(node, t, eyeIcon, eyeOffIcon)}</div></div>`;
}

export function renderTemplateFrame(model, t, eyeIcon, eyeOffIcon) {
  const draft = model.networkTemplateDraft ?? defaultTemplateDraft();
  const canAddNode = draft.nodes.length < 3;
  return `<div class="panel network-template-frame"><h3>${t("network.templateFrame")}</h3><label>${t("network.templateName")}<input id="network-template-name" value="${escapeHtml(draft.name)}" /></label><div class="network-stack">${draft.nodes.map((node, index) => renderNode(node, index, model, t, eyeIcon, eyeOffIcon)).join("")}</div>${canAddNode ? `<button id="network-add-node" style="margin-top:10px;">${t("network.addNode")}</button>` : `<p class="meta" style="margin-top:10px;">${t("network.maxNodes")}</p>`}<input id="network-import-file" type="file" class="hidden" /><div class="top-actions" style="margin-top:12px;"><button id="network-template-save">${t("action.save")}</button></div></div>`;
}

export const networkImportUtils = createNetworkImportUtils({
  ensureNodeDefaults,
  defaultTemplateDraft,
  normalizeOpenVpnTransport
});

export const networkDraftUtils = createNetworkDraftUtils({
  ensureNodeDefaults,
  defaultTemplateDraft,
  normalizeConnectionType,
  normalizeProtocol,
  normalizeOpenVpnTransport,
  PROTOCOLS
});
