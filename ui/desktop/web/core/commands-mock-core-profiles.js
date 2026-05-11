export function executeMockProfileCommands(command, request, profiles, io){
  const { nowIso, writeMockProfiles } = io;
  if (command === "create_profile") {
    const id = crypto.randomUUID();
    const profile = {
      id,
      name: request.name ?? "Profile",
      description: request.description ?? null,
      tags: request.tags ?? [],
      state: "ready",
      engine: request.engine ?? "chromium",
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


  return undefined;
}
