import test from "node:test";
import assert from "node:assert/strict";

import {
  ensureNodeDefaults,
  networkImportUtils
} from "../../features/network/view-template-editor.js";

test("network import: parses openvpn remote/proto/certs", () => {
  const source = `
client
remote vpn.example.com 443 tcp
proto udp
<ca>CA_DATA</ca>
<cert>CERT_DATA</cert>
<key>KEY_DATA</key>
`;
  const parsed = networkImportUtils.parseOpenVpnConfig(source, ensureNodeDefaults({ connectionType: "vpn", protocol: "openvpn" }));
  assert.equal(parsed.host, "vpn.example.com");
  assert.equal(parsed.port, "443");
  assert.equal(parsed.settings.transport, "udp");
  assert.equal(parsed.settings.caCert, "CA_DATA");
  assert.equal(parsed.settings.clientCert, "CERT_DATA");
  assert.equal(parsed.settings.clientKey, "KEY_DATA");
});

test("network import: parses wireguard conf endpoint and keys", () => {
  const source = `
[Interface]
PrivateKey = PRIV
Address = 10.20.0.2/32
DNS = 1.1.1.1

[Peer]
PublicKey = PUB
AllowedIPs = 0.0.0.0/0
PersistentKeepalive = 25
Endpoint = wg.example.com:51820
`;
  const parsed = networkImportUtils.parseWireguardConfig(source, ensureNodeDefaults({ connectionType: "vpn", protocol: "wireguard" }));
  assert.equal(parsed.host, "wg.example.com");
  assert.equal(parsed.port, "51820");
  assert.equal(parsed.settings.privateKey, "PRIV");
  assert.equal(parsed.settings.publicKey, "PUB");
  assert.equal(parsed.settings.allowedIps, "0.0.0.0/0");
});

test("network import: parses vless link", () => {
  const { node } = networkImportUtils.parseV2RayLink(
    "vless://abcd-efgh@v.example.com:443?type=ws&security=tls&sni=sni.example#MyVless",
    ensureNodeDefaults({ connectionType: "v2ray", protocol: "vless" })
  );
  assert.equal(node.protocol, "vless");
  assert.equal(node.host, "v.example.com");
  assert.equal(node.port, "443");
  assert.equal(node.settings.uuid, "abcd-efgh");
  assert.equal(node.settings.network, "ws");
  assert.equal(node.settings.tls, "on");
});

test("network import: parses vmess link", () => {
  const vmessPayload = "eyJ2IjoiMiIsInBzIjoiVk1FU1MiLCJhZGQiOiJ2bS5leGFtcGxlLmNvbSIsInBvcnQiOiI0NDMiLCJpZCI6IjExMTExMTExLTIyMjItMzMzMy00NDQ0LTU1NTU1NTU1NTU1NSIsImFpZCI6IjAiLCJuZXQiOiJ3cyIsInRscyI6InRscyIsImhvc3QiOiJjZG4uZXhhbXBsZS5jb20iLCJwYXRoIjoiL3dzIn0=";
  const { node } = networkImportUtils.parseV2RayLink(
    `vmess://${vmessPayload}`,
    ensureNodeDefaults({ connectionType: "v2ray", protocol: "vmess" })
  );
  assert.equal(node.protocol, "vmess");
  assert.equal(node.host, "vm.example.com");
  assert.equal(node.settings.uuid, "11111111-2222-3333-4444-555555555555");
  assert.equal(node.settings.network, "ws");
  assert.equal(node.settings.tls, "on");
});

test("network import: parses trojan link", () => {
  const { node } = networkImportUtils.parseV2RayLink(
    "trojan://my-pass@t.example.com:443?security=tls&sni=cdn.example.com&type=tcp#Trojan",
    ensureNodeDefaults({ connectionType: "v2ray", protocol: "trojan" })
  );
  assert.equal(node.protocol, "trojan");
  assert.equal(node.host, "t.example.com");
  assert.equal(node.password, "my-pass");
  assert.equal(node.settings.tls, "on");
  assert.equal(node.settings.sni, "cdn.example.com");
});

test("network import: parses shadowsocks link", () => {
  const payload = "YWVzLTI1Ni1nY206cGFzc0BzLmV4YW1wbGUuY29tOjgzODg=";
  const { node } = networkImportUtils.parseV2RayLink(
    `ss://${payload}#MySS`,
    ensureNodeDefaults({ connectionType: "v2ray", protocol: "shadowsocks" })
  );
  assert.equal(node.protocol, "shadowsocks");
  assert.equal(node.host, "s.example.com");
  assert.equal(node.port, "8388");
  assert.equal(node.password, "pass");
  assert.equal(node.settings.method, "aes-256-gcm");
});

test("network import: parses amnezia vpn:// and conf", () => {
  const keyNode = networkImportUtils.parseAmneziaInput(
    "vpn://amnezia-example-key",
    ensureNodeDefaults({ connectionType: "vpn", protocol: "amnezia" })
  );
  assert.equal(keyNode.protocol, "amnezia");
  assert.equal(keyNode.settings.amneziaKey, "vpn://amnezia-example-key");

  const confNode = networkImportUtils.parseAmneziaInput(
    `
[Interface]
PrivateKey = PRIV
Address = 10.10.0.2/32

[Peer]
PublicKey = PUB
AllowedIPs = 0.0.0.0/0
Endpoint = a.example.com:51820
`,
    ensureNodeDefaults({ connectionType: "vpn", protocol: "amnezia" })
  );
  assert.equal(confNode.host, "a.example.com");
  assert.equal(confNode.port, "51820");
  assert.equal(confNode.settings.privateKey, "PRIV");
  assert.equal(confNode.settings.publicKey, "PUB");
});
