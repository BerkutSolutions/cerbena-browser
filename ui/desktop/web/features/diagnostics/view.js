import { appendRuntimeLog, runGuardrailCheck } from "./api.js";

export function renderDiagnostics(t, model) {
  const notice = model.diagnosticsNotice ? `<p class="notice ${model.diagnosticsNotice.type}">${model.diagnosticsNotice.text}</p>` : "";
  return `
  <div class="panel">
    <h2>${t("nav.diagnostics")}</h2>
    ${notice}
    <div class="grid-two">
      <label>${t("diagnostics.role")}<select id="diag-role"><option value="viewer">viewer</option><option value="operator">operator</option><option value="admin">admin</option></select></label>
      <label>${t("diagnostics.operation")}<input id="diag-operation" value="profile.launch" /></label>
      <label>${t("diagnostics.rateToken")}<input id="diag-token" value="ui-session" /></label>
      <label>${t("diagnostics.grantedProfiles")}<input id="diag-granted" value="" placeholder="uuid,uuid"/></label>
    </div>
    <div class="top-actions" style="margin-top:10px;">
      <button id="diag-guardrails">${t("diagnostics.runGuardrails")}</button>
      <button id="diag-log">${t("diagnostics.writeLog")}</button>
    </div>
  </div>`;
}

export function wireDiagnostics(root, model, rerender, t) {
  root.querySelector("#diag-guardrails")?.addEventListener("click", async ()=>{
    const grantedRaw = root.querySelector("#diag-granted").value.split(",").map((v)=>v.trim()).filter(Boolean);
    const req = {
      token: root.querySelector("#diag-token").value,
      role: root.querySelector("#diag-role").value,
      operation: root.querySelector("#diag-operation").value,
      profileId: model.selectedProfileId,
      grantedProfileIds: grantedRaw.length ? grantedRaw : [model.selectedProfileId],
      grant: null
    };
    const result = await runGuardrailCheck(req);
    model.diagnosticsNotice = { type: result.ok ? "success" : "error", text: result.ok ? t("diagnostics.guardrailsOk") : String(result.data.error) };
    await rerender();
  });

  root.querySelector("#diag-log")?.addEventListener("click", async ()=>{
    const result = await appendRuntimeLog(`[diag] manual log at ${new Date().toISOString()}`);
    model.diagnosticsNotice = { type: result.ok ? "success" : "error", text: result.ok ? t("diagnostics.logAppended") : String(result.data.error) };
    await rerender();
  });
}
