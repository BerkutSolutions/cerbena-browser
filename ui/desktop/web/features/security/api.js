import { callCommand } from "../../core/commands.js";

export async function setDefaultProfileForLinks(request) {
  return callCommand("set_default_profile_for_links", { request });
}

export async function dispatchExternalLink(url) {
  return callCommand("dispatch_external_link", { request: { url } });
}

export async function executeLaunchHook(policy) {
  return callCommand("execute_launch_hook", { request: { policy } });
}

export async function resolvePipPolicy(requested, platformSupported) {
  return callCommand("resolve_pip_policy", { request: { requested, platformSupported } });
}

export async function getGlobalSecuritySettings() {
  return callCommand("get_global_security_settings");
}

export async function saveGlobalSecuritySettings(request) {
  return callCommand("save_global_security_settings", { request });
}

export async function pickCertificateFiles() {
  return callCommand("pick_certificate_files");
}
