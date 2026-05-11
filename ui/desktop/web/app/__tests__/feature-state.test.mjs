import test from "node:test";
import assert from "node:assert/strict";
import { createShellModel } from "../feature-state.js";

test("createShellModel keeps feature state containers isolated", () => {
  const model = createShellModel();
  model.featureState.profiles.profileNotice = { type: "success", text: "ok" };
  model.featureState.settings.settingsNotice = { type: "error", text: "bad" };

  assert.equal(model.profileNotice?.text, "ok");
  assert.equal(model.settingsNotice?.text, "bad");
  assert.equal(model.featureState.home.homeNotice, null);
});

test("alias writes map to owning feature container", () => {
  const model = createShellModel();
  model.networkNotice = { type: "info", text: "network-only" };
  model.syncNotice = { type: "info", text: "sync-only" };

  assert.deepEqual(model.featureState.network.networkNotice, { type: "info", text: "network-only" });
  assert.deepEqual(model.featureState.sync.syncNotice, { type: "info", text: "sync-only" });
});
