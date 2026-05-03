import { callCommand } from "../../core/commands.js";

export async function importSearchProviders(providers) {
  return callCommand("import_search_providers", { request: { providers } });
}

export async function setDefaultSearchProvider(providerId) {
  return callCommand("set_default_search_provider", { request: { providerId } });
}

export async function getLinkRoutingOverview() {
  return callCommand("get_link_routing_overview");
}

export async function setDefaultProfileForLinks(request) {
  return callCommand("set_default_profile_for_links", { request });
}

export async function clearDefaultProfileForLinks() {
  return callCommand("clear_default_profile_for_links");
}

export async function saveLinkTypeProfileBinding(request) {
  return callCommand("save_link_type_profile_binding", { request });
}

export async function removeLinkTypeProfileBinding(linkType) {
  return callCommand("remove_link_type_profile_binding", { request: { linkType } });
}

export async function dispatchExternalLink(url) {
  return callCommand("dispatch_external_link", { request: { url } });
}

export async function consumePendingExternalLink() {
  return callCommand("consume_pending_external_link");
}

export async function getDevicePostureReport() {
  return callCommand("get_device_posture_report");
}

export async function refreshDevicePostureReport() {
  return callCommand("refresh_device_posture_report");
}

export async function getLauncherUpdateState() {
  return callCommand("get_launcher_update_state");
}

export async function setLauncherAutoUpdate(enabled) {
  return callCommand("set_launcher_auto_update", { enabled });
}

export async function checkLauncherUpdates(manual = true) {
  return callCommand("check_launcher_updates", { manual });
}

export async function launchUpdaterPreview() {
  return callCommand("launch_updater_preview");
}

export async function getShellPreferencesState() {
  return callCommand("get_shell_preferences_state");
}

export async function saveShellPreferences(request) {
  return callCommand("save_shell_preferences", { request });
}

export async function hideWindowToTray() {
  return callCommand("window_hide_to_tray");
}

export async function restoreWindowFromTray() {
  return callCommand("window_restore_from_tray");
}

export async function confirmAppExit() {
  return callCommand("confirm_app_exit");
}

export async function openDefaultAppsSettings() {
  return callCommand("open_default_apps_settings");
}
