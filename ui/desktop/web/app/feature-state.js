function aliasStateProperty(model, alias, container, key) {
  Object.defineProperty(model, alias, {
    configurable: true,
    enumerable: true,
    get() {
      return model.featureState[container][key];
    },
    set(value) {
      model.featureState[container][key] = value;
    }
  });
}

export function createShellModel() {
  const model = {
    featureState: {
      profiles: {
        profiles: [],
        selectedProfileId: null,
        profileNotice: null,
        profileActionPendingIds: null,
        profileLaunchOverlay: null,
        linuxSandboxHintShown: false
      },
      home: {
        homeDashboard: null,
        homeNotice: null,
        homeMetricsRenderTimer: null
      },
      settings: {
        settingsNotice: null,
        settingsProvider: "duckduckgo",
        linkRoutingOverview: null,
        shellPreferencesState: null,
        linkLaunchModal: null,
        defaultBrowserStartupModal: null,
        defaultLinkProfileModal: null,
        trayClosePromptModal: null,
        systemStartupProfileLaunchHandled: false
      },
      network: {
        networkDraft: null,
        networkNotice: null,
        networkTemplates: null,
        networkTemplateDraft: null,
        networkGlobalRoute: null,
        networkNodeTestState: {},
        networkPingState: {},
        networkPingPoller: null,
        networkPingInFlight: false,
        networkLastPingAt: 0,
        networkLoaded: false
      },
      dns: {
        dnsNotice: null
      },
      extensions: {
        extensionState: null,
        extensionLibraryState: null,
        extensionNotice: null,
        profileExtensionStateMap: null
      },
      logs: {
        runtimeLogs: null
      },
      traffic: {
        trafficState: null,
        trafficNotice: null,
        trafficPoller: null
      },
      identity: {
        identityDraft: null,
        identityPreview: null,
        identityNotice: null
      },
      security: {
        securityNotice: null
      },
      sync: {
        syncOverview: null,
        syncNotice: null
      },
      panic: {
        panicUi: null
      }
    },
    serviceCatalog: null,
    notice: null,
    appLifecycleOverlay: null
  };

  aliasStateProperty(model, "profiles", "profiles", "profiles");
  aliasStateProperty(model, "selectedProfileId", "profiles", "selectedProfileId");
  aliasStateProperty(model, "profileNotice", "profiles", "profileNotice");
  aliasStateProperty(model, "profileActionPendingIds", "profiles", "profileActionPendingIds");
  aliasStateProperty(model, "profileLaunchOverlay", "profiles", "profileLaunchOverlay");
  aliasStateProperty(model, "linuxSandboxHintShown", "profiles", "linuxSandboxHintShown");

  aliasStateProperty(model, "homeDashboard", "home", "homeDashboard");
  aliasStateProperty(model, "homeNotice", "home", "homeNotice");
  aliasStateProperty(model, "homeMetricsRenderTimer", "home", "homeMetricsRenderTimer");

  aliasStateProperty(model, "settingsNotice", "settings", "settingsNotice");
  aliasStateProperty(model, "settingsProvider", "settings", "settingsProvider");
  aliasStateProperty(model, "linkRoutingOverview", "settings", "linkRoutingOverview");
  aliasStateProperty(model, "shellPreferencesState", "settings", "shellPreferencesState");
  aliasStateProperty(model, "linkLaunchModal", "settings", "linkLaunchModal");
  aliasStateProperty(model, "defaultBrowserStartupModal", "settings", "defaultBrowserStartupModal");
  aliasStateProperty(model, "defaultLinkProfileModal", "settings", "defaultLinkProfileModal");
  aliasStateProperty(model, "trayClosePromptModal", "settings", "trayClosePromptModal");
  aliasStateProperty(model, "systemStartupProfileLaunchHandled", "settings", "systemStartupProfileLaunchHandled");

  aliasStateProperty(model, "networkDraft", "network", "networkDraft");
  aliasStateProperty(model, "networkNotice", "network", "networkNotice");
  aliasStateProperty(model, "networkTemplates", "network", "networkTemplates");
  aliasStateProperty(model, "networkTemplateDraft", "network", "networkTemplateDraft");
  aliasStateProperty(model, "networkGlobalRoute", "network", "networkGlobalRoute");
  aliasStateProperty(model, "networkNodeTestState", "network", "networkNodeTestState");
  aliasStateProperty(model, "networkPingState", "network", "networkPingState");
  aliasStateProperty(model, "networkPingPoller", "network", "networkPingPoller");
  aliasStateProperty(model, "networkPingInFlight", "network", "networkPingInFlight");
  aliasStateProperty(model, "networkLastPingAt", "network", "networkLastPingAt");
  aliasStateProperty(model, "networkLoaded", "network", "networkLoaded");

  aliasStateProperty(model, "dnsNotice", "dns", "dnsNotice");
  aliasStateProperty(model, "extensionState", "extensions", "extensionState");
  aliasStateProperty(model, "extensionLibraryState", "extensions", "extensionLibraryState");
  aliasStateProperty(model, "extensionNotice", "extensions", "extensionNotice");
  aliasStateProperty(model, "profileExtensionStateMap", "extensions", "profileExtensionStateMap");
  aliasStateProperty(model, "runtimeLogs", "logs", "runtimeLogs");
  aliasStateProperty(model, "trafficState", "traffic", "trafficState");
  aliasStateProperty(model, "trafficNotice", "traffic", "trafficNotice");
  aliasStateProperty(model, "trafficPoller", "traffic", "trafficPoller");
  aliasStateProperty(model, "identityDraft", "identity", "identityDraft");
  aliasStateProperty(model, "identityPreview", "identity", "identityPreview");
  aliasStateProperty(model, "identityNotice", "identity", "identityNotice");
  aliasStateProperty(model, "securityNotice", "security", "securityNotice");
  aliasStateProperty(model, "syncOverview", "sync", "syncOverview");
  aliasStateProperty(model, "syncNotice", "sync", "syncNotice");
  aliasStateProperty(model, "panicUi", "panic", "panicUi");
  return model;
}
