import test from "node:test";
import assert from "node:assert/strict";

import {
  buildReleaseUrl,
  ensureSettingsModel,
  linkBindingLabel,
  releaseVersionForLink,
  runtimeToolStatusLabel,
  startupProfileLabel,
  syncProfileId
} from "../../features/diagnostics/settings-view-core-support.js";

function t(key) {
  return key;
}

test("settings logic: ensureSettingsModel seeds defaults and keeps existing state", () => {
  const model = {
    selectedProfileId: "p-1",
    profiles: [{ id: "p-1", name: "Alpha" }]
  };
  const state = ensureSettingsModel(model);
  assert.equal(state.activeTab, "general");
  assert.equal(state.syncProfileId, "p-1");
  assert.equal(state.linkTestUrl, "https://duckduckgo.com");

  state.activeTab = "sync";
  const stateSecond = ensureSettingsModel(model);
  assert.equal(stateSecond.activeTab, "sync");
});

test("settings logic: syncProfileId falls back to first profile when missing", () => {
  const model = {
    selectedProfileId: null,
    profiles: [{ id: "p-a", name: "A" }, { id: "p-b", name: "B" }],
    settingsState: { activeTab: "general", syncProfileId: null }
  };
  assert.equal(syncProfileId(model), "p-a");
});

test("settings logic: startupProfileLabel and linkBindingLabel guard unknown profile ids", () => {
  const model = {
    profiles: [{ id: "p-1", name: "Alpha" }],
    linkRoutingOverview: { globalProfileId: "p-1" }
  };

  assert.equal(startupProfileLabel(model, "p-1", t), "Alpha");
  assert.equal(startupProfileLabel(model, "", t), "settings.startupProfile.none");
  assert.equal(
    linkBindingLabel({ profileId: "missing", usesGlobalDefault: false }, model, t),
    "missing"
  );
  assert.equal(
    linkBindingLabel({ profileId: null, usesGlobalDefault: true }, model, t),
    "Alpha links.binding.globalDefault"
  );
});

test("settings logic: release URL uses trusted fallback for non-http input", () => {
  const updateState = {
    releaseUrl: "javascript:alert(1)",
    latestVersion: "v1.2.3"
  };
  assert.equal(releaseVersionForLink(updateState, "1.2.0"), "1.2.3");
  assert.equal(
    buildReleaseUrl(updateState, "1.2.0"),
    "https://github.com/BerkutSolutions/cerbena-browser/releases/tag/v1.2.3"
  );
  assert.equal(
    buildReleaseUrl({ releaseUrl: "https://example.org/release" }, "1.2.0"),
    "https://example.org/release"
  );
});

test("settings logic: runtimeToolStatusLabel reports version/docker/empty states", () => {
  assert.equal(runtimeToolStatusLabel({ status: "ready", version: "1.0.0" }, t), "1.0.0");
  assert.equal(runtimeToolStatusLabel({ status: "docker", version: "" }, t), "settings.tools.inDocker");
  assert.equal(runtimeToolStatusLabel({ status: "missing", version: "" }, t), "");
});
