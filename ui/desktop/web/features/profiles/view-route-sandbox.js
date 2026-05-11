function escapeHtml(value) {
  return String(value ?? "")
    .replaceAll("&", "&amp;")
    .replaceAll("<", "&lt;")
    .replaceAll(">", "&gt;")
    .replaceAll("\"", "&quot;");
}

function normalizeRouteTemplateNodes(template) {
  if (Array.isArray(template?.nodes) && template.nodes.length) {
    return template.nodes.map((node, index) => ({
      id: node.id ?? node.nodeId ?? `node-${index + 1}`,
      connectionType: (node.connectionType ?? node.connection_type ?? "").toLowerCase(),
      protocol: (node.protocol ?? "").toLowerCase(),
      host: node.host ?? "",
      port: node.port != null ? Number(node.port) : 0,
      username: node.username ?? "",
      password: node.password ?? "",
      bridges: node.bridges ?? "",
      settings: node.settings ?? {}
    }));
  }
  if (template?.connectionType || template?.connection_type) {
    return [{
      id: "node-1",
      connectionType: (template.connectionType ?? template.connection_type ?? "").toLowerCase(),
      protocol: (template.protocol ?? "").toLowerCase(),
      host: template.host ?? "",
      port: template.port != null ? Number(template.port) : 0,
      username: template.username ?? "",
      password: template.password ?? "",
      bridges: template.bridges ?? "",
      settings: {}
    }];
  }
  return [];
}

function templateSupportsRouteMode(template, routeMode) {
  const nodes = normalizeRouteTemplateNodes(template);
  if (routeMode === "direct" || routeMode === "tor") return true;
  if (routeMode === "proxy") return nodes.some((node) => node.connectionType === "proxy");
  if (routeMode === "vpn") return nodes.some((node) => node.connectionType === "vpn" || node.connectionType === "v2ray");
  if (routeMode === "hybrid") {
    const hasProxy = nodes.some((node) => node.connectionType === "proxy");
    const hasVpnLike = nodes.some((node) => node.connectionType === "vpn" || node.connectionType === "v2ray");
    return hasProxy && hasVpnLike;
  }
  return false;
}

export function routeTemplateOptions(routeTemplates, selectedTemplateId, routeMode, t) {
  const options = [`<option value="">${t("network.routeTemplate.none")}</option>`];
  for (const template of routeTemplates) {
    if (!templateSupportsRouteMode(template, routeMode)) continue;
    const chain = normalizeRouteTemplateNodes(template)
      .map((node) => `${t(`network.node.${node.connectionType}`)}:${node.protocol}`)
      .join(" -> ");
    options.push(`<option value="${template.id}" ${template.id === selectedTemplateId ? "selected" : ""}>${escapeHtml(template.name)} (${escapeHtml(chain)})</option>`);
  }
  return options.join("");
}

export function normalizeProfileRouteMode(routeMode) {
  const normalized = String(routeMode ?? "direct").toLowerCase();
  if (normalized === "proxy" || normalized === "vpn" || normalized === "tor") {
    return normalized;
  }
  return "direct";
}

export function buildRoutePolicyPayload(routeMode, selectedTemplate, killSwitchEnabled, t) {
  const base = {
    route_mode: routeMode,
    proxy: null,
    vpn: null,
    kill_switch_enabled: Boolean(killSwitchEnabled)
  };
  if (routeMode === "direct" || routeMode === "tor") return base;
  if (!selectedTemplate) {
    throw new Error(t("network.templateRequired"));
  }
  const nodes = normalizeRouteTemplateNodes(selectedTemplate);
  if (routeMode === "proxy") {
    const node = nodes.find((item) => item.connectionType === "proxy");
    if (!node) throw new Error(t("network.templateTypeMismatch"));
    base.proxy = {
      protocol: node.protocol,
      host: node.host,
      port: Number(node.port ?? 0),
      username: node.username || null,
      password: node.password || null
    };
    return base;
  }
  if (routeMode === "vpn") {
    const node = nodes.find((item) => item.connectionType === "vpn" || item.connectionType === "v2ray");
    if (!node) throw new Error(t("network.templateTypeMismatch"));
    base.vpn = {
      protocol: node.protocol,
      endpoint_host: node.host,
      endpoint_port: Number(node.port ?? 0),
      profile_ref: selectedTemplate.name
    };
    return base;
  }
  if (routeMode === "hybrid") {
    const proxyNode = nodes.find((item) => item.connectionType === "proxy");
    const vpnNode = nodes.find((item) => item.connectionType === "vpn" || item.connectionType === "v2ray");
    if (!proxyNode || !vpnNode) {
      throw new Error(t("network.templateTypeMismatch"));
    }
    base.proxy = {
      protocol: proxyNode.protocol,
      host: proxyNode.host,
      port: Number(proxyNode.port ?? 0),
      username: proxyNode.username || null,
      password: proxyNode.password || null
    };
    base.vpn = {
      protocol: vpnNode.protocol,
      endpoint_host: vpnNode.host,
      endpoint_port: Number(vpnNode.port ?? 0),
      profile_ref: selectedTemplate.name
    };
    return base;
  }
  return base;
}

function sandboxModeLabel(mode, t) {
  return t(`network.sandbox.mode.${mode}`) || mode;
}

function sandboxAdapterLabel(kind, t) {
  return t(`network.sandbox.adapter.${kind}`) || kind;
}

