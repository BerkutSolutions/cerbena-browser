import { callCommand } from "../../core/commands.js";

export async function listProfiles() {
  return callCommand("list_profiles");
}

export async function createProfile(payload) {
  return callCommand("create_profile", { request: payload });
}

export async function updateProfile(payload) {
  return callCommand("update_profile", { request: payload });
}

export async function deleteProfile(profileId) {
  return callCommand("delete_profile", { request: { profileId } });
}

export async function duplicateProfile(profileId, newName) {
  return callCommand("duplicate_profile", { request: { profileId, newName } });
}

export async function launchProfile(profileId, launchUrl = null, devicePostureAckId = null) {
  const request = { profileId };
  if (launchUrl) {
    request.launchUrl = launchUrl;
  }
  if (devicePostureAckId) {
    request.devicePostureAckId = devicePostureAckId;
  }
  return callCommand("launch_profile", { request });
}

export async function stopProfile(profileId) {
  return callCommand("stop_profile", { request: { profileId } });
}

export async function acknowledgeWayfernTos(profileId = null) {
  const request = {};
  if (profileId) {
    request.profileId = profileId;
  }
  return callCommand("acknowledge_wayfern_tos", { request });
}

export async function getWayfernTermsStatus() {
  return callCommand("get_wayfern_terms_status");
}

export async function ensureEngineBinaries() {
  return callCommand("ensure_engine_binaries");
}

export async function copyProfileCookies(sourceProfileId, targetProfileIds) {
  return callCommand("copy_profile_cookies", {
    request: { sourceProfileId, targetProfileIds }
  });
}

export async function setProfilePassword(profileId, password) {
  return callCommand("set_profile_password", { request: { profileId, password } });
}

export async function unlockProfile(profileId, password) {
  return callCommand("unlock_profile", { request: { profileId, password } });
}

export async function validateProfileModal(payload) {
  return callCommand("validate_profile_modal", { payload });
}

export async function exportProfile(profileId, passphrase) {
  return callCommand("export_profile", { request: { profileId, passphrase } });
}

export async function importProfile(archiveJson, expectedProfileId, passphrase) {
  return callCommand("import_profile", {
    request: { archiveJson, expectedProfileId, passphrase }
  });
}

export async function pickCertificateFiles() {
  return callCommand("pick_certificate_files");
}
