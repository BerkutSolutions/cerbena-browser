import {
  exportExtensionLibrary,
  importExtensionLibrary,
  importExtensionLibraryItem,
  listExtensionLibrary,
  removeExtensionLibraryItem,
  refreshExtensionLibraryUpdates,
  setExtensionProfiles,
  updateExtensionLibraryItem,
  updateExtensionLibraryPreferences
} from "./api.js";
import { askInputModal, closeModalOverlay, showModalOverlay } from "../../core/modal.js";
import { uniqueTags, wireTagPicker, collectTagOptions } from "../../core/tag-picker.js";
import { bindExtensionsWire } from "./view-wire.js";
import { createExtensionActions } from "./view-actions.js";
import {
  renderExtensions,
  extensionModalHtml,
  engineScopeLabel,
  normalizeEngineScope,
  packageVariants,
  variantDetailCard
} from "./view-render.js";

const {
  createFromLocalFile,
  createFromUrl,
  importLocalFolder,
  openExtensionModal,
  saveExtensionItem
} = createExtensionActions({
  askInputModal,
  closeModalOverlay,
  engineScopeLabel,
  extensionModalHtml,
  hydrateExtensionsModel,
  importExtensionLibraryItem,
  listExtensionLibrary,
  packageVariants,
  removeExtensionLibraryItem,
  setExtensionProfiles,
  showModalOverlay,
  uniqueTags,
  updateExtensionLibraryItem,
  variantDetailCard,
  wireTagPicker
});

export { renderExtensions };

export async function hydrateExtensionsModel(model) {
  try {
    await refreshExtensionLibraryUpdates();
  } catch {
    // Keep the library usable even if background update refresh fails.
  }
  const result = await listExtensionLibrary();
  model.extensionLibraryState = result.ok ? JSON.parse(result.data || "{}") : { autoUpdateEnabled: false, items: {} };
  model.extensionLibraryFilter = model.extensionLibraryFilter ?? "all";
  model.extensionLibraryTagFilter = uniqueTags(model.extensionLibraryTagFilter ?? []);
}

export function wireExtensions(root, model, rerender, t) {
  bindExtensionsWire(root, model, rerender, t, {
    uniqueTags,
    wireTagPicker,
    collectExtensionTags: (state) => collectTagOptions(Object.values(state?.items ?? {}), (item) => item.tags ?? []),
    hydrateExtensionsModel,
    createFromUrl,
    createFromLocalFile,
    importLocalFolder,
    importExtensionLibrary,
    exportExtensionLibrary,
    updateExtensionLibraryPreferences,
    openExtensionModal,
    saveExtensionItem,
    normalizeEngineScope
  });
}
