import { callCommand } from "../../core/commands.js";

export async function saveSyncControls(profileId, model) {
  return callCommand("save_sync_controls", { request: { profileId, model } });
}

export async function getSyncOverview(profileId) {
  return callCommand("get_sync_overview", { profileId });
}

export async function addSyncConflict(profileId, item) {
  return callCommand("add_sync_conflict", { request: { profileId, item } });
}

export async function clearSyncConflicts(profileId) {
  return callCommand("clear_sync_conflicts", { profileId });
}

export async function createBackupSnapshot(profileId) {
  return callCommand("create_backup_snapshot", { request: { profileId } });
}

export async function restoreSnapshot(request) {
  return callCommand("restore_snapshot", { request: { request } });
}

export async function syncHealthPing(profileId = null) {
  if (profileId) {
    return callCommand("sync_health_ping", { profileId });
  }
  return callCommand("sync_health_ping");
}
