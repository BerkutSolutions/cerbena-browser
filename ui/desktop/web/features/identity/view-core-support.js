import { applyIdentityAutoGeo, generateAutoPreset } from "./api.js";
import {
  buildRealPreset,
  findTemplateAutoPlatform,
  firstTemplateKeyForPlatform,
  inferIdentityUiState,
  listIdentityTemplates,
  normalizeAutoPlatform,
  normalizeTemplatePlatform
} from "./shared.js";

function escapeHtml(value) {
  return String(value ?? "")
    .replaceAll("&", "&amp;")
    .replaceAll("<", "&lt;")
    .replaceAll(">", "&gt;")
    .replaceAll("\"", "&quot;");
}

function fallbackPreset() {
  return buildRealPreset(123456);
}

function parseJsonField(value, fallback) {
  try {
    return JSON.parse(value);
  } catch {
    return fallback;
  }
}

function setNotice(model, type, text) {
  model.identityNotice = { type, text, at: Date.now() };
}

function ensureIdentityUi(model) {
  if (!model.identityDraft) {
    model.identityDraft = fallbackPreset();
  }
  model.identityUi = {
    ...inferIdentityUiState(model.identityDraft),
    ...(model.identityUi ?? {})
  };
  model.identityUi.mode = ["real", "auto", "manual"].includes(String(model.identityUi.mode ?? "").toLowerCase())
    ? String(model.identityUi.mode).toLowerCase()
    : "real";
  model.identityUi.autoPlatform = normalizeAutoPlatform(model.identityUi.autoPlatform ?? model.identityDraft?.auto_platform);
  if (!model.identityUi.templateKey) {
    model.identityUi.templateKey = firstTemplateKeyForPlatform(model.identityUi.autoPlatform);
  }
  model.identityUi.templatePlatform = normalizeTemplatePlatform(model.identityUi.templatePlatform);
  model.identityUi.templateLabel = listIdentityTemplates((key) => key)
    .find((item) => item.key === model.identityUi.templateKey)?.label ?? model.identityUi.templateLabel ?? "";
  return model.identityUi;
}

function collectPreset(root, uiState) {
  return {
    mode: "manual",
    auto_platform: findTemplateAutoPlatform(uiState.templateKey),
    display_name: root.querySelector("#identity-display-name").value.trim() || uiState.templateLabel || null,
    core: {
      user_agent: root.querySelector("#identity-ua").value,
      platform: root.querySelector("#identity-platform").value,
      platform_version: root.querySelector("#identity-platform-version").value,
      brand: root.querySelector("#identity-brand").value,
      brand_version: root.querySelector("#identity-brand-version").value,
      vendor: root.querySelector("#identity-vendor").value,
      vendor_sub: root.querySelector("#identity-vendor-sub").value,
      product_sub: root.querySelector("#identity-product-sub").value
    },
    hardware: {
      cpu_threads: Number(root.querySelector("#identity-cpu").value),
      max_touch_points: Number(root.querySelector("#identity-touch").value),
      device_memory_gb: Number(root.querySelector("#identity-memory").value)
    },
    screen: {
      width: Number(root.querySelector("#identity-screen-width").value),
      height: Number(root.querySelector("#identity-screen-height").value),
      device_pixel_ratio: Number(root.querySelector("#identity-dpr").value),
      avail_width: Number(root.querySelector("#identity-avail-width").value),
      avail_height: Number(root.querySelector("#identity-avail-height").value),
      color_depth: Number(root.querySelector("#identity-color-depth").value)
    },
    window: {
      outer_width: Number(root.querySelector("#identity-outer-width").value),
      outer_height: Number(root.querySelector("#identity-outer-height").value),
      inner_width: Number(root.querySelector("#identity-inner-width").value),
      inner_height: Number(root.querySelector("#identity-inner-height").value),
      screen_x: Number(root.querySelector("#identity-screen-x").value),
      screen_y: Number(root.querySelector("#identity-screen-y").value)
    },
    locale: {
      navigator_language: root.querySelector("#identity-lang").value,
      languages: parseJsonField(root.querySelector("#identity-languages").value, ["en-US", "en"]),
      do_not_track: root.querySelector("#identity-dnt").value,
      timezone_iana: root.querySelector("#identity-tz").value,
      timezone_offset_minutes: Number(root.querySelector("#identity-tz-offset").value)
    },
    geo: {
      latitude: Number(root.querySelector("#identity-lat").value),
      longitude: Number(root.querySelector("#identity-lon").value),
      accuracy_meters: Number(root.querySelector("#identity-accuracy").value)
    },
    auto_geo: {
      enabled: root.querySelector("#identity-auto-geo-enabled").checked
    },
    webgl: {
      vendor: root.querySelector("#identity-webgl-vendor").value,
      renderer: root.querySelector("#identity-webgl-renderer").value,
      params_json: root.querySelector("#identity-webgl-params").value
    },
    canvas_noise_seed: Number(root.querySelector("#identity-canvas-seed").value),
    fonts: parseJsonField(root.querySelector("#identity-fonts").value, ["Arial"]),
    audio: {
      sample_rate: Number(root.querySelector("#identity-audio-rate").value),
      max_channels: Number(root.querySelector("#identity-audio-channels").value)
    },
    battery: {
      charging: root.querySelector("#identity-battery-charging").checked,
      level: Number(root.querySelector("#identity-battery-level").value)
    }
  };
}

