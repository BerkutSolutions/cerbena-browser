import { buildPreset, IDENTITY_TEMPLATES } from "../../core/catalogs.js";

export const AUTO_IDENTITY_PLATFORMS = [
  { key: "windows", labelKey: "identity.platform.windows" },
  { key: "windows8", labelKey: "identity.platform.windows8" },
  { key: "macos", labelKey: "identity.platform.macos" },
  { key: "linux", labelKey: "identity.platform.linux" },
  { key: "debian", labelKey: "identity.platform.debian" },
  { key: "ubuntu", labelKey: "identity.platform.ubuntu" },
  { key: "ios", labelKey: "identity.platform.ios" },
  { key: "android", labelKey: "identity.platform.android" }
];

export const TEMPLATE_PLATFORM_OPTIONS = [
  { key: "windows", labelKey: "identity.platform.windows" },
  { key: "macos", labelKey: "identity.platform.macos" },
  { key: "linux", labelKey: "identity.platform.linux" },
  { key: "ios", labelKey: "identity.platform.ios" },
  { key: "android", labelKey: "identity.platform.android" }
];

const KNOWN_AUTO_PLATFORM_KEYS = new Set([
  ...AUTO_IDENTITY_PLATFORMS.map((item) => item.key)
]);

export function normalizeTemplatePlatform(value) {
  const normalized = String(value ?? "windows").toLowerCase();
  return TEMPLATE_PLATFORM_OPTIONS.some((item) => item.key === normalized) ? normalized : "windows";
}

export function normalizeIdentityMode(value) {
  return String(value ?? "manual").toLowerCase() === "auto" ? "auto" : "manual";
}

export function normalizeAutoPlatform(value) {
  const normalized = String(value ?? "windows").toLowerCase();
  return KNOWN_AUTO_PLATFORM_KEYS.has(normalized) ? normalized : "windows";
}

export function listIdentityPlatforms(t) {
  return AUTO_IDENTITY_PLATFORMS.map((item) => ({
    ...item,
    label: t(item.labelKey)
  }));
}

export function listIdentityTemplatePlatforms(t) {
  return TEMPLATE_PLATFORM_OPTIONS.map((item) => ({
    ...item,
    label: t(item.labelKey)
  }));
}

export function listIdentityTemplates(t, options = {}) {
  const allowedPlatforms = Array.isArray(options.platforms) && options.platforms.length
    ? new Set(options.platforms.map((item) => normalizeAutoPlatform(item)))
    : null;
  const allowedFamilies = Array.isArray(options.platformFamilies) && options.platformFamilies.length
    ? new Set(options.platformFamilies.map((item) => normalizeTemplatePlatform(item)))
    : null;
  return IDENTITY_TEMPLATES
    .filter((item) => !allowedPlatforms || allowedPlatforms.has(normalizeAutoPlatform(item.autoPlatform)))
    .filter((item) => !allowedFamilies || allowedFamilies.has(normalizeTemplatePlatform(item.platformFamily)))
    .map((item) => ({
      ...item,
      label: item.label ?? t(item.labelKey)
    }));
}

export function firstTemplateKeyForPlatform(platform) {
  const normalized = normalizeAutoPlatform(platform);
  return IDENTITY_TEMPLATES.find((item) => normalizeAutoPlatform(item.autoPlatform) === normalized)?.key
    ?? IDENTITY_TEMPLATES[0]?.key
    ?? "";
}

export function firstTemplateKeyForTemplatePlatform(platformFamily) {
  const normalized = normalizeTemplatePlatform(platformFamily);
  return IDENTITY_TEMPLATES.find((item) => normalizeTemplatePlatform(item.platformFamily) === normalized)?.key
    ?? IDENTITY_TEMPLATES[0]?.key
    ?? "";
}

export function detectTemplateKey(preset) {
  if (!preset?.core) return firstTemplateKeyForPlatform(preset?.auto_platform ?? "windows");
  const corePlatform = String(preset.core.platform ?? "").toLowerCase();
  const brand = String(preset.core.brand ?? "").toLowerCase();
  const userAgent = String(preset.core.user_agent ?? "");
  const width = Number(preset.screen?.width ?? 0);
  const height = Number(preset.screen?.height ?? 0);
  const exact = IDENTITY_TEMPLATES.find((item) =>
    item.core.user_agent === userAgent
    || (
      item.core.platform.toLowerCase() === corePlatform
      && item.core.brand.toLowerCase() === brand
      && Number(item.screen.width) === width
      && Number(item.screen.height) === height
    )
  );
  if (exact) return exact.key;
  return IDENTITY_TEMPLATES.find((item) => item.core.platform.toLowerCase() === corePlatform)?.key
    ?? firstTemplateKeyForPlatform(preset?.auto_platform ?? "windows");
}

export function inferIdentityUiState(preset) {
  const mode = normalizeIdentityMode(preset?.mode);
  const autoPlatform = normalizeAutoPlatform(preset?.auto_platform ?? preset?.autoPlatform);
  const templateKey = detectTemplateKey(preset);
  const templatePlatform = normalizeTemplatePlatform(
    IDENTITY_TEMPLATES.find((item) => item.key === templateKey)?.platformFamily ?? autoPlatform
  );
  return {
    mode,
    autoPlatform,
    templateKey,
    templatePlatform
  };
}

export function buildManualPreset(templateKey, seed = Date.now()) {
  const preset = buildPreset(templateKey, seed);
  preset.mode = "manual";
  preset.auto_platform = findTemplateAutoPlatform(templateKey);
  return preset;
}

export function cloneIdentityPreset(preset) {
  return JSON.parse(JSON.stringify(preset));
}

export function findTemplateAutoPlatform(templateKey) {
  return normalizeAutoPlatform(
    IDENTITY_TEMPLATES.find((item) => item.key === templateKey)?.autoPlatform ?? "windows"
  );
}
