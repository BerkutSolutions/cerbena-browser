import test from "node:test";
import assert from "node:assert/strict";

import {
  checkLauncherUpdates,
  getLauncherUpdateState,
  launchUpdaterPreview,
  setLauncherAutoUpdate
} from "../../features/settings/api.js";

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

test("scenario updater: happy path keeps state coherent across check flow", async () => {
  ensureRuntimeMocks();

  const enabled = await setLauncherAutoUpdate(true);
  assert.equal(enabled.ok, true);
  assert.equal(enabled.data.autoUpdateEnabled, true);

  const checked = await checkLauncherUpdates(true);
  assert.equal(checked.ok, true);
  assert.equal(checked.data.status, "up_to_date");
  assert.equal(checked.data.hasUpdate, false);

  const state = await getLauncherUpdateState();
  assert.equal(state.ok, true);
  assert.equal(state.data.autoUpdateEnabled, true);
  assert.equal(state.data.status, "up_to_date");
});

test("scenario updater: guarded path preserves disabled mode and surfaces preview failure", async () => {
  ensureRuntimeMocks();

  const disabled = await setLauncherAutoUpdate(false);
  assert.equal(disabled.ok, true);
  assert.equal(disabled.data.autoUpdateEnabled, false);

  const checked = await checkLauncherUpdates(false);
  assert.equal(checked.ok, true);
  assert.equal(checked.data.autoUpdateEnabled, false);

  const preview = await launchUpdaterPreview();
  assert.equal(preview.ok, false);
  assert.match(String(preview.data.error), /requires Tauri runtime/i);

  const stateAfterPreview = await getLauncherUpdateState();
  assert.equal(stateAfterPreview.ok, true);
  assert.equal(stateAfterPreview.data.autoUpdateEnabled, false);
});
