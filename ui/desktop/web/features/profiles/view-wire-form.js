import { askConfirmPrompt } from "./view-actions.js";
import { applyPolicyPresetToDraft } from "../dns/policy-store.js";
import {
  buildManualPreset,
  cloneIdentityPreset,
  firstTemplateKeyForTemplatePlatform,
  normalizeAutoPlatform,
  normalizeTemplatePlatform
} from "../identity/shared.js";
import { wireTagPicker, uniqueTags } from "../../core/tag-picker.js";
import { saveDnsPolicy, saveNetworkSandboxProfileSettings, saveVpnProxyPolicy } from "../network/api.js";
import { hasAssignedProfileCertificates, makeUniqueId, slugId } from "./view-helpers.js";
import { attachProfileModalSubmitHandler } from "./view-wire-form-submit.js";
import { wireIdentityExtensionsAdvancedTab } from "./view-wire-form-identity-extensions.js";
import { wireRouteNetworkSecurityTab } from "./view-wire-form-route-network-security.js";

function domainStatusIcon(type) {
  if (type === "allow") {
    return `<svg viewBox="0 0 16 16" aria-hidden="true"><path d="M3.5 8.5 6.5 11.5 12.5 4.5" fill="none" stroke="currentColor" stroke-width="1.7" stroke-linecap="round" stroke-linejoin="round"/></svg>`;
  }
  return `<svg viewBox="0 0 16 16" aria-hidden="true"><path d="M4 4 12 12M12 4 4 12" fill="none" stroke="currentColor" stroke-width="1.7" stroke-linecap="round"/></svg>`;
}

function domainStatusLabel(type, t) {
  return type === "allow" ? t("profile.security.domainAllowed") : t("profile.security.domainBlocked");
}

function buildDomainEntries(allowDomains, denyDomains) {
  const allowed = (allowDomains ?? []).map((domain) => ({ domain, type: "allow" }));
  const denied = (denyDomains ?? []).map((domain) => ({ domain, type: "deny" }));
  return [...denied, ...allowed];
}

function extensionDisplayName(model, extensionId) {
  return model.extensionLibraryState?.items?.[extensionId]?.display_name
    ?? model.extensionLibraryState?.items?.[extensionId]?.name
    ?? extensionId;
}

