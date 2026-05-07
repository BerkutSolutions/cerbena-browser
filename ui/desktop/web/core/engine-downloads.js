const DOWNLOAD_EVENTS = ["engine-download-progress", "network-runtime-progress"];
const CONTAINER_ID = "engine-download-notifications";

async function cancelEngineDownload(engine) {
  if (!engine || !window.__TAURI__?.core?.invoke) return false;
  try {
    const correlationId = `engine-cancel-${Date.now()}`;
    await window.__TAURI__.core.invoke("cancel_engine_download", {
      engine,
      correlationId,
    });
    return true;
  } catch {
    return false;
  }
}

function getListen() {
  return window.__TAURI__?.event?.listen ?? null;
}

function formatBytes(bytes) {
  if (!bytes) return "0 B";
  const units = ["B", "KB", "MB", "GB"];
  let value = bytes;
  let unit = 0;
  while (value >= 1024 && unit < units.length - 1) {
    value /= 1024;
    unit += 1;
  }
  return `${value.toFixed(value >= 100 || unit === 0 ? 0 : 1)} ${units[unit]}`;
}

function formatEta(seconds, t) {
  if (!seconds || !Number.isFinite(seconds) || seconds <= 0) {
    return t("engineDownload.calculating");
  }
  if (seconds < 60) return `${Math.round(seconds)}s`;
  const minutes = Math.floor(seconds / 60);
  const remaining = Math.round(seconds % 60);
  return `${minutes}m ${remaining}s`;
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

function titleFor(progress, t) {
  const engineName = progressDisplayName(progress);
  const template = progress.stage === "completed"
    ? t("engineDownload.readyTitle")
    : t("engineDownload.title");
  return template
    .replace("{engine}", engineName)
    .replace("{version}", progress.version);
}

function progressDisplayName(progress) {
  if (progress.engine === "chromium") return "Chromium";
  if (progress.engine === "ungoogled-chromium") return "Ungoogled Chromium";
  if (progress.engine === "librewolf") return "LibreWolf";
  if (progress.tool === "sing-box") return "sing-box";
  if (progress.tool === "openvpn") return "OpenVPN";
  if (progress.tool === "tor-bundle") return "Tor";
  if (progress.engine) return String(progress.engine);
  if (progress.tool) return String(progress.tool);
  return "Runtime";
}

function asNumber(value, fallback = 0) {
  const parsed = Number(value);
  return Number.isFinite(parsed) ? parsed : fallback;
}

function normalizeProgress(payload) {
  if (!payload || typeof payload !== "object") return null;

  const downloaded = asNumber(payload.downloaded_bytes ?? payload.downloadedBytes, 0);
  const totalRaw = payload.total_bytes ?? payload.totalBytes ?? null;
  const total = totalRaw == null ? null : asNumber(totalRaw, 0);
  const speed = asNumber(payload.speed_bytes_per_sec ?? payload.speedBytesPerSec, 0);
  const percentageRaw = payload.percentage;
  const computedPercentage = total && total > 0 ? (downloaded / total) * 100 : 0;
  const percentage = asNumber(percentageRaw, computedPercentage);
  const etaFromPayload = payload.eta_seconds ?? payload.etaSeconds ?? null;
  const etaComputed = speed > 0 && total && total > downloaded ? (total - downloaded) / speed : null;
  const etaSeconds = etaFromPayload == null ? etaComputed : asNumber(etaFromPayload, etaComputed ?? 0);

  return {
    engine: payload.engine ?? null,
    tool: payload.tool ?? null,
    version: String(payload.version ?? "pending"),
    stage: String(payload.stage ?? "downloading"),
    host: payload.host ?? null,
    downloaded_bytes: downloaded,
    total_bytes: total,
    percentage,
    speed_bytes_per_sec: speed,
    eta_seconds: etaSeconds,
    message: payload.message ? String(payload.message) : null,
  };
}

function detailsFor(progress, t) {
  const host = progress.host ? progress.host : null;
  if (progress.stage === "error") {
    return progress.message ? String(progress.message) : "Download failed";
  }
  if (progress.stage === "cancelled") {
    return progress.message ? String(progress.message) : "Download interrupted by user.";
  }
  if (progress.stage === "pending") return t("engineDownload.resolving");
  if (progress.stage === "connecting") {
    return host ? `${t("engineDownload.resolving")} ${host}` : t("engineDownload.resolving");
  }
  if (progress.stage === "resolving") return t("engineDownload.resolving");
  if (progress.stage === "resolved") return t("engineDownload.resolved");
  if (progress.stage === "extracting") return t("engineDownload.extracting");
  if (progress.stage === "verifying") return t("engineDownload.verifying");
  if (progress.stage === "completed") return t("engineDownload.completed");
  if ((progress.downloaded_bytes ?? 0) === 0) {
    return host ? `${t("engineDownload.calculating")} ${host}` : t("engineDownload.calculating");
  }
  const speed = progress.speed_bytes_per_sec > 0 ? `${formatBytes(progress.speed_bytes_per_sec)}/s` : "0 B/s";
  const eta = formatEta(progress.eta_seconds, t);
  const transferred = progress.total_bytes
    ? `${formatBytes(progress.downloaded_bytes ?? 0)} / ${formatBytes(progress.total_bytes)}`
    : `${formatBytes(progress.downloaded_bytes ?? 0)}`;
  return `${(progress.percentage ?? 0).toFixed(1)}% / ${transferred} / ${speed} / ${eta}`;
}

function render(entry, t) {
  const percent = Math.max(0, Math.min(100, entry.progress.percentage ?? 0));
  entry.element.innerHTML = `
    <div class="engine-download-card">
      <button type="button" class="engine-download-close" aria-label="close">x</button>
      <div class="engine-download-icon">dl</div>
      <div class="engine-download-body">
        <strong>${titleFor(entry.progress, t)}</strong>
        <p>${detailsFor(entry.progress, t)}</p>
        <div class="engine-download-track">
          <div class="engine-download-fill" style="width:${percent}%"></div>
        </div>
      </div>
    </div>
  `;
  entry.element.querySelector(".engine-download-close")?.addEventListener("click", async () => {
    if (entry.progress?.engine) {
      const cancelled = await cancelEngineDownload(entry.progress.engine);
      entry.progress = {
        ...entry.progress,
        stage: "cancelled",
        message: cancelled ? "Download interrupted by user." : "Download cancellation requested.",
      };
      render(entry, t);
      window.setTimeout(() => {
        if (entry.element.isConnected) {
          entry.remove();
        }
      }, 6500);
      return;
    }
    entry.remove();
  });
}

export async function initEngineDownloadNotifications(i18n) {
  const listen = getListen();
  if (!listen) return () => {};

  const container = ensureContainer();
  const entries = new Map();
  const t = i18n.t;
  const unlistenCallbacks = [];
  let staleTimer = null;

  const handleProgress = (payload) => {
    const progress = normalizeProgress(payload);
    if (!progress) return;
    if (progress.stage === "resolving" || progress.stage === "resolved") return;

    const source = progress.engine ? "engine" : "network";
    const identity = progress.engine ?? progress.tool ?? "runtime";
    const key = `${source}:${identity}:${progress.version}`;
    let entry = entries.get(key);
    if (!entry) {
      const element = document.createElement("div");
      element.className = "engine-download-notification";
      const remove = () => {
        entries.delete(key);
        element.remove();
      };
      entry = { element, progress, remove };
      entries.set(key, entry);
      container.appendChild(element);
    }
    entry.progress = progress;
    entry.lastUpdateAt = Date.now();
    render(entry, t);
    if (progress.stage === "completed" || progress.stage === "error" || progress.stage === "cancelled") {
      window.setTimeout(() => {
        if (entries.get(key) === entry) {
          entry.remove();
        }
      }, progress.stage === "completed" ? 3200 : 6500);
    }
  };

  staleTimer = window.setInterval(() => {
    const now = Date.now();
    for (const [key, entry] of entries) {
      const stage = entry?.progress?.stage;
      if (stage === "completed" || stage === "error") continue;
      const lastUpdateAt = Number(entry?.lastUpdateAt ?? 0);
      if (!lastUpdateAt || now - lastUpdateAt < 45000) continue;
      entry.progress = {
        ...entry.progress,
        stage: "error",
        message: "Download stalled. Please retry."
      };
      entry.lastUpdateAt = now;
      render(entry, t);
      window.setTimeout(() => {
        if (entries.get(key) === entry) {
          entry.remove();
        }
      }, 6500);
    }
  }, 12000);

  for (const eventName of DOWNLOAD_EVENTS) {
    const unlisten = await listen(eventName, (event) => handleProgress(event.payload));
    unlistenCallbacks.push(unlisten);
  }

  return () => {
    try {
      if (staleTimer) {
        window.clearInterval(staleTimer);
      }
      for (const unlisten of unlistenCallbacks) {
        unlisten();
      }
    } catch {}
  };
}
