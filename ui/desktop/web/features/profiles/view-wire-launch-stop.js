export async function handleProfileLaunchActionImpl(root, model, rerender, t, profile, deps) {
  const {
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
    openProfileModal,
    setNotice
  } = deps;

  const profileId = profile.id;
  model.profileActionPendingIds.add(profileId);
  model.profileLaunchOverlay = {
    profileId,
    stageKey: "starting",
    messageKey: "profile.launchProgress.starting",
    done: false
  };
  rerender();
  try {
    const launchResult = await launchProfile(profileId);
    if (!launchResult.ok) {
      model.profileLaunchOverlay = null;
      const errorText = String(launchResult.data.error);
      const dockerIssue = classifyDockerRuntimeIssue(errorText);
      const postureAction = resolveDevicePostureAction(errorText);
      if (dockerIssue) {
        await showDockerHelpModal(t, dockerIssue);
        setNotice(model, "error", resolveProfileErrorMessage(t, errorText));
      } else if (postureAction) {
        const postureResult = await getDevicePostureReport();
        const report = postureResult.ok ? postureResult.data : null;
        const detail = report ? postureFindingLines(t, report) : "";
        if (postureAction.kind === "confirm") {
          const accepted = await askConfirmPrompt(
            t,
            t("devicePosture.confirmTitle"),
            `${t("devicePosture.confirmDescription")}${detail ? `\n\n${detail}` : ""}`
          );
          if (accepted) {
            const confirmedLaunch = await launchProfile(profileId, null, postureAction.reportId);
            setNotice(
              model,
              confirmedLaunch.ok ? "success" : "error",
              confirmedLaunch.ok ? t("profile.notice.launched") : resolveProfileErrorMessage(t, confirmedLaunch.data.error)
            );
          }
        } else {
          setNotice(
            model,
            "error",
            `${t("devicePosture.refusedDescription")}${detail ? ` ${detail}` : ""}`
          );
        }
      } else if (errorText.includes("profile.security.chromium_certificates_not_supported")) {
        const accepted = await askConfirmPrompt(
          t,
          t("profile.security.chromiumCertificatesBlockedTitle"),
          t("profile.security.chromiumCertificatesBlockedDescription")
        );
        if (accepted) {
          await openProfileModal(root, model, rerender, t, profile);
        } else {
          setNotice(model, "error", resolveProfileErrorMessage(t, errorText));
        }
      } else {
        setNotice(model, "error", resolveProfileErrorMessage(t, errorText));
      }
    } else {
      setNotice(model, "success", t("profile.notice.launched"));
      if (!model.linuxSandboxHintShown) {
        const sandboxStatus = await getLinuxBrowserSandboxStatus();
        if (sandboxStatus.ok && sandboxStatus.data && sandboxStatus.data.status === "missing") {
          model.linuxSandboxHintShown = true;
          await showLinuxSandboxLaunchModal(t);
        }
      }
    }
  } finally {
    model.profileActionPendingIds.delete(profileId);
  }
}

export async function handleProfileStopActionImpl(model, t, profileId, deps) {
  const { stopProfile, setNotice } = deps;
  model.profileActionPendingIds.add(profileId);
  try {
    const stopResult = await stopProfile(profileId);
    setNotice(model, stopResult.ok ? "success" : "error", stopResult.ok ? t("profile.notice.stopped") : String(stopResult.data.error));
  } finally {
    model.profileActionPendingIds.delete(profileId);
  }
}
