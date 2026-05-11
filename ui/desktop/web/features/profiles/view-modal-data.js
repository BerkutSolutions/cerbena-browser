export async function loadModalGeneralDependencies(model, deps) {
  const {
    listExtensionLibrary,
    getServiceCatalog,
    getGlobalSecuritySettings,
    normalizeGlobalSecuritySettings
  } = deps;
  if (!model.extensionLibraryState) {
    const libraryResult = await listExtensionLibrary();
    if (libraryResult.ok) {
      try {
        model.extensionLibraryState = JSON.parse(libraryResult.data || "{}");
      } catch {}
    }
  }
  if (!model.serviceCatalog) {
    const catalogResult = await getServiceCatalog();
    if (catalogResult.ok) {
      try {
        model.serviceCatalog = JSON.parse(catalogResult.data);
      } catch {}
    }
  }
  let globalSecurity = { certificates: [], blocklists: [] };
  const globalSecurityResult = await getGlobalSecuritySettings();
  if (globalSecurityResult.ok) {
    try {
      globalSecurity = normalizeGlobalSecuritySettings(globalSecurityResult.data);
    } catch {}
  }
  return { globalSecurity };
}

export async function loadModalNetworkAndSyncDependencies(existing, deps) {
  const { getNetworkState, getSyncOverview } = deps;
  let profileNetworkState = { payload: null, selectedTemplateId: null, connectionTemplates: [] };
  const networkStateResult = await getNetworkState(existing?.id ?? "");
  if (networkStateResult.ok) {
    try {
      profileNetworkState = JSON.parse(networkStateResult.data || "{}");
    } catch {}
  }
  let syncOverview = null;
  if (existing?.id) {
    const syncResult = await getSyncOverview(existing.id);
    if (syncResult.ok) {
      syncOverview = syncResult.data;
    }
  }
  return { profileNetworkState, syncOverview };
}

export async function loadModalIdentityAndExtensionsDependencies(model, existing, deps) {
  const { listProfileExtensions, getIdentityProfile } = deps;
  let identityPreset = null;
  if (existing?.id) {
    const extensionResult = await listProfileExtensions(existing.id);
    if (extensionResult.ok) {
      try {
        const payload = JSON.parse(extensionResult.data || "{}");
        model.profileExtensionStateMap = model.profileExtensionStateMap ?? {};
        model.profileExtensionStateMap[existing.id] = payload.extensions ?? [];
      } catch {}
    }
  }
  if (existing?.id) {
    const identityResult = await getIdentityProfile(existing.id);
    if (identityResult.ok) {
      identityPreset = identityResult.data ?? null;
    }
  }
  return { identityPreset };
}
