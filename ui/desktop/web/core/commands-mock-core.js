import { APP_VERSION } from "./app-version.js";
import { executeMockProfileCommands } from "./commands-mock-core-profiles.js";
import {
  nowIso,
  readMockDevicePosture,
  readMockLinkRouting,
  readMockProfiles,
  readMockRuntimeTools,
  readMockShellPreferences,
  readMockSyncStore,
  readMockUpdateState,
  writeMockDevicePosture,
  writeMockLinkRouting,
  writeMockProfiles,
  writeMockRuntimeTools,
  writeMockShellPreferences,
  writeMockSyncStore,
  writeMockUpdateState
} from "./commands-mock-core-store.js";

export function executeMockCommand(command, args) {
  const request = args?.request ?? {};
  const profiles = readMockProfiles();

  if (command === "list_profiles") return profiles;
  if (command === "validate_profile_modal") return true;

  const profileResult = executeMockProfileCommands(command, request, profiles, { nowIso, writeMockProfiles });
  if (profileResult !== undefined) return profileResult;

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

  if (command === "read_profile_logs") {
    return [
      `[${new Date().toISOString()}][launcher] Mock profile log for ${request.profileId ?? "profile"}`,
      "[live] Runtime log preview is available only in the desktop shell."
    ];
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

  if (command === "get_runtime_tools_status") {
    const tools = readMockRuntimeTools();
    return [
      {
        id: "docker",
        nameKey: "settings.tools.docker",
        status: tools.docker ? "installed" : "missing",
        version: tools.docker ? "29.2.1" : null,
        action: tools.docker ? "none" : "external",
        detailKey: tools.docker ? "settings.tools.detail.dockerReady" : "settings.tools.detail.dockerMissing"
      },
      {
        id: "chromium",
        nameKey: "settings.tools.chromium",
        status: tools.chromium ? "installed" : "missing",
        version: tools.chromium ? APP_VERSION : null,
        action: tools.chromium ? "none" : "internal",
        detailKey: null
      },
      {
        id: "ungoogled-chromium",
        nameKey: "settings.tools.ungoogledChromium",
        status: tools["ungoogled-chromium"] ? "installed" : "missing",
        version: tools["ungoogled-chromium"] ? APP_VERSION : null,
        action: tools["ungoogled-chromium"] ? "none" : "internal",
        detailKey: null
      },
      {
        id: "firefox-esr",
        nameKey: "settings.tools.firefoxEsr",
        status: tools["firefox-esr"] ? "installed" : "missing",
        version: tools["firefox-esr"] ? APP_VERSION : null,
        action: tools["firefox-esr"] ? "none" : "internal",
        detailKey: null
      },
      {
        id: "librewolf",
        nameKey: "settings.tools.librewolf",
        status: tools.librewolf ? "installed" : "missing",
        version: tools.librewolf ? APP_VERSION : null,
        action: tools.librewolf ? "none" : "internal",
        detailKey: null
      },
      {
        id: "sing-box",
        nameKey: "settings.tools.singBox",
        status: tools["sing-box"] ? "installed" : "missing",
        version: tools["sing-box"] ? "1.12.0" : null,
        action: tools["sing-box"] ? "none" : "internal",
        detailKey: null
      },
      {
        id: "openvpn",
        nameKey: "settings.tools.openvpn",
        status: tools.openvpn ? "installed" : "missing",
        version: tools.openvpn ? "2.6.16-I001" : null,
        action: tools.openvpn ? "none" : "internal",
        detailKey: "settings.tools.detail.localOrDocker"
      },
      {
        id: "amneziawg",
        nameKey: "settings.tools.amneziawg",
        status: tools.amneziawg ? "installed" : "missing",
        version: tools.amneziawg ? "2.0.0" : null,
        action: tools.amneziawg ? "none" : "internal",
        detailKey: "settings.tools.detail.localOrDocker"
      },
      {
        id: "tor-bundle",
        nameKey: "settings.tools.torBundle",
        status: tools["tor-bundle"] ? "installed" : "missing",
        version: tools["tor-bundle"] ? "15.0.9" : null,
        action: tools["tor-bundle"] ? "none" : "internal",
        detailKey: null
      }
    ];
  }

  if (command === "install_runtime_tool") {
    const tools = readMockRuntimeTools();
    tools[request.toolId] = true;
    writeMockRuntimeTools(tools);
    return executeMockCommand("get_runtime_tools_status").find((item) => item.id === request.toolId);
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
      launchOnSystemStartup: Boolean(state.launchOnSystemStartup),
      startupProfileId: state.startupProfileId ?? null,
      launchedFromSystemStartup: Boolean(state.launchedFromSystemStartup),
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
        : {}),
      ...(request.launchOnSystemStartup !== undefined
        ? { launchOnSystemStartup: Boolean(request.launchOnSystemStartup) }
        : {}),
      ...(request.startupProfileId !== undefined
        ? { startupProfileId: request.startupProfileId || null }
        : {})
    };
    if (state.minimizeToTrayEnabled) {
      state.closeToTrayPromptDeclined = false;
    }
    writeMockShellPreferences(state);
    const routing = readMockLinkRouting();
    return {
      ...state,
      launchOnSystemStartup: Boolean(state.launchOnSystemStartup),
      startupProfileId: state.startupProfileId ?? null,
      launchedFromSystemStartup: Boolean(state.launchedFromSystemStartup),
      shouldPromptDefaultBrowserPreference: !state.defaultBrowserPromptDecided,
      shouldPromptDefaultLinkProfile: Boolean(state.checkDefaultBrowserOnStartup && state.isDefaultBrowser && !routing.globalProfileId)
    };
  }

  if (
    command === "window_hide_to_tray" ||
    command === "window_restore_from_tray" ||
    command === "confirm_app_exit" ||
    command === "open_external_url"
  ) {
    return true;
  }

  throw new Error("This command requires Tauri runtime.");
}

