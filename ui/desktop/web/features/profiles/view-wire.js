import { wireProfileListInteractionsImpl } from "./view-wire-list.js";
import { wireProfileModalOrchestrationImpl } from "./view-wire-modal-orchestration.js";
import { openProfileModalFormImpl } from "./view-wire-form.js";
import { handleProfileLogsActionImpl, wireProfileTransferCommandsImpl } from "./view-wire-transfer-log.js";

export function wireProfilesImpl(root, model, rerender, t, deps) {
  const {
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
  } = deps;

  wireProfileModalOrchestrationImpl(root, model, rerender, t, {
    openProfileModal
  });

  wireProfileTransferCommandsImpl(root, model, rerender, t, {
    askInputPrompt,
    importProfile,
    setNotice,
    hydrateProfilesModel,
    selectionState,
    exportProfileArchiveAction,
    openCopyCookiesModal
  });

  wireProfileListInteractionsImpl(root, model, rerender, t, {
    escapeHtml,
    selectionState,
    ensureProfilesViewState,
    handleProfileLogsAction: (profile) =>
      handleProfileLogsActionImpl(profile, t, { openProfileLogsModal, readProfileLogs }),
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
    setNotice,
    hydrateProfilesModel
  });

  root.querySelector("#profiles-clear-selection")?.addEventListener("click", () => {
    model.selectedProfileIds = [];
    rerender();
  });

  root.querySelector("#profiles-add-group")?.addEventListener("click", async () => {
    const groupName = await askInputPrompt(t, t("profile.bulk.addGroup"), t("profile.bulk.groupName"));
    if (!groupName) return;
    await applyBulkTag(model, "group", groupName);
    setNotice(model, "success", t("profile.bulk.groupSaved"));
    await hydrateProfilesModel(model);
    rerender();
  });

  root.querySelector("#profiles-add-ext-group")?.addEventListener("click", async () => {
    const groupName = await askInputPrompt(t, t("profile.bulk.addExtGroup"), t("profile.bulk.extGroupName"));
    if (!groupName) return;
    await applyBulkTag(model, "ext-group", groupName);
    setNotice(model, "success", t("profile.bulk.extGroupSaved"));
    await hydrateProfilesModel(model);
    rerender();
  });
}

export async function applyBulkTagImpl(model, prefix, value, deps) {
  const { selectionState, updateProfile } = deps;

  const normalized = `${prefix}:${value.trim()}`;
  for (const profileId of selectionState(model)) {
    const profile = model.profiles.find((item) => item.id === profileId);
    if (!profile) continue;
    const baseTags = (profile.tags ?? []).filter((tag) => !tag.startsWith(`${prefix}:`));
    baseTags.push(normalized);
    await updateProfile({
      profileId: profile.id,
      tags: baseTags,
      expectedUpdatedAt: profile.updated_at
    });
  }
}

export function openCopyCookiesModalImpl(root, model, rerender, t, deps) {
  const {
    selectionState,
    copyCookiesModalHtml,
    copyProfileCookies,
    setNotice,
    resolveProfileErrorMessage,
    hydrateProfilesModel
  } = deps;

  const selectedProfiles = model.profiles.filter((profile) => selectionState(model).includes(profile.id));
  document.body.insertAdjacentHTML("beforeend", copyCookiesModalHtml(t, model.profiles, selectedProfiles));
  const overlay = document.body.querySelector("#profile-cookie-overlay");
  const close = () => overlay.remove();
  overlay.querySelector("#profile-cookie-close")?.addEventListener("click", close);
  overlay.querySelector("#profile-cookie-cancel")?.addEventListener("click", close);
  overlay.addEventListener("click", (event) => {
    if (event.target === overlay) close();
  });
  overlay.querySelector("#profile-cookie-submit")?.addEventListener("click", async () => {
    const sourceProfileId = overlay.querySelector("#profile-cookie-source")?.value?.trim();
    if (!sourceProfileId) return;
    const result = await copyProfileCookies(sourceProfileId, selectedProfiles.map((profile) => profile.id));
    if (result.ok) {
      const skipped = result.data.skipped_targets?.length
        ? ` ${t("profile.cookies.skipped")}: ${result.data.skipped_targets.length}.`
        : "";
      setNotice(model, "success", `${t("profile.cookies.copied")} ${result.data.copied_targets}.${skipped}`);
      close();
      await hydrateProfilesModel(model);
      rerender();
      return;
    }
    setNotice(model, "error", resolveProfileErrorMessage(t, result.data.error));
    close();
    rerender();
  });
}

export async function openProfileModalImpl(root, model, rerender, t, existing, deps) {
  return openProfileModalFormImpl(root, model, rerender, t, existing, deps);
}
