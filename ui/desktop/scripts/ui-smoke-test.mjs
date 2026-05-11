import fs from "node:fs";
import path from "node:path";
import { pathToFileURL } from "node:url";

const root = path.resolve(process.cwd(), "web");
const i18nRoot = path.join(root, "i18n");
const featureRegistryPath = path.join(root, "core", "feature-registry.js");

function readJson(file) {
  const raw = fs.readFileSync(file, "utf8").replace(/^\uFEFF/, "");
  return JSON.parse(raw);
}

function assert(condition, message) {
  if (!condition) {
    throw new Error(message);
  }
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
    }
  };
}

function installBrowserMocks() {
  if (!globalThis.__uiSmokeIntervals) {
    globalThis.__uiSmokeIntervals = [];
  }

  globalThis.setInterval = () => {
    const id = globalThis.__uiSmokeIntervals.length + 1;
    globalThis.__uiSmokeIntervals.push(id);
    return id;
  };

  globalThis.clearInterval = () => {};

  if (!globalThis.screen) {
    globalThis.screen = {
      width: 1366,
      height: 768,
      availWidth: 1366,
      availHeight: 728,
      colorDepth: 24,
      pixelDepth: 24
    };
  }

  if (!globalThis.window) {
    globalThis.window = {
      outerWidth: 1366,
      outerHeight: 768,
      innerWidth: 1366,
      innerHeight: 728,
      devicePixelRatio: 1,
      screen: globalThis.screen,
      matchMedia() {
        return {
          matches: false,
          media: "",
          onchange: null,
          addListener() {},
          removeListener() {},
          addEventListener() {},
          removeEventListener() {},
          dispatchEvent() {
            return false;
          }
        };
      }
    };
  }

  if (!globalThis.document) {
    globalThis.document = {
      documentElement: { lang: "en" },
      body: { dataset: {} },
      querySelector() {
        return null;
      },
      querySelectorAll() {
        return [];
      },
      addEventListener() {},
      removeEventListener() {}
    };
  }
}

function makeRootMock() {
  return {
    querySelector() {
      return null;
    },
    querySelectorAll() {
      return [];
    },
    addEventListener() {}
  };
}

function makeFeatureModel() {
  return {
    selectedProfileId: "p1",
    profiles: [{ id: "p1", name: "Profile 1", tags: [] }],
    featureState: {},
    networkTemplates: [],
    networkGlobalRoute: {
      globalVpnEnabled: false,
      blockWithoutVpn: false,
      defaultTemplateId: null
    },
    networkTemplateDraft: {
      templateId: null,
      name: "",
      nodes: []
    },
    extensionLibraryState: {
      autoUpdateEnabled: false,
      items: {}
    },
    trafficEntries: [],
    dnsDrafts: {},
    dnsTemplates: [],
    dnsUiState: {},
    dnsPolicyPresets: {},
    serviceCatalog: { categories: [] },
    shellPreferencesState: {},
    linkRoutingOverview: { globalProfileId: null, supportedTypes: [] },
    launcherUpdateState: {},
    runtimeToolsStatus: [],
    syncOverview: null
  };
}

