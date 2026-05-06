import { callCommand } from "../../core/commands.js";

export async function listExtensionLibrary() {
  return callCommand("list_extension_library");
}

export async function importExtensionLibraryItem(request) {
  return callCommand("import_extension_library_item", { request });
}

export async function updateExtensionLibraryItem(request) {
  return callCommand("update_extension_library_item", { request });
}

export async function updateExtensionLibraryPreferences(request) {
  return callCommand("update_extension_library_preferences", { request });
}

export async function refreshExtensionLibraryUpdates() {
  return callCommand("refresh_extension_library_updates");
}

export async function exportExtensionLibrary(mode) {
  return callCommand("export_extension_library", { request: { mode } });
}

export async function importExtensionLibrary(mode) {
  return callCommand("import_extension_library", { request: { mode } });
}

export async function setExtensionProfiles(extensionId, assignedProfileIds) {
  return callCommand("set_extension_profiles", { request: { extensionId, assignedProfileIds } });
}

export async function removeExtensionLibraryItem(extensionId, variantEngineScope = null) {
  return callCommand("remove_extension_library_item", { request: { extensionId, variantEngineScope } });
}
