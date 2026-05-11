import test from "node:test";
import assert from "node:assert/strict";

import { handleProfileLaunchActionImpl } from "../../features/profiles/view-wire-launch-stop.js";
import { createProfile, deleteProfile, launchProfile } from "../../features/profiles/api.js";

function t(key) {
  return key;
}

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
}

test("scenario profile launch: device-posture confirm flow retries and recovers to success", async () => {
  const notices = [];
  const model = {
    profileActionPendingIds: new Set(),
    profileLaunchOverlay: null
  };
  const profile = { id: "p-1" };
  let firstAttempt = true;

  const deps = {
    launchProfile: async (_profileId, _launchUrl, postureAckId) => {
      if (firstAttempt) {
        firstAttempt = false;
        return { ok: false, data: { error: "posture-check-required" } };
      }
      assert.equal(postureAckId, "report-1");
      return { ok: true, data: { id: profile.id, state: "running" } };
    },
    classifyDockerRuntimeIssue: () => null,
    resolveDevicePostureAction: () => ({ kind: "confirm", reportId: "report-1" }),
    getDevicePostureReport: async () => ({ ok: true, data: { findings: [] } }),
    postureFindingLines: () => "",
    askConfirmPrompt: async () => true,
    resolveProfileErrorMessage: (_t, error) => String(error),
    showDockerHelpModal: async () => {},
    showLinuxSandboxLaunchModal: async () => {},
    getLinuxBrowserSandboxStatus: async () => ({ ok: true, data: { status: "ok" } }),
    openProfileModal: async () => {},
    setNotice: (_model, type, text) => notices.push({ type, text })
  };

  await handleProfileLaunchActionImpl({}, model, () => {}, t, profile, deps);
  assert.equal(model.profileActionPendingIds.has("p-1"), false);
  assert.deepEqual(notices.at(-1), { type: "success", text: "profile.notice.launched" });
});

test("scenario profile launch: hard failure clears pending state and reports error", async () => {
  const notices = [];
  const model = {
    profileActionPendingIds: new Set(),
    profileLaunchOverlay: null
  };
  const profile = { id: "p-hard-fail" };
  const deps = {
    launchProfile: async () => ({ ok: false, data: { error: "route runtime unavailable" } }),
    classifyDockerRuntimeIssue: () => null,
    resolveDevicePostureAction: () => null,
    getDevicePostureReport: async () => ({ ok: false, data: {} }),
    postureFindingLines: () => "",
    askConfirmPrompt: async () => false,
    resolveProfileErrorMessage: (_t, error) => String(error),
    showDockerHelpModal: async () => {},
    showLinuxSandboxLaunchModal: async () => {},
    getLinuxBrowserSandboxStatus: async () => ({ ok: true, data: { status: "ok" } }),
    openProfileModal: async () => {},
    setNotice: (_model, type, text) => notices.push({ type, text })
  };

  await handleProfileLaunchActionImpl({}, model, () => {}, t, profile, deps);
  assert.equal(model.profileActionPendingIds.has(profile.id), false);
  assert.deepEqual(notices.at(-1), { type: "error", text: "route runtime unavailable" });
});

test("scenario profile launch: API launch failure then retry succeeds with visible recovery", async () => {
  ensureRuntimeMocks();
  const first = await launchProfile("missing-profile-id");
  assert.equal(first.ok, false);
  assert.match(String(first.data.error), /profile not found/i);

  const created = await createProfile({ name: "Retry Profile", engine: "chromium" });
  assert.equal(created.ok, true);
  const retry = await launchProfile(created.data.id);
  assert.equal(retry.ok, true);
  assert.equal(retry.data.state, "running");
  await deleteProfile(created.data.id);
});
