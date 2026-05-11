export function createNetworkImportUtils(deps) {
  const { ensureNodeDefaults, defaultTemplateDraft, normalizeOpenVpnTransport } = deps;
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
  const remoteMatch = content.match(/^\s*remote\s+([^\s]+)(?:\s+(\d+))?(?:\s+([^\s#;]+))?/im);
  if (remoteMatch) {
    next.host = remoteMatch[1] ?? next.host;
    next.port = remoteMatch[2] ?? next.port;
    const remoteTransport = normalizeOpenVpnTransport(remoteMatch[3]);
    if (remoteTransport) {
      next.settings.transport = remoteTransport;
    }
  }
  const protoMatch = content.match(/^\s*proto\s+([^\s]+)/im);
  if (protoMatch) {
    const value = normalizeOpenVpnTransport(protoMatch[1]);
    if (value) {
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


  return {
    parseWireguardConfig,
    parseOpenVpnConfig,
    parseAmneziaInput,
    parseV2RayLink,
    applyImportedNode
  };
}
