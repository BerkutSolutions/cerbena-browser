import fs from "node:fs";
import path from "node:path";
import { pathToFileURL } from "node:url";

const root = path.resolve(process.cwd(), "web");
const i18nRoot = path.join(root, "i18n");
const featureRegistryPath = path.join(root, "core", "feature-registry.js");

function readJson(file) {
  const raw = fs.readFileSync(file, "utf8").replace(/^\uFEFF/, "");
  return JSON.parse(raw);
}

function assert(condition, message) {
  if (!condition) {
    throw new Error(message);
  }
}

async function main() {
  const en = readJson(path.join(i18nRoot, "en", "common.json"));
  const ru = readJson(path.join(i18nRoot, "ru", "common.json"));

  const required = [
    "nav.home",
    "nav.profiles",
    "nav.extensions",
    "nav.security",
    "nav.identity",
    "nav.dns",
    "nav.network",
    "nav.settings",
    "extensions.tableTitle",
    "network.chainTitle",
    "settings.searchProvider",
    "settings.tab.links",
    "settings.tab.sync",
    "links.table.title"
  ];

  for (const key of required) {
    assert(typeof en[key] === "string" && en[key].length > 0, `Missing EN key: ${key}`);
    assert(typeof ru[key] === "string" && ru[key].length > 0, `Missing RU key: ${key}`);
  }

  const registrySrc = fs.readFileSync(featureRegistryPath, "utf8");
  const expectedOrder = [
    "home",
    "extensions",
    "security",
    "identity",
    "dns",
    "network",
    "traffic",
    "settings"
  ];

  for (const key of expectedOrder) {
    assert(registrySrc.includes(`{ key: "${key}"`), `Feature missing in registry: ${key}`);
  }

  const mainSrc = fs.readFileSync(path.join(root, "app", "main.js"), "utf8");
  assert(!mainSrc.includes("quick-launch"), "Legacy quick-launch button should be removed from shell.");
  assert(!mainSrc.includes("new-profile"), "Legacy new-profile shell button should be removed.");

  const css = fs.readFileSync(path.join(root, "styles", "app.css"), "utf8");
  assert(css.includes("#top-frame"), "Top frame style is missing.");
  assert(css.includes("#sidebar-frame"), "Sidebar frame style is missing.");

  const { buildPreset } = await import(pathToFileURL(path.join(root, "core", "catalogs.js")).href);
  const preset = buildPreset("win_7_edge_109", Date.now());
  assert(Number.isInteger(preset.canvas_noise_seed), "Identity preset canvas seed must stay integral.");
  assert(preset.canvas_noise_seed >= 0, "Identity preset canvas seed must stay non-negative for Rust u64 serialization.");

  console.log("ui smoke test passed");
}

main();
