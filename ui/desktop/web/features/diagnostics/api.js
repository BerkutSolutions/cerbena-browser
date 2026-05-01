import { callCommand } from "../../core/commands.js";

export async function importSearchProviders(providers) {
  return callCommand("import_search_providers", { request: { providers } });
}

export async function setDefaultSearchProvider(providerId) {
  return callCommand("set_default_search_provider", { request: { providerId } });
}

export async function runGuardrailCheck(request) {
  return callCommand("run_guardrail_check", { request });
}

export async function appendRuntimeLog(entry) {
  return callCommand("append_runtime_log", { entry });
}

export async function readRuntimeLogs() {
  return callCommand("read_runtime_logs");
}
