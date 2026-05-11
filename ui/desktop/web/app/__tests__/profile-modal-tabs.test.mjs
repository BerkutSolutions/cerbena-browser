import test from "node:test";
import assert from "node:assert/strict";

import { renderProfileModalHtml } from "../../features/profiles/view-modal-shell.js";

function t(key) {
  return key;
}

function makeDeps() {
  return {
    mergedProfileExtensions: () => ({ enabled: [], disabled: [] }),
    profileSecurityFlags: () => ({
      allowSystemAccess: false,
      allowKeepassxc: false,
      disableExtensionsLaunch: false
    }),
    certificateEntriesForProfile: () => [],
    loadPolicyPresets: () => ({ normal: { blocklists: [], blockedServices: [], allowDomains: [], denyDomains: [] } }),
    summarizePolicyPreset: () => ({ blocklists: 0, blockedServices: 0, allowDomains: 0, denyDomains: 0 }),
    listIdentityTemplates: () => [{ key: "tpl-1", label: "Template 1", platformFamily: "desktop", autoPlatform: "windows" }],
    listIdentityPlatforms: () => [{ key: "windows", label: "Windows" }],
    listIdentityTemplatePlatforms: () => [{ key: "desktop", label: "Desktop" }],
    inferIdentityUiState: () => ({ mode: "manual", templatePlatform: "desktop", templateKey: "tpl-1", autoPlatform: "windows" }),
    buildRealPreset: () => ({ display_name: "Preset" }),
    routeTemplateOptions: () => `<option value="">none</option>`,
    globalRouteNoticeHtml: () => "",
    buildTagPickerMarkup: () => `<div id="profile-tags"></div>`,
    collectProfileTags: () => [],
    profileTags: () => [],
    dnsTemplateOptions: () => `<option value="">custom</option>`,
    globalBlocklistOptions: () => [],
    buildDomainEntries: () => [],
    eyeIcon: () => "<svg></svg>",
    extensionLibraryOptions: () => `<option value="">none</option>`,
    option: (value, label) => `<option value="${value}">${label}</option>`,
    escapeHtml: (value) => String(value ?? ""),
    DOMAIN_OPTIONS: ["example.org"],
    templateSummaryLabel: () => "Template 1",
    templateDropdownOptionsHtml: () => "<div>template-options</div>",
    templateInputValue: () => "tpl-1",
    searchOptions: () => `<option value="duckduckgo">DuckDuckGo</option>`,
    normalizeProfileRouteMode: () => "direct"
  };
}

test("profile modal: renders and contains all tabs with key controls", () => {
  const profile = {
    id: "profile-1",
    name: "P1",
    engine: "chromium",
    tags: [],
    panic_frame_enabled: false
  };
  const dnsDraft = { mode: "system", servers: "", selectedBlocklists: [], allowlist: "", denylist: "" };
  const globalSecurity = { certificates: [], blocklists: [] };
  const model = { profiles: [], serviceCatalog: {} };
  const networkState = { payload: { route_mode: "direct", kill_switch_enabled: true }, connectionTemplates: [] };
  const syncOverview = { controls: { server: { server_url: "", key_id: "", sync_enabled: false } } };
  const identityPreset = { display_name: "Preset" };

  const html = renderProfileModalHtml(
    t,
    profile,
    dnsDraft,
    globalSecurity,
    model,
    networkState,
    syncOverview,
    identityPreset,
    makeDeps()
  );

  for (const tab of ["general", "identity", "vpn", "dns", "extensions", "security", "sync", "advanced"]) {
    assert.ok(html.includes(`data-tab="${tab}"`), `missing tab ${tab}`);
    assert.ok(html.includes(`data-pane="${tab}"`), `missing pane ${tab}`);
  }

  assert.ok(html.includes('name="defaultSearchProvider"'));
  assert.ok(html.includes('id="profile-route-mode"'));
  assert.ok(html.includes('name="dnsMode"'));
  assert.ok(html.includes('id="profile-extension-add"'));
  assert.ok(html.includes('name="passwordLock"'));
  assert.ok(html.includes('name="profilePassword"'));
  assert.ok(html.includes('name="profilePasswordConfirm"'));
  assert.ok(html.includes('name="syncServer"'));
  assert.ok(html.includes('name="syncKey"'));
  assert.ok(html.includes('name="singlePageMode"'));
});
