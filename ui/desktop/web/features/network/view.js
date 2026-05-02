import {
  deleteConnectionTemplate,
  getNetworkState,
  pingConnectionTemplate,
  saveNetworkSandboxGlobalSettings,
  saveGlobalRouteSettings,
  saveConnectionTemplate,
  testConnectionTemplateRequest
} from "./api.js";
import { askInputModal } from "../../core/modal.js";

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
const PING_INTERVAL_MS = 30000;

function escapeHtml(value) {
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

function normalizeConnectionType(type) {
  const value = (type ?? "").toLowerCase();
  if (value === "xray") return "v2ray";
  return value;
}

function ensureNodeDefaults(node) {
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
  const next = {
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
  return next;
}

function normalizeTemplateNodes(template) {
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
    return [
      ensureNodeDefaults({
        nodeId: makeNodeId(),
        connectionType: template.connectionType ?? template.connection_type,
        protocol: template.protocol,
        host: template.host ?? "",
        port: template.port != null ? String(template.port) : "",
        username: template.username ?? "",
        password: template.password ?? "",
        bridges: template.bridges ?? "",
        settings: {}
      })
    ];
  }
  return [ensureNodeDefaults({})];
}

function defaultTemplateDraft() {
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

function templateStatus(model, template, t) {
  const ping = model.networkPingState?.[template.id];
  if (!ping) return `<span class="badge">${t("network.status.unknown")}</span>`;
  const className = ping.reachable ? "success" : "error";
  const label = ping.reachable
    ? `${escapeHtml(String(ping.latencyMs ?? "-"))} ms`
    : t("network.status.unavailable");
  return `<span class="badge ${className}" title="${escapeHtml(ping.message ?? "")}">${label}</span>`;
}

function templateChainLabel(template, t) {
  const nodes = normalizeTemplateNodes(template);
  return nodes
    .map((node) => `${t(`network.node.${node.connectionType}`)}:${node.protocol}`)
    .join(" -> ");
}

function templateRow(template, model, t) {
  return `
    <tr data-template-id="${template.id}">
      <td>${escapeHtml(template.name)}</td>
      <td>${escapeHtml(templateChainLabel(template, t))}</td>
      <td>${templateStatus(model, template, t)}</td>
      <td class="actions network-table-actions-cell">
        <div class="dns-dropdown network-actions-dropdown">
          <button type="button" class="dns-dropdown-toggle network-actions-toggle" data-template-menu-toggle="${template.id}">...</button>
        </div>
      </td>
    </tr>
  `;
}

function settingsInput(nodeId, key, value, label, type = "text", attrs = "") {
  if (type === "textarea") {
    return `
      <label>${label}
        <textarea data-node-id="${nodeId}" data-node-setting="${key}" ${attrs}>${escapeHtml(value ?? "")}</textarea>
      </label>
    `;
  }
  return `
    <label>${label}
      <input data-node-id="${nodeId}" data-node-setting="${key}" type="${type}" value="${escapeHtml(value ?? "")}" ${attrs}/>
    </label>
  `;
}

function selectSetting(nodeId, key, value, label, options) {
  return `
    <label>${label}
      <select data-node-id="${nodeId}" data-node-setting="${key}">
        ${options.map((option) => `<option value="${option.value}" ${option.value === value ? "selected" : ""}>${option.label}</option>`).join("")}
      </select>
    </label>
  `;
}

function commonEndpointFields(node, t, includeAuth = false) {
  return `
    <label>${t("network.host")}
      <input data-node-id="${node.nodeId}" data-node-field="host" value="${escapeHtml(node.host)}" />
    </label>
    <label>${t("network.port")}
      <input data-node-id="${node.nodeId}" data-node-field="port" type="number" min="1" max="65535" value="${escapeHtml(node.port)}" />
    </label>
    ${includeAuth ? `
      <label>${t("network.login")}
        <input data-node-id="${node.nodeId}" data-node-field="username" value="${escapeHtml(node.username)}" />
      </label>
      <label>${t("network.password")}
        <input data-node-id="${node.nodeId}" data-node-field="password" value="${escapeHtml(node.password)}" />
      </label>
    ` : ""}
  `;
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

function openvpnFields(node, t) {
  return `
    ${commonEndpointFields(node, t, true)}
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

function proxyFields(node, t) {
  return commonEndpointFields(node, t, true);
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
  if (node.protocol !== "obfs4") {
    return `<p class="meta">${t("network.torSimpleMode")}</p>`;
  }
  return `
    <label class="grid-span-2">${t("network.torBridges")}
      <textarea data-node-id="${node.nodeId}" data-node-field="bridges" rows="5" placeholder="obfs4 IP:PORT fingerprint cert=... iat-mode=0">${escapeHtml(node.bridges)}</textarea>
    </label>
  `;
}

function nodeFields(node, t) {
  if (node.connectionType === "vpn") {
    if (node.protocol === "openvpn") return openvpnFields(node, t);
    if (node.protocol === "amnezia") return amneziaFields(node, t);
    return wireguardFields(node, t);
  }
  if (node.connectionType === "proxy") {
    return proxyFields(node, t);
  }
  if (node.connectionType === "v2ray") {
    if (node.protocol === "vmess") return vmessFields(node, t);
    if (node.protocol === "vless") return vlessFields(node, t);
    if (node.protocol === "trojan") return trojanFields(node, t);
    return shadowsocksFields(node, t);
  }
  return torFields(node, t);
}

function nodeTestStatus(model, node) {
  return model.networkNodeTestState?.[node.nodeId] ?? null;
}

function nodeImportActions(node, t) {
  if (node.connectionType === "vpn" && node.protocol === "wireguard") {
    return `<button data-action="import-file" data-node-id="${node.nodeId}" data-import-kind="wg">${t("network.importWg")}</button>`;
  }
  if (node.connectionType === "vpn" && node.protocol === "openvpn") {
    return `<button data-action="import-file" data-node-id="${node.nodeId}" data-import-kind="ovpn">${t("network.importOvpn")}</button>`;
  }
  if (node.connectionType === "vpn" && node.protocol === "amnezia") {
    return `
      <button data-action="import-file" data-node-id="${node.nodeId}" data-import-kind="amnezia">${t("network.importAmnezia")}</button>
      <button data-action="import-amnezia-key" data-node-id="${node.nodeId}">${t("network.importVpnKey")}</button>
    `;
  }
  if (node.connectionType === "v2ray") {
    return `<button data-action="import-link" data-node-id="${node.nodeId}">${t("network.importLink")}</button>`;
  }
  return "";
}

function renderNode(node, index, model, t) {
  const ping = nodeTestStatus(model, node);
  const pingText = ping ? `${ping.reachable ? "OK" : "ERR"}${ping.latencyMs ? ` ${ping.latencyMs}ms` : ""}` : t("network.status.unknown");
  const canRemove = (model.networkTemplateDraft?.nodes?.length ?? 0) > 1;
  return `
    <div class="network-node" data-node-id="${node.nodeId}">
      <div class="network-node-head">
        <strong>${t("network.chainNode")} ${index + 1}</strong>
        <div class="top-actions">
          <span class="badge ${ping?.reachable ? "success" : ping ? "error" : ""}">${escapeHtml(pingText)}</span>
          ${nodeImportActions(node, t)}
          <button data-action="test-node" data-node-id="${node.nodeId}">${t("network.testConnection")}</button>
          ${canRemove ? `<button data-action="remove-node" data-node-id="${node.nodeId}" class="danger">${t("extensions.remove")}</button>` : ""}
        </div>
      </div>
      <div class="grid-two">
        <label>${t("network.nodeType")}
          <select data-node-id="${node.nodeId}" data-node-field="connectionType">
            ${CONNECTION_TYPES.map((type) => `<option value="${type}" ${type === node.connectionType ? "selected" : ""}>${t(`network.node.${type}`)}</option>`).join("")}
          </select>
        </label>
        <label>${t("network.protocol")}
          <select data-node-id="${node.nodeId}" data-node-field="protocol">
            ${protocolOptions(node.connectionType, node.protocol)}
          </select>
        </label>
        ${nodeFields(node, t)}
      </div>
    </div>
  `;
}

function templateFrame(model, t) {
  const draft = model.networkTemplateDraft ?? defaultTemplateDraft();
  const canAddNode = draft.nodes.length < 3;
  return `
    <div class="panel network-template-frame">
      <h3>${t("network.templateFrame")}</h3>
      <label>${t("network.templateName")}
        <input id="network-template-name" value="${escapeHtml(draft.name)}" />
      </label>
      <div class="network-stack">
        ${draft.nodes.map((node, index) => renderNode(node, index, model, t)).join("")}
      </div>
      ${canAddNode ? `<button id="network-add-node" style="margin-top:10px;">${t("network.addNode")}</button>` : `<p class="meta" style="margin-top:10px;">${t("network.maxNodes")}</p>`}
      <input id="network-import-file" type="file" class="hidden" />
      <div class="top-actions" style="margin-top:12px;">
        <button id="network-template-save">${t("action.save")}</button>
      </div>
    </div>
  `;
}

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
      ${globalVpnEnabled ? sandboxFrame(model, t, { scope: "global" }) : ""}
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
            ${templates.length ? templates.map((template) => templateRow(template, model, t)).join("") : `<tr><td colspan="4" class="meta">${t("network.templatesEmpty")}</td></tr>`}
          </tbody>
        </table>
      </div>
    </div>
  `;
}

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
  const template = selectedTemplateId
    ? templates.find((item) => item.id === selectedTemplateId)
    : null;
  if (template) {
    return `${template.name} (${templateChainLabel(template, t)})`;
  }
  if (scope === "global") {
    return t("network.sandbox.routeUnknown");
  }
  if ((payload?.routeMode ?? "").toLowerCase() === "direct") {
    return t("network.sandbox.routeDirect");
  }
  return t("network.sandbox.routeUnknown");
}

function formatSandboxReason(reason, sandbox, adapter, activeRoute, t) {
  const value = String(reason || "").trim();
  if (!value) {
    return t("network.sandbox.unknown");
  }
  const exactMap = {
    "Template is compatible with isolated userspace runtime": "network.sandbox.reason.userspaceCompatible",
    "Profile is pinned to compatibility-native mode": "network.sandbox.reason.compatibilityPinned",
    "Legacy profile was auto-adapted to compatibility-native mode": "network.sandbox.reason.legacyMigrated",
    "Global sandbox policy allows compatibility-native fallback": "network.sandbox.reason.globalCompatibilityFallback",
    "This Amnezia profile requires a machine-wide compatibility backend; isolated mode forbids that path": "network.sandbox.reason.isolatedBlockedByNative",
    "Container sandbox mode is selected; launcher will validate the host runtime and per-profile sandbox capacity during launch": "network.sandbox.reason.containerSelected",
    "Docker Desktop container runtime is available and can build a profile-scoped isolated route helper on first launch": "network.sandbox.reason.containerReady",
    "Selected route is not compatible with container isolation yet": "network.sandbox.reason.containerUnsupported"
  };
  if (exactMap[value]) {
    const localized = t(exactMap[value]);
    return localized
      .replace("{route}", activeRoute)
      .replace("{mode}", sandboxModeLabel(sandbox?.requestedMode || "auto", t));
  }
  const capacityMatch = value.match(/Container sandbox capacity is exhausted \((\d+)\/(\d+) active\)/i);
  if (capacityMatch) {
    return t("network.sandbox.reason.containerCapacity")
      .replace("{active}", capacityMatch[1])
      .replace("{max}", capacityMatch[2]);
  }
  if (value.startsWith("container runtime probe failed:")) {
    return t("network.sandbox.reason.containerProbeFailed");
  }
  if (value.startsWith("docker runtime is not installed or not reachable:")) {
    return t("network.sandbox.reason.containerRuntimeMissing");
  }
  if (value === "No resolved strategy yet") {
    return t("network.sandbox.unknown");
  }
  if (sandbox?.effectiveMode === "container" && !adapter?.available) {
    return t("network.sandbox.reason.containerProbeFailed");
  }
  return value;
}

function sandboxFrame(model, t, options = {}) {
  const scope = options.scope || "profile";
  const sandbox = model.networkSandbox ?? null;
  const isGlobal = scope === "global";
  if (!isGlobal && !model.selectedProfileId) {
    return `
      <div class="panel" style="margin-bottom:12px;">
        <h4>${t("network.sandbox.title")}</h4>
        <p class="meta">${t("network.sandbox.profileRequired")}</p>
      </div>
    `;
  }
  if (!sandbox) {
    return `
      <div class="panel" style="margin-bottom:12px;">
        <h4>${t("network.sandbox.title")}</h4>
        <p class="meta">${t("network.sandbox.loading")}</p>
      </div>
    `;
  }
  const adapter = sandbox.adapter ?? {
    adapterKind: "unknown",
    runtimeKind: "unknown",
    available: true,
    requiresSystemNetworkAccess: false,
    maxHelperProcesses: 0,
    estimatedMemoryMb: 0,
    activeSandboxes: 0,
    maxActiveSandboxes: 0,
    supportsNativeIsolation: false,
    reason: t("network.sandbox.unknown")
  };
  const routeSummary = activeRouteSummary(model, t, isGlobal ? "global" : "profile");
  const resolutionBadge = adapter.available
    ? sandboxBadge("success", t("network.sandbox.available"))
    : sandboxBadge("error", t("network.sandbox.unavailable"));
  const nativeWarning = sandbox.effectiveMode === "blocked"
    ? ""
    : adapter.requiresSystemNetworkAccess
    ? `<p class="notice error">${t("network.sandbox.nativeWarning")}</p>`
    : sandbox.requiresNativeBackend && sandbox.effectiveMode === "container"
      ? `<p class="notice success">${t("network.sandbox.containerNativeIsolated")}</p>`
      : `<p class="meta">${t("network.sandbox.isolatedHint")}</p>`;
  const selectedMode = isGlobal
    ? (sandbox.globalPolicyEnabled
      ? (["isolated", "compatibility-native", "container"].includes(sandbox.requestedMode) ? sandbox.requestedMode : "isolated")
      : "isolated")
    : (sandbox.preferredMode ?? "auto");
  const modeOptions = ["isolated", "compatibility-native", "container"]
    .map((mode) => `<option value="${mode}" ${mode === selectedMode ? "selected" : ""}>${sandboxModeLabel(mode, t)}</option>`)
    .join("");
  return `
    <div class="panel" style="margin-top:12px; margin-bottom:12px;">
      <div class="top-actions" style="align-items:flex-start; justify-content:space-between; gap:12px;">
        <div>
          <h4 style="margin:0 0 6px 0;">${isGlobal ? t("network.sandbox.globalTitle") : t("network.sandbox.title")}</h4>
          ${isGlobal ? "" : `<p class="meta" style="margin:0;">${t("network.sandbox.subtitle")}</p>`}
        </div>
        ${resolutionBadge}
      </div>
      <div class="grid-two" style="margin-top:12px;">
        <div>
          <strong>${t("network.sandbox.effectiveMode")}</strong>
          <p>${escapeHtml(sandboxModeLabel(sandbox.effectiveMode, t))}</p>
        </div>
        <div>
          <strong>${t("network.sandbox.activeRoute")}</strong>
          <p>${escapeHtml(routeSummary)}</p>
        </div>
        <div>
          <strong>${t("network.sandbox.adapterLabel")}</strong>
          <p>${escapeHtml(sandboxAdapterLabel(adapter.adapterKind, t))}</p>
        </div>
        <div>
          <strong>${t("network.sandbox.runtimeLabel")}</strong>
          <p>${escapeHtml(adapter.runtimeKind || "unknown")}</p>
        </div>
        <div>
          <strong>${t("network.sandbox.requestedMode")}</strong>
          <p>${escapeHtml(sandboxModeLabel(sandbox.requestedMode, t))}</p>
        </div>
        <div>
          <strong>${t("network.sandbox.resourceBudget")}</strong>
          <p>${t("network.sandbox.resourceBudgetValue")
            .replace("{helpers}", String(adapter.maxHelperProcesses ?? 0))
            .replace("{memory}", String(adapter.estimatedMemoryMb ?? 0))}</p>
        </div>
        <div>
          <strong>${t("network.sandbox.slotUsage")}</strong>
          <p>${escapeHtml(`${adapter.activeSandboxes ?? 0}/${adapter.maxActiveSandboxes ?? 0}`)}</p>
        </div>
      </div>
      ${isGlobal ? `
        <label class="checkbox-inline" style="margin-top:12px;">
          <input id="network-global-sandbox-enabled" type="checkbox" ${sandbox.globalPolicyEnabled ? "checked" : ""} />
          <span>${t("network.sandbox.globalEnable")}</span>
        </label>
      ` : ""}
      <label style="margin-top:12px;">${isGlobal ? t("network.sandbox.globalChooseMode") : t("network.sandbox.chooseMode")}
        <select id="${isGlobal ? "network-global-sandbox-mode" : "network-sandbox-mode"}" ${isGlobal && !sandbox.globalPolicyEnabled ? "disabled" : ""}>
          ${modeOptions}
        </select>
      </label>
      <p class="meta" style="margin-top:8px;">${escapeHtml(formatSandboxReason(adapter.reason || sandbox.lastResolutionReason, sandbox, adapter, routeSummary, t))}</p>
      ${nativeWarning}
      ${adapter.requiresSystemNetworkAccess ? `<p class="notice error">${t("network.sandbox.systemWideWarning")}</p>` : ""}
      ${sandbox.effectiveMode === "container" ? `<p class="notice">${t("network.sandbox.containerMvp")}</p>` : ""}
      ${sandbox.effectiveMode === "blocked" ? `<p class="notice error">${t("network.sandbox.blockedHint").replace("{route}", routeSummary)}</p>` : ""}
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
        ${templateFrame(model, t)}
      </div>
    </div>
  `;
}

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

async function refreshTemplatePings(model, force = false) {
  if (!Array.isArray(model.networkTemplates) || !model.networkTemplates.length) return;
  if (model.networkPingInFlight) return false;
  const now = Date.now();
  if (!force && model.networkLastPingAt && now - model.networkLastPingAt < PING_INTERVAL_MS) {
    return false;
  }
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

function decodeBase64(payload) {
  const normalized = String(payload ?? "")
    .replace(/-/g, "+")
    .replace(/_/g, "/");
  const padded = normalized.padEnd(Math.ceil(normalized.length / 4) * 4, "=");
  return atob(padded);
}

function parseHostPort(endpoint) {
  const trimmed = String(endpoint ?? "").trim();
  if (!trimmed) return { host: "", port: "" };
  const idx = trimmed.lastIndexOf(":");
  if (idx <= 0) return { host: trimmed, port: "" };
  return { host: trimmed.slice(0, idx), port: trimmed.slice(idx + 1) };
}

function parseIniSections(text) {
  const sections = {};
  let section = "";
  for (const rawLine of String(text ?? "").split(/\r?\n/)) {
    const line = rawLine.trim();
    if (!line || line.startsWith("#") || line.startsWith(";")) continue;
    if (line.startsWith("[") && line.endsWith("]")) {
      section = line.slice(1, -1).trim().toLowerCase();
      continue;
    }
    if (!section) continue;
    const splitAt = line.indexOf("=");
    if (splitAt <= 0) continue;
    const key = line.slice(0, splitAt).trim().toLowerCase();
    const value = line.slice(splitAt + 1).trim();
    if (!sections[section]) sections[section] = {};
    sections[section][key] = value;
  }
  return sections;
}

function parseWireguardConfig(text, node) {
  const lines = String(text ?? "").split(/\r?\n/);
  let section = "";
  const next = ensureNodeDefaults({ ...node, connectionType: "vpn", protocol: "wireguard" });
  for (const rawLine of lines) {
    const line = rawLine.trim();
    if (!line || line.startsWith("#") || line.startsWith(";")) continue;
    if (line.startsWith("[") && line.endsWith("]")) {
      section = line.slice(1, -1).trim().toLowerCase();
      continue;
    }
    const splitAt = line.indexOf("=");
    if (splitAt <= 0) continue;
    const key = line.slice(0, splitAt).trim().toLowerCase();
    const value = line.slice(splitAt + 1).trim();
    if (section === "interface") {
      if (key === "privatekey") next.settings.privateKey = value;
      if (key === "address") next.settings.address = value;
      if (key === "dns") next.settings.dns = value;
    }
    if (section === "peer") {
      if (key === "publickey") next.settings.publicKey = value;
      if (key === "allowedips") next.settings.allowedIps = value;
      if (key === "persistentkeepalive") next.settings.persistentKeepalive = value;
      if (key === "endpoint") {
        const endpoint = parseHostPort(value);
        next.host = endpoint.host;
        next.port = endpoint.port;
      }
    }
  }
  return next;
}

function parseOpenVpnConfig(text, node) {
  const content = String(text ?? "");
  const next = ensureNodeDefaults({ ...node, connectionType: "vpn", protocol: "openvpn" });
  next.settings.ovpnRaw = content;
  const remoteMatch = content.match(/^\s*remote\s+([^\s]+)(?:\s+(\d+))?/im);
  if (remoteMatch) {
    next.host = remoteMatch[1] ?? next.host;
    next.port = remoteMatch[2] ?? next.port;
  }
  const protoMatch = content.match(/^\s*proto\s+([^\s]+)/im);
  if (protoMatch) {
    const value = protoMatch[1].toLowerCase();
    if (value === "udp" || value === "tcp") {
      next.settings.transport = value;
    }
  }
  const caMatch = content.match(/<ca>([\s\S]*?)<\/ca>/im);
  if (caMatch) next.settings.caCert = caMatch[1].trim();
  const certMatch = content.match(/<cert>([\s\S]*?)<\/cert>/im);
  if (certMatch) next.settings.clientCert = certMatch[1].trim();
  const keyMatch = content.match(/<key>([\s\S]*?)<\/key>/im);
  if (keyMatch) next.settings.clientKey = keyMatch[1].trim();
  return next;
}

function parseAmneziaConfig(rawConfig, node) {
  const content = String(rawConfig ?? "").replace(/\r/g, "").trim();
  if (!content) {
    throw new Error("empty amnezia config");
  }
  const sections = parseIniSections(content);
  const iface = sections.interface ?? {};
  const peer = sections.peer ?? {};
  const endpoint = parseHostPort(peer.endpoint ?? "");
  if (!endpoint.host || !endpoint.port) {
    throw new Error("amnezia config does not contain endpoint");
  }
  const next = ensureNodeDefaults({
    ...node,
    connectionType: "vpn",
    protocol: "amnezia",
    host: endpoint.host,
    port: endpoint.port,
    username: "",
    password: "",
    settings: {
      ...(node.settings ?? {}),
      amneziaKey: content
    }
  });
  const settingMap = [
    ["privatekey", "privateKey"],
    ["address", "address"],
    ["dns", "dns"],
    ["publickey", "publicKey"],
    ["presharedkey", "preSharedKey"],
    ["allowedips", "allowedIps"],
    ["persistentkeepalive", "persistentKeepalive"],
    ["mtu", "mtu"],
    ["jc", "jc"],
    ["jmin", "jmin"],
    ["jmax", "jmax"],
    ["s1", "s1"],
    ["s2", "s2"],
    ["s3", "s3"],
    ["s4", "s4"],
    ["h1", "h1"],
    ["h2", "h2"],
    ["h3", "h3"],
    ["h4", "h4"],
    ["i1", "i1"],
    ["i2", "i2"],
    ["i3", "i3"],
    ["i4", "i4"],
    ["i5", "i5"]
  ];
  for (const [source, target] of settingMap) {
    const value = iface[source] ?? peer[source];
    if (value != null) {
      next.settings[target] = String(value).trim();
    }
  }
  return next;
}

function parseAmneziaInput(rawInput, node) {
  const key = String(rawInput ?? "").trim();
  const keyLine = key
    .split(/\r?\n/)
    .map((line) => line.trim())
    .find((line) => line.length > 0) ?? "";
  if (!keyLine) throw new Error("empty amnezia input");
  if (keyLine.toLowerCase().startsWith("vpn://")) {
    return ensureNodeDefaults({
      ...node,
      connectionType: "vpn",
      protocol: "amnezia",
      host: "",
      port: "",
      username: "",
      password: "",
      settings: {
        ...(node.settings ?? {}),
        amneziaKey: keyLine
      }
    });
  }
  const looksLikeConf = (key.includes("[Interface]") || key.includes("[interface]"))
    && (key.includes("[Peer]") || key.includes("[peer]"));
  if (looksLikeConf) {
    return parseAmneziaConfig(key, node);
  }
  throw new Error("amnezia input must be vpn:// key or awg .conf");
}

function parseV2RayLink(rawLink, node) {
  const link = String(rawLink ?? "").trim();
  if (!link) throw new Error("empty link");
  const lower = link.toLowerCase();
  if (lower.startsWith("vmess://")) {
    const payload = link.slice("vmess://".length).trim();
    const decoded = decodeBase64(payload);
    const cfg = JSON.parse(decoded);
    const next = ensureNodeDefaults({
      ...node,
      connectionType: "v2ray",
      protocol: "vmess",
      host: cfg.add || cfg.host || "",
      port: cfg.port != null ? String(cfg.port) : "443",
      settings: {
        ...(node.settings ?? {}),
        uuid: cfg.id || "",
        alterId: cfg.aid != null ? String(cfg.aid) : "0",
        security: cfg.scy || "auto",
        network: cfg.net || "tcp",
        wsPath: cfg.path || "",
        wsHost: cfg.host || cfg.sni || "",
        tls: cfg.tls && cfg.tls !== "none" ? "on" : "off",
        sni: cfg.sni || ""
      }
    });
    return { node: next, name: cfg.ps || "" };
  }

  const url = new URL(link);
  if (url.protocol === "vless:") {
    const security = (url.searchParams.get("security") || "").trim().toLowerCase();
    const isTlsLike = security === "tls" || security === "reality";
    const next = ensureNodeDefaults({
      ...node,
      connectionType: "v2ray",
      protocol: "vless",
      host: url.hostname,
      port: url.port || "443",
      settings: {
        ...(node.settings ?? {}),
        uuid: decodeURIComponent(url.username || ""),
        flow: url.searchParams.get("flow") || "",
        network: url.searchParams.get("type") || url.searchParams.get("network") || "tcp",
        tls: isTlsLike ? "on" : "off",
        sni: url.searchParams.get("sni") || "",
        securityMode: security || "none",
        realityPublicKey: url.searchParams.get("pbk") || "",
        realityShortId: url.searchParams.get("sid") || "",
        realityFingerprint: url.searchParams.get("fp") || "",
        realitySpiderX: decodeURIComponent(url.searchParams.get("spx") || "")
      }
    });
    return { node: next, name: decodeURIComponent((url.hash || "").replace(/^#/, "")) };
  }

  if (url.protocol === "trojan:") {
    const next = ensureNodeDefaults({
      ...node,
      connectionType: "v2ray",
      protocol: "trojan",
      host: url.hostname,
      port: url.port || "443",
      password: decodeURIComponent(url.username || ""),
      settings: {
        ...(node.settings ?? {}),
        tls: url.searchParams.get("security") === "none" ? "off" : "on",
        sni: url.searchParams.get("sni") || "",
        alpn: url.searchParams.get("alpn") || "",
        network: url.searchParams.get("type") || "tcp"
      }
    });
    return { node: next, name: decodeURIComponent((url.hash || "").replace(/^#/, "")) };
  }

  if (url.protocol === "ss:") {
    const parsed = parseShadowsocksLink(link);
    const next = ensureNodeDefaults({
      ...node,
      connectionType: "v2ray",
      protocol: "shadowsocks",
      host: parsed.host,
      port: parsed.port,
      password: parsed.password,
      settings: {
        ...(node.settings ?? {}),
        method: parsed.method || "aes-256-gcm"
      }
    });
    return { node: next, name: parsed.name || "" };
  }

  throw new Error("unsupported link protocol");
}

function parseShadowsocksLink(link) {
  const withoutScheme = link.slice("ss://".length);
  const [beforeHash, hashPart = ""] = withoutScheme.split("#");
  const [main] = beforeHash.split("?");
  let decoded = main;
  if (!main.includes("@")) {
    decoded = decodeBase64(main);
  }
  const [userInfo, endpoint] = decoded.split("@");
  if (!endpoint) throw new Error("invalid ss link");
  const [method, password] = userInfo.split(":");
  const hostPort = parseHostPort(endpoint);
  return {
    method: method || "",
    password: password || "",
    host: hostPort.host,
    port: hostPort.port || "8388",
    name: decodeURIComponent(hashPart || "")
  };
}

function applyImportedNode(model, nodeId, nextNode, suggestedName = "") {
  const draft = model.networkTemplateDraft ?? defaultTemplateDraft();
  draft.nodes = draft.nodes.map((node) => (node.nodeId === nodeId ? ensureNodeDefaults(nextNode) : node));
  if (!draft.name.trim() && suggestedName.trim()) {
    draft.name = suggestedName.trim();
  }
  model.networkTemplateDraft = draft;
}

export async function hydrateNetworkModel(model) {
  if (model.networkLoaded && model.networkTemplates) return;
  const result = await getNetworkState("");
  const state = result.ok ? JSON.parse(result.data) : {
    payload: null,
    selectedTemplateId: null,
    connectionTemplates: [],
    globalRoute: {
      globalVpnEnabled: false,
      blockWithoutVpn: true,
      defaultTemplateId: null
    }
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

export function wireNetwork(root, model, rerender, t) {
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
        ? parseWireguardConfig(raw, current)
        : importKind === "ovpn"
          ? parseOpenVpnConfig(raw, current)
          : parseAmneziaInput(raw, current);
      applyImportedNode(model, nodeId, nextNode, file.name.replace(/\.[^.]+$/, ""));
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

  for (const button of root.querySelectorAll("[data-action='import-link']")) {
    button.addEventListener("click", async () => {
      syncTemplateDraft(root, model);
      const nodeId = button.getAttribute("data-node-id");
      const link = await askInputModal(t, {
        title: t("network.importLink"),
        label: t("network.importLinkPrompt"),
        defaultValue: ""
      });
      if (!link) return;
      const draft = model.networkTemplateDraft ?? defaultTemplateDraft();
      const node = draft.nodes.find((item) => item.nodeId === nodeId);
      if (!node) return;
      try {
        const parsed = parseV2RayLink(link, node);
        applyImportedNode(model, nodeId, parsed.node, parsed.name || "");
        model.networkNotice = { type: "success", text: t("network.importApplied") };
      } catch (error) {
        model.networkNotice = { type: "error", text: formatNetworkError(error, t) };
      }
      await rerender();
    });
  }

  for (const button of root.querySelectorAll("[data-action='import-amnezia-key']")) {
    button.addEventListener("click", async () => {
      syncTemplateDraft(root, model);
      const nodeId = button.getAttribute("data-node-id");
      const key = await askInputModal(t, {
        title: t("network.importVpnKey"),
        label: t("network.importAmneziaPrompt"),
        defaultValue: "",
        multiline: true
      });
      if (!key) return;
      const draft = model.networkTemplateDraft ?? defaultTemplateDraft();
      const node = draft.nodes.find((item) => item.nodeId === nodeId);
      if (!node) return;
      try {
        const nextNode = parseAmneziaInput(key, node);
        applyImportedNode(model, nodeId, nextNode, "");
        model.networkNotice = { type: "success", text: t("network.importApplied") };
      } catch (error) {
        model.networkNotice = { type: "error", text: formatNetworkError(error, t) };
      }
      await rerender();
    });
  }

  let floatingTemplateMenu = null;
  const closeAllTemplateMenus = () => {
    floatingTemplateMenu?.remove();
    floatingTemplateMenu = null;
    for (const row of root.querySelectorAll("[data-template-id].menu-open")) {
      row.classList.remove("menu-open");
    }
  };
  const positionTemplateMenu = (menu, toggle) => {
    if (!menu || !toggle) return;
    const rect = toggle.getBoundingClientRect();
    const width = Math.min(360, Math.max(280, rect.width));
    const viewportWidth = window.innerWidth || document.documentElement.clientWidth || 0;
    const left = Math.max(12, Math.min(rect.right - width, viewportWidth - width - 12));
    menu.style.left = `${left}px`;
    menu.style.top = `${Math.round(rect.bottom + 8)}px`;
    menu.style.width = `${width}px`;
  };
  root.addEventListener("click", (event) => {
    const target = event.target;
    if (target?.closest?.(".network-floating-menu")) return;
    if (target?.closest?.("[data-template-menu-toggle]")) return;
    closeAllTemplateMenus();
  });

  const handleTemplateAction = async (template, action) => {
    closeAllTemplateMenus();
    if (!template || !action) return;
    if (action === "ping") {
      const ping = await pingConnectionTemplate(template.id);
      model.networkNotice = {
        type: ping.ok && ping.data.reachable ? "success" : "error",
        text: ping.ok ? ping.data.message : formatNetworkError(ping.data.error, t)
      };
      if (ping.ok) {
        model.networkPingState = {
          ...(model.networkPingState ?? {}),
          [template.id]: ping.data
        };
      }
      await rerender();
      return;
    }
    if (action === "edit") {
      model.networkTemplateDraft = {
        templateId: template.id,
        name: template.name,
        nodes: normalizeTemplateNodes(template)
      };
      model.networkNodeTestState = {};
      await rerender();
      return;
    }
    if (action === "set-default") {
      const current = model.networkGlobalRoute ?? {};
      if (!current.globalVpnEnabled) {
        model.networkNotice = { type: "error", text: t("network.defaultTemplateRequiresGlobal") };
        await rerender();
        return;
      }
      await saveGlobalSettings({
        globalVpnEnabled: true,
        blockWithoutVpn: Boolean(current.blockWithoutVpn),
        defaultTemplateId: template.id
      });
      return;
    }
    if (action === "delete") {
      const result = await deleteConnectionTemplate(template.id);
      model.networkNotice = {
        type: result.ok ? "success" : "error",
        text: result.ok ? t("network.templateDeleted") : formatNetworkError(result.data.error, t)
      };
      if (result.ok) {
        model.networkLoaded = false;
        await hydrateNetworkModel(model);
      }
      await rerender();
    }
  };

  const buildTemplateMenu = (template) => {
    const globalRoute = model.networkGlobalRoute ?? {};
    const globalVpnEnabled = Boolean(globalRoute.globalVpnEnabled);
    const blockWithoutVpn = Boolean(globalRoute.blockWithoutVpn);
    const isDefault = (globalRoute.defaultTemplateId ?? "") === template.id;
    const menu = document.createElement("div");
    menu.className = "dns-dropdown-menu network-actions-menu network-floating-menu";
    menu.innerHTML = `
      <button type="button" class="dns-dropdown-option" data-action="ping">${t("network.testConnection")}</button>
      <button type="button" class="dns-dropdown-option" data-action="edit">${t("extensions.edit")}</button>
      <button type="button" class="dns-dropdown-option" data-action="delete">${t("extensions.remove")}</button>
      ${globalVpnEnabled ? `<button type="button" class="dns-dropdown-option" data-action="set-default">
        ${isDefault ? t("network.defaultTemplateActive") : t("network.defaultTemplateSet")}
      </button>` : ""}
    `;
    for (const actionButton of menu.querySelectorAll("[data-action]")) {
      actionButton.addEventListener("click", async (event) => {
        event.preventDefault();
        event.stopPropagation();
        await handleTemplateAction(template, actionButton.getAttribute("data-action"));
      });
    }
    menu.addEventListener("click", (event) => event.stopPropagation());
    return menu;
  };

  for (const row of root.querySelectorAll("[data-template-id]")) {
    const templateId = row.getAttribute("data-template-id");
    const menuToggle = row.querySelector("[data-template-menu-toggle]");
    menuToggle?.addEventListener("click", (event) => {
      event.preventDefault();
      event.stopPropagation();
      const template = (model.networkTemplates ?? []).find((item) => item.id === templateId);
      const shouldOpen = !floatingTemplateMenu || row.classList.contains("menu-open") === false;
      closeAllTemplateMenus();
      if (shouldOpen && template) {
        row.classList.add("menu-open");
        floatingTemplateMenu = buildTemplateMenu(template);
        document.body.appendChild(floatingTemplateMenu);
        positionTemplateMenu(floatingTemplateMenu, menuToggle);
      }
    });
  }
}
