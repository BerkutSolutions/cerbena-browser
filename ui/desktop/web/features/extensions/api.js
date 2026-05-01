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

export async function setExtensionProfiles(extensionId, assignedProfileIds) {
  return callCommand("set_extension_profiles", { request: { extensionId, assignedProfileIds } });
}

export async function removeExtensionLibraryItem(extensionId) {
  return callCommand("remove_extension_library_item", { request: { extensionId } });
}
