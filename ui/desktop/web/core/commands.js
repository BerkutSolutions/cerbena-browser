import { nextCorrelationId, responseEnvelope } from "./contract.js";
import { executeMockCommand } from "./commands-mock.js";
import { APP_VERSION } from "./app-version.js";

let invokeImpl = async () => {
  throw new Error("Tauri invoke is unavailable in browser preview mode.");
};

if (typeof window !== "undefined" && window.__TAURI__?.core?.invoke) {
  invokeImpl = window.__TAURI__.core.invoke;
}

export async function callCommand(command, args = {}) {
  const correlationId = nextCorrelationId();
  try {
    const response = await invokeImpl(command, {
      ...args,
      correlationId
    });
    return responseEnvelope(true, response.data, response.messageKey ?? "command.success", correlationId);
  } catch (error) {
    const message = String(error);
    if (message.includes("Tauri invoke is unavailable")) {
      try {
        const data = executeMockCommand(command, args);
        return responseEnvelope(true, data, "command.mock.success", correlationId);
      } catch (mockError) {
        return responseEnvelope(false, { error: String(mockError) }, "command.mock.failed", correlationId);
      }
    }

    return responseEnvelope(false, { error: String(error) }, "command.failed", correlationId);
  }
}

export async function openExternalUrl(url) {
  return callCommand("open_external_url", {
    request: { url }
  });
}
