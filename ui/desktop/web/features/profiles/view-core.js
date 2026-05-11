import {
  copyProfileCookies,
  createProfile,
  deleteProfile,
  exportProfile,
  importProfile,
  launchProfile,
  listProfiles,
  pickCertificateFiles,
  readProfileLogs,
  setProfilePassword,
  stopProfile,
  updateProfile,
  validateProfileModal
} from "./api.js";
import { SEARCH_PROVIDER_PRESETS } from "../../core/catalogs.js";
import {
  listExtensionLibrary,
  listProfileExtensions,
  saveProfileExtensions,
  setExtensionProfiles
} from "../extensions/api.js";
import { generateAutoPreset, getIdentityProfile, saveIdentityProfile } from "../identity/api.js";
import { getDevicePostureReport, getLinuxBrowserSandboxStatus } from "../settings/api.js";
import {
  buildRealPreset,
  buildManualPreset,
  cloneIdentityPreset,
  firstTemplateKeyForTemplatePlatform,
  inferIdentityUiState,
  listIdentityPlatforms,
  listIdentityTemplatePlatforms,
  listIdentityTemplates,
  normalizeTemplatePlatform,
  normalizeAutoPlatform
} from "../identity/shared.js";
import {
  getNetworkState,
  getServiceCatalog,
  previewNetworkSandboxSettings,
  saveDnsPolicy,
  saveNetworkSandboxProfileSettings,
  saveVpnProxyPolicy
} from "../network/api.js";
import { getGlobalSecuritySettings, saveGlobalSecuritySettings } from "../security/api.js";
import { getSyncOverview, saveSyncControls } from "../sync/api.js";
import { blockedServicesToPairs, loadDnsTemplates, loadProfileDnsDraft, saveProfileDnsDraft } from "../dns/store.js";
import { applyPolicyPresetToDraft, loadPolicyPresets, summarizePolicyPreset } from "../dns/policy-store.js";
import { closeModalOverlay, showModalOverlay } from "../../core/modal.js";
import { buildTagPickerMarkup, uniqueTags, wireTagPicker } from "../../core/tag-picker.js";
import { copyCookiesModalHtml, renderProfilesSectionHtml, rowHtml } from "./view-list.js";
import { renderProfileModalHtml } from "./view-modal-shell.js";
import {
  classifyDockerRuntimeIssue,
  openProfileLogsModal,
  showDockerHelpModal,
  showLinuxSandboxLaunchModal
} from "./view-launch-overlays.js";
import {
  loadModalGeneralDependencies,
  loadModalIdentityAndExtensionsDependencies,
  loadModalNetworkAndSyncDependencies
} from "./view-modal-data.js";
import {
  DOMAIN_OPTIONS,
  option,
  escapeHtml,
  engineIcon,
  pencilIcon,
  exportIcon,
  trashIcon,
  closeIcon,
  playIcon,
  stopIcon,
  terminalIcon,
  usersIcon,
  puzzleIcon,
  cookieIcon,
  profileTags,
  collectProfileTags,
  certificateEntriesForProfile,
  hasAssignedProfileCertificates,
  slugId,
  makeUniqueId,
  normalizeGlobalSecuritySettings,
  buildGlobalSecuritySaveRequest,
  syncManagedCertificateAssignments
} from "./view-helpers.js";
import {
  buildRoutePolicyPayload,
  globalRouteNoticeHtml,
  normalizeProfileRouteMode,
  renderProfileSandboxFrame,
  routeTemplateOptions
} from "./view-route-sandbox.js";
import {
  askConfirmPrompt,
  askInputPrompt,
  exportProfileArchiveAction
} from "./view-actions.js";
import {
  applyBulkTagImpl,
  openCopyCookiesModalImpl,
  openProfileModalImpl,
  wireProfilesImpl
} from "./view-wire.js";
import {
    assignedProfileExtensionIds,
    buildDomainEntries,
  dnsTemplateOptions,
  domainStatusIcon,
  domainStatusLabel,
  ensureProfilesViewState,
  extensionDisplayName,
  extensionLibraryItem,
  extensionLibraryOptions,
  globalBlocklistOptions,
  extensionScopeAllowed,
  eyeIcon,
  isChromiumFamilyEngine,
  isFirefoxFamilyEngine,
  mergedProfileExtensions,
  postureFindingLines,
  profileSecurityFlags,
  profileSortAria,
  resolveDevicePostureAction,
  resolveProfileErrorMessage,
  searchOptions,
  selectionState,
  setNotice,
  sortedProfiles,
  templateDropdownOptionsHtml,
  templateInputValue,
  templateSummaryLabel
} from "./view-core-support.js";

export function renderProfilesSection(t, model) {
  const selectedIds = selectionState(model);
  const sortState = ensureProfilesViewState(model);
  const rows = sortedProfiles(model).map((profile) => rowHtml(profile, selectedIds.includes(profile.id), t)).join("");
  const notice = model.profileNotice ? `<p class="notice ${model.profileNotice.type}">${model.profileNotice.text}</p>` : "";
  const allSelected = model.profiles.length > 0 && selectedIds.length === model.profiles.length;
  return renderProfilesSectionHtml(
    t,
    model,
    rows,
    (key) => profileSortAria(sortState, key),
    allSelected,
    notice,
    selectedIds.length
  );
}

export function renderProfiles(t, model) {
  return `
    <div class="feature-page">
      ${renderProfilesSection(t, model)}
    </div>
  `;
}

export async function hydrateProfilesModel(model) {
  const res = await listProfiles();
  model.profiles = res.ok ? res.data : [];
  const selected = new Set(selectionState(model));
  model.selectedProfileIds = model.profiles.map((profile) => profile.id).filter((id) => selected.has(id));
}


export function wireProfiles(root, model, rerender, t) {
  return wireProfilesImpl(root, model, rerender, t, {
    escapeHtml,
    selectionState,
    ensureProfilesViewState,
    askInputPrompt,
    importProfile,
    setNotice,
    hydrateProfilesModel,
    openProfileLogsModal,
    readProfileLogs,
    launchProfile,
    classifyDockerRuntimeIssue,
    resolveDevicePostureAction,
    getDevicePostureReport,
    postureFindingLines,
    askConfirmPrompt,
    resolveProfileErrorMessage,
    showDockerHelpModal,
    showLinuxSandboxLaunchModal,
    getLinuxBrowserSandboxStatus,
    stopProfile,
    deleteProfile,
    openProfileModal,
    exportProfileArchiveAction,
    openCopyCookiesModal,
    applyBulkTag
  });
}

async function applyBulkTag(model, prefix, value) {
  return applyBulkTagImpl(model, prefix, value, { selectionState, updateProfile });
}

function openCopyCookiesModal(root, model, rerender, t) {
  return openCopyCookiesModalImpl(root, model, rerender, t, {
    selectionState,
    copyCookiesModalHtml,
    copyProfileCookies,
    setNotice,
    resolveProfileErrorMessage,
    hydrateProfilesModel
  });
}

async function openProfileModal(root, model, rerender, t, existing) {
  return openProfileModalImpl(root, model, rerender, t, existing, {
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
  });
}
