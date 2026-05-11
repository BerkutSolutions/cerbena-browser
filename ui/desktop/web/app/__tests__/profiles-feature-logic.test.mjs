import test from "node:test";
import assert from "node:assert/strict";

import {
  buildRoutePolicyPayload,
  normalizeProfileRouteMode
} from "../../features/profiles/view-route-sandbox.js";
import {
  certificateEntriesForProfile,
  syncManagedCertificateAssignments
} from "../../features/profiles/view-helpers.js";

function t(key) {
  return key;
}

test("profiles logic: normalizeProfileRouteMode keeps supported values and defaults unknown to direct", () => {
  assert.equal(normalizeProfileRouteMode("vpn"), "vpn");
  assert.equal(normalizeProfileRouteMode("proxy"), "proxy");
  assert.equal(normalizeProfileRouteMode("tor"), "tor");
  assert.equal(normalizeProfileRouteMode("unexpected"), "direct");
  assert.equal(normalizeProfileRouteMode(""), "direct");
});

test("profiles logic: buildRoutePolicyPayload shapes proxy payload with normalized nullable credentials", () => {
  const payload = buildRoutePolicyPayload(
    "proxy",
    {
      id: "tpl-1",
      name: "Proxy route",
      nodes: [
        {
          nodeId: "node-1",
          connectionType: "proxy",
          protocol: "socks5",
          host: "127.0.0.1",
          port: "1080",
          username: "",
          password: ""
        }
      ]
    },
    true,
    t
  );

  assert.equal(payload.route_mode, "proxy");
  assert.equal(payload.kill_switch_enabled, true);
  assert.deepEqual(payload.proxy, {
    protocol: "socks5",
    host: "127.0.0.1",
    port: 1080,
    username: null,
    password: null
  });
  assert.equal(payload.vpn, null);
});

test("profiles logic: buildRoutePolicyPayload throws templateRequired/templateTypeMismatch for guarded flows", () => {
  assert.throws(() => buildRoutePolicyPayload("vpn", null, false, t), /network\.templateRequired/);
  assert.throws(
    () =>
      buildRoutePolicyPayload(
        "hybrid",
        {
          id: "tpl-2",
          name: "Only proxy",
          nodes: [{ nodeId: "node-1", connectionType: "proxy", protocol: "http", host: "h", port: "80" }]
        },
        false,
        t
      ),
    /network\.templateTypeMismatch/
  );
});

test("profiles logic: certificateEntriesForProfile merges tags and global assignments without duplicates", () => {
  const entries = certificateEntriesForProfile(
    {
      id: "p-1",
      tags: ["cert-id:cert-a", "cert-id:cert-a", "cert:C:\\legacy\\client.pem"]
    },
    {
      certificates: [
        { id: "cert-a", profileIds: ["p-1"] },
        { id: "cert-b", profileIds: ["p-1"] },
        { id: "cert-c", profileIds: [] }
      ]
    }
  );

  assert.deepEqual(entries, [
    { kind: "id", value: "cert-a" },
    { kind: "id", value: "cert-b" },
    { kind: "path", value: "C:\\legacy\\client.pem" }
  ]);
});

test("profiles logic: syncManagedCertificateAssignments updates only selected profile bindings", () => {
  const next = syncManagedCertificateAssignments(
    {
      certificates: [
        { id: "cert-a", profileIds: ["p-2", "p-1"] },
        { id: "cert-b", profileIds: [] },
        { id: "cert-c", profileIds: ["p-3"] }
      ]
    },
    "p-1",
    [{ kind: "id", value: "cert-b" }]
  );

  const byId = Object.fromEntries(next.certificates.map((item) => [item.id, item.profileIds]));
  assert.deepEqual(byId["cert-a"], ["p-2"]);
  assert.deepEqual(byId["cert-b"], ["p-1"]);
  assert.deepEqual(byId["cert-c"], ["p-3"]);
});
