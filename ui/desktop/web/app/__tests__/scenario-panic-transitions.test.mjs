import test from "node:test";
import assert from "node:assert/strict";

import { createPanicUi } from "../main-panic.js";

function t(key) {
  return key;
}

function makeElement() {
  const handlers = new Map();
  return {
    addEventListener(type, handler) {
      handlers.set(type, handler);
    },
    async fire(type, event = {}) {
      const handler = handlers.get(type);
      if (handler) {
        await handler({ target: this, ...event });
      }
    }
  };
}

function makeRoot(map) {
  return {
    querySelector(selector) {
      return map.get(selector) ?? null;
    },
    querySelectorAll(selector) {
      return map.get(`${selector}[]`) ?? [];
    }
  };
}

test("scenario panic: cancel closes panic menu and clears panicUi state", async () => {
  globalThis.window = { __PANIC_FRAME_MODE: "menu" };
  const commandCalls = [];
  const model = {
    panicUi: { open: true, mode: "panel", sites: ["example.com"], search: "", notice: null },
    profileNotice: null
  };
  const profile = { id: "p-1", panic_protected_sites: ["example.com"], updated_at: 1 };

  const panicUi = createPanicUi({
    selectedProfile: () => profile,
    shouldRenderPanicControls: () => true,
    fireIcon: () => "",
    fireHeroIcon: () => "",
    panicFrameStyleAttr: () => "",
    callCommand: async (command, payload) => {
      commandCalls.push([command, payload?.request?.profileId ?? null]);
      return { ok: true };
    },
    panicWipeProfile: async () => ({ ok: true, data: {} }),
    launchProfile: async () => ({ ok: true, data: {} }),
    updateProfile: async () => ({ ok: true, data: {} }),
    sleep: async () => {}
  });

  const cancel = makeElement();
  const root = makeRoot(
    new Map([
      ["#panic-modal-cancel", cancel],
      ["#panic-menu-close", makeElement()],
      ["#panic-modal-clean", makeElement()],
      ["[data-panic-site-remove][]", []]
    ])
  );
  let rerenders = 0;
  panicUi.wirePanicInteractions(root, model, async () => {
    rerenders += 1;
  }, { t }, { currentFeature: "profiles" });

  await cancel.fire("click");
  assert.equal(model.panicUi, null);
  assert.deepEqual(commandCalls, [["panic_frame_hide_menu", "p-1"]]);
  assert.equal(rerenders, 0);
});

test("scenario panic: clean flow relaunches profile and returns shell to normal", async () => {
  globalThis.window = { __PANIC_FRAME_MODE: "menu" };
  const calls = [];
  const model = {
    panicUi: { open: true, mode: "panel", sites: ["example.com"], search: "", notice: null },
    profileNotice: null
  };
  const profile = { id: "p-9", panic_protected_sites: ["example.com"], updated_at: 1 };

  const panicUi = createPanicUi({
    selectedProfile: () => profile,
    shouldRenderPanicControls: () => true,
    fireIcon: () => "",
    fireHeroIcon: () => "",
    panicFrameStyleAttr: () => "",
    callCommand: async (command) => {
      calls.push(command);
      return { ok: true };
    },
    panicWipeProfile: async (request) => {
      calls.push(["wipe", request.profileId, request.mode]);
      return { ok: true, data: { profileId: request.profileId } };
    },
    launchProfile: async (profileId) => {
      calls.push(["launch", profileId]);
      return { ok: true, data: { id: profileId } };
    },
    updateProfile: async () => ({ ok: true, data: {} }),
    sleep: async () => {}
  });

  const clean = makeElement();
  const root = makeRoot(
    new Map([
      ["#panic-modal-clean", clean],
      ["#panic-menu-close", makeElement()],
      ["#panic-modal-cancel", makeElement()],
      ["[data-panic-site-remove][]", []]
    ])
  );
  let rerenders = 0;
  panicUi.wirePanicInteractions(root, model, async () => {
    rerenders += 1;
  }, { t }, { currentFeature: "profiles" });

  await clean.fire("click");
  assert.equal(model.panicUi, null);
  assert.deepEqual(model.profileNotice, { type: "success", text: "home.panicDone" });
  assert.ok(calls.some((entry) => Array.isArray(entry) && entry[0] === "wipe"));
  assert.ok(calls.some((entry) => Array.isArray(entry) && entry[0] === "launch"));
  assert.ok(calls.includes("panic_frame_hide_menu"));
  assert.ok(rerenders >= 1);
});
