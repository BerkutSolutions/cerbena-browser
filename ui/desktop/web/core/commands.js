import { nextCorrelationId, responseEnvelope } from "./contract.js";

let invokeImpl = async () => {
  throw new Error("Tauri invoke is unavailable in browser preview mode.");
};

if (typeof window !== "undefined" && window.__TAURI__?.core?.invoke) {
  invokeImpl = window.__TAURI__.core.invoke;
}

const MOCK_STORAGE_KEY = "launcher.mock.profiles.v1";
const MOCK_LINK_ROUTING_KEY = "launcher.mock.link-routing.v1";
const MOCK_DEVICE_POSTURE_KEY = "launcher.mock.device-posture.v1";
const MOCK_SYNC_KEY = "launcher.mock.sync.v1";
const MOCK_UPDATES_KEY = "launcher.mock.updates.v1";
const MOCK_SHELL_PREFS_KEY = "launcher.mock.shell-prefs.v1";

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
    return JSON.parse(localStorage.getItem(MOCK_UPDATES_KEY) ?? "{\"currentVersion\":\"1.0.11\",\"repositoryUrl\":\"https://github.com/BerkutSolutions/cerbena-browser\",\"autoUpdateEnabled\":false,\"lastCheckedAt\":null,\"latestVersion\":null,\"releaseUrl\":null,\"hasUpdate\":false,\"status\":\"idle\",\"lastError\":null,\"stagedVersion\":null,\"stagedAssetName\":null,\"canAutoApply\":false}");
  } catch {
    return {
      currentVersion: "1.0.11",
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
    return JSON.parse(localStorage.getItem(MOCK_SHELL_PREFS_KEY) ?? "{\"checkDefaultBrowserOnStartup\":true,\"defaultBrowserPromptDecided\":false,\"minimizeToTrayEnabled\":false,\"closeToTrayPromptDeclined\":false,\"isDefaultBrowser\":false}");
  } catch {
    return {
      checkDefaultBrowserOnStartup: true,
      defaultBrowserPromptDecided: false,
      minimizeToTrayEnabled: false,
      closeToTrayPromptDeclined: false,
      isDefaultBrowser: false
    };
  }
}

function writeMockShellPreferences(value) {
  localStorage.setItem(MOCK_SHELL_PREFS_KEY, JSON.stringify(value));
}

function nowIso() {
  return new Date().toISOString();
}