function profileRouteSummary(template, t) {
  if (!template) return t("network.sandbox.routeUnknown");
  const chain = normalizeRouteTemplateNodes(template)
    .map((node) => `${t(`network.node.${node.connectionType}`)}:${node.protocol}`)
    .join(" -> ");
  return `${template.name} (${chain})`;
}

function compatibleSandboxModeOptions(modes, selectedMode, t) {
  return (modes ?? [])
    .map((mode) => `<option value="${mode}" ${mode === selectedMode ? "selected" : ""}>${sandboxModeLabel(mode, t)}</option>`)
    .join("");
}

function formatProfileSandboxReason(reason, sandbox, adapter, selectedTemplate, t) {
  const value = String(reason || "").trim();
  if (!value) {
    return t("network.sandbox.unknown");
  }
  const activeRoute = profileRouteSummary(selectedTemplate, t);
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
    return t(exactMap[value])
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

export function renderProfileSandboxFrame(preview, selectedTemplate, selectedModeOverride, t) {
  if (!selectedTemplate || !preview?.sandbox || !(preview.compatibleModes ?? []).length) {
    return "";
  }
  const sandbox = preview.sandbox;
  const adapter = sandbox.adapter ?? {};
  const selectedMode = (selectedModeOverride && (preview.compatibleModes ?? []).includes(selectedModeOverride))
    ? selectedModeOverride
    : (sandbox.preferredMode && (preview.compatibleModes ?? []).includes(sandbox.preferredMode))
      ? sandbox.preferredMode
      : (preview.compatibleModes?.includes(sandbox.effectiveMode) ? sandbox.effectiveMode : preview.compatibleModes?.[0] ?? "isolated");
  const sandboxReason = formatProfileSandboxReason(
    adapter.reason || sandbox.lastResolutionReason,
    sandbox,
    adapter,
    selectedTemplate,
    t
  );
  const nativeWarning = sandbox.effectiveMode === "blocked"
    ? ""
    : adapter.requiresSystemNetworkAccess
      ? `<p class="notice error">${t("network.sandbox.nativeWarning")}</p>`
      : sandbox.requiresNativeBackend && sandbox.effectiveMode === "container"
        ? `<p class="notice success">${t("network.sandbox.containerNativeIsolated")}</p>`
        : `<p class="meta">${t("network.sandbox.isolatedHint")}</p>`;
  return `
    <div class="security-frame" id="profile-sandbox-frame">
      <h4>${t("network.sandbox.title")}</h4>
      <div class="grid-two">
        <div>
          <strong>${t("network.sandbox.activeRoute")}</strong>
          <p>${escapeHtml(profileRouteSummary(selectedTemplate, t))}</p>
        </div>
        <div>
          <strong>${t("network.sandbox.adapterLabel")}</strong>
          <p>${escapeHtml(sandboxAdapterLabel(adapter.adapterKind || "unknown", t))}</p>
        </div>
        <div>
          <strong>${t("network.sandbox.runtimeLabel")}</strong>
          <p>${escapeHtml(adapter.runtimeKind || "unknown")}</p>
        </div>
        <div>
          <strong>${t("network.sandbox.effectiveMode")}</strong>
          <p>${escapeHtml(sandboxModeLabel(sandbox.effectiveMode, t))}</p>
        </div>
      </div>
      <label style="margin-top:12px;">${t("network.sandbox.chooseMode")}
        <select id="profile-sandbox-mode">
          ${compatibleSandboxModeOptions(preview.compatibleModes, selectedMode, t)}
        </select>
      </label>
      <p class="meta" style="margin-top:8px;">${escapeHtml(sandboxReason)}</p>
      ${nativeWarning}
      ${adapter.requiresSystemNetworkAccess ? `<p class="notice error">${t("network.sandbox.systemWideWarning")}</p>` : ""}
      ${sandbox.effectiveMode === "container" ? `<p class="notice">${t("network.sandbox.containerMvp")}</p>` : ""}
      ${sandbox.effectiveMode === "blocked" ? `<p class="notice error">${t("network.sandbox.blockedHint").replace("{route}", profileRouteSummary(selectedTemplate, t))}</p>` : ""}
    </div>
  `;
}

export function globalRouteNoticeHtml(networkState, routeMode, t) {
  const normalizedMode = normalizeProfileRouteMode(routeMode || "direct");
  if (normalizedMode !== "direct") {
    return "";
  }
  const globalRoute = networkState?.globalRoute ?? {};
  const defaultTemplateId = String(globalRoute.defaultTemplateId ?? "").trim();
  if (!defaultTemplateId || (!globalRoute.globalVpnEnabled && !globalRoute.blockWithoutVpn)) {
    return "";
  }
  const template = (networkState?.connectionTemplates ?? []).find((item) => item.id === defaultTemplateId);
  if (!template) {
    return "";
  }
  const reasonKey = globalRoute.globalVpnEnabled && globalRoute.blockWithoutVpn
    ? "profile.route.globalRouteInheritedGlobalAndBlock"
    : globalRoute.globalVpnEnabled
      ? "profile.route.globalRouteInheritedGlobalOnly"
      : "profile.route.globalRouteInheritedBlockOnly";
  return `<p class="notice success">${escapeHtml(t(reasonKey).replace("{route}", profileRouteSummary(template, t)))}</p>`;
}
