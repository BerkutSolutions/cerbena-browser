export function bindExtensionsWire(root, model, rerender, t, deps) {
  const localPicker = root.querySelector("#extension-local-picker");
  const importMenu = root.querySelector("#extension-import-menu");
  const exportMenu = root.querySelector("#extension-export-menu");
  const filterTagState = {
    selected: deps.uniqueTags(model.extensionLibraryTagFilter ?? []),
    available: deps.collectExtensionTags(model.extensionLibraryState ?? { items: {} })
  };
  const filterTagPicker = deps.wireTagPicker(root, {
    id: "extension-filter-tags",
    state: filterTagState,
    emptyLabel: t("extensions.tags.filterAll"),
    searchPlaceholder: t("extensions.tags.search"),
    allowCreate: false,
    onChange(selected) {
      model.extensionLibraryTagFilter = deps.uniqueTags(selected ?? []);
      rerender();
    }
  });
  filterTagPicker?.rerender(filterTagState.available, filterTagState.selected);
  root.querySelector("#extension-add-url")?.addEventListener("click", async () => {
    await deps.createFromUrl(model, rerender, t);
  });
  root.querySelector("#extension-import-toggle")?.addEventListener("click", (event) => {
    event.preventDefault();
    event.stopPropagation();
    importMenu?.classList.toggle("hidden");
    exportMenu?.classList.add("hidden");
  });
  root.querySelector("#extension-export-toggle")?.addEventListener("click", (event) => {
    event.preventDefault();
    event.stopPropagation();
    exportMenu?.classList.toggle("hidden");
    importMenu?.classList.add("hidden");
  });
  for (const button of root.querySelectorAll("[data-extension-import-mode]")) {
    button.addEventListener("click", async () => {
      importMenu?.classList.add("hidden");
      const mode = button.getAttribute("data-extension-import-mode");
      if (mode === "local-file") {
        localPicker?.click();
        return;
      }
      if (mode === "local-folder") {
        const result = await deps.importLocalFolder();
        model.extensionNotice = {
          type: result.ok ? "success" : "error",
          text: result.ok ? t("extensions.installed") : String(result.data.error)
        };
        await deps.hydrateExtensionsModel(model);
        await rerender();
        return;
      }
      const result = await deps.importExtensionLibrary(mode);
      model.extensionNotice = {
        type: result.ok ? "success" : "error",
        text: result.ok
          ? t("extensions.transfer.imported").replace("{count}", String(result.data.imported))
          : String(result.data.error)
      };
      await deps.hydrateExtensionsModel(model);
      await rerender();
    });
  }
  for (const button of root.querySelectorAll("[data-extension-export-mode]")) {
    button.addEventListener("click", async () => {
      exportMenu?.classList.add("hidden");
      const result = await deps.exportExtensionLibrary(button.getAttribute("data-extension-export-mode"));
      model.extensionNotice = {
        type: result.ok ? "success" : "error",
        text: result.ok
          ? t("extensions.transfer.exported").replace("{count}", String(result.data.exported))
          : String(result.data.error)
      };
      await rerender();
    });
  }
  root.querySelector("#extension-auto-update-all")?.addEventListener("change", async (event) => {
    const result = await deps.updateExtensionLibraryPreferences({
      autoUpdateEnabled: Boolean(event.target.checked)
    });
    model.extensionNotice = {
      type: result.ok ? "success" : "error",
      text: result.ok ? t("action.save") : String(result.data.error)
    };
    await deps.hydrateExtensionsModel(model);
    await rerender();
  });
  root.querySelector("#extension-library-filter")?.addEventListener("change", async (event) => {
    model.extensionLibraryFilter = event.target.value || "all";
    await rerender();
  });
  localPicker?.addEventListener("change", async (event) => {
    const file = event.target.files?.[0];
    await deps.createFromLocalFile(model, rerender, t, file, "local_file");
    event.target.value = "";
  });

  const dropzone = root.querySelector("#extension-dropzone");
  dropzone?.addEventListener("dragover", (event) => {
    event.preventDefault();
    dropzone.classList.add("is-active");
  });
  dropzone?.addEventListener("dragleave", () => dropzone.classList.remove("is-active"));
  dropzone?.addEventListener("drop", async (event) => {
    event.preventDefault();
    dropzone.classList.remove("is-active");
    const file = event.dataTransfer?.files?.[0];
    await deps.createFromLocalFile(model, rerender, t, file, "dropped_file");
  });
  dropzone?.addEventListener("click", async () => {
    localPicker?.click();
  });

  root.addEventListener("click", (event) => {
    if (!event.target.closest("#extension-import-toggle") && !event.target.closest("#extension-import-menu")) {
      importMenu?.classList.add("hidden");
    }
    if (!event.target.closest("#extension-export-toggle") && !event.target.closest("#extension-export-menu")) {
      exportMenu?.classList.add("hidden");
    }
    if (!event.target.closest("[data-tag-picker='extension-filter-tags']")) {
      filterTagPicker?.close();
    }
  });

  for (const card of root.querySelectorAll(".extension-library-card")) {
    const openModal = async () => {
      const extensionId = card.getAttribute("data-extension-id");
      const item = model.extensionLibraryState?.items?.[extensionId];
      if (!item) return;
      const payload = await deps.openExtensionModal(
        t,
        model.profiles ?? [],
        item,
        deps.collectExtensionTags(model.extensionLibraryState ?? { items: {} })
      );
      if (!payload) return;
      if (payload.removedExtensionId) {
        model.extensionNotice = { type: "success", text: t("extensions.removed") };
        await deps.hydrateExtensionsModel(model);
        await rerender();
        return;
      }
      await deps.saveExtensionItem(model, rerender, t, item, payload);
    };

    card.addEventListener("click", async () => {
      await openModal();
    });

    card.addEventListener("keydown", async (event) => {
      if (event.key !== "Enter" && event.key !== " ") return;
      event.preventDefault();
      await openModal();
    });
  }
}
