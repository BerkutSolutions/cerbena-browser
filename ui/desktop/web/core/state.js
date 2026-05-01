import { featureRegistry, isKnownFeature } from "./feature-registry.js";

const FEATURE_STORAGE_KEY = "launcher.feature";
const LOCALE_STORAGE_KEY = "launcher.locale";

function pickSavedLocale() {
  const savedLocale = localStorage.getItem(LOCALE_STORAGE_KEY);
  return savedLocale === "ru" || savedLocale === "en" ? savedLocale : "ru";
}

export function createUiState() {
  const savedFeature = localStorage.getItem(FEATURE_STORAGE_KEY);
  return {
    currentFeature: isKnownFeature(savedFeature) ? savedFeature : featureRegistry[0].key,
    locale: pickSavedLocale(),
    sidebarCollapsed: false,
    correlationId: crypto.randomUUID()
  };
}

export function persistSelectedFeature(feature) {
  localStorage.setItem(FEATURE_STORAGE_KEY, feature);
}