function mockProfileCommand(command, args) {
  const request = args?.request ?? {};
  const profiles = readMockProfiles();

  if (command === "list_profiles") return profiles;
  if (command === "validate_profile_modal") return true;

  if (command === "create_profile") {
    const id = crypto.randomUUID();
    const profile = {
      id,
      name: request.name ?? "Profile",
      description: request.description ?? null,
      tags: request.tags ?? [],
      state: "ready",
      engine: request.engine ?? "wayfern",
      default_start_page: request.defaultStartPage ?? null,
      default_search_provider: request.defaultSearchProvider ?? "duckduckgo",
      ephemeral_mode: Boolean(request.ephemeralMode),
      password_lock_enabled: Boolean(request.passwordLockEnabled),
      panic_frame_enabled: Boolean(request.panicFrameEnabled),
      panic_frame_color: request.panicFrameColor ?? null,
      panic_protected_sites: request.panicProtectedSites ?? [],
      ephemeral_retain_paths: request.ephemeralRetainPaths ?? [],
      created_at: nowIso(),
      updated_at: nowIso()
    };
    profiles.push(profile);
    writeMockProfiles(profiles);
    return profile;
  }

  if (command === "update_profile") {
    const index = profiles.findIndex((p) => p.id === request.profileId);
    if (index < 0) throw new Error("profile not found");
    profiles[index] = {
      ...profiles[index],
      ...(request.name ? { name: request.name } : {}),
      ...(request.description !== undefined ? { description: request.description } : {}),
      ...(request.tags ? { tags: request.tags } : {}),
      ...(request.defaultStartPage !== undefined ? { default_start_page: request.defaultStartPage } : {}),
      ...(request.defaultSearchProvider !== undefined ? { default_search_provider: request.defaultSearchProvider } : {}),
      ...(request.ephemeralMode !== undefined ? { ephemeral_mode: request.ephemeralMode } : {}),
      ...(request.passwordLockEnabled !== undefined ? { password_lock_enabled: request.passwordLockEnabled } : {}),
      ...(request.panicFrameEnabled !== undefined ? { panic_frame_enabled: request.panicFrameEnabled } : {}),
      ...(request.panicFrameColor !== undefined ? { panic_frame_color: request.panicFrameColor } : {}),
      ...(request.panicProtectedSites !== undefined ? { panic_protected_sites: request.panicProtectedSites } : {}),
      ...(request.ephemeralRetainPaths ? { ephemeral_retain_paths: request.ephemeralRetainPaths } : {}),
      updated_at: nowIso()
    };
    writeMockProfiles(profiles);
    return profiles[index];
  }

  if (command === "delete_profile") {
    writeMockProfiles(profiles.filter((p) => p.id !== request.profileId));
    return true;
  }

  if (command === "duplicate_profile") {
    const source = profiles.find((p) => p.id === request.profileId);
    if (!source) throw new Error("profile not found");
    const duplicate = {
      ...source,
      id: crypto.randomUUID(),
      name: request.newName || `${source.name}-copy`,
      state: "ready",
      created_at: nowIso(),
      updated_at: nowIso()
    };
    profiles.push(duplicate);
    writeMockProfiles(profiles);
    return duplicate;
  }

  if (command === "launch_profile" || command === "stop_profile") {
    const index = profiles.findIndex((p) => p.id === request.profileId);
    if (index < 0) throw new Error("profile not found");
    profiles[index].state = command === "launch_profile" ? "running" : "stopped";
    profiles[index].updated_at = nowIso();
    writeMockProfiles(profiles);
    return profiles[index];
  }

  if (command === "set_profile_password" || command === "unlock_profile") return true;
  if (command === "pick_certificate_files") return [];
  if (command === "cancel_engine_download") return true;

  if (command === "save_sync_controls") {
    const sync = readMockSyncStore();
    sync.controls[request.profileId] = request.model;
    writeMockSyncStore(sync);
    return true;
  }

  if (command === "get_sync_overview") {
    const sync = readMockSyncStore();
    return {
      profileId: args.profileId,
      controls: sync.controls[args.profileId] ?? {
        server: { server_url: "", key_id: "", sync_enabled: false },
        status: { level: "warning", message_key: "sync.disabled", last_sync_unix_ms: null },
        conflicts: [],
        can_backup: true,
        can_restore: true
      },
      conflicts: sync.conflicts[args.profileId] ?? [],
      snapshots: sync.snapshots[args.profileId] ?? []
    };
  }

  if (command === "create_backup_snapshot") {
    const sync = readMockSyncStore();
    const profileId = request.profileId;
    const snapshot = {
      snapshot_id: `snap-${profileId}-${Date.now()}`,
      profile_id: profileId,
      created_at_unix_ms: Date.now(),
      encrypted_blob_b64: btoa(JSON.stringify({ profileId, ts: Date.now() })),
      integrity_sha256_hex: "mock"
    };
    sync.snapshots[profileId] = [...(sync.snapshots[profileId] ?? []), snapshot];
    writeMockSyncStore(sync);
    return snapshot;
  }

  if (command === "restore_snapshot") {
    return {
      restored_snapshot_id: request.request?.snapshotId ?? request.request?.snapshot_id ?? "mock",
      restored_profile_id: request.request?.profileId ?? request.request?.profile_id ?? "",
      restored_items: 1,
      skipped_items: 0
    };
  }

  if (command === "panic_wipe_profile") {
    const index = profiles.findIndex((p) => p.id === request.profileId);
    if (index >= 0) {
      profiles[index].state = "stopped";
      profiles[index].updated_at = nowIso();
      writeMockProfiles(profiles);
    }
    return JSON.stringify({ profileId: request.profileId, wipedPaths: 1, mode: request.mode });
  }

  if (command === "panic_frame_show_menu" || command === "panic_frame_hide_menu") {
    return true;
  }

  if (command === "get_link_routing_overview") {
    const routing = readMockLinkRouting();
    const supported = [
      ["http", "links.type.http", true],
      ["https", "links.type.https", true],
      ["ftp", "links.type.ftp", false],
      ["mailto", "links.type.mailto", false],
      ["irc", "links.type.irc", false],
      ["mms", "links.type.mms", false],
      ["news", "links.type.news", false],
      ["nntp", "links.type.nntp", false],
      ["sms", "links.type.sms", false],
      ["smsto", "links.type.smsto", false],
      ["snews", "links.type.snews", false],
      ["tel", "links.type.tel", false],
      ["urn", "links.type.urn", false],
      ["webcal", "links.type.webcal", false],
      ["magnet", "links.type.magnet", false],
      ["tg", "links.type.tg", false],
      ["discord", "links.type.discord", false],
      ["slack", "links.type.slack", false],
      ["zoommtg", "links.type.zoommtg", false],
      ["file:mht", "links.type.fileMht", false],
      ["file:mhtml", "links.type.fileMhtml", false],
      ["file:pdf", "links.type.filePdf", false],
      ["file:shtml", "links.type.fileShtml", false],
      ["file:svg", "links.type.fileSvg", false],
      ["file:xhtml", "links.type.fileXhtml", false]
    ];
    return {
      globalProfileId: routing.globalProfileId ?? null,
      supportedTypes: supported.map(([linkType, labelKey, allowGlobalDefault]) => ({
        linkType,
        labelKey,
        profileId: routing.typeBindings?.[linkType] ?? null,
        usesGlobalDefault: allowGlobalDefault && !routing.typeBindings?.[linkType] && Boolean(routing.globalProfileId),
        allowGlobalDefault
      }))
    };
  }

  if (command === "set_default_profile_for_links") {
    const routing = readMockLinkRouting();
    routing.globalProfileId = request.profileId ?? null;
    writeMockLinkRouting(routing);
    return true;
  }

  if (command === "clear_default_profile_for_links") {
    const routing = readMockLinkRouting();
    routing.globalProfileId = null;
    writeMockLinkRouting(routing);
    return true;
  }

  if (command === "save_link_type_profile_binding") {
    const routing = readMockLinkRouting();
    routing.typeBindings = routing.typeBindings ?? {};
    routing.typeBindings[request.linkType] = request.profileId;
    writeMockLinkRouting(routing);
    return true;
  }

  if (command === "remove_link_type_profile_binding") {
    const routing = readMockLinkRouting();
    if (routing.typeBindings) {
      delete routing.typeBindings[request.linkType];
    }
    writeMockLinkRouting(routing);
    return true;
  }

  if (command === "dispatch_external_link") {
    const url = String(request.url ?? "").trim();
    if (!url) throw new Error("link URL is required");
    const routing = readMockLinkRouting();
    const schemeMatch = url.match(/^([a-z0-9+.-]+):/i);
    let linkType = (schemeMatch?.[1] ?? "https").toLowerCase();
    if (linkType === "file") {
      const fileMatch = url.match(/\.([a-z0-9]+)$/i);
      const ext = (fileMatch?.[1] ?? "").toLowerCase();
      const normalizedExt = ext === "xhy" || ext === "xht" ? "xhtml" : ext;
      linkType = `file:${normalizedExt}`;
    }
    const allowGlobalDefault = linkType === "http" || linkType === "https";
    const targetProfileId = routing.typeBindings?.[linkType] ?? (allowGlobalDefault ? routing.globalProfileId ?? null : null);
    return {
      status: targetProfileId ? "resolved" : "prompt",
      linkType,
      url,
      targetProfileId,
      resolutionScope: routing.typeBindings?.[linkType] ? "type" : (allowGlobalDefault && routing.globalProfileId ? "global" : null)
    };
  }

  if (command === "consume_pending_external_link") {
    return null;
  }

  if (command === "get_wayfern_terms_status") {
    return { pendingProfileIds: [] };
  }

  if (command === "get_device_posture_report" || command === "refresh_device_posture_report") {
    const report = {
      reportId: "mock-posture",
      checkedAtEpochMs: Date.now(),
      hostName: "preview-host",
      exePath: "preview",
      status: "healthy",
      reaction: "allow",
      findings: []
    };
    writeMockDevicePosture(report);
    return command === "get_device_posture_report" ? (readMockDevicePosture() ?? report) : report;
  }

  if (command === "get_launcher_update_state") {
    return readMockUpdateState();
  }

  if (command === "set_launcher_auto_update") {
    const state = readMockUpdateState();
    state.autoUpdateEnabled = Boolean(args.enabled);
    writeMockUpdateState(state);
    return state;
  }

  if (command === "check_launcher_updates") {
    const state = readMockUpdateState();
    state.lastCheckedAt = nowIso();
    state.latestVersion = state.currentVersion;
    state.hasUpdate = false;
    state.status = "up_to_date";
    state.lastError = null;
    writeMockUpdateState(state);
    return state;
  }

  if (command === "get_shell_preferences_state") {
    const state = readMockShellPreferences();
    const routing = readMockLinkRouting();
    return {
      ...state,
      shouldPromptDefaultBrowserPreference: !state.defaultBrowserPromptDecided,
      shouldPromptDefaultLinkProfile: Boolean(state.checkDefaultBrowserOnStartup && state.isDefaultBrowser && !routing.globalProfileId)
    };
  }

  if (command === "save_shell_preferences") {
    const state = {
      ...readMockShellPreferences(),
      ...(request.checkDefaultBrowserOnStartup !== undefined
        ? { checkDefaultBrowserOnStartup: Boolean(request.checkDefaultBrowserOnStartup) }
        : {}),
      ...(request.defaultBrowserPromptDecided !== undefined
        ? { defaultBrowserPromptDecided: Boolean(request.defaultBrowserPromptDecided) }
        : {}),
      ...(request.minimizeToTrayEnabled !== undefined
        ? { minimizeToTrayEnabled: Boolean(request.minimizeToTrayEnabled) }
        : {}),
      ...(request.closeToTrayPromptDeclined !== undefined
        ? { closeToTrayPromptDeclined: Boolean(request.closeToTrayPromptDeclined) }
        : {})
    };
    if (state.minimizeToTrayEnabled) {
      state.closeToTrayPromptDeclined = false;
    }
    writeMockShellPreferences(state);
    const routing = readMockLinkRouting();
    return {
      ...state,
      shouldPromptDefaultBrowserPreference: !state.defaultBrowserPromptDecided,
      shouldPromptDefaultLinkProfile: Boolean(state.checkDefaultBrowserOnStartup && state.isDefaultBrowser && !routing.globalProfileId)
    };
  }

  if (
    command === "window_hide_to_tray" ||
    command === "window_restore_from_tray" ||
    command === "confirm_app_exit"
  ) {
    return true;
  }

  throw new Error("This command requires Tauri runtime.");
}

export async function callCommand(command, args = {}) {
  const correlationId = nextCorrelationId();
  try {
    const response = await invokeImpl(command, {
      ...args,
      correlationId
    });
    return responseEnvelope(true, response.data, response.messageKey ?? "command.success", correlationId);
  } catch (error) {
    const message = String(error);
    if (message.includes("Tauri invoke is unavailable")) {
      try {
        const data = mockProfileCommand(command, args);
        return responseEnvelope(true, data, "command.mock.success", correlationId);
      } catch (mockError) {
        return responseEnvelope(false, { error: String(mockError) }, "command.mock.failed", correlationId);
      }
    }

    return responseEnvelope(false, { error: String(error) }, "command.failed", correlationId);
  }
}