async function main() {
  installBrowserMocks();
  globalThis.localStorage = globalThis.localStorage ?? makeLocalStorageMock();

  const en = readJson(path.join(i18nRoot, "en", "common.json"));
  const ru = readJson(path.join(i18nRoot, "ru", "common.json"));

  const required = [
    "nav.home",
    "nav.profiles",
    "nav.extensions",
    "nav.security",
    "nav.identity",
    "nav.dns",
    "nav.network",
    "nav.settings",
    "extensions.tableTitle",
    "network.chainTitle",
    "settings.searchProvider",
    "settings.tab.links",
    "settings.tab.sync",
    "links.table.title"
  ];

  for (const key of required) {
    assert(typeof en[key] === "string" && en[key].length > 0, `Missing EN key: ${key}`);
    assert(typeof ru[key] === "string" && ru[key].length > 0, `Missing RU key: ${key}`);
  }

  const registrySrc = fs.readFileSync(featureRegistryPath, "utf8");
  const expectedOrder = [
    "home",
    "extensions",
    "security",
    "identity",
    "dns",
    "network",
    "traffic",
    "settings"
  ];

  for (const key of expectedOrder) {
    assert(registrySrc.includes(`{ key: "${key}"`), `Feature missing in registry: ${key}`);
  }

  const mainSrc = fs.readFileSync(path.join(root, "app", "main.js"), "utf8");
  assert(!mainSrc.includes("quick-launch"), "Legacy quick-launch button should be removed from shell.");
  assert(!mainSrc.includes("new-profile"), "Legacy new-profile shell button should be removed.");

  const css = fs.readFileSync(path.join(root, "styles", "app.css"), "utf8");
  assert(css.includes("#top-frame"), "Top frame style is missing.");
  assert(css.includes("#sidebar-frame"), "Sidebar frame style is missing.");

  const { buildPreset } = await import(pathToFileURL(path.join(root, "core", "catalogs.js")).href);
  const preset = buildPreset("win_7_edge_109", Date.now());
  assert(Number.isInteger(preset.canvas_noise_seed), "Identity preset canvas seed must stay integral.");
  assert(preset.canvas_noise_seed >= 0, "Identity preset canvas seed must stay non-negative for Rust u64 serialization.");

  const t = (key) => key;
  const rerender = async () => {};
  const rootMock = makeRootMock();

  const featureTests = [
    {
      key: "home",
      modulePath: path.join(root, "features", "home", "view.js"),
      renderExport: "renderHome",
      wireExport: "wireHome"
    },
    {
      key: "extensions",
      modulePath: path.join(root, "features", "extensions", "view.js"),
      renderExport: "renderExtensions",
      wireExport: "wireExtensions"
    },
    {
      key: "security",
      modulePath: path.join(root, "features", "security", "view.js"),
      renderExport: "renderSecurity",
      wireExport: "wireSecurity"
    },
    {
      key: "identity",
      modulePath: path.join(root, "features", "identity", "view.js"),
      renderExport: "renderIdentity",
      wireExport: "wireIdentity"
    },
    {
      key: "dns",
      modulePath: path.join(root, "features", "dns", "view.js"),
      renderExport: "renderDns",
      wireExport: "wireDns"
    },
    {
      key: "network",
      modulePath: path.join(root, "features", "network", "view.js"),
      renderExport: "renderNetwork",
      wireExport: "wireNetwork"
    },
    {
      key: "traffic",
      modulePath: path.join(root, "features", "traffic", "view.js"),
      renderExport: "renderTraffic",
      wireExport: "wireTraffic"
    },
    {
      key: "logs",
      modulePath: path.join(root, "features", "diagnostics", "logs-view.js"),
      renderExport: "renderLogs",
      wireExport: "wireLogs"
    },
    {
      key: "settings",
      modulePath: path.join(root, "features", "diagnostics", "settings-view.js"),
      renderExport: "renderSettings",
      wireExport: "wireSettings"
    }
  ];

  for (const feature of featureTests) {
    const mod = await import(pathToFileURL(feature.modulePath).href);
    assert(typeof mod[feature.renderExport] === "function", `Missing render export for ${feature.key}`);
    assert(typeof mod[feature.wireExport] === "function", `Missing wire export for ${feature.key}`);
    const model = makeFeatureModel();
    const html = mod[feature.renderExport](t, model);
    assert(typeof html === "string" && html.length > 0, `Render failed for tab: ${feature.key}`);
    await Promise.resolve(mod[feature.wireExport](rootMock, model, rerender, t));
  }

  const settingsModule = await import(pathToFileURL(path.join(root, "features", "diagnostics", "settings-view.js")).href);
  for (const tab of ["general", "links", "sync"]) {
    const model = makeFeatureModel();
    model.settingsState = {
      activeTab: tab,
      linkTestUrl: "https://example.com",
      syncProfileId: "p1",
      globalLinkProfileDraft: "",
      linkProfileDrafts: {},
      startupProfileDraft: ""
    };
    const html = settingsModule.renderSettings(t, model);
    assert(html.includes(`data-settings-pane="${tab}"`), `Settings tab pane missing: ${tab}`);
    if (tab === "general") {
      assert(html.includes("settings-update-check"), "Settings general tab: update check control missing");
      assert(html.includes("settings-posture-refresh"), "Settings general tab: posture refresh control missing");
    }
    if (tab === "links") {
      assert(html.includes("settings-links-open"), "Settings links tab: open link test control missing");
      assert(html.includes("settings-links-global-apply"), "Settings links tab: global binding apply control missing");
    }
    if (tab === "sync") {
      assert(html.includes("settings-sync-save"), "Settings sync tab: save control missing");
      assert(html.includes("settings-sync-restore"), "Settings sync tab: restore control missing");
    }
  }

  console.log("ui smoke test passed");
}

main();
