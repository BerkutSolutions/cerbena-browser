import { closeModalOverlay, showModalOverlay } from "../../core/modal.js";
import { openExternalUrl } from "../../core/commands.js";
import { closeIcon, escapeHtml } from "./view-helpers.js";

function dockerHelpModalHtml(t, mode) {
  const isMissing = mode === "missing";
  const title = isMissing ? t("docker.modal.missingTitle") : t("docker.modal.stoppedTitle");
  const body = isMissing ? t("docker.modal.missingBody") : t("docker.modal.stoppedBody");
  const actionLabel = isMissing ? t("docker.modal.download") : t("action.close");
  return `
    <div class="profiles-modal-overlay" id="docker-help-overlay">
      <div class="profiles-modal-window profiles-modal-window-sm">
        <div class="action-modal">
          <h3>${escapeHtml(title)}</h3>
          <p class="meta">${escapeHtml(body)}</p>
          <footer class="modal-actions">
            <button type="button" id="docker-help-cancel">${t("action.cancel")}</button>
            <button type="button" id="docker-help-submit">${escapeHtml(actionLabel)}</button>
          </footer>
        </div>
      </div>
    </div>
  `;
}

function linuxSandboxLaunchModalHtml(t) {
  return `
    <div class="profiles-modal-overlay" id="linux-sandbox-launch-overlay">
      <div class="profiles-modal-window profiles-modal-window-sm">
        <div class="action-modal">
          <h3>${escapeHtml(t("linuxSandbox.modal.title"))}</h3>
          <p class="meta">${escapeHtml(t("linuxSandbox.modal.launchWarning"))}</p>
          <footer class="modal-actions">
            <button type="button" id="linux-sandbox-launch-close">${t("action.close")}</button>
            <button type="button" id="linux-sandbox-launch-open">${t("linuxSandbox.modal.openDocs")}</button>
          </footer>
        </div>
      </div>
    </div>
  `;
}

function profileLogsModalHtml(t, profile, lines) {
  const content = Array.isArray(lines) && lines.length
    ? lines.join("\n")
    : t("profile.logs.empty");
  return `
    <div class="profiles-modal-overlay" id="profile-logs-overlay">
      <div class="profiles-modal-window profiles-modal-window-md action-modal profile-logs-modal">
        <div class="profiles-cookie-head">
          <h3>${escapeHtml(t("profile.logs.title"))}: ${escapeHtml(profile?.name ?? t("profile.launchProgress.profileFallback"))}</h3>
          <button type="button" class="profiles-icon-btn" id="profile-logs-close" aria-label="${t("action.close")}">${closeIcon()}</button>
        </div>
        <div class="profile-logs-console-shell">
          <pre class="preview-box profile-logs-pre">${escapeHtml(content)}</pre>
        </div>
        <footer class="modal-actions">
          <button type="button" id="profile-logs-refresh">${t("action.refresh")}</button>
          <button type="button" id="profile-logs-dismiss">${t("action.close")}</button>
        </footer>
      </div>
    </div>`;
}

export function classifyDockerRuntimeIssue(errorText) {
  const text = String(errorText ?? "").toLowerCase();
  if (
    text.includes("docker runtime is not installed or not reachable:")
    && (text.includes("program not found") || text.includes("not recognized"))
  ) {
    return "missing";
  }
  if (
    text.includes("network sandbox adapter 'container-vm' is not available:")
    || text.includes("container sandbox runtime is unavailable:")
    || text.includes("container runtime probe failed:")
    || text.includes("docker desktop server runtime is unavailable")
    || text.includes("error during connect")
  ) {
    return "stopped";
  }
  return null;
}

export async function showDockerHelpModal(t, mode) {
  return new Promise((resolve) => {
    const existing = document.body.querySelector("#docker-help-overlay");
    if (existing) {
      closeModalOverlay(existing);
    }
    document.body.insertAdjacentHTML("beforeend", dockerHelpModalHtml(t, mode));
    const overlay = document.body.querySelector("#docker-help-overlay");
    if (!overlay) {
      resolve(false);
      return;
    }
    const close = (value) => closeModalOverlay(overlay, () => resolve(value));
    showModalOverlay(overlay);
    overlay.querySelector("#docker-help-cancel")?.addEventListener("click", () => close(false));
    overlay.querySelector("#docker-help-submit")?.addEventListener("click", async () => {
      if (mode === "missing") {
        await openExternalUrl("https://www.docker.com/products/docker-desktop/");
      }
      close(true);
    });
    overlay.addEventListener("click", (event) => {
      if (event.target === overlay) {
        close(false);
      }
    });
  });
}

export async function showLinuxSandboxLaunchModal(t) {
  return new Promise((resolve) => {
    const existing = document.body.querySelector("#linux-sandbox-launch-overlay");
    if (existing) {
      closeModalOverlay(existing);
    }
    document.body.insertAdjacentHTML("beforeend", linuxSandboxLaunchModalHtml(t));
    const overlay = document.body.querySelector("#linux-sandbox-launch-overlay");
    if (!overlay) {
      resolve(false);
      return;
    }
    const close = (value) => closeModalOverlay(overlay, () => resolve(value));
    showModalOverlay(overlay);
    overlay.querySelector("#linux-sandbox-launch-close")?.addEventListener("click", () => close(false));
    overlay.querySelector("#linux-sandbox-launch-open")?.addEventListener("click", async () => {
      await openExternalUrl("https://chromium.googlesource.com/chromium/src/+/main/docs/security/apparmor-userns-restrictions.md");
      close(true);
    });
    overlay.addEventListener("click", (event) => {
      if (event.target === overlay) close(false);
    });
  });
}

export async function openProfileLogsModal(profile, t, readProfileLogs) {
  const existing = document.body.querySelector("#profile-logs-overlay");
  if (existing) {
    closeModalOverlay(existing);
  }
  const initial = await readProfileLogs(profile.id);
  const lines = initial.ok ? initial.data : [String(initial.data.error)];
  document.body.insertAdjacentHTML("beforeend", profileLogsModalHtml(t, profile, lines));
  const overlay = document.body.querySelector("#profile-logs-overlay");
  if (!overlay) return;

  const reload = async () => {
    const result = await readProfileLogs(profile.id);
    const text = result.ok
      ? ((result.data ?? []).join("\n") || t("profile.logs.empty"))
      : String(result.data.error);
    const pre = overlay.querySelector(".profile-logs-pre");
    if (pre) pre.textContent = text;
  };
  const close = () => closeModalOverlay(overlay);
  showModalOverlay(overlay);
  overlay.querySelector("#profile-logs-close")?.addEventListener("click", close);
  overlay.querySelector("#profile-logs-dismiss")?.addEventListener("click", close);
  overlay.querySelector("#profile-logs-refresh")?.addEventListener("click", reload);
  overlay.addEventListener("click", (event) => {
    if (event.target === overlay) {
      close();
    }
  });
}
