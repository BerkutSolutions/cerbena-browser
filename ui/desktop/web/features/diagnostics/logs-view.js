import { readRuntimeLogs } from "./api.js";

export function renderLogs(t, model) {
  const rows = model.runtimeLogs ?? [];
  const body = rows.length
    ? rows.map((line) => escapeHtml(String(line))).join("\n")
    : t("logs.empty");
  return `
  <div class="panel logs-panel">
    <div class="row-between logs-panel-header">
      <div>
        <h2>${t("nav.logs")}</h2>
        <p class="meta">${t("logs.subtitle")}</p>
      </div>
      <button id="logs-refresh">${t("action.refresh")}</button>
    </div>
    <pre class="preview-box logs-console">${body}</pre>
  </div>`;
}

export async function hydrateLogsModel(model) {
  const result = await readRuntimeLogs();
  model.runtimeLogs = result.ok ? result.data : [];
}

export function wireLogs(root, model, rerender) {
  root.querySelector("#logs-refresh")?.addEventListener("click", async () => {
    await hydrateLogsModel(model);
    await rerender();
  });
}

function escapeHtml(value) {
  return value
    .replaceAll("&", "&amp;")
    .replaceAll("<", "&lt;")
    .replaceAll(">", "&gt;")
    .replaceAll('"', "&quot;")
    .replaceAll("'", "&#39;");
}
