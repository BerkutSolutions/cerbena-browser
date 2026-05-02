import { callCommand } from "../core/commands.js";
import { loadDictionaries, createI18n } from "../i18n/runtime.js";

function escapeHtml(value) {
  return String(value ?? "")
    .replaceAll("&", "&amp;")
    .replaceAll("<", "&lt;")
    .replaceAll(">", "&gt;")
    .replaceAll('"', "&quot;");
}

function updaterModeFromUrl() {
  const mode = new URLSearchParams(window.location.search).get("mode");
  return mode === "preview" ? "preview" : "auto";
}

function interpolate(template, values = {}) {
  return String(template ?? "").replace(/\{([a-zA-Z0-9_]+)\}/g, (_, key) => String(values?.[key] ?? `{${key}}`));
}

function translateMaybe(value, t, values) {
  const text = String(value ?? "");
  if (!text.startsWith("i18n:")) return text;
  return interpolate(t(text.slice(5)), values);
}

function stepMarkup(step, t) {
  return `
    <li class="healthcheck-step" data-status="${escapeHtml(step.status || "idle")}">
      <span class="healthcheck-step-icon" data-status="${escapeHtml(step.status || "idle")}"></span>
      <div class="healthcheck-step-body">
        <div class="healthcheck-step-title">${escapeHtml(t(step.titleKey || "updater.steps.unknown"))}</div>
        <div class="healthcheck-step-desc">${escapeHtml(translateMaybe(step.detail || "", t))}</div>
      </div>
    </li>
  `;
}

function render(root, i18n, overview) {
  const t = i18n.t;
  const badge = overview?.dryRun ? t("updater.previewBadge") : t("updater.autoBadge");
  const closeLabel = overview?.canClose
    ? t(overview?.closeLabelKey || "action.close")
    : t("updater.running");
  root.innerHTML = `
    <div class="healthcheck-container updater-shell">
      <div class="panel updater-hero">
        <div class="hc-title-row">
          <div>
            <h1 class="healthcheck-title">${escapeHtml(t("updater.title"))}</h1>
            <p class="meta">${escapeHtml(t("updater.subtitle"))}</p>
          </div>
        </div>
        <div class="row-between">
          <p class="meta">${escapeHtml(badge)}</p>
          <p class="meta">${escapeHtml(t(`updater.status.${overview?.status || "idle"}`))}</p>
        </div>
        <div class="updater-version-grid">
          <div class="updater-version-card">
            <span class="meta">${escapeHtml(t("updater.currentVersion"))}</span>
          <strong>${escapeHtml(overview?.currentVersion || "1.0.7-2")}</strong>
          </div>
          <div class="updater-version-card">
            <span class="meta">${escapeHtml(t("updater.targetVersion"))}</span>
            <strong>${escapeHtml(overview?.targetVersion || t("updater.awaitingTarget"))}</strong>
          </div>
        </div>
        <p class="meta">${escapeHtml(t(overview?.summaryKey || "updater.summary.ready"))}</p>
        <p class="meta">${escapeHtml(translateMaybe(overview?.summaryDetail || "", t))}</p>
        ${overview?.releaseUrl ? `<p class="meta"><a href="${escapeHtml(overview.releaseUrl)}" target="_blank" rel="noreferrer">${escapeHtml(t("updater.releaseLink"))}</a></p>` : ""}
      </div>
      <div class="panel updater-pipeline-panel">
        <div class="healthcheck-section-title">${escapeHtml(t("updater.pipelineTitle"))}</div>
        <ul class="healthcheck-steps">
          ${(overview?.steps || []).map((step) => stepMarkup(step, t)).join("")}
        </ul>
      </div>
      <div class="healthcheck-actions">
        <button id="updater-close"${overview?.canClose ? "" : " disabled"}>${escapeHtml(closeLabel)}</button>
      </div>
    </div>
  `;
  root.querySelector("#updater-close")?.addEventListener("click", async () => {
    if (!overview?.canClose) return;
    await callCommand("window_close");
  });
}

async function getOverview() {
  const result = await callCommand("get_updater_overview");
  return result.ok ? result.data : null;
}

async function startFlow() {
  const result = await callCommand("start_updater_flow");
  return result.ok ? result.data : null;
}

async function init() {
  const locale = localStorage.getItem("launcher.locale")
    ?? document.documentElement.lang
    ?? (navigator.language?.toLowerCase().startsWith("ru") ? "ru" : "en");
  const dictionaries = await loadDictionaries();
  const i18n = createI18n(dictionaries, locale);
  const root = document.getElementById("app");
  let overview = await getOverview();
  render(root, i18n, overview);
  await startFlow();

  const listen = window.__TAURI__?.event?.listen ?? null;
  if (listen) {
    await listen("updater-progress", (event) => {
      overview = event.payload;
      render(root, i18n, overview);
    });
  }

  window.setInterval(async () => {
    const next = await getOverview();
    if (!next) return;
    overview = next;
    render(root, i18n, overview);
  }, updaterModeFromUrl() === "preview" ? 900 : 1400);
}

init().catch((error) => {
  const root = document.getElementById("app");
  root.innerHTML = `
    <div class="healthcheck-container">
      <div class="panel">
        <h1 class="healthcheck-title">Cerbena Updater</h1>
        <p class="notice error">${escapeHtml(String(error))}</p>
      </div>
    </div>
  `;
});
