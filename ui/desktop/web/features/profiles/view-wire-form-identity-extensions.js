export function wireIdentityExtensionsAdvancedTab(ctx) {
  const {
    overlay,
    t,
    model,
    existing,
    buildRealPreset,
    inferIdentityUiState,
    listIdentityTemplates,
    templateDropdownOptionsHtml,
    templateSummaryLabel,
    normalizeTemplatePlatform,
    normalizeAutoPlatform,
    firstTemplateKeyForTemplatePlatform,
    buildManualPreset,
    profileTags,
    collectProfileTags,
    wireTagPicker,
    uniqueTags,
    escapeHtml,
    extensionDisplayName,
    markDirty
  } = ctx;

  const identityStateNode = overlay.querySelector("#profile-identity-state");
  const identityTemplatesNode = overlay.querySelector("#profile-identity-templates");
  const identityModeField = overlay.querySelector("#profile-identity-mode");
  const identityPlatformField = overlay.querySelector("#profile-platform-target");
  const identityPlatformRow = overlay.querySelector("#profile-identity-platform-row");
  const identityTemplateRow = overlay.querySelector("#profile-identity-template-row");
  const identityTemplatePlatformField = overlay.querySelector("#profile-identity-template-platform");
  const identityDisplayNameField = overlay.querySelector("#profile-identity-display-name");
  const identityRealHint = overlay.querySelector("#profile-identity-real-hint");
  const identityAutoHint = overlay.querySelector("#profile-identity-auto-hint");
  const identityTemplateField = overlay.querySelector("[name='identityTemplate']");
  const identityTemplateToggle = overlay.querySelector("#profile-identity-template-toggle");
  const identityTemplateMenu = overlay.querySelector("#profile-identity-template-menu");
  const identityTemplateSummary = overlay.querySelector("#profile-identity-template-summary");
  const identityTemplateSearch = overlay.querySelector("#profile-identity-template-search");
  const identityTemplateOptions = overlay.querySelector("#profile-identity-template-options");
  const extensionsTable = overlay.querySelector("#profile-extensions-table");
  const extensionSelect = overlay.querySelector("[name='extensionSelect']");

  let identityPresetState = (() => {
    try {
      return JSON.parse(identityStateNode?.dataset?.preset ?? "{}");
    } catch {
      return buildRealPreset();
    }
  })();
  const identityUiState = (() => {
    try {
      return JSON.parse(identityStateNode?.dataset?.ui ?? "{}");
    } catch {
      return inferIdentityUiState(identityPresetState);
    }
  })();
  const identityTemplates = (() => {
    try {
      return JSON.parse(identityTemplatesNode?.dataset?.templates ?? "[]");
    } catch {
      return listIdentityTemplates(t);
    }
  })();
  const filteredIdentityTemplates = () => identityTemplates.filter((item) =>
    normalizeTemplatePlatform(item.platformFamily) === normalizeTemplatePlatform(identityUiState.templatePlatform)
  );
  const applyIdentityTemplateSearch = () => {
    const query = String(identityTemplateSearch?.value ?? "").trim().toLowerCase();
    for (const optionEl of overlay.querySelectorAll("[data-identity-template-option]")) {
      const haystack = optionEl.getAttribute("data-identity-template-option") || "";
      optionEl.classList.toggle("hidden", Boolean(query) && !haystack.includes(query));
    }
  };
  const renderIdentityTemplateOptions = () => {
    if (!identityTemplateOptions) return;
    identityTemplateOptions.innerHTML = templateDropdownOptionsHtml(
      t,
      filteredIdentityTemplates(),
      identityUiState.templateKey
    );
    for (const checkbox of identityTemplateOptions.querySelectorAll("[data-identity-template-key]")) {
      checkbox.addEventListener("change", () => {
        selectIdentityTemplate(checkbox.getAttribute("data-identity-template-key"));
        identityTemplateMenu?.classList.add("hidden");
      });
    }
    applyIdentityTemplateSearch();
  };
  const renderIdentityControls = () => {
    const isAuto = identityUiState.mode === "auto";
    const isReal = identityUiState.mode === "real";
    if (identityModeField) identityModeField.value = isReal ? "real" : isAuto ? "auto" : "manual";
    if (identityPlatformField) identityPlatformField.value = normalizeAutoPlatform(identityUiState.autoPlatform);
    if (identityTemplatePlatformField) {
      identityTemplatePlatformField.value = normalizeTemplatePlatform(identityUiState.templatePlatform);
    }
    identityPlatformRow?.classList.toggle("hidden", !isAuto);
    identityTemplateRow?.classList.toggle("hidden", isAuto || isReal);
    identityRealHint?.classList.toggle("hidden", !isReal);
    identityAutoHint?.classList.toggle("hidden", !isAuto);
    if (identityTemplateField) identityTemplateField.value = isAuto || isReal ? "" : identityUiState.templateKey;
    renderIdentityTemplateOptions();
    if (identityTemplateSummary) {
      identityTemplateSummary.textContent = templateSummaryLabel(t, identityTemplates, identityUiState.templateKey);
    }
  };
  const selectIdentityTemplate = (templateKey) => {
    identityUiState.templateKey = templateKey || firstTemplateKeyForTemplatePlatform(identityUiState.templatePlatform);
    identityPresetState = buildManualPreset(identityUiState.templateKey, Date.now());
    identityUiState.autoPlatform = normalizeAutoPlatform(identityPresetState.auto_platform);
    identityUiState.templatePlatform = normalizeTemplatePlatform(
      identityTemplates.find((item) => item.key === identityUiState.templateKey)?.platformFamily ?? identityUiState.templatePlatform
    );
    if (identityDisplayNameField) {
      identityDisplayNameField.value = identityTemplates.find((item) => item.key === identityUiState.templateKey)?.label ?? identityDisplayNameField.value;
    }
    markDirty();
    renderIdentityControls();
  };
  identityModeField?.addEventListener("change", () => {
    identityUiState.mode = identityModeField.value === "real"
      ? "real"
      : identityModeField.value === "auto"
        ? "auto"
        : "manual";
    if (identityUiState.mode === "real") {
      identityPresetState = buildRealPreset(Date.now());
    } else if (identityUiState.mode === "manual") {
      if (!identityUiState.templateKey) {
        identityUiState.templateKey = firstTemplateKeyForTemplatePlatform(identityUiState.templatePlatform);
      }
      if (identityPresetState?.mode === "auto" || identityPresetState?.mode === "real") {
        identityPresetState = buildManualPreset(identityUiState.templateKey, Date.now());
      }
    }
    markDirty();
    renderIdentityControls();
  });
  identityPlatformField?.addEventListener("change", () => {
    identityUiState.autoPlatform = normalizeAutoPlatform(identityPlatformField.value);
    markDirty();
  });
  identityTemplatePlatformField?.addEventListener("change", () => {
    identityUiState.templatePlatform = normalizeTemplatePlatform(identityTemplatePlatformField.value);
    identityUiState.templateKey = firstTemplateKeyForTemplatePlatform(identityUiState.templatePlatform);
    identityPresetState = buildManualPreset(identityUiState.templateKey, Date.now());
    identityUiState.autoPlatform = normalizeAutoPlatform(identityPresetState.auto_platform);
    if (identityDisplayNameField) {
      identityDisplayNameField.value = identityTemplates.find((item) => item.key === identityUiState.templateKey)?.label ?? identityDisplayNameField.value;
    }
    markDirty();
    renderIdentityControls();
  });
  identityTemplateToggle?.addEventListener("click", (event) => {
    event.preventDefault();
    event.stopPropagation();
    identityTemplateMenu?.classList.toggle("hidden");
    if (!identityTemplateMenu?.classList.contains("hidden")) {
      setTimeout(() => identityTemplateSearch?.focus(), 0);
    }
  });
  identityTemplateMenu?.addEventListener("click", (event) => event.stopPropagation());
  identityTemplateSearch?.addEventListener("input", applyIdentityTemplateSearch);
  renderIdentityControls();

  const tagsState = uniqueTags(profileTags(existing ?? { tags: [] }) ?? []);
  const profileTagState = {
    selected: [...tagsState],
    available: collectProfileTags(model.profiles)
  };
  const profileTagPicker = wireTagPicker(overlay, {
    id: "profile-tags",
    state: profileTagState,
    emptyLabel: t("profile.tags.empty"),
    searchPlaceholder: t("profile.tags.search"),
    createLabel: (value) => t("profile.tags.create").replace("{tag}", value),
    onChange(selected) {
      markDirty();
      tagsState.splice(0, tagsState.length, ...uniqueTags(selected ?? []));
    }
  });
  profileTagPicker?.rerender(profileTagState.available, profileTagState.selected);

  const extensionState = (() => {
    try {
      return {
        enabled: JSON.parse(extensionsTable?.dataset?.enabled ?? "[]"),
        disabled: JSON.parse(extensionsTable?.dataset?.disabled ?? "[]")
      };
    } catch {
      return { enabled: [], disabled: [] };
    }
  })();
  const renderExtensions = () => {
    if (!extensionsTable) return;
    const rows = [];
    for (const id of extensionState.enabled) {
      rows.push(`<tr><td>${escapeHtml(extensionDisplayName(model, id))}</td><td>${t("extensions.status.enabled")}</td><td class="actions"><button type="button" data-ext-toggle="${escapeHtml(id)}">${t("extensions.disable")}</button><button type="button" data-ext-remove="${escapeHtml(id)}">${t("extensions.remove")}</button></td></tr>`);
    }
    for (const id of extensionState.disabled) {
      rows.push(`<tr><td>${escapeHtml(extensionDisplayName(model, id))}</td><td>${t("extensions.status.disabled")}</td><td class="actions"><button type="button" data-ext-toggle="${escapeHtml(id)}">${t("extensions.enable")}</button><button type="button" data-ext-remove="${escapeHtml(id)}">${t("extensions.remove")}</button></td></tr>`);
    }
    extensionsTable.innerHTML = rows.join("") || `<tr><td colspan="3" class="meta">${t("extensions.empty")}</td></tr>`;
    for (const btn of extensionsTable.querySelectorAll("[data-ext-toggle]")) {
      btn.addEventListener("click", () => {
        const id = btn.getAttribute("data-ext-toggle");
        if (extensionState.enabled.includes(id)) {
          extensionState.enabled = extensionState.enabled.filter((x) => x !== id);
          if (!extensionState.disabled.includes(id)) extensionState.disabled.push(id);
        } else {
          extensionState.disabled = extensionState.disabled.filter((x) => x !== id);
          if (!extensionState.enabled.includes(id)) extensionState.enabled.push(id);
        }
        renderExtensions();
      });
    }
    for (const btn of extensionsTable.querySelectorAll("[data-ext-remove]")) {
      btn.addEventListener("click", () => {
        const id = btn.getAttribute("data-ext-remove");
        extensionState.enabled = extensionState.enabled.filter((x) => x !== id);
        extensionState.disabled = extensionState.disabled.filter((x) => x !== id);
        renderExtensions();
      });
    }
  };
  renderExtensions();
  overlay.querySelector("#profile-extension-add")?.addEventListener("click", () => {
    const id = extensionSelect?.value?.trim();
    if (!id) return;
    if (!extensionState.enabled.includes(id) && !extensionState.disabled.includes(id)) {
      extensionState.enabled.push(id);
      renderExtensions();
      markDirty();
    }
  });

  return {
    identityUiState,
    identityTemplates,
    getIdentityPresetState: () => identityPresetState,
    tagsState,
    extensionState,
    profileTagPicker,
    identityTemplateToggle,
    identityTemplateMenu
  };
}
