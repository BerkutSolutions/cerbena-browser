import test from "node:test";
import assert from "node:assert/strict";
import { createRefreshBoundaries } from "../main-runtime-refresh.js";

test("network boundary refreshes only network/dns feature branch by default", async () => {
  const calls = [];
  const rerender = async (options) => {
    calls.push(options);
  };
  const state = { currentFeature: "network" };
  const boundaries = createRefreshBoundaries({ rerender, state });

  await boundaries.network();
  assert.equal(calls.length, 1);
  assert.equal(calls[0].refreshProfiles, false);
  assert.equal(calls[0].refreshFeature, true);
  assert.equal(calls[0].refreshOverlay, false);
});

test("settings boundary avoids unrelated profile refresh by default", async () => {
  const calls = [];
  const rerender = async (options) => {
    calls.push(options);
  };
  const state = { currentFeature: "settings" };
  const boundaries = createRefreshBoundaries({ rerender, state });

  await boundaries.settings();
  assert.equal(calls.length, 1);
  assert.equal(calls[0].refreshProfiles, false);
  assert.equal(calls[0].refreshFeature, true);
  assert.equal(calls[0].refreshOverlay, false);
});
