import { IDENTITY_TEMPLATES } from "./identity-template-catalog.js";

export const SEARCH_PROVIDER_PRESETS = [
  { id: "google", name: "Google", template: "https://www.google.com/search?q={query}" },
  { id: "duckduckgo", name: "DuckDuckGo", template: "https://duckduckgo.com/?q={query}" },
  { id: "bing", name: "Bing", template: "https://www.bing.com/search?q={query}" },
  { id: "yandex", name: "Yandex", template: "https://yandex.com/search/?text={query}" },
  { id: "brave", name: "Brave", template: "https://search.brave.com/search?q={query}" },
  { id: "startpage", name: "Startpage", template: "https://www.startpage.com/search?q={query}" },
  { id: "ecosia", name: "Ecosia", template: "https://www.ecosia.org/search?q={query}" },
  { id: "qwant", name: "Qwant", template: "https://www.qwant.com/?q={query}" },
  { id: "searxng", name: "SearXNG", template: "https://searx.be/search?q={query}" }
];

function cloneJson(value) {
  return JSON.parse(JSON.stringify(value));
}

function toUnsignedSeed(seed) {
  const numeric = Number.isFinite(Number(seed)) ? Math.trunc(Number(seed)) : Date.now();
  return numeric >>> 0;
}

export function findIdentityTemplate(templateKey) {
  return IDENTITY_TEMPLATES.find((item) => item.key === templateKey) ?? IDENTITY_TEMPLATES[0];
}

export function buildPreset(templateKey, seed = Date.now()) {
  const base = cloneJson(findIdentityTemplate(templateKey));
  const normalizedSeed = toUnsignedSeed(seed);
  const batteryDrift = (((normalizedSeed >>> 4) % 5) - 2) * 0.02;
  return {
    mode: "manual",
    auto_platform: base.autoPlatform,
    display_name: base.label,
    core: { ...base.core },
    hardware: { ...base.hardware },
    screen: { ...base.screen },
    window: { ...base.window },
    locale: { ...base.locale, languages: [...base.locale.languages] },
    geo: { ...base.geo },
    auto_geo: { enabled: false },
    webgl: { ...base.webgl },
    canvas_noise_seed: (normalizedSeed ^ ((base.screen.width & 0xffff) << 8) ^ base.screen.height) >>> 0,
    fonts: [...base.fonts],
    audio: { sample_rate: 48000, max_channels: 2 },
    battery: {
      charging: Boolean(base.battery.charging),
      level: Math.max(0.18, Math.min(0.99, base.battery.level + batteryDrift))
    }
  };
}

export { IDENTITY_TEMPLATES };
