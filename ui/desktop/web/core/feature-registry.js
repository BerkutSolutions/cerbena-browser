export const featureRegistry = [
  { key: "home", labelKey: "nav.home" },
  { key: "extensions", labelKey: "nav.extensions" },
  { key: "security", labelKey: "nav.security" },
  { key: "identity", labelKey: "nav.identity" },
  { key: "dns", labelKey: "nav.dns" },
  { key: "network", labelKey: "nav.network" },
  { key: "traffic", labelKey: "nav.traffic" },
  { key: "logs", labelKey: "nav.logs" },
  { key: "settings", labelKey: "nav.settings" }
];

export function assertFeatureKey(key) {
  if (!featureRegistry.some((entry) => entry.key === key)) {
    throw new Error(`Unknown feature key: ${key}`);
  }
}

export function isKnownFeature(key) {
  return featureRegistry.some((entry) => entry.key === key);
}
