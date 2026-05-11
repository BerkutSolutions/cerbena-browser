import { resolveProfileSaveErrorImpl } from "./view-wire-security.js";
import { askConfirmPrompt } from "./view-actions.js";
import { blockedServicesToPairs, saveProfileDnsDraft } from "../dns/store.js";
import {
  buildManualPreset,
  cloneIdentityPreset,
  firstTemplateKeyForTemplatePlatform,
  normalizeAutoPlatform,
  normalizeTemplatePlatform
} from "../identity/shared.js";
import { generateAutoPreset } from "../identity/api.js";
import { saveDnsPolicy, saveNetworkSandboxProfileSettings, saveVpnProxyPolicy } from "../network/api.js";
import { buildRoutePolicyPayload } from "./view-route-sandbox.js";
import { buildProfileSecuritySaversImpl } from "./view-wire-security.js";

function isChromiumFamilyEngine(engine) {
  return ["chromium", "wayfern", "ungoogled_chromium", "camoufox"].includes(String(engine ?? "").toLowerCase());
}

export function attachProfileModalSubmitHandler(ctx) {
  const {
    form,
    t,
    model,
    existing,
    rerender,
    setNotice,
    closeModalOverlay,
    overlay,
    tagsState,
    extensionState,
    certificateState,
    blocklistState,
    allowState,
    denyState,
    dnsDraft,
    blocklistItems,
    globalSecurity,
    syncOverview,
    profileNetworkState,
    identityUiState,
    identityTemplates,
    identityPresetState,
    initialSandboxMode,
    buildRealPreset,
    validateProfileModal,
    createProfile,
    updateProfile,
    saveSyncControls,
    saveIdentityProfile,
    saveProfileExtensions,
    saveGlobalSecuritySettings,
    syncManagedCertificateAssignments,
    buildGlobalSecuritySaveRequest,
    setProfilePassword,
    hydrateProfilesModel
  } = ctx;

  form.addEventListener("submit", async (event) => {
    event.preventDefault();
    const baseTags = tagsState.slice();
    const tags = baseTags.filter((x) => !x.startsWith("policy:")
      && !x.startsWith("dns-template:")
      && !x.startsWith("ext:")
      && !x.startsWith("ext-disabled:")
      && !x.startsWith("cert-id:")
      && !x.startsWith("cert:")
      && x !== "ext-system-access"
      && x !== "ext-keepassxc"
      && x !== "ext-launch-disabled");
    tags.push(`policy:${form.policyLevel.value}`);
    if (form.dnsMode.value === "custom" && form.dnsTemplateId.value) {
      tags.push(`dns-template:${form.dnsTemplateId.value}`);
    }
    tags.push(...certificateState.filter((item) => item.kind === "id").map((item) => `cert-id:${item.value}`));
    tags.push(...certificateState.filter((item) => item.kind === "path").map((item) => `cert:${item.value}`));
    if (form.disableExtensionsLaunch.checked && extensionState.enabled.length && !form.allowKeepassxc.checked) {
      setNotice(model, "error", t("profile.security.disableExtensionsLaunchBlocked"));
      rerender();
      return;
    }
    if (form.disableExtensionsLaunch.checked) tags.push("ext-launch-disabled");
    if (form.allowSystemAccess.checked) {
      const accepted = await askConfirmPrompt(t, t("profile.security.allowSystemAccess"), t("profile.security.systemAccessWarning"));
      if (!accepted) return;
      tags.push("ext-system-access");
    }
    if (form.allowKeepassxc.checked) tags.push("ext-keepassxc");
    const preservedLockedAppTags = (existing?.tags ?? []).filter((tag) =>
      tag.startsWith("locked-app:") && tag !== "locked-app:custom"
    );
    tags.push(...preservedLockedAppTags);
    if (isChromiumFamilyEngine(form.engine.value) && form.singlePageMode?.checked) tags.push("locked-app:custom");

    const defaultStartPageValue = String(form.defaultStartPage.value ?? "").trim();
    if (isChromiumFamilyEngine(form.engine.value) && form.singlePageMode?.checked) {
      const normalizedStartPage = /^[a-zA-Z][a-zA-Z0-9+.-]*:/.test(defaultStartPageValue)
        ? defaultStartPageValue
        : `https://${defaultStartPageValue}`;
      let startUrl = null;
      try {
        startUrl = new URL(normalizedStartPage);
      } catch {
        startUrl = null;
      }
      if (!startUrl?.host) {
        setNotice(model, "error", t("profile.field.singlePageInvalidUrl"));
        rerender();
        return;
      }
    }

    const payload = {
      name: form.name.value,
      description: form.description.value || null,
      tags,
      engine: form.engine.value,
      defaultStartPage: defaultStartPageValue || null,
      defaultSearchProvider: form.singlePageMode?.checked ? null : (form.defaultSearchProvider.value || null),
      ephemeralMode: form.ephemeral.checked,
      passwordLockEnabled: form.passwordLock.checked,
      panicFrameEnabled: form.panicFrameEnabled.checked,
      panicFrameColor: form.panicFrameEnabled.checked ? (form.panicFrameColor.value || "#ff8652") : null,
      panicProtectedSites: existing?.panic_protected_sites ?? [],
      ephemeralRetainPaths: []
    };
    if (payload.passwordLockEnabled) {
      const passwordValue = String(form.profilePassword.value ?? "");
      const passwordConfirm = String(form.profilePasswordConfirm.value ?? "");
      if (!passwordValue || !passwordConfirm) {
        setNotice(model, "error", t("profile.security.passwordRequired"));
        rerender();
        return;
      }
      if (passwordValue !== passwordConfirm) {
        setNotice(model, "error", t("profile.security.passwordMismatch"));
        rerender();
        return;
      }
    }

    const identityModeValue = form.identityMode.value === "real"
      ? "real"
      : form.identityMode.value === "auto" ? "auto" : "manual";
    const identityPlatformTarget = identityModeValue === "auto"
      ? normalizeAutoPlatform(form.platformTarget.value || identityUiState.autoPlatform)
      : null;
    const identityTemplateKey = identityModeValue === "manual"
      ? (form.identityTemplate.value || identityUiState.templateKey || firstTemplateKeyForTemplatePlatform(identityUiState.templatePlatform))
      : null;

    const validate = await validateProfileModal({
      general: {
        name: payload.name,
        description: payload.description,
        tags: payload.tags,
        default_start_page: payload.defaultStartPage,
        default_search_provider: payload.defaultSearchProvider
      },
      identity: { mode: identityModeValue, platform_target: identityPlatformTarget, template_key: identityTemplateKey },
      vpn_proxy: { route_mode: form.profileRouteMode.value, proxy_url: null, vpn_profile_ref: form.profileRouteTemplateId.value || null },
      dns: {
        resolver_mode: form.dnsMode.value,
        servers: form.dnsServers.value.split(",").map((v) => v.trim()).filter(Boolean),
        blocklists: [...blocklistState],
        allow_domains: allowState
      },
      extensions: { enabled_extension_ids: extensionState.enabled },
      security: { password_lock_enabled: form.passwordLock.checked, ephemeral_mode: form.ephemeral.checked, ephemeral_retain_paths: [] },
      sync: { server: form.syncServer.value || null, key_id: form.syncKey.value || null },
      advanced: { launch_hook: form.launchHook.value || null }
    });
    if (!validate.ok) {
      setNotice(model, "error", `${t("profile.modal.validationError")}: ${validate.data.error}`);
      rerender();
      return;
    }

    let identityPresetToSave = null;
    try {
      if (identityModeValue === "real") {
        identityPresetToSave = buildRealPreset(Date.now());
      } else if (identityModeValue === "auto") {
        const generatedPreset = await generateAutoPreset(identityPlatformTarget, Date.now());
        if (!generatedPreset.ok) throw new Error(String(generatedPreset.data.error));
        identityPresetToSave = generatedPreset.data;
      } else {
        if (identityTemplateKey && identityTemplateKey !== identityUiState.templateKey) {
          ctx.identityPresetState = buildManualPreset(identityTemplateKey, Date.now());
        }
        identityUiState.mode = "manual";
        identityUiState.templateKey = identityTemplateKey || identityUiState.templateKey;
        identityUiState.templatePlatform = normalizeTemplatePlatform(
          identityTemplates.find((item) => item.key === identityUiState.templateKey)?.platformFamily ?? identityUiState.templatePlatform
        );
        identityPresetToSave = cloneIdentityPreset(ctx.identityPresetState ?? identityPresetState ?? buildManualPreset(identityUiState.templateKey, Date.now()));
        identityPresetToSave.mode = "manual";
        identityPresetToSave.display_name = String(form.identityDisplayName?.value ?? "").trim() || null;
      }
    } catch (error) {
      setNotice(model, "error", String(error));
      rerender();
      return;
    }

    const dnsPayload = {
      profile_id: existing?.id ?? "",
      dns_config: {
        mode: form.dnsMode.value,
        servers: form.dnsServers.value.split(",").map((v) => v.trim()).filter(Boolean),
        doh_url: null,
        dot_server_name: null
      },
      selected_blocklists: blocklistItems.filter((item) => blocklistState.has(item.id)).map((item) => ({
        list_id: item.id,
        domains: item.domains ?? [],
        updated_at_epoch: Math.floor(Date.now() / 1000)
      })),
      selected_services: blockedServicesToPairs(dnsDraft.blockedServices ?? {}),
      domain_allowlist: allowState,
      domain_denylist: denyState,
      domain_exceptions: []
    };
    const routeMode = form.profileRouteMode.value;
    const routeTemplateId = form.profileRouteTemplateId.value || null;
    const selectedRouteTemplate = (profileNetworkState.connectionTemplates ?? []).find((item) => item.id === routeTemplateId) ?? null;
    let routePayload = null;
    try {
      const killSwitchEnabled = routeMode === "direct" ? false : Boolean(form.profileKillSwitch?.checked);
      routePayload = buildRoutePolicyPayload(routeMode, selectedRouteTemplate, killSwitchEnabled, t);
    } catch (error) {
      setNotice(model, "error", String(error));
      rerender();
      return;
    }
    const saveRoutePolicy = async (profileId) =>
      saveVpnProxyPolicy(profileId, routePayload, routeMode === "direct" ? null : routeTemplateId);
    const saveSandboxPolicy = async (profileId) => {
      if (routeMode === "direct" || !routeTemplateId) return { ok: true };
      const selectedMode = overlay.querySelector("#profile-sandbox-mode")?.value || null;
      if (!selectedMode || selectedMode === initialSandboxMode) return { ok: true };
      return saveNetworkSandboxProfileSettings(profileId, selectedMode);
    };
    const saveSyncPolicy = async (profileId) => {
      const serverUrl = form.syncServer.value.trim();
      const keyId = form.syncKey.value.trim();
      const enabled = Boolean(form.syncEnabled?.checked);
      const syncModel = {
        server: { server_url: serverUrl, key_id: keyId, sync_enabled: enabled },
        status: {
          level: enabled ? "healthy" : "warning",
          message_key: enabled ? "sync.healthy" : "sync.disabled",
          last_sync_unix_ms: syncOverview?.controls?.status?.last_sync_unix_ms ?? null
        },
        conflicts: syncOverview?.conflicts ?? [],
        can_backup: true,
        can_restore: true
      };
      return saveSyncControls(profileId, syncModel);
    };
    const saveIdentityPolicy = async (profileId) => saveIdentityProfile(profileId, identityPresetToSave);
    const saveProfileExtensionState = async (profileId) => saveProfileExtensions(
      profileId,
      [
        ...extensionState.enabled.map((libraryItemId) => ({ libraryItemId, enabled: true })),
        ...extensionState.disabled.map((libraryItemId) => ({ libraryItemId, enabled: false }))
      ]
    );
    const { saveProfilePassword, saveManagedCertificates, getSecurityState } = buildProfileSecuritySaversImpl({
      payload,
      form,
      globalSecurity,
      certificateState,
      syncManagedCertificateAssignments,
      buildGlobalSecuritySaveRequest,
      saveGlobalSecuritySettings,
      setProfilePassword
    });

    const processSave = async (profileId, successMessage) => {
      const extensionResult = await saveProfileExtensionState(profileId);
      const certificateResult = await saveManagedCertificates(profileId);
      dnsPayload.profile_id = profileId;
      saveProfileDnsDraft(profileId, {
        ...dnsDraft,
        mode: dnsPayload.dns_config.mode,
        servers: dnsPayload.dns_config.servers.join(","),
        allowlist: allowState.join(","),
        denylist: denyState.join(","),
        selectedBlocklists: [...blocklistState]
      });
      const dnsResult = await saveDnsPolicy(profileId, dnsPayload);
      const routeResult = await saveRoutePolicy(profileId);
      const sandboxResult = await saveSandboxPolicy(profileId);
      const syncResult = await saveSyncPolicy(profileId);
      const identityResult = await saveIdentityPolicy(profileId);
      const passwordResult = await saveProfilePassword(profileId);
      if (extensionResult.ok && certificateResult.ok && dnsResult.ok && routeResult.ok && sandboxResult.ok && syncResult.ok && identityResult.ok && passwordResult.ok) {
        setNotice(model, "success", successMessage);
      } else {
        setNotice(model, "error", resolveProfileSaveErrorImpl(t, {
          extensionResult,
          certificateResult,
          dnsResult,
          routeResult,
          sandboxResult,
          syncResult,
          identityResult,
          passwordResult
        }));
      }
      return getSecurityState();
    };

    if (existing) {
      const engineChanged = String(existing.engine ?? "chromium") !== String(form.engine.value ?? "chromium");
      const updateResult = await updateProfile({
        profileId: existing.id,
        name: payload.name,
        description: payload.description,
        tags: payload.tags,
        engine: form.engine.value,
        defaultStartPage: payload.defaultStartPage,
        defaultSearchProvider: payload.defaultSearchProvider,
        ephemeralMode: payload.ephemeralMode,
        passwordLockEnabled: payload.passwordLockEnabled,
        panicFrameEnabled: payload.panicFrameEnabled,
        panicFrameColor: payload.panicFrameColor,
        panicProtectedSites: payload.panicProtectedSites,
        ephemeralRetainPaths: payload.ephemeralRetainPaths,
        expectedUpdatedAt: existing.updated_at
      });
      if (updateResult.ok) {
        ctx.globalSecurity = await processSave(existing.id, engineChanged ? t("profile.runtime.engineChangedReset") : t("profile.runtime.appliedNow"));
      } else {
        setNotice(model, "error", String(updateResult.data.error));
      }
    } else {
      const createResult = await createProfile(payload);
      if (createResult.ok) {
        ctx.globalSecurity = await processSave(createResult.data.id, t("profile.create.success"));
      } else {
        setNotice(model, "error", String(createResult.data.error));
      }
    }

    closeModalOverlay(overlay, async () => {
      await hydrateProfilesModel(model);
      rerender();
    });
  });
}
