import {
  addSyncConflict,
  clearSyncConflicts,
  createBackupSnapshot,
  getSyncOverview,
  restoreSnapshot,
  saveSyncControls,
  syncHealthPing
} from "./api.js";

export function renderSync(t, model) {
  const info = model.syncOverview;
  const notice = model.syncNotice ? `<p class="notice ${model.syncNotice.type}">${model.syncNotice.text}</p>` : "";
  return `
    <div class="panel">
      <h2>${t("nav.sync")}</h2>
      ${notice}
      <div class="grid-two">
        <label>${t("sync.serverUrl")}<input id="sync-url" value="${info?.controls?.server?.server_url ?? ""}" /></label>
        <label>${t("sync.keyId")}<input id="sync-key" value="${info?.controls?.server?.key_id ?? ""}" /></label>
        <label><input id="sync-enabled" type="checkbox" ${info?.controls?.server?.sync_enabled ? "checked" : ""}/> ${t("sync.enabled")}</label>
      </div>
      <div class="top-actions" style="margin-top:10px;">
        <button id="sync-save">${t("sync.saveConfig")}</button>
        <button id="sync-ping">${t("sync.healthPing")}</button>
        <button id="sync-conflict">${t("sync.addConflict")}</button>
        <button id="sync-clear-conflicts">${t("sync.clearConflicts")}</button>
      </div>
      <div class="top-actions" style="margin-top:10px;">
        <button id="sync-backup">${t("sync.createBackup")}</button>
        <button id="sync-restore">${t("sync.restoreLatest")}</button>
      </div>
      <div class="panel" style="margin-top:10px;"><pre class="preview-box">${JSON.stringify(info ?? {}, null, 2)}</pre></div>
    </div>
  `;
}

export async function hydrateSyncModel(model) {
  if (!model.selectedProfileId) return;
  const result = await getSyncOverview(model.selectedProfileId);
  model.syncOverview = result.ok ? result.data : null;
}

export function wireSync(root, model, rerender, t) {
  root.querySelector("#sync-save")?.addEventListener("click", async ()=>{
    const modelPayload = {
      server: {
        server_url: root.querySelector("#sync-url").value,
        key_id: root.querySelector("#sync-key").value,
        sync_enabled: root.querySelector("#sync-enabled").checked
      },
      status: { level: "healthy", message_key: "sync.status.healthy", last_sync_unix_ms: Date.now() },
      conflicts: [],
      can_backup: true,
      can_restore: true
    };
    const result = await saveSyncControls(model.selectedProfileId, modelPayload);
    model.syncNotice = { type: result.ok ? "success" : "error", text: result.ok ? t("sync.saved") : String(result.data.error) };
    await hydrateSyncModel(model);
    await rerender();
  });

  root.querySelector("#sync-ping")?.addEventListener("click", async ()=>{
    const ping = await syncHealthPing(model.selectedProfileId ?? null);
    model.syncNotice = { type: ping.ok ? "success" : "error", text: ping.ok ? t("sync.healthy") : String(ping.data.error) };
    await rerender();
  });

  root.querySelector("#sync-conflict")?.addEventListener("click", async ()=>{
    await addSyncConflict(model.selectedProfileId, { object_key: "bookmarks", local_revision: 2, remote_revision: 3, action_hint_key: "choose_latest" });
    await hydrateSyncModel(model);
    await rerender();
  });

  root.querySelector("#sync-clear-conflicts")?.addEventListener("click", async ()=>{
    await clearSyncConflicts(model.selectedProfileId);
    await hydrateSyncModel(model);
    await rerender();
  });

  root.querySelector("#sync-backup")?.addEventListener("click", async ()=>{
    const result = await createBackupSnapshot(model.selectedProfileId);
    model.syncNotice = { type: result.ok ? "success" : "error", text: result.ok ? t("sync.backupCreated") : String(result.data.error) };
    await hydrateSyncModel(model);
    await rerender();
  });

  root.querySelector("#sync-restore")?.addEventListener("click", async ()=>{
    const latest = model.syncOverview?.snapshots?.[model.syncOverview.snapshots.length - 1];
    if (!latest) {
      model.syncNotice = { type: "error", text: t("sync.noSnapshots") };
      await rerender();
      return;
    }
    const request = {
      profile_id: model.selectedProfileId,
      snapshot_id: latest.snapshot_id,
      scope: "full",
      include_prefixes: [],
      expected_schema_version: 1
    };
    const restored = await restoreSnapshot(request);
    model.syncNotice = { type: restored.ok ? "success" : "error", text: restored.ok ? t("sync.restored") : String(restored.data.error) };
    await rerender();
  });
}
