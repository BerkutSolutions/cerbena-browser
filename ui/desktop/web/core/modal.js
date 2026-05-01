function escapeHtml(value) {
  return String(value ?? "")
    .replaceAll("&", "&amp;")
    .replaceAll("<", "&lt;")
    .replaceAll(">", "&gt;")
    .replaceAll("\"", "&quot;");
}

function inputModalHtml(id, t, options) {
  const title = options.title ?? "";
  const label = options.label ?? "";
  const value = options.defaultValue ?? "";
  const placeholder = options.placeholder ?? "";
  const submitLabel = options.submitLabel ?? t("action.submit");
  const cancelLabel = options.cancelLabel ?? t("action.cancel");
  const multiline = Boolean(options.multiline);
  const field = multiline
    ? `<textarea id="${id}-input" rows="6" placeholder="${escapeHtml(placeholder)}">${escapeHtml(value)}</textarea>`
    : `<input id="${id}-input" value="${escapeHtml(value)}" placeholder="${escapeHtml(placeholder)}" />`;
  return `
  <div class="profiles-modal-overlay" id="${id}">
    <div class="profiles-modal-window profiles-modal-window-sm action-modal">
      <h3>${escapeHtml(title)}</h3>
      <label>${escapeHtml(label)}${field}</label>
      <footer class="modal-actions">
        <button type="button" data-modal-cancel>${escapeHtml(cancelLabel)}</button>
        <button type="button" data-modal-submit>${escapeHtml(submitLabel)}</button>
      </footer>
    </div>
  </div>`;
}

function confirmModalHtml(id, t, options) {
  const title = options.title ?? "";
  const description = options.description ?? "";
  const submitLabel = options.submitLabel ?? t("action.confirm");
  const cancelLabel = options.cancelLabel ?? t("action.cancel");
  return `
  <div class="profiles-modal-overlay" id="${id}">
    <div class="profiles-modal-window profiles-modal-window-sm action-modal">
      <h3>${escapeHtml(title)}</h3>
      <p class="meta">${escapeHtml(description)}</p>
      <footer class="modal-actions">
        <button type="button" data-modal-cancel>${escapeHtml(cancelLabel)}</button>
        <button type="button" data-modal-submit>${escapeHtml(submitLabel)}</button>
      </footer>
    </div>
  </div>`;
}

export function showModalOverlay(overlay) {
  if (!overlay) return;
  overlay.classList.add("modal-enter");
  requestAnimationFrame(() => {
    overlay.classList.add("modal-enter-active");
  });
}

export function closeModalOverlay(overlay, onClosed) {
  if (!overlay) {
    onClosed?.();
    return;
  }
  if (overlay.dataset.modalClosing === "true") {
    return;
  }
  overlay.dataset.modalClosing = "true";
  overlay.classList.remove("modal-enter-active");
  overlay.classList.add("modal-closing");
  const finish = () => {
    if (!overlay.isConnected) {
      onClosed?.();
      return;
    }
    overlay.remove();
    onClosed?.();
  };
  overlay.addEventListener("transitionend", finish, { once: true });
  setTimeout(finish, 220);
}

export async function askInputModal(t, options = {}) {
  return new Promise((resolve) => {
    const id = `action-modal-${Date.now().toString(36)}-${Math.random().toString(36).slice(2, 8)}`;
    document.body.insertAdjacentHTML("beforeend", inputModalHtml(id, t, options));
    const overlay = document.body.querySelector(`#${id}`);
    if (!overlay) {
      resolve(null);
      return;
    }
    const input = overlay.querySelector(`#${id}-input`);
    const close = (value = null) => {
      closeModalOverlay(overlay, () => resolve(value));
    };
    showModalOverlay(overlay);
    overlay.querySelector("[data-modal-cancel]")?.addEventListener("click", () => close(null));
    overlay.querySelector("[data-modal-submit]")?.addEventListener("click", () => {
      const value = String(input?.value ?? "").trim();
      close(value || null);
    });
    overlay.addEventListener("click", (event) => {
      if (event.target === overlay) {
        close(null);
      }
    });
    overlay.addEventListener("keydown", (event) => {
      if (event.key === "Escape") {
        event.preventDefault();
        close(null);
      }
      if (event.key === "Enter" && !options.multiline) {
        event.preventDefault();
        const value = String(input?.value ?? "").trim();
        close(value || null);
      }
    });
    setTimeout(() => input?.focus(), 0);
  });
}

export async function askConfirmModal(t, options = {}) {
  return new Promise((resolve) => {
    const id = `confirm-modal-${Date.now().toString(36)}-${Math.random().toString(36).slice(2, 8)}`;
    document.body.insertAdjacentHTML("beforeend", confirmModalHtml(id, t, options));
    const overlay = document.body.querySelector(`#${id}`);
    if (!overlay) {
      resolve(false);
      return;
    }
    const close = (value) => {
      closeModalOverlay(overlay, () => resolve(value));
    };
    showModalOverlay(overlay);
    overlay.querySelector("[data-modal-cancel]")?.addEventListener("click", () => close(false));
    overlay.querySelector("[data-modal-submit]")?.addEventListener("click", () => close(true));
    overlay.addEventListener("click", (event) => {
      if (event.target === overlay) {
        close(false);
      }
    });
    overlay.addEventListener("keydown", (event) => {
      if (event.key === "Escape") {
        event.preventDefault();
        close(false);
      }
      if (event.key === "Enter") {
        event.preventDefault();
        close(true);
      }
    });
    setTimeout(() => overlay.querySelector("[data-modal-submit]")?.focus(), 0);
  });
}
