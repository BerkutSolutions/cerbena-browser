import test from "node:test";
import assert from "node:assert/strict";

import { renderHome, hydrateHomeModel } from "../../features/home/view.js";
import { renderExtensions } from "../../features/extensions/view.js";
import { renderSecurity } from "../../features/security/view.js";
import { renderIdentity } from "../../features/identity/view.js";
import { renderDns } from "../../features/dns/view.js";
import { renderNetwork } from "../../features/network/view.js";
import { renderTraffic, hydrateTrafficModel, wireTraffic } from "../../features/traffic/view.js";
import { renderLogs, hydrateLogsModel, wireLogs } from "../../features/diagnostics/logs-view.js";
import { renderSettings, hydrateSettingsModel } from "../../features/settings/view.js";

function t(key) {
  return key;
}

function makeLocalStorageMock() {
  const storage = new Map();
  return {
    getItem(key) {
      return storage.has(key) ? storage.get(key) : null;
    },
    setItem(key, value) {
      storage.set(key, String(value));
    },
    removeItem(key) {
      storage.delete(key);
    },
    clear() {
      storage.clear();
    }
  };
}

function ensureRuntimeMocks() {
  globalThis.localStorage = makeLocalStorageMock();
  globalThis.window = globalThis.window ?? {};
  globalThis.window.setInterval = globalThis.setInterval;
  globalThis.window.clearInterval = globalThis.clearInterval;
  globalThis.document = globalThis.document ?? {
    fonts: { check: () => true },
    createElement() {
      return {
        getContext() {
          return null;
        }
      };
    }
  };
}

function makeFakeElement(initial = {}) {
  const listeners = new Map();
  return {
    value: initial.value ?? "",
    checked: Boolean(initial.checked),
    attrs: { ...(initial.attrs ?? {}) },
    classList: {
      toggle() {}
    },
    addEventListener(type, handler) {
      listeners.set(type, handler);
    },
    async fire(type, event = {}) {
      const handler = listeners.get(type);
      if (handler) {
        await handler({ target: this, ...event });
      }
    },
    getAttribute(name) {
      return this.attrs[name] ?? null;
    }
  };
}

function makeFakeRoot(map) {
  return {
    querySelector(selector) {
      return map.get(selector) ?? null;
    },
    querySelectorAll(selector) {
      return map.get(`${selector}[]`) ?? [];
    }
  };
}

test("tab home: renders metrics and profile section", async () => {
  ensureRuntimeMocks();
  const model = { profiles: [] };
  await hydrateHomeModel(model);
  const html = renderHome(t, model);
  assert.ok(html.includes("home-metrics-grid"));
  assert.ok(html.includes("profiles-table"));
});

test("tab extensions: renders library actions and cards area", () => {
  const model = {
    extensionLibraryState: { autoUpdateEnabled: false, items: {} },
    extensionLibraryFilter: "all",
    extensionLibraryTagFilter: [],
    profiles: []
  };
  const html = renderExtensions(t, model);
  assert.ok(html.includes("extension-add-url"));
  assert.ok(html.includes("extension-import-toggle"));
  assert.ok(html.includes("extension-export-toggle"));
  assert.ok(html.includes("extension-library-grid"));
});

test("tab security: renders certificate controls", () => {
  const model = { securityState: { certificates: [] }, profiles: [] };
  const html = renderSecurity(t, model);
  assert.ok(html.includes("sec-cert-add"));
  assert.ok(html.includes("security-table-frame"));
});

test("tab identity: renders mode, preview and save controls", () => {
  const model = { selectedProfileId: "p-1" };
  const html = renderIdentity(t, model);
  assert.ok(html.includes("identity-mode"));
  assert.ok(html.includes("identity-preview"));
  assert.ok(html.includes("identity-save"));
});

test("tab dns: renders policy, blocklist and suffix controls", () => {
  const model = {
    selectedProfileId: "p-1",
    profiles: [{ id: "p-1", tags: [] }],
    serviceCatalog: { categories: {} },
    dnsTemplates: [],
    dnsUiState: {},
    securityState: { blockedDomainSuffixes: [], blocklists: [] }
  };
  const html = renderDns(t, model);
  assert.ok(html.includes("dns-mode"));
  assert.ok(html.includes("dns-policy-table"));
  assert.ok(html.includes("dns-suffix-toggle"));
});

test("tab network: renders chain list and template editor", () => {
  const model = {
    networkTemplates: [],
    networkGlobalRoute: {},
    networkTemplateDraft: { templateId: "", name: "", nodes: [] },
    networkNodeTestState: {}
  };
  const html = renderNetwork(t, model);
  assert.ok(html.includes("network-list-frame"));
  assert.ok(html.includes("network-template-frame"));
});

test("tab traffic: filters wire updates model without system access", async () => {
  ensureRuntimeMocks();
  const model = { profiles: [], trafficPoller: true };
  await hydrateTrafficModel(model);
  renderTraffic(t, model);

  const requestInput = makeFakeElement({ value: "example.org" });
  const statusSelect = makeFakeElement({ value: "blocked" });
  const root = makeFakeRoot(new Map([
    ["#traffic-filter-from", makeFakeElement({ value: "" })],
    ["#traffic-filter-to", makeFakeElement({ value: "" })],
    ["#traffic-filter-request", requestInput],
    ["#traffic-filter-response", makeFakeElement({ value: "" })],
    ["#traffic-filter-profile", makeFakeElement({ value: "all" })],
    ["#traffic-filter-status", statusSelect],
    ["#traffic-refresh", makeFakeElement({ value: "" })],
    ["[data-traffic-menu-toggle][]", []],
    ["[data-traffic-scope][]", []]
  ]));

  let rerenders = 0;
  wireTraffic(root, model, async () => {
    rerenders += 1;
  }, t);

  await requestInput.fire("input");
  await statusSelect.fire("change");

  assert.equal(model.trafficFilters.requestQuery, "example.org");
  assert.equal(model.trafficFilters.status, "blocked");
  assert.ok(rerenders >= 2);
});

test("tab logs: refresh wiring hydrates runtime logs", async () => {
  ensureRuntimeMocks();
  const model = {};
  await hydrateLogsModel(model);
  const html = renderLogs(t, model);
  assert.ok(html.includes("logs-refresh"));

  const refreshButton = makeFakeElement();
  const root = makeFakeRoot(new Map([["#logs-refresh", refreshButton]]));
  let rerenderCalls = 0;
  wireLogs(root, model, async () => {
    rerenderCalls += 1;
  });
  await refreshButton.fire("click");
  assert.ok(Array.isArray(model.runtimeLogs));
  assert.ok(rerenderCalls >= 1);
});

test("tab settings: renders all sub-tabs and hydrates state", async () => {
  ensureRuntimeMocks();
  const model = { profiles: [{ id: "p-1", name: "Profile 1" }] };
  await hydrateSettingsModel(model);
  const html = renderSettings(t, model);
  assert.ok(html.includes('data-settings-tab="general"'));
  assert.ok(html.includes('data-settings-tab="links"'));
  assert.ok(html.includes('data-settings-tab="sync"'));
  assert.ok(html.includes("settings-page"));
});
