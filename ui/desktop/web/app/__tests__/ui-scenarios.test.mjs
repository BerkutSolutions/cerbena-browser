import test from "node:test";
import assert from "node:assert/strict";
import { Buffer } from "node:buffer";

import {
  createProfile,
  deleteProfile,
  launchProfile,
  listProfiles,
  stopProfile
} from "../../features/profiles/api.js";
import {
  checkLauncherUpdates,
  dispatchExternalLink,
  getLauncherUpdateState,
  getLinkRoutingOverview,
  setDefaultProfileForLinks,
  setLauncherAutoUpdate
} from "../../features/settings/api.js";
import { networkDraftUtils } from "../../features/network/view-template-editor.js";

function makeLocalStorageMock() {
  const storage = new Map();
  return {
    getItem(key) {
      return storage.has(key) ? storage.get(key) : null;
    },
    setItem(key, value) {
      storage.set(key, String(value));
    },
    removeItem(key) {
      storage.delete(key);
    },
    clear() {
      storage.clear();
    }
  };
}

function ensureRuntimeMocks() {
  globalThis.localStorage = makeLocalStorageMock();
  if (!globalThis.crypto?.randomUUID) {
    globalThis.crypto = {
      randomUUID() {
        return `mock-${Date.now()}-${Math.random().toString(16).slice(2)}`;
      }
    };
  }
  if (!globalThis.btoa) {
    globalThis.btoa = (value) => Buffer.from(String(value), "utf8").toString("base64");
  }
}

function t(key) {
  return key;
}

test("scenario: profile lifecycle create -> launch -> stop -> delete", async () => {
  ensureRuntimeMocks();
  const created = await createProfile({ name: "Scenario Profile", engine: "chromium" });
  assert.equal(created.ok, true);
  const profileId = created.data.id;

  const launched = await launchProfile(profileId);
  assert.equal(launched.ok, true);
  assert.equal(launched.data.state, "running");

  const stopped = await stopProfile(profileId);
  assert.equal(stopped.ok, true);
  assert.equal(stopped.data.state, "stopped");

  const listed = await listProfiles();
  assert.equal(listed.ok, true);
  assert.equal(listed.data.some((profile) => profile.id === profileId), true);

  const removed = await deleteProfile(profileId);
  assert.equal(removed.ok, true);
  const listedAfterDelete = await listProfiles();
  assert.equal(listedAfterDelete.data.some((profile) => profile.id === profileId), false);
});

test("scenario: network template draft validation catches bad endpoint and accepts valid", () => {
  ensureRuntimeMocks();
  const draft = {
    templateId: "",
    name: "WG test",
    nodes: [
      {
        nodeId: "node-1",
        connectionType: "vpn",
        protocol: "wireguard",
        host: "",
        port: "0",
        username: "",
        password: "",
        bridges: "",
        settings: {}
      }
    ]
  };

  const invalid = networkDraftUtils.validateDraft(draft, t);
  assert.equal(invalid, "network.error.hostInvalid");

  draft.nodes[0].host = "vpn.example.org";
  draft.nodes[0].port = "51820";
  const valid = networkDraftUtils.validateDraft(draft, t);
  assert.equal(valid, "");
});

test("scenario: settings update and link routing flow stay coherent", async () => {
  ensureRuntimeMocks();
  const profile = await createProfile({ name: "Routing Profile", engine: "chromium" });
  assert.equal(profile.ok, true);
  const profileId = profile.data.id;

  const autoUpdate = await setLauncherAutoUpdate(true);
  assert.equal(autoUpdate.ok, true);
  assert.equal(autoUpdate.data.autoUpdateEnabled, true);

  const checked = await checkLauncherUpdates(true);
  assert.equal(checked.ok, true);
  assert.equal(checked.data.status, "up_to_date");

  const beforeRouting = await getLinkRoutingOverview();
  assert.equal(beforeRouting.ok, true);

  const setGlobal = await setDefaultProfileForLinks({ profileId });
  assert.equal(setGlobal.ok, true);

  const dispatched = await dispatchExternalLink("https://example.org");
  assert.equal(dispatched.ok, true);
  assert.equal(dispatched.data.status, "resolved");
  assert.equal(dispatched.data.targetProfileId, profileId);

  const updateState = await getLauncherUpdateState();
  assert.equal(updateState.ok, true);
  assert.equal(updateState.data.autoUpdateEnabled, true);

  const afterRouting = await getLinkRoutingOverview();
  assert.equal(afterRouting.ok, true);
  assert.equal(afterRouting.data.globalProfileId, profileId);

  await deleteProfile(profileId);
});
