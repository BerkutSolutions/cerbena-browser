import { readRuntimeLogs } from "./api.js";

export function renderLogs(t, model) {
  const rows = model.runtimeLogs ?? [];
  return `
  <div class="panel">
    <div class="row-between">
      <h2>${t("nav.logs")}</h2>
      <button id="logs-refresh">Refresh</button>
    </div>
    <pre class="preview-box">${rows.join("\n") || "No logs yet"}</pre>
  </div>`;
}

export async function hydrateLogsModel(model) {
  const result = await readRuntimeLogs();
  model.runtimeLogs = result.ok ? result.data : [];
}

export function wireLogs(root, model, rerender) {
  root.querySelector("#logs-refresh")?.addEventListener("click", async ()=>{
    await hydrateLogsModel(model);
    await rerender();
  });
}
