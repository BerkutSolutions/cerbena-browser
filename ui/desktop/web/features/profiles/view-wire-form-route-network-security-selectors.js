export function selectRouteNetworkSecurityNodes(overlay){
  const profileRouteMode = overlay.querySelector("#profile-route-mode");
  const profileRouteTemplate = overlay.querySelector("#profile-route-template");
  const profileRouteTemplateRow = overlay.querySelector("#profile-route-template-row");
  const profileKillSwitchRow = overlay.querySelector("#profile-kill-switch-row");
  const profileKillSwitchInput = overlay.querySelector("[name='profileKillSwitch']");
  const profileGlobalRouteNoticeSlot = overlay.querySelector("#profile-global-route-notice-slot");
  const profileSandboxSlot = overlay.querySelector("#profile-sandbox-frame-slot");
  const profileDnsModeField = overlay.querySelector("#profile-dns-mode");
  const profileDnsServersRow = overlay.querySelector("#profile-dns-servers-row");
  const profileDnsTemplateRow = overlay.querySelector("#profile-dns-template-row");
  const passwordLockField = overlay.querySelector("[name='passwordLock']");
  const panicFrameEnabledField = overlay.querySelector("[name='panicFrameEnabled']");
  const panicColorRow = overlay.querySelector("#profile-panic-color-row");
  const profileEngineField = overlay.querySelector("#profile-engine");
  const singlePageModeField = overlay.querySelector("#profile-single-page-mode");
  const defaultSearchRow = overlay.querySelector("#profile-default-search-row");
  const singlePageHint = overlay.querySelector("#profile-single-page-hint");
  const passwordFields = overlay.querySelector("#profile-password-fields");
  const passwordValueField = overlay.querySelector("[name='profilePassword']");
  const passwordConfirmField = overlay.querySelector("[name='profilePasswordConfirm']");
  const passwordToggleButton = overlay.querySelector("#profile-password-toggle");
  const domainTable = overlay.querySelector("#profile-domain-table");
  const domainInput = overlay.querySelector("#profile-domain-input");
  const domainTypeField = overlay.querySelector("#profile-domain-type");
  const domainSearchField = overlay.querySelector("#profile-domain-search");
  const domainFilterField = overlay.querySelector("#profile-domain-filter");
  const certificateTable = overlay.querySelector("#profile-certificates-table");
  const profileCertificateSelectField = overlay.querySelector("[name='profileCertificateSelect']");
  const profileCertificateEngineGuard = overlay.querySelector("#profile-certificate-engine-guard");


  return {
    profileRouteMode, profileRouteTemplate, profileRouteTemplateRow, profileKillSwitchRow, profileKillSwitchInput,
    profileGlobalRouteNoticeSlot, profileSandboxSlot, profileDnsModeField, profileDnsServersRow, profileDnsTemplateRow,
    passwordLockField, panicFrameEnabledField, panicColorRow, profileEngineField, singlePageModeField,
    defaultSearchRow, singlePageHint, passwordFields, passwordValueField, passwordConfirmField, passwordToggleButton,
    domainTable, domainInput, domainTypeField, domainSearchField, domainFilterField, certificateTable,
    profileCertificateSelectField, profileCertificateEngineGuard
  };
}
