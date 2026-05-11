import { APP_VERSION } from "./app-version.js";

const MOCK_STORAGE_KEY = "launcher.mock.profiles.v1";
const MOCK_LINK_ROUTING_KEY = "launcher.mock.link-routing.v1";
const MOCK_DEVICE_POSTURE_KEY = "launcher.mock.device-posture.v1";
const MOCK_SYNC_KEY = "launcher.mock.sync.v1";
const MOCK_UPDATES_KEY = "launcher.mock.updates.v1";
const MOCK_SHELL_PREFS_KEY = "launcher.mock.shell-prefs.v1";
const MOCK_RUNTIME_TOOLS_KEY = "launcher.mock.runtime-tools.v1";

function readMockProfiles() {
  try {
    return JSON.parse(localStorage.getItem(MOCK_STORAGE_KEY) ?? "[]");
  } catch {
    return [];
  }
}

function writeMockProfiles(items) {
  localStorage.setItem(MOCK_STORAGE_KEY, JSON.stringify(items));
}

function readMockLinkRouting() {
  try {
    return JSON.parse(localStorage.getItem(MOCK_LINK_ROUTING_KEY) ?? "{\"globalProfileId\":null,\"typeBindings\":{}}");
  } catch {
    return { globalProfileId: null, typeBindings: {} };
  }
}

function writeMockLinkRouting(value) {
  localStorage.setItem(MOCK_LINK_ROUTING_KEY, JSON.stringify(value));
}

function readMockDevicePosture() {
  try {
    return JSON.parse(localStorage.getItem(MOCK_DEVICE_POSTURE_KEY) ?? "null");
  } catch {
    return null;
  }
}

function writeMockDevicePosture(value) {
  localStorage.setItem(MOCK_DEVICE_POSTURE_KEY, JSON.stringify(value));
}

function readMockSyncStore() {
  try {
    return JSON.parse(localStorage.getItem(MOCK_SYNC_KEY) ?? "{\"controls\":{},\"conflicts\":{},\"snapshots\":{}}");
  } catch {
    return { controls: {}, conflicts: {}, snapshots: {} };
  }
}

function writeMockSyncStore(value) {
  localStorage.setItem(MOCK_SYNC_KEY, JSON.stringify(value));
}

function readMockUpdateState() {
  try {
    return JSON.parse(localStorage.getItem(MOCK_UPDATES_KEY) ?? JSON.stringify({
      currentVersion: APP_VERSION,
      repositoryUrl: "https://github.com/BerkutSolutions/cerbena-browser",
      autoUpdateEnabled: false,
      lastCheckedAt: null,
      latestVersion: null,
      releaseUrl: null,
      hasUpdate: false,
      status: "idle",
      lastError: null,
      stagedVersion: null,
      stagedAssetName: null,
      canAutoApply: false
    }));
  } catch {
    return {
      currentVersion: APP_VERSION,
      repositoryUrl: "https://github.com/BerkutSolutions/cerbena-browser",
      autoUpdateEnabled: false,
      lastCheckedAt: null,
      latestVersion: null,
      releaseUrl: null,
      hasUpdate: false,
      status: "idle",
      lastError: null,
      stagedVersion: null,
      stagedAssetName: null,
      canAutoApply: false
    };
  }
}

function writeMockUpdateState(value) {
  localStorage.setItem(MOCK_UPDATES_KEY, JSON.stringify(value));
}

function readMockShellPreferences() {
  try {
    return JSON.parse(localStorage.getItem(MOCK_SHELL_PREFS_KEY) ?? "{\"checkDefaultBrowserOnStartup\":true,\"defaultBrowserPromptDecided\":false,\"minimizeToTrayEnabled\":false,\"closeToTrayPromptDeclined\":false,\"launchOnSystemStartup\":false,\"startupProfileId\":null,\"isDefaultBrowser\":false,\"launchedFromSystemStartup\":false}");
  } catch {
    return {
      checkDefaultBrowserOnStartup: true,
      defaultBrowserPromptDecided: false,
      minimizeToTrayEnabled: false,
      closeToTrayPromptDeclined: false,
      launchOnSystemStartup: false,
      startupProfileId: null,
      isDefaultBrowser: false,
      launchedFromSystemStartup: false
    };
  }
}

function writeMockShellPreferences(value) {
  localStorage.setItem(MOCK_SHELL_PREFS_KEY, JSON.stringify(value));
}

function readMockRuntimeTools() {
  try {
    return JSON.parse(localStorage.getItem(MOCK_RUNTIME_TOOLS_KEY) ?? "{\"docker\":false,\"chromium\":true,\"ungoogled-chromium\":false,\"firefox-esr\":true,\"librewolf\":true,\"sing-box\":true,\"openvpn\":false,\"amneziawg\":false,\"tor-bundle\":true}");
  } catch {
    return {
      docker: false,
      chromium: true,
      "ungoogled-chromium": false,
      "firefox-esr": true,
      librewolf: true,
      "sing-box": true,
      openvpn: false,
      amneziawg: false,
      "tor-bundle": true
    };
  }
}

function writeMockRuntimeTools(value) {
  localStorage.setItem(MOCK_RUNTIME_TOOLS_KEY, JSON.stringify(value));
}

function nowIso() {
  return new Date().toISOString();
}


export {
  readMockProfiles, writeMockProfiles, readMockLinkRouting, writeMockLinkRouting,
  readMockDevicePosture, writeMockDevicePosture, readMockSyncStore, writeMockSyncStore,
  readMockUpdateState, writeMockUpdateState, readMockShellPreferences, writeMockShellPreferences,
  readMockRuntimeTools, writeMockRuntimeTools, nowIso
};
