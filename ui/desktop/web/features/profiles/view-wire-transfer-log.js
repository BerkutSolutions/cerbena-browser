export function wireProfileTransferCommandsImpl(root, model, rerender, t, deps) {
  const {
    askInputPrompt,
    importProfile,
    setNotice,
    hydrateProfilesModel,
    selectionState,
    exportProfileArchiveAction,
    openCopyCookiesModal
  } = deps;

  root.querySelector("#profile-import")?.addEventListener("click", async () => {
    const archiveJson = await askInputPrompt(t, t("profile.import.title"), t("profile.import.archive"));
    if (!archiveJson) return;
    const expectedProfileId = await askInputPrompt(t, t("profile.import.title"), t("profile.import.profileId"));
    if (!expectedProfileId) return;
    const passphrase = await askInputPrompt(t, t("profile.import.title"), t("profile.import.passphrase"));
    if (!passphrase) return;
    const result = await importProfile(archiveJson, expectedProfileId, passphrase);
    setNotice(model, result.ok ? "success" : "error", result.ok ? t("profile.import.success") : String(result.data.error));
    await hydrateProfilesModel(model);
    rerender();
  });

  root.querySelector("#profiles-export-selection")?.addEventListener("click", async () => {
    const selectedIds = selectionState(model);
    if (selectedIds.length !== 1) return;
    const changed = await exportProfileArchiveAction(model, t, selectedIds[0], (type, text) => setNotice(model, type, text));
    if (!changed) return;
    await hydrateProfilesModel(model);
    rerender();
  });

  root.querySelector("#profiles-copy-cookies")?.addEventListener("click", () => {
    openCopyCookiesModal(root, model, rerender, t);
  });
}

export function handleProfileLogsActionImpl(profile, t, deps) {
  const { openProfileLogsModal, readProfileLogs } = deps;
  return openProfileLogsModal(profile, t, readProfileLogs);
}
