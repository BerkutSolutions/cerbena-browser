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
  const normalized = String(value ?? "real").toLowerCase();
  return ["real", "auto", "manual"].includes(normalized) ? normalized : "real";
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

function detectBrandFromUserAgent(userAgent) {
  const ua = String(userAgent ?? "");
  const patterns = [
    { brand: "Edge", vendor: "Microsoft", regex: /Edg\/([\d.]+)/ },
    { brand: "Opera", vendor: "Opera", regex: /OPR\/([\d.]+)/ },
    { brand: "Firefox", vendor: "Mozilla", regex: /Firefox\/([\d.]+)/ },
    { brand: "Chrome", vendor: "Google", regex: /Chrome\/([\d.]+)/ },
    { brand: "Safari", vendor: "Apple", regex: /Version\/([\d.]+).*Safari\// }
  ];
  for (const item of patterns) {
    const match = ua.match(item.regex);
    if (match) {
      return {
        brand: item.brand,
        brandVersion: match[1]?.split(".")[0] ?? match[1] ?? "",
        vendor: item.vendor
      };
    }
  }
  return {
    brand: "Unknown",
    brandVersion: "",
    vendor: globalThis.navigator?.vendor || "Unknown"
  };
}

function detectPlatformVersion(userAgent) {
  const ua = String(userAgent ?? "");
  return ua.match(/Windows NT ([\d.]+)/)?.[1]
    ?? ua.match(/Mac OS X ([\d_]+)/)?.[1]?.replaceAll("_", ".")
    ?? ua.match(/Android ([\d.]+)/)?.[1]
    ?? ua.match(/CPU (?:iPhone )?OS ([\d_]+)/)?.[1]?.replaceAll("_", ".")
    ?? ua.match(/Linux ([\w.-]+)/)?.[1]
    ?? "";
}

function detectWebGlProfile() {
  try {
    const canvas = document.createElement("canvas");
    const gl = canvas.getContext("webgl") || canvas.getContext("experimental-webgl");
    if (!gl) {
      return {
        vendor: "Unavailable",
        renderer: "Unavailable",
        params_json: "{\"webgl\":false}"
      };
    }
    const debugInfo = gl.getExtension("WEBGL_debug_renderer_info");
    const vendor = debugInfo ? gl.getParameter(debugInfo.UNMASKED_VENDOR_WEBGL) : gl.getParameter(gl.VENDOR);
    const renderer = debugInfo ? gl.getParameter(debugInfo.UNMASKED_RENDERER_WEBGL) : gl.getParameter(gl.RENDERER);
    return {
      vendor: String(vendor || "Unknown"),
      renderer: String(renderer || "Unknown"),
      params_json: JSON.stringify({
        webgl: true,
        maxTextureSize: gl.getParameter(gl.MAX_TEXTURE_SIZE),
        maxRenderbufferSize: gl.getParameter(gl.MAX_RENDERBUFFER_SIZE),
        antialias: Boolean(gl.getContextAttributes?.().antialias)
      })
    };
  } catch {
    return {
      vendor: "Unavailable",
      renderer: "Unavailable",
      params_json: "{\"webgl\":false}"
    };
  }
}

function detectInstalledFonts() {
  const candidates = [
    "Arial",
    "Segoe UI",
    "Tahoma",
    "Verdana",
    "Times New Roman",
    "Courier New",
    "Georgia",
    "Trebuchet MS"
  ];
  if (!document?.fonts?.check) {
    return ["Arial"];
  }
  const detected = candidates.filter((font) => document.fonts.check(`12px "${font}"`));
  return detected.length ? detected : ["Arial"];
}

function detectAudioProfile() {
  try {
    const Ctx = window.AudioContext || window.webkitAudioContext;
    if (!Ctx) {
      return { sample_rate: 48000, max_channels: 2 };
    }
    const context = new Ctx();
    const sampleRate = Number(context.sampleRate) || 48000;
    const maxChannels = Number(context.destination?.maxChannelCount ?? 2) || 2;
    context.close?.();
    return {
      sample_rate: sampleRate,
      max_channels: Math.max(1, Math.min(32, maxChannels))
    };
  } catch {
    return { sample_rate: 48000, max_channels: 2 };
  }
}

export function buildRealPreset(seed = Date.now()) {
  const normalizedSeed = Number.isFinite(Number(seed)) ? Math.trunc(Number(seed)) : Date.now();
  const nav = globalThis.navigator ?? {};
  const screenObj = globalThis.screen ?? {};
  const userAgent = String(nav.userAgent ?? "Mozilla/5.0");
  const brandProfile = detectBrandFromUserAgent(userAgent);
  const webgl = detectWebGlProfile();
  const audio = detectAudioProfile();
  const width = Number(screenObj.width ?? window.outerWidth ?? window.innerWidth ?? 1366);
  const height = Number(screenObj.height ?? window.outerHeight ?? window.innerHeight ?? 768);
  const availWidth = Math.min(width, Number(screenObj.availWidth ?? width));
  const availHeight = Math.min(height, Number(screenObj.availHeight ?? height));
  const language = String(nav.language ?? "en-US");
  const languages = Array.isArray(nav.languages) && nav.languages.length ? [...nav.languages] : [language];
  const timezone = Intl.DateTimeFormat().resolvedOptions().timeZone || "UTC";
  const timezoneOffsetMinutes = Number(new Date().getTimezoneOffset()) || 0;
  return {
    mode: "real",
    auto_platform: null,
    display_name: "Real device",
    core: {
      user_agent: userAgent,
      platform: String(nav.userAgentData?.platform ?? nav.platform ?? "Unknown"),
      platform_version: detectPlatformVersion(userAgent),
      brand: brandProfile.brand,
      brand_version: brandProfile.brandVersion,
      vendor: String(nav.vendor || brandProfile.vendor || "Unknown"),
      vendor_sub: "",
      product_sub: String(nav.productSub ?? "20030107")
    },
    hardware: {
      cpu_threads: Math.max(1, Math.min(256, Number(nav.hardwareConcurrency ?? 8) || 8)),
      max_touch_points: Math.max(0, Math.min(16, Number(nav.maxTouchPoints ?? 0) || 0)),
      device_memory_gb: Math.max(1, Math.min(1024, Number(nav.deviceMemory ?? 8) || 8))
    },
    screen: {
      width,
      height,
      device_pixel_ratio: Number(window.devicePixelRatio ?? 1) || 1,
      avail_width: availWidth,
      avail_height: availHeight,
      color_depth: Number(screenObj.colorDepth ?? 24) || 24
    },
    window: {
      outer_width: Math.max(320, Number(window.outerWidth ?? width) || width),
      outer_height: Math.max(240, Number(window.outerHeight ?? height) || height),
      inner_width: Math.max(320, Number(window.innerWidth ?? availWidth) || availWidth),
      inner_height: Math.max(240, Number(window.innerHeight ?? availHeight) || availHeight),
      screen_x: Number(window.screenX ?? window.screenLeft ?? 0) || 0,
      screen_y: Number(window.screenY ?? window.screenTop ?? 0) || 0
    },
    locale: {
      navigator_language: language,
      languages,
      do_not_track: String(nav.doNotTrack ?? "unspecified"),
      timezone_iana: timezone,
      timezone_offset_minutes: timezoneOffsetMinutes
    },
    geo: {
      latitude: 0,
      longitude: 0,
      accuracy_meters: 100000
    },
    auto_geo: { enabled: false },
    webgl,
    canvas_noise_seed: (normalizedSeed ^ ((width & 0xffff) << 8) ^ height) >>> 0,
    fonts: detectInstalledFonts(),
    audio,
    battery: {
      charging: true,
      level: 1
    }
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
