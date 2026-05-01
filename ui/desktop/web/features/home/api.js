import { callCommand } from "../../core/commands.js";

export async function buildHomeDashboard(request) {
  return callCommand("build_home_dashboard", { request });
}

export async function panicWipeProfile(request) {
  return callCommand("panic_wipe_profile", { request });
}

export async function appendRuntimeLog(entry) {
  return callCommand("append_runtime_log", { entry });
}
