import { bootstrap, renderFatalError } from "./main-runtime.js";

window.addEventListener("error", (event) => renderFatalError(event.error ?? event.message));
window.addEventListener("unhandledrejection", (event) => renderFatalError(event.reason));
bootstrap().catch((error) => renderFatalError(error));