export async function openProfileModalFormImpl(root, model, rerender, t, existing, deps) {
  const {
    loadModalGeneralDependencies,
    listExtensionLibrary,
    getServiceCatalog,
    getGlobalSecuritySettings,
    normalizeGlobalSecuritySettings,
    loadProfileDnsDraft,
    loadModalNetworkAndSyncDependencies,
    getNetworkState,
    getSyncOverview,
    loadModalIdentityAndExtensionsDependencies,
    listProfileExtensions,
    getIdentityProfile,
    buildRealPreset,
    renderProfileModalHtml,
    mergedProfileExtensions,
    profileSecurityFlags,
    certificateEntriesForProfile,
    loadPolicyPresets,
    summarizePolicyPreset,
    listIdentityTemplates,
    listIdentityPlatforms,
    listIdentityTemplatePlatforms,
    inferIdentityUiState,
    routeTemplateOptions,
    globalRouteNoticeHtml,
    buildTagPickerMarkup,
    collectProfileTags,
    profileTags,
    dnsTemplateOptions,
    globalBlocklistOptions,
    buildDomainEntries,
    eyeIcon,
    extensionLibraryOptions,
    option,
    escapeHtml,
    DOMAIN_OPTIONS,
    templateSummaryLabel,
    templateDropdownOptionsHtml,
    templateInputValue,
    searchOptions,
    normalizeProfileRouteMode,
    showModalOverlay,
    previewNetworkSandboxSettings,
    renderProfileSandboxFrame,
    saveProfileExtensions,
    validateProfileModal,
    createProfile,
    updateProfile,
    setProfilePassword,
    saveIdentityProfile,
    saveSyncControls,
    saveGlobalSecuritySettings,
    buildGlobalSecuritySaveRequest,
    syncManagedCertificateAssignments,
    pickCertificateFiles,
    hydrateProfilesModel,
    setNotice,
    closeModalOverlay
  } = deps;

  let { globalSecurity } = await loadModalGeneralDependencies(model, {
    listExtensionLibrary,
    getServiceCatalog,
    getGlobalSecuritySettings,
    normalizeGlobalSecuritySettings
  });
  const globalSecurityRef = { current: globalSecurity };

  const dnsDraftKey = existing?.id ?? "create-profile";
  const dnsDraft = loadProfileDnsDraft(dnsDraftKey, model.serviceCatalog);
  const { profileNetworkState, syncOverview } = await loadModalNetworkAndSyncDependencies(existing, {
    getNetworkState,
    getSyncOverview
  });
  let { identityPreset } = await loadModalIdentityAndExtensionsDependencies(model, existing, {
    listProfileExtensions,
    getIdentityProfile
  });
  if (!identityPreset) identityPreset = buildRealPreset();

  document.body.insertAdjacentHTML(
    "beforeend",
    renderProfileModalHtml(t, existing, dnsDraft, globalSecurityRef.current, model, profileNetworkState, syncOverview, identityPreset, {
      mergedProfileExtensions,
      profileSecurityFlags,
      certificateEntriesForProfile,
      loadPolicyPresets,
      summarizePolicyPreset,
      listIdentityTemplates,
      listIdentityPlatforms,
      listIdentityTemplatePlatforms,
      inferIdentityUiState,
      buildRealPreset,
      routeTemplateOptions,
      globalRouteNoticeHtml,
      buildTagPickerMarkup,
      collectProfileTags,
      profileTags,
      dnsTemplateOptions,
      globalBlocklistOptions,
      buildDomainEntries,
      eyeIcon,
      extensionLibraryOptions,
      option,
      escapeHtml,
      DOMAIN_OPTIONS,
      templateSummaryLabel,
      templateDropdownOptionsHtml,
      templateInputValue,
      searchOptions,
      normalizeProfileRouteMode
    })
  );

  const overlay = document.body.querySelector("#profile-modal-overlay");
  showModalOverlay(overlay);
  const form = document.body.querySelector("#profile-form");
  let dirty = false;
  const markDirty = () => { dirty = true; };

  const identityExtState = wireIdentityExtensionsAdvancedTab({
    overlay,
    t,
    model,
    existing,
    buildRealPreset,
    inferIdentityUiState,
    listIdentityTemplates,
    templateDropdownOptionsHtml,
    templateSummaryLabel,
    normalizeTemplatePlatform,
    normalizeAutoPlatform,
    firstTemplateKeyForTemplatePlatform,
    buildManualPreset,
    profileTags,
    collectProfileTags,
    wireTagPicker,
    uniqueTags,
    escapeHtml,
    extensionDisplayName,
    markDirty
  });

  const routeNetworkSecurityState = wireRouteNetworkSecurityTab({
    overlay,
    form,
    t,
    model,
    existing,
    dnsDraft,
    globalSecurityRef,
    profileNetworkState,
    loadPolicyPresets,
    summarizePolicyPreset,
    globalBlocklistOptions,
    normalizeProfileRouteMode,
    routeTemplateOptions,
    globalRouteNoticeHtml,
    previewNetworkSandboxSettings,
    renderProfileSandboxFrame,
    applyPolicyPresetToDraft,
    escapeHtml,
    hasAssignedProfileCertificates,
    domainStatusIcon,
    domainStatusLabel,
    buildDomainEntries,
    slugId,
    makeUniqueId,
    pickCertificateFiles,
    setNotice,
    rerender,
    getGlobalSecuritySettings,
    normalizeGlobalSecuritySettings,
    saveGlobalSecuritySettings,
    buildGlobalSecuritySaveRequest,
    markDirty
  });

  for (const button of overlay.querySelectorAll("[data-tab]")) {
    button.addEventListener("click", () => {
      const tab = button.getAttribute("data-tab");
      for (const b of overlay.querySelectorAll("[data-tab]")) b.classList.remove("active");
      button.classList.add("active");
      for (const pane of overlay.querySelectorAll(".tab-pane")) {
        pane.classList.toggle("hidden", pane.getAttribute("data-pane") !== tab);
      }
    });
  }

  const closeModal = async () => {
    if (!dirty) {
      closeModalOverlay(overlay);
      return;
    }
    const leave = await askConfirmPrompt(t, t("profile.modal.closeTitle"), t("profile.modal.closeDirty"));
    if (leave) closeModalOverlay(overlay);
  };

  overlay.querySelector("#profile-cancel")?.addEventListener("click", closeModal);
  overlay.addEventListener("click", (event) => {
    if (routeNetworkSecurityState.blocklistDropdown && !routeNetworkSecurityState.blocklistDropdown.contains(event.target)) {
      routeNetworkSecurityState.blocklistMenu?.classList.add("hidden");
    }
    if (
      identityExtState.identityTemplateToggle
      && !identityExtState.identityTemplateToggle.contains(event.target)
      && identityExtState.identityTemplateMenu
      && !identityExtState.identityTemplateMenu.contains(event.target)
    ) {
      identityExtState.identityTemplateMenu.classList.add("hidden");
    }
    if (!event.target.closest("[data-tag-picker='profile-tags']")) {
      identityExtState.profileTagPicker?.close();
    }
    if (event.target === overlay) closeModal();
  });

  attachProfileModalSubmitHandler({
    form,
    t,
    model,
    existing,
    rerender,
    setNotice,
    closeModalOverlay,
    overlay,
    tagsState: identityExtState.tagsState,
    extensionState: identityExtState.extensionState,
    certificateState: routeNetworkSecurityState.certificateState,
    blocklistState: routeNetworkSecurityState.blocklistState,
    allowState: routeNetworkSecurityState.allowState,
    denyState: routeNetworkSecurityState.denyState,
    dnsDraft,
    blocklistItems: routeNetworkSecurityState.blocklistItems,
    globalSecurity: globalSecurityRef.current,
    syncOverview,
    profileNetworkState,
    identityUiState: identityExtState.identityUiState,
    identityTemplates: identityExtState.identityTemplates,
    identityPresetState: identityExtState.getIdentityPresetState(),
    initialSandboxMode: routeNetworkSecurityState.initialSandboxMode,
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
  });
}
