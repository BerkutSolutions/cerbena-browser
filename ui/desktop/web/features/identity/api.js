import { callCommand } from "../../core/commands.js";

export async function generateAutoPreset(platform, seed) {
  return callCommand("generate_identity_auto_preset", {
    request: { platform, seed }
  });
}

export async function validateIdentityPreset(preset) {
  return callCommand("validate_identity_preset_command", { preset });
}

export async function previewIdentityPreset(preset, activeRoute) {
  return callCommand("preview_identity_preset", { preset, activeRoute });
}

export async function validateIdentitySave(preset, activeRoute) {
  return callCommand("validate_identity_save", { preset, activeRoute });
}

export async function saveIdentityProfile(profileId, preset) {
  return callCommand("save_identity_profile", {
    request: { profileId, preset }
  });
}

export async function getIdentityProfile(profileId) {
  return callCommand("get_identity_profile", { request: { profileId } });
}

export async function applyIdentityAutoGeo(preset, source) {
  return callCommand("apply_identity_auto_geolocation", { request: { preset, source } });
}
