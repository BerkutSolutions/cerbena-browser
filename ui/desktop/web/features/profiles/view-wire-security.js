export function buildProfileSecuritySaversImpl(context) {
  const {
    payload,
    form,
    globalSecurity,
    certificateState,
    syncManagedCertificateAssignments,
    buildGlobalSecuritySaveRequest,
    saveGlobalSecuritySettings,
    setProfilePassword
  } = context;

  let currentSecurityState = globalSecurity;

  const saveProfilePassword = async (profileId) => {
    if (!payload.passwordLockEnabled) {
      return { ok: true };
    }
    return setProfilePassword(profileId, form.profilePassword.value);
  };

  const saveManagedCertificates = async (profileId) => {
    currentSecurityState = syncManagedCertificateAssignments(currentSecurityState, profileId, certificateState);
    return saveGlobalSecuritySettings(buildGlobalSecuritySaveRequest(currentSecurityState));
  };

  return {
    saveProfilePassword,
    saveManagedCertificates,
    getSecurityState: () => currentSecurityState
  };
}

export function resolveProfileSaveErrorImpl(t, results) {
  const {
    extensionResult,
    certificateResult,
    dnsResult,
    routeResult,
    sandboxResult,
    syncResult,
    identityResult,
    passwordResult
  } = results;
  if (!extensionResult.ok) return String(extensionResult.data.error);
  if (!passwordResult.ok) return String(passwordResult.data.error);
  if (!certificateResult.ok) return String(certificateResult.data.error);
  if (!dnsResult.ok) return String(dnsResult.data.error);
  if (!routeResult.ok) return String(routeResult.data.error);
  if (!sandboxResult.ok) return String(sandboxResult.data.error);
  if (!syncResult.ok) return String(syncResult.data.error);
  if (!identityResult.ok) return String(identityResult.data.error);
  return t("profile.modal.validationError");
}
