import test from "node:test";
import assert from "node:assert/strict";

import {
  defaultTemplateDraft,
  ensureNodeDefaults,
  networkDraftUtils
} from "../../features/network/view-template-editor.js";

function t(key) {
  return key;
}

test("network logic: validateDraft rejects tor obfs4 without bridges", () => {
  const draft = {
    templateId: "",
    name: "TOR",
    nodes: [
      ensureNodeDefaults({
        nodeId: "node-1",
        connectionType: "tor",
        protocol: "obfs4",
        host: "tor.example.net",
        port: "443",
        bridges: ""
      })
    ]
  };

  assert.equal(networkDraftUtils.validateDraft(draft, t), "network.error.torBridgesRequired");
});

test("network logic: validateDraft enforces v2ray uuid and password guards", () => {
  const vmessDraft = {
    templateId: "",
    name: "VMESS",
    nodes: [
      ensureNodeDefaults({
        nodeId: "node-1",
        connectionType: "v2ray",
        protocol: "vmess",
        host: "v.example.net",
        port: "443",
        settings: { uuid: "invalid-uuid" }
      })
    ]
  };
  assert.equal(networkDraftUtils.validateDraft(vmessDraft, t), "network.error.uuidInvalid");

  const trojanDraft = {
    templateId: "",
    name: "Trojan",
    nodes: [
      ensureNodeDefaults({
        nodeId: "node-1",
        connectionType: "v2ray",
        protocol: "trojan",
        host: "t.example.net",
        port: "443",
        password: ""
      })
    ]
  };
  assert.equal(networkDraftUtils.validateDraft(trojanDraft, t), "network.error.passwordRequired");
});

test("network logic: validateDraft accepts amnezia conf and vpn:// key", () => {
  const confDraft = {
    templateId: "",
    name: "Amnezia conf",
    nodes: [
      ensureNodeDefaults({
        nodeId: "node-1",
        connectionType: "vpn",
        protocol: "amnezia",
        settings: {
          amneziaKey: "[Interface]\nPrivateKey = A\n[Peer]\nEndpoint = host:51820"
        }
      })
    ]
  };
  assert.equal(networkDraftUtils.validateDraft(confDraft, t), "");

  const keyDraft = {
    templateId: "",
    name: "Amnezia key",
    nodes: [
      ensureNodeDefaults({
        nodeId: "node-1",
        connectionType: "vpn",
        protocol: "amnezia",
        settings: {
          amneziaKey: "vpn://amnezia-link"
        }
      })
    ]
  };
  assert.equal(networkDraftUtils.validateDraft(keyDraft, t), "");
});

test("network logic: templateRequest trims fields and keeps settings value '0'", () => {
  const draft = defaultTemplateDraft();
  draft.name = "  Mixed  ";
  draft.nodes = [
    ensureNodeDefaults({
      nodeId: "node-1",
      connectionType: "vpn",
      protocol: "openvpn",
      host: "  vpn.example.org ",
      port: " 443 ",
      username: "  user ",
      password: "  pass ",
      settings: {
        transport: " tcp ",
        mtu: "0",
        emptyValue: "   "
      }
    })
  ];

  const request = networkDraftUtils.templateRequest(draft);
  assert.equal(request.name, "Mixed");
  assert.equal(request.host, "vpn.example.org");
  assert.equal(request.port, 443);
  assert.equal(request.username, "user");
  assert.equal(request.password, "pass");
  assert.deepEqual(request.nodes[0].settings, { transport: "tcp", mtu: "0" });
});

test("network logic: formatNetworkError maps known import failures to i18n keys", () => {
  const mapped = networkDraftUtils.formatNetworkError("amnezia config does not contain endpoint", t);
  assert.equal(mapped, "network.error.amneziaEndpointMissing");

  const passthrough = networkDraftUtils.formatNetworkError("custom-runtime-error", t);
  assert.equal(passthrough, "custom-runtime-error");
});