async function refreshAutoDraft(model, platform, t) {
  const result = await generateAutoPreset(platform, Date.now());
  if (!result.ok) {
    setNotice(model, "error", String(result.data.error));
    return false;
  }
  model.identityDraft = result.data;
  model.identityUi = {
    ...ensureIdentityUi(model),
    mode: "auto",
    autoPlatform: platform
  };
  return true;
}

async function resolveEffectivePreset(root, model, t) {
  const uiState = ensureIdentityUi(model);
  if (uiState.mode === "real") {
    const preset = buildRealPreset(Date.now());
    model.identityDraft = preset;
    return preset;
  }
  if (uiState.mode === "auto") {
    const result = await generateAutoPreset(uiState.autoPlatform, Date.now());
    if (!result.ok) {
      setNotice(model, "error", String(result.data.error));
      return null;
    }
    model.identityDraft = result.data;
    return result.data;
  }

  let preset = collectPreset(root, uiState);
  if (preset.auto_geo.enabled) {
    const withGeo = await applyIdentityAutoGeo(preset, {
      timezone_iana: preset.locale.timezone_iana,
      timezone_offset_minutes: preset.locale.timezone_offset_minutes,
      latitude: preset.geo.latitude,
      longitude: preset.geo.longitude,
      accuracy_meters: preset.geo.accuracy_meters,
      language: preset.locale.navigator_language
    });
    if (withGeo.ok) {
      preset = withGeo.data;
    } else {
      setNotice(model, "error", String(withGeo.data.error));
      return null;
    }
  }
  model.identityDraft = preset;
  return preset;
}

