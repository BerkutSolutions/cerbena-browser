import {
  applyIdentityAutoGeo,
  generateAutoPreset,
  getIdentityProfile,
  previewIdentityPreset,
  saveIdentityProfile,
  validateIdentitySave
} from "./api.js";
import {
  buildRealPreset,
  buildManualPreset,
  findTemplateAutoPlatform,
  firstTemplateKeyForPlatform,
  firstTemplateKeyForTemplatePlatform,
  inferIdentityUiState,
  listIdentityPlatforms,
  listIdentityTemplatePlatforms,
  listIdentityTemplates,
  normalizeTemplatePlatform,
  normalizeAutoPlatform
} from "./shared.js";
import {
  collectPreset,
  ensureIdentityUi,
  escapeHtml,
  fallbackPreset,
  refreshAutoDraft,
  renderManualFields,
  resolveEffectivePreset,
  setNotice
} from "./view-core-support.js";

export function renderIdentity(t, model) {
  const uiState = ensureIdentityUi(model);
  const preset = model.identityDraft ?? fallbackPreset();
  const isAuto = uiState.mode === "auto";
  const isReal = uiState.mode === "real";
  const notice = model.identityNotice ? `<p class="notice ${model.identityNotice.type}">${model.identityNotice.text}</p>` : "";
  const templatePlatformOptions = listIdentityTemplatePlatforms(t)
    .map((item) => `<option value="${item.key}" ${item.key === uiState.templatePlatform ? "selected" : ""}>${escapeHtml(item.label)}</option>`)
    .join("");
  const templateOptions = listIdentityTemplates(t, { platformFamilies: [uiState.templatePlatform] })
    .map((item) => `<option value="${item.key}" ${item.key === uiState.templateKey ? "selected" : ""}>${escapeHtml(item.label)}</option>`)
    .join("");
  const platformOptions = listIdentityPlatforms(t)
    .map((item) => `<option value="${item.key}" ${item.key === uiState.autoPlatform ? "selected" : ""}>${escapeHtml(item.label)}</option>`)
    .join("");
  return `
    <div class="feature-page">
      <div class="feature-page-head row-between">
        <h2>${t("nav.identity")}</h2>
        <div class="top-actions">
          <button id="identity-load">${t("identity.action.load")}</button>
          <button id="identity-save">${t("identity.action.save")}</button>
          <button id="identity-validate">${t("identity.action.validate")}</button>
        </div>
      </div>
      ${notice}
      <div class="grid-two" style="margin-top:10px;">
        <label>${t("identity.field.mode")}
          <select id="identity-mode" class="identity-mode-select">
            <option value="real" ${isReal ? "selected" : ""}>${t("identity.mode.real")}</option>
            <option value="auto" ${isAuto ? "selected" : ""}>${t("identity.mode.auto")}</option>
            <option value="manual" ${!isAuto && !isReal ? "selected" : ""}>${t("identity.mode.manual")}</option>
          </select>
        </label>
        ${isReal ? `` : isAuto ? `
          <label>${t("identity.field.autoPlatform")}
            <select id="identity-auto-platform">${platformOptions}</select>
          </label>
        ` : `
          <label>${t("identity.field.platformTemplate")}
            <select id="identity-template-platform">${templatePlatformOptions}</select>
          </label>
          <label>${t("profile.identity.template")}
            <select id="identity-template">${templateOptions}</select>
          </label>
        `}
      </div>
      ${isReal ? `
        <p class="meta" style="margin-top:10px;">${t("identity.realHint")}</p>
      ` : isAuto ? `
        <p class="meta" style="margin-top:10px;">${t("identity.autoHint")}</p>
      ` : `
        <div class="top-actions" style="margin-top:10px;">
          <button id="identity-manual-generate">${t("identity.action.generate")}</button>
        </div>
        ${renderManualFields(preset, t)}
      `}
      <div class="panel" style="margin-top:12px;">
        <strong>${t("identity.preview.title")}</strong>
        <pre id="identity-preview" class="preview-box">${escapeHtml(model.identityPreview ?? "-")}</pre>
      </div>
    </div>
  `;
}

