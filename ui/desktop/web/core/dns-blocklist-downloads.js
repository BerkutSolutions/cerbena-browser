const EVENT_NAME = "dns-blocklist-progress";
const CONTAINER_ID = "engine-download-notifications";
const ENTRY_ID = "dns-blocklist-notification";

function getListen() {
  return window.__TAURI__?.event?.listen ?? null;
}

function ensureContainer() {
  let container = document.getElementById(CONTAINER_ID);
  if (!container) {
    container = document.createElement("div");
    container.id = CONTAINER_ID;
    document.body.appendChild(container);
  }
  return container;
}

function formatElapsed(seconds) {
  if (!Number.isFinite(seconds) || seconds <= 0) return "0s";
  if (seconds < 60) return `${Math.round(seconds)}s`;
  const minutes = Math.floor(seconds / 60);
  const remaining = Math.round(seconds % 60);
  return `${minutes}m ${remaining}s`;
}

function render(element, payload, title) {
  const percent = Math.max(0, Math.min(100, Number(payload.progress ?? 0)));
  element.innerHTML = `
    <div class="engine-download-card dns-blocklist-card">
      <div class="engine-download-icon">dns</div>
      <div class="engine-download-body">
        <strong>${title}</strong>
        <p>${payload.name || ""}</p>
        <p>${percent.toFixed(0)}% / ${payload.processed ?? 0} / ${payload.total ?? 0} / ${formatElapsed(payload.elapsedSeconds ?? 0)}</p>
        <div class="engine-download-track">
          <div class="engine-download-fill" style="width:${percent}%"></div>
        </div>
      </div>
    </div>
  `;
}

export async function initDnsBlocklistNotifications(i18n) {
  const listen = getListen();
  if (!listen) return () => {};

  const container = ensureContainer();
  let element = null;
  const title = i18n?.t?.("security.blocklists.progressTitle") ?? "DNS blocklist update";

  const unlisten = await listen(EVENT_NAME, (event) => {
    const payload = event.payload ?? {};
    if (!element) {
      element = document.createElement("div");
      element.id = ENTRY_ID;
      element.className = "engine-download-notification";
      container.appendChild(element);
    }
    render(element, payload, title);
    if (payload.stage === "completed") {
      window.setTimeout(() => {
        element?.remove();
        element = null;
      }, 3200);
    }
  });

  return () => {
    try {
      unlisten();
    } catch {}
  };
}
