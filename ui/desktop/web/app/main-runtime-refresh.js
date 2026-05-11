export async function hydrateCurrentFeatureModel(featureKey, model, deps) {
  const {
    hydrateNetworkModel,
    hydrateDnsModel,
    hydrateExtensionsModel,
    hydrateTrafficModel,
    hydrateHomeModel,
    hydrateLogsModel,
    hydrateSettingsModel
  } = deps;

  if (featureKey === "network" || featureKey === "dns") await hydrateNetworkModel(model);
  if (featureKey === "dns") await hydrateDnsModel(model);
  if (featureKey === "extensions") await hydrateExtensionsModel(model);
  if (featureKey === "traffic") await hydrateTrafficModel(model);
  if (featureKey === "home") await hydrateHomeModel(model);
  if (featureKey === "logs") await hydrateLogsModel(model);
  if (featureKey === "settings") await hydrateSettingsModel(model);
}

export function createRefreshBoundaries({ rerender, state }) {
  return {
    featureSelected: async (options = {}) =>
      rerender({
        refreshProfiles: state.currentFeature === "home" || state.currentFeature === "settings",
        refreshFeature: true,
        refreshOverlay: true,
        ...options
      }),
    profiles: async (options = {}) =>
      rerender({
        refreshProfiles: true,
        refreshFeature: state.currentFeature === "home",
        refreshOverlay: true,
        ...options
      }),
    network: async (options = {}) =>
      rerender({
        refreshProfiles: false,
        refreshFeature: state.currentFeature === "network" || state.currentFeature === "dns",
        refreshOverlay: false,
        ...options
      }),
    settings: async (options = {}) =>
      rerender({
        refreshProfiles: false,
        refreshFeature: state.currentFeature === "settings",
        refreshOverlay: false,
        ...options
      }),
    panic: async (options = {}) =>
      rerender({
        refreshProfiles: true,
        refreshFeature: state.currentFeature === "home",
        refreshOverlay: true,
        ...options
      }),
    shell: async (options = {}) =>
      rerender({
        refreshProfiles: false,
        refreshFeature: false,
        refreshOverlay: false,
        ...options
      })
  };
}
