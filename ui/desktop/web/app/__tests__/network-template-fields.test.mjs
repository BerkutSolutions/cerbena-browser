import test from "node:test";
import assert from "node:assert/strict";

import { ensureNodeDefaults, renderTemplateFrame } from "../../features/network/view-template-editor.js";

function t(key) {
  return key;
}

function renderWithNode(node) {
  const model = {
    networkTemplateDraft: {
      templateId: "",
      name: "Template",
      nodes: [ensureNodeDefaults(node)]
    },
    networkNodeTestState: {}
  };
  return renderTemplateFrame(model, t, () => "", () => "");
}

test("network template fields: wireguard includes full key/address set", () => {
  const html = renderWithNode({ connectionType: "vpn", protocol: "wireguard" });
  for (const setting of ["publicKey", "privateKey", "allowedIps", "address", "dns", "persistentKeepalive"]) {
    assert.ok(html.includes(`data-node-setting="${setting}"`), `missing setting ${setting}`);
  }
});

test("network template fields: openvpn includes auth + transport + cert fields", () => {
  const html = renderWithNode({ connectionType: "vpn", protocol: "openvpn" });
  for (const field of ["host", "port", "username", "password"]) {
    assert.ok(html.includes(`data-node-field="${field}"`), `missing field ${field}`);
  }
  for (const setting of ["transport", "caCert", "clientCert", "clientKey", "ovpnRaw"]) {
    assert.ok(html.includes(`data-node-setting="${setting}"`), `missing setting ${setting}`);
  }
});

test("network template fields: amnezia includes key textarea", () => {
  const html = renderWithNode({ connectionType: "vpn", protocol: "amnezia" });
  assert.ok(html.includes('data-node-setting="amneziaKey"'));
});

test("network template fields: proxy includes login/password", () => {
  const html = renderWithNode({ connectionType: "proxy", protocol: "socks5" });
  for (const field of ["host", "port", "username", "password"]) {
    assert.ok(html.includes(`data-node-field="${field}"`), `missing field ${field}`);
  }
});

test("network template fields: vmess includes uuid, alterId, network/tls controls", () => {
  const html = renderWithNode({ connectionType: "v2ray", protocol: "vmess" });
  for (const setting of ["uuid", "alterId", "security", "network", "tls", "sni"]) {
    assert.ok(html.includes(`data-node-setting="${setting}"`), `missing setting ${setting}`);
  }
});

test("network template fields: vless includes flow and hidden reality fields", () => {
  const html = renderWithNode({ connectionType: "v2ray", protocol: "vless" });
  for (const setting of ["uuid", "flow", "network", "tls", "sni", "securityMode", "realityPublicKey", "realityShortId", "realityFingerprint", "realitySpiderX"]) {
    assert.ok(html.includes(`data-node-setting="${setting}"`), `missing setting ${setting}`);
  }
});

test("network template fields: trojan includes password/alpn/tls", () => {
  const html = renderWithNode({ connectionType: "v2ray", protocol: "trojan" });
  assert.ok(html.includes('data-node-field="password"'));
  for (const setting of ["tls", "sni", "alpn", "network"]) {
    assert.ok(html.includes(`data-node-setting="${setting}"`), `missing setting ${setting}`);
  }
});

test("network template fields: shadowsocks includes password and method", () => {
  const html = renderWithNode({ connectionType: "v2ray", protocol: "shadowsocks" });
  assert.ok(html.includes('data-node-field="password"'));
  assert.ok(html.includes('data-node-setting="method"'));
});

test("network template fields: tor obfs4 requires bridges, tor none uses simple mode", () => {
  const obfs4 = renderWithNode({ connectionType: "tor", protocol: "obfs4" });
  assert.ok(obfs4.includes('data-node-field="bridges"'));

  const none = renderWithNode({ connectionType: "tor", protocol: "none" });
  assert.ok(none.includes("network.torSimpleMode"));
});