export async function wireIdentity(root, model, rerender, t) {
  ensureIdentityUi(model);

  root.querySelector("#identity-load")?.addEventListener("click", async () => {
    if (!model.selectedProfileId) return;
    const result = await getIdentityProfile(model.selectedProfileId);
    model.identityDraft = result.ok ? result.data ?? fallbackPreset() : fallbackPreset();
    model.identityUi = inferIdentityUiState(model.identityDraft);
    if (model.identityUi.mode === "real") {
      model.identityDraft = buildRealPreset(Date.now());
    } else if (model.identityUi.mode === "auto") {
      await refreshAutoDraft(model, model.identityUi.autoPlatform, t);
    }
    rerender();
  });

  root.querySelector("#identity-mode")?.addEventListener("change", async (event) => {
    const mode = event.target.value === "real" ? "real" : event.target.value === "auto" ? "auto" : "manual";
    const uiState = ensureIdentityUi(model);
    uiState.mode = mode;
    if (mode === "real") {
      model.identityDraft = buildRealPreset(Date.now());
    } else if (mode === "auto") {
      await refreshAutoDraft(model, uiState.autoPlatform, t);
    } else {
      model.identityDraft = buildManualPreset(uiState.templateKey, Date.now());
    }
    rerender();
  });

  root.querySelector("#identity-auto-platform")?.addEventListener("change", async (event) => {
    const platform = normalizeAutoPlatform(event.target.value);
    ensureIdentityUi(model).autoPlatform = platform;
    await refreshAutoDraft(model, platform, t);
    rerender();
  });

  root.querySelector("#identity-template")?.addEventListener("change", async (event) => {
    const uiState = ensureIdentityUi(model);
    const templateKey = event.target.value || firstTemplateKeyForTemplatePlatform(uiState.templatePlatform);
    uiState.templateKey = templateKey;
    model.identityDraft = buildManualPreset(templateKey, Date.now());
    uiState.templateLabel = listIdentityTemplates(t, { platformFamilies: [uiState.templatePlatform] })
      .find((item) => item.key === templateKey)?.label ?? uiState.templateLabel ?? "";
    rerender();
  });

  root.querySelector("#identity-template-platform")?.addEventListener("change", async (event) => {
    const templatePlatform = normalizeTemplatePlatform(event.target.value);
    const uiState = ensureIdentityUi(model);
    uiState.templatePlatform = templatePlatform;
    uiState.templateKey = firstTemplateKeyForTemplatePlatform(templatePlatform);
    model.identityDraft = buildManualPreset(uiState.templateKey, Date.now());
    uiState.templateLabel = listIdentityTemplates(t, { platformFamilies: [templatePlatform] })
      .find((item) => item.key === uiState.templateKey)?.label ?? uiState.templateLabel ?? "";
    rerender();
  });

  root.querySelector("#identity-manual-generate")?.addEventListener("click", async () => {
    const templateKey = ensureIdentityUi(model).templateKey;
    model.identityDraft = buildManualPreset(templateKey, Date.now());
    setNotice(model, "success", t("identity.notice.autoGenerated"));
    rerender();
  });

  root.querySelector("#identity-validate")?.addEventListener("click", async () => {
    const preset = await resolveEffectivePreset(root, model, t);
    if (!preset) {
      rerender();
      return;
    }
    const result = await validateIdentitySave(preset, "direct");
    model.identityPreview = result.ok ? result.data : JSON.stringify(result.data, null, 2);
    setNotice(model, result.ok ? "success" : "error", result.ok ? t("identity.notice.validated") : t("identity.notice.validationFailed"));
    rerender();
  });

  root.querySelector("#identity-save")?.addEventListener("click", async () => {
    if (!model.selectedProfileId) {
      setNotice(model, "error", t("identity.notice.selectProfile"));
      rerender();
      return;
    }

    const preset = await resolveEffectivePreset(root, model, t);
    if (!preset) {
      rerender();
      return;
    }

    const preview = await previewIdentityPreset(preset, "direct");
    model.identityPreview = preview.ok ? preview.data : JSON.stringify(preview.data, null, 2);

    const validated = await validateIdentitySave(preset, "direct");
    if (!validated.ok) {
      setNotice(model, "error", t("identity.notice.validationFailed"));
      rerender();
      return;
    }

    const outcome = JSON.parse(validated.data);
    if (!outcome.allowed_to_save) {
      setNotice(model, "error", t("identity.notice.saveBlocked"));
      model.identityPreview = validated.data;
      rerender();
      return;
    }

    const saved = await saveIdentityProfile(model.selectedProfileId, preset);
    setNotice(model, saved.ok ? "success" : "error", saved.ok ? t("identity.notice.saved") : String(saved.data.error));
    model.identityDraft = preset;
    model.identityUi = inferIdentityUiState(preset);
    rerender();
  });
}