function renderManualFields(preset, t) {
  return `
    <label>${t("identity.field.displayName")}<input id="identity-display-name" value="${escapeHtml(preset.display_name ?? "")}" /></label>
    <label>${t("identity.field.userAgent")}<textarea id="identity-ua">${escapeHtml(preset.core.user_agent)}</textarea></label>
    <div class="grid-two">
      <label>${t("identity.field.platform")}<input id="identity-platform" value="${escapeHtml(preset.core.platform)}" /></label>
      <label>${t("identity.field.platformVersion")}<input id="identity-platform-version" value="${escapeHtml(preset.core.platform_version)}" /></label>
      <label>${t("identity.field.brand")}<input id="identity-brand" value="${escapeHtml(preset.core.brand)}" /></label>
      <label>${t("identity.field.brandVersion")}<input id="identity-brand-version" value="${escapeHtml(preset.core.brand_version)}" /></label>
      <label>${t("identity.field.vendor")}<input id="identity-vendor" value="${escapeHtml(preset.core.vendor)}" /></label>
      <label>${t("identity.field.vendorSub")}<input id="identity-vendor-sub" value="${escapeHtml(preset.core.vendor_sub)}" /></label>
      <label>${t("identity.field.productSub")}<input id="identity-product-sub" value="${escapeHtml(preset.core.product_sub)}" /></label>
      <label>${t("identity.field.cpuThreads")}<input id="identity-cpu" type="number" value="${preset.hardware.cpu_threads}" /></label>
      <label>${t("identity.field.touchPoints")}<input id="identity-touch" type="number" value="${preset.hardware.max_touch_points}" /></label>
      <label>${t("identity.field.deviceMemory")}<input id="identity-memory" type="number" value="${preset.hardware.device_memory_gb}" /></label>
      <label>${t("identity.field.screenWidth")}<input id="identity-screen-width" type="number" value="${preset.screen.width}" /></label>
      <label>${t("identity.field.screenHeight")}<input id="identity-screen-height" type="number" value="${preset.screen.height}" /></label>
      <label>${t("identity.field.dpr")}<input id="identity-dpr" type="number" step="0.01" value="${preset.screen.device_pixel_ratio}" /></label>
      <label>${t("identity.field.availWidth")}<input id="identity-avail-width" type="number" value="${preset.screen.avail_width}" /></label>
      <label>${t("identity.field.availHeight")}<input id="identity-avail-height" type="number" value="${preset.screen.avail_height}" /></label>
      <label>${t("identity.field.colorDepth")}<input id="identity-color-depth" type="number" value="${preset.screen.color_depth}" /></label>
    </div>
    <details>
      <summary>${t("identity.summary.advanced")}</summary>
      <div class="grid-two">
        <label>${t("identity.field.outerWidth")}<input id="identity-outer-width" type="number" value="${preset.window.outer_width}" /></label>
        <label>${t("identity.field.outerHeight")}<input id="identity-outer-height" type="number" value="${preset.window.outer_height}" /></label>
        <label>${t("identity.field.innerWidth")}<input id="identity-inner-width" type="number" value="${preset.window.inner_width}" /></label>
        <label>${t("identity.field.innerHeight")}<input id="identity-inner-height" type="number" value="${preset.window.inner_height}" /></label>
        <label>${t("identity.field.screenX")}<input id="identity-screen-x" type="number" value="${preset.window.screen_x}" /></label>
        <label>${t("identity.field.screenY")}<input id="identity-screen-y" type="number" value="${preset.window.screen_y}" /></label>
        <label>${t("identity.field.language")}<input id="identity-lang" value="${escapeHtml(preset.locale.navigator_language)}" /></label>
        <label>${t("identity.field.languagesJson")}<input id="identity-languages" value='${escapeHtml(JSON.stringify(preset.locale.languages))}' /></label>
        <label>${t("identity.field.dnt")}<input id="identity-dnt" value="${escapeHtml(preset.locale.do_not_track)}" /></label>
        <label>${t("identity.field.timezone")}<input id="identity-tz" value="${escapeHtml(preset.locale.timezone_iana)}" /></label>
        <label>${t("identity.field.tzOffset")}<input id="identity-tz-offset" type="number" value="${preset.locale.timezone_offset_minutes}" /></label>
        <label>${t("identity.field.lat")}<input id="identity-lat" type="number" step="0.0001" value="${preset.geo.latitude}" /></label>
        <label>${t("identity.field.lon")}<input id="identity-lon" type="number" step="0.0001" value="${preset.geo.longitude}" /></label>
        <label>${t("identity.field.accuracy")}<input id="identity-accuracy" type="number" step="0.1" value="${preset.geo.accuracy_meters}" /></label>
        <label><input id="identity-auto-geo-enabled" type="checkbox" ${preset.auto_geo.enabled ? "checked" : ""}/> ${t("identity.field.autoGeo")}</label>
        <label>${t("identity.field.webglVendor")}<input id="identity-webgl-vendor" value="${escapeHtml(preset.webgl.vendor)}" /></label>
        <label>${t("identity.field.webglRenderer")}<input id="identity-webgl-renderer" value="${escapeHtml(preset.webgl.renderer)}" /></label>
        <label>${t("identity.field.webglParams")}<input id="identity-webgl-params" value='${escapeHtml(preset.webgl.params_json)}' /></label>
        <label>${t("identity.field.canvasSeed")}<input id="identity-canvas-seed" type="number" value="${preset.canvas_noise_seed}" /></label>
        <label>${t("identity.field.fontsJson")}<input id="identity-fonts" value='${escapeHtml(JSON.stringify(preset.fonts))}' /></label>
        <label>${t("identity.field.audioRate")}<input id="identity-audio-rate" type="number" value="${preset.audio.sample_rate}" /></label>
        <label>${t("identity.field.audioChannels")}<input id="identity-audio-channels" type="number" value="${preset.audio.max_channels}" /></label>
        <label><input id="identity-battery-charging" type="checkbox" ${preset.battery.charging ? "checked" : ""}/> ${t("identity.field.batteryCharging")}</label>
        <label>${t("identity.field.batteryLevel")}<input id="identity-battery-level" type="number" step="0.01" min="0" max="1" value="${preset.battery.level}" /></label>
      </div>
    </details>
  `;
}


export {
  escapeHtml, fallbackPreset, parseJsonField, setNotice, ensureIdentityUi, collectPreset,
  refreshAutoDraft, resolveEffectivePreset, renderManualFields
};
