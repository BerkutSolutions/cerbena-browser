export function createExtensionActions(deps) {
  const {
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
  } = deps;

  function readAssignedProfiles(overlay, itemId) {
    return [...overlay.querySelectorAll(`[data-modal-profile-assign^='${itemId}:']:checked`)]
      .map((checkbox) => checkbox.getAttribute("data-modal-profile-assign").split(":")[1]);
  }

  async function openExtensionModal(t, profiles, item, availableTags) {
    return new Promise((resolve) => {
      document.body.insertAdjacentHTML("beforeend", extensionModalHtml(t, profiles, item));
      const overlay = document.body.querySelector("#extension-library-overlay");
      if (!overlay) {
        resolve(null);
        return;
      }
      const close = (payload = null) => closeModalOverlay(overlay, () => resolve(payload));
      const tagState = { selected: uniqueTags(item.tags ?? []), available: [] };
      const bindDropdowns = () => {
        for (const button of overlay.querySelectorAll("[data-modal-profile-menu-toggle]")) {
          button.addEventListener("click", () => {
            const id = button.getAttribute("data-modal-profile-menu-toggle");
            overlay.querySelector(`[data-modal-profile-menu='${id}']`)?.classList.toggle("hidden");
          });
        }
      };
      bindDropdowns();
      const tagPicker = wireTagPicker(overlay, {
        id: "extension-tags",
        state: tagState,
        emptyLabel: t("extensions.tags.empty"),
        searchPlaceholder: t("extensions.tags.search"),
        createLabel: (value) => t("extensions.tags.create").replace("{tag}", value)
      });
      tagPicker?.rerender(uniqueTags([...(availableTags ?? []), ...(item.tags ?? [])]), item.tags ?? []);
      showModalOverlay(overlay);
      overlay.querySelector("#extension-library-close")?.addEventListener("click", () => close());
      overlay.querySelector("#extension-library-cancel")?.addEventListener("click", () => close());
      overlay.addEventListener("click", (event) => {
        if (event.target === overlay) close();
      });
      overlay.querySelector("#extension-library-save")?.addEventListener("click", () => {
        close({
          extensionId: item.id,
          displayName: item.displayName,
          version: item.version,
          tags: uniqueTags(tagState.selected ?? []),
          assignedProfileIds: readAssignedProfiles(overlay, item.id),
          autoUpdateEnabled: Boolean(overlay.querySelector("#extension-auto-update")?.checked),
          preserveOnPanicWipe: Boolean(overlay.querySelector("#extension-preserve-on-panic")?.checked),
          protectDataFromPanicWipe: Boolean(overlay.querySelector("#extension-protect-data-on-panic")?.checked)
        });
      });
      for (const button of overlay.querySelectorAll("[data-action='remove-variant']")) {
        button.addEventListener("click", async () => {
          const scope = String(button.getAttribute("data-engine-scope") ?? "");
          const confirmed = window.confirm(item.displayName ?? t("extensions.remove"));
          if (!confirmed) return;
          button.disabled = true;
          const result = await removeExtensionLibraryItem(item.id, scope);
          button.disabled = false;
          if (!result.ok) return;
          const nextVariants = packageVariants(item).filter((variant) => String(variant.engineScope) !== scope);
          if (!nextVariants.length) {
            close({ removedExtensionId: item.id });
            return;
          }
          item.packageVariants = nextVariants;
          const variantList = overlay.querySelector(".extension-library-variant-list");
          if (variantList) {
            variantList.innerHTML = nextVariants.map((variant) => variantDetailCard(item, variant, t)).join("");
          }
          const chip = overlay.querySelector(".extension-library-modal-engine-chip");
          if (chip) chip.textContent = engineScopeLabel(item.engineScope, t);
        });
      }
    });
  }

  function readFileAsBase64(file) {
    return new Promise((resolve, reject) => {
      const reader = new FileReader();
      reader.onload = () => {
        try {
          const bytes = new Uint8Array(reader.result);
          let binary = "";
          const chunkSize = 0x8000;
          for (let index = 0; index < bytes.length; index += chunkSize) {
            binary += String.fromCharCode(...bytes.subarray(index, index + chunkSize));
          }
          resolve(btoa(binary));
        } catch (error) {
          reject(error);
        }
      };
      reader.onerror = () => reject(reader.error);
      reader.readAsArrayBuffer(file);
    });
  }

  async function importLocalPackage(file, sourceKind) {
    const packageBytesBase64 = await readFileAsBase64(file);
    return importExtensionLibraryItem({
      sourceKind,
      sourceValue: file.name,
      assignedProfileIds: [],
      packageFileName: file.name,
      packageBytesBase64
    });
  }

  async function importLocalFolder() {
    return importExtensionLibraryItem({
      sourceKind: "local_folder_picker",
      sourceValue: "",
      assignedProfileIds: []
    });
  }

  async function createFromUrl(model, rerender, t) {
    const url = await askInputModal(t, {
      title: t("extensions.addStoreUrl"),
      label: t("extensions.storePrompt"),
      defaultValue: "https://addons.mozilla.org/firefox/addon/ublock-origin/"
    });
    if (!url) return;
    const result = await importExtensionLibraryItem({
      sourceKind: "store_url",
      sourceValue: url,
      storeUrl: url,
      assignedProfileIds: []
    });
    model.extensionNotice = { type: result.ok ? "success" : "error", text: result.ok ? t("extensions.installed") : String(result.data.error) };
    await hydrateExtensionsModel(model);
    await rerender();
  }

  async function createFromLocalFile(model, rerender, t, file, sourceKind) {
    if (!file) return;
    const result = await importLocalPackage(file, sourceKind);
    model.extensionNotice = { type: result.ok ? "success" : "error", text: result.ok ? t("extensions.installed") : String(result.data.error) };
    await hydrateExtensionsModel(model);
    await rerender();
  }

  async function saveExtensionItem(model, rerender, t, item, payload) {
    const updateResult = await updateExtensionLibraryItem({
      extensionId: item.id,
      displayName: payload.displayName || item.displayName,
      version: payload.version || item.version,
      storeUrl: item.storeUrl || null,
      logoUrl: item.logoUrl || null,
      tags: payload.tags ?? [],
      autoUpdateEnabled: payload.autoUpdateEnabled,
      preserveOnPanicWipe: payload.preserveOnPanicWipe,
      protectDataFromPanicWipe: payload.protectDataFromPanicWipe
    });
    let result = updateResult;
    if (updateResult.ok) {
      result = await setExtensionProfiles(item.id, payload.assignedProfileIds);
    }
    model.extensionNotice = {
      type: result.ok ? "success" : "error",
      text: result.ok ? t("extensions.edited") : String(result.data.error)
    };
    await hydrateExtensionsModel(model);
    await rerender();
  }

  return {
    createFromLocalFile,
    createFromUrl,
    importLocalFolder,
    openExtensionModal,
    saveExtensionItem
  };
}
