import {
  importExtensionLibraryItem,
  listExtensionLibrary,
  removeExtensionLibraryItem,
  setExtensionProfiles,
  updateExtensionLibraryItem
} from "./api.js";
import { askInputModal } from "../../core/modal.js";

function escapeHtml(value) {
  return String(value ?? "")
    .replaceAll("&", "&amp;")
    .replaceAll("<", "&lt;")
    .replaceAll(">", "&gt;")
    .replaceAll("\"", "&quot;");
}

function normalizeEngineScope(value) {
  const scope = String(value ?? "chromium/firefox").toLowerCase();
  if (scope === "firefox") return "firefox";
  if (scope === "chromium") return "chromium";
  return "chromium/firefox";
}

function profileMatchesScope(profile, scope) {
  if (scope === "firefox") return profile.engine === "camoufox";
  if (scope === "chromium") return profile.engine === "wayfern";
  return true;
}

function compatibleProfiles(item, profiles) {
  const scope = normalizeEngineScope(item.engineScope);
  return (profiles ?? []).filter((profile) => profileMatchesScope(profile, scope));
}

function profileSummary(item, profiles, t) {
  const assigned = (item.assignedProfileIds ?? [])
    .map((id) => profiles.find((profile) => profile.id === id)?.name ?? id)
    .filter(Boolean);
  return assigned.length ? assigned.join(", ") : t("extensions.assign.none");
}

function sourceLabel(item) {
  return item.storeUrl || item.packageFileName || item.sourceValue || "";
}

function extensionCard(item, profiles, t) {
  const name = item.displayName ?? "Extension";
  const version = item.version ?? "1.0.1";
  const engine = item.engineScope ?? "chromium/firefox";
  const scopedProfiles = compatibleProfiles(item, profiles);
  return `
    <tr data-extension-id="${item.id}">
      <td>
        <div class="profiles-name">${escapeHtml(name)}</div>
        <div class="meta">${escapeHtml(sourceLabel(item))}</div>
      </td>
      <td>${escapeHtml(version)}</td>
      <td>${escapeHtml(engine)}</td>
      <td>
        <div class="dns-dropdown">
          <button type="button" class="dns-dropdown-toggle" data-profile-menu-toggle="${item.id}">${escapeHtml(profileSummary(item, profiles, t))}</button>
          <div class="dns-dropdown-menu hidden" data-profile-menu="${item.id}">
            ${scopedProfiles.length ? scopedProfiles.map((profile) => `
              <label class="dns-dropdown-option">
                <input
                  type="checkbox"
                  data-profile-assign="${item.id}:${profile.id}"
                  ${(item.assignedProfileIds ?? []).includes(profile.id) ? "checked" : ""}
                />
                <span>${escapeHtml(profile.name)}</span>
              </label>
            `).join("") : `<div class="meta">${t("extensions.noCompatibleProfiles")}</div>`}
          </div>
        </div>
      </td>
      <td class="actions">
        <button data-action="edit">${t("extensions.edit")}</button>
        <button data-action="remove">${t("extensions.remove")}</button>
      </td>
    </tr>
  `;
}

function addModalHtml(t, profiles, draft = {}) {
  const scopedProfiles = compatibleProfiles(draft, profiles);
  return `
    <div class="profiles-modal-overlay" id="extension-library-overlay">
      <div class="profiles-modal-window profiles-modal-window-md">
        <div class="profiles-cookie-head">
          <h3>${draft.id ? t("extensions.editTitle") : t("extensions.importTitle")}</h3>
          <button type="button" class="profiles-icon-btn" id="extension-library-close" aria-label="${t("action.cancel")}">x</button>
        </div>
        <div class="grid-two">
          <label>${t("extensions.name")}<input id="extension-name" value="${escapeHtml(draft.displayName ?? "")}" /></label>
          <label>${t("extensions.version")}<input id="extension-version" value="${escapeHtml(draft.version ?? "1.0.1")}" /></label>
          <label>${t("extensions.engine")}<select id="extension-engine">
            <option value="chromium" ${normalizeEngineScope(draft.engineScope) === "chromium" ? "selected" : ""}>chromium</option>
            <option value="firefox" ${normalizeEngineScope(draft.engineScope) === "firefox" ? "selected" : ""}>firefox</option>
            <option value="chromium/firefox" ${normalizeEngineScope(draft.engineScope) === "chromium/firefox" ? "selected" : ""}>chromium/firefox</option>
          </select></label>
          <label>${t("extensions.logoUrl")}<input id="extension-logo" value="${escapeHtml(draft.logoUrl ?? "")}" /></label>
          <label>${t("extensions.logoUpload")}<input id="extension-logo-file" type="file" accept="image/*" /></label>
          <label class="grid-span-2">${t("extensions.storeUrl")}<input id="extension-store-url" value="${escapeHtml(draft.storeUrl ?? "")}" /></label>
        </div>
        <div class="security-frame" style="margin-top:12px;">
          <h4>${t("extensions.assignProfiles")}</h4>
          <div class="dns-dropdown-menu" style="position:static;display:grid;grid-template-columns:repeat(2,minmax(0,1fr));">
            ${scopedProfiles.length ? scopedProfiles.map((profile) => `
              <label class="dns-dropdown-option">
                <input
                  type="checkbox"
                  data-library-profile="${profile.id}"
                  ${(draft.assignedProfileIds ?? []).includes(profile.id) ? "checked" : ""}
                />
                <span>${escapeHtml(profile.name)}</span>
              </label>
            `).join("") : `<div class="meta">${t("extensions.noCompatibleProfiles")}</div>`}
          </div>
        </div>
        <footer class="modal-actions">
          <button type="button" id="extension-library-cancel">${t("action.cancel")}</button>
          <button type="button" id="extension-library-save">${t("action.save")}</button>
        </footer>
      </div>
    </div>
  `;
}

function readAssignedProfiles(overlay) {
  return [...overlay.querySelectorAll("[data-library-profile]:checked")].map((checkbox) => checkbox.getAttribute("data-library-profile"));
}

async function openLibraryModal(root, t, profiles, draft = null) {
  return new Promise((resolve) => {
    document.body.insertAdjacentHTML("beforeend", addModalHtml(t, profiles, draft ?? {}));
    const overlay = document.body.querySelector("#extension-library-overlay");
    const close = (payload = null) => {
      overlay.remove();
      resolve(payload);
    };
    overlay.querySelector("#extension-library-close")?.addEventListener("click", () => close());
    overlay.querySelector("#extension-library-cancel")?.addEventListener("click", () => close());
    overlay.addEventListener("click", (event) => {
      if (event.target === overlay) close();
    });
    overlay.querySelector("#extension-library-save")?.addEventListener("click", async () => {
      const file = overlay.querySelector("#extension-logo-file")?.files?.[0] ?? null;
      const uploadedLogoUrl = file ? await readFileAsDataUrl(file) : null;
      close({
        extensionId: draft?.id ?? null,
        displayName: overlay.querySelector("#extension-name")?.value?.trim() || null,
        version: overlay.querySelector("#extension-version")?.value?.trim() || null,
        engineScope: overlay.querySelector("#extension-engine")?.value || null,
        logoUrl: uploadedLogoUrl || overlay.querySelector("#extension-logo")?.value?.trim() || null,
        storeUrl: overlay.querySelector("#extension-store-url")?.value?.trim() || null,
        assignedProfileIds: readAssignedProfiles(overlay)
      });
    });
  });
}

function readFileAsDataUrl(file) {
  return new Promise((resolve, reject) => {
    const reader = new FileReader();
    reader.onload = () => resolve(reader.result);
    reader.onerror = () => reject(reader.error);
    reader.readAsDataURL(file);
  });
}

function readFileAsBase64(file) {
  return new Promise((resolve, reject) => {
    const reader = new FileReader();
    reader.onload = () => {
      try {
        const bytes = new Uint8Array(reader.result);
        let binary = "";
        const chunkSize = 0x8000;
        for (let index = 0; index < bytes.length; index += chunkSize) {
          binary += String.fromCharCode(...bytes.subarray(index, index + chunkSize));
        }
        resolve(btoa(binary));
      } catch (error) {
        reject(error);
      }
    };
    reader.onerror = () => reject(reader.error);
    reader.readAsArrayBuffer(file);
  });
}

async function importLocalPackage(file, sourceKind) {
  const packageBytesBase64 = await readFileAsBase64(file);
  return importExtensionLibraryItem({
    sourceKind,
    sourceValue: file.name,
    assignedProfileIds: [],
    packageFileName: file.name,
    packageBytesBase64
  });
}

export function renderExtensions(t, model) {
  const state = model.extensionLibraryState ?? { items: {} };
  const items = Object.values(state.items ?? {});
  const notice = model.extensionNotice ? `<p class="notice ${model.extensionNotice.type}">${model.extensionNotice.text}</p>` : "";
  return `
    <div class="feature-page">
      <div class="feature-page-head row-between">
        <div>
          <h2>${t("nav.extensions")}</h2>
        </div>
        <div class="top-actions">
          <button id="extension-add-url">${t("extensions.addStoreUrl")}</button>
          <button id="extension-add-local">${t("extensions.addLocal")}</button>
        </div>
      </div>
      ${notice}
      <input id="extension-local-picker" type="file" accept=".zip,.xpi,.crx,application/zip,application/x-xpinstall" style="display:none;" />
      <div class="security-frame">
        <h4>${t("extensions.dropTitle")}</h4>
        <div id="extension-dropzone" class="profiles-target-box" style="min-height:96px;justify-content:center;cursor:pointer;">
          <div class="meta">${t("extensions.dropHint")}</div>
        </div>
      </div>
      <div class="panel" style="margin-top:12px;">
        <table class="extensions-table">
          <thead>
            <tr>
              <th>${t("extensions.name")}</th>
              <th>${t("extensions.version")}</th>
              <th>${t("extensions.engine")}</th>
              <th>${t("extensions.assignProfiles")}</th>
              <th>${t("extensions.actions")}</th>
            </tr>
          </thead>
          <tbody>
            ${items.length ? items.map((item) => extensionCard(item, model.profiles ?? [], t)).join("") : `<tr><td colspan="5" class="meta">${t("extensions.empty")}</td></tr>`}
          </tbody>
        </table>
      </div>
    </div>
  `;
}

export async function hydrateExtensionsModel(model) {
  const result = await listExtensionLibrary();
  model.extensionLibraryState = result.ok ? JSON.parse(result.data || "{}") : { items: {} };
}

async function createFromUrl(model, rerender, t) {
  const url = await askInputModal(t, {
    title: t("extensions.addStoreUrl"),
    label: t("extensions.storePrompt"),
    defaultValue: "https://addons.mozilla.org/firefox/addon/ublock-origin/"
  });
  if (!url) return;
  const result = await importExtensionLibraryItem({
    sourceKind: "store_url",
    sourceValue: url,
    storeUrl: url,
    assignedProfileIds: []
  });
  model.extensionNotice = { type: result.ok ? "success" : "error", text: result.ok ? t("extensions.installed") : String(result.data.error) };
  await hydrateExtensionsModel(model);
  await rerender();
}

async function createFromLocalFile(model, rerender, t, file, sourceKind) {
  if (!file) return;
  const result = await importLocalPackage(file, sourceKind);
  model.extensionNotice = { type: result.ok ? "success" : "error", text: result.ok ? t("extensions.installed") : String(result.data.error) };
  await hydrateExtensionsModel(model);
  await rerender();
}

export function wireExtensions(root, model, rerender, t) {
  const localPicker = root.querySelector("#extension-local-picker");
  root.querySelector("#extension-add-url")?.addEventListener("click", async () => {
    await createFromUrl(model, rerender, t);
  });
  root.querySelector("#extension-add-local")?.addEventListener("click", async () => {
    localPicker?.click();
  });
  localPicker?.addEventListener("change", async (event) => {
    const file = event.target.files?.[0];
    await createFromLocalFile(model, rerender, t, file, "local_file");
    event.target.value = "";
  });

  const dropzone = root.querySelector("#extension-dropzone");
  dropzone?.addEventListener("dragover", (event) => {
    event.preventDefault();
    dropzone.classList.add("is-active");
  });
  dropzone?.addEventListener("dragleave", () => dropzone.classList.remove("is-active"));
  dropzone?.addEventListener("drop", async (event) => {
    event.preventDefault();
    dropzone.classList.remove("is-active");
    const file = event.dataTransfer?.files?.[0];
    await createFromLocalFile(model, rerender, t, file, "dropped_file");
  });
  dropzone?.addEventListener("click", async () => {
    localPicker?.click();
  });

  for (const button of root.querySelectorAll("[data-profile-menu-toggle]")) {
    button.addEventListener("click", () => {
      const id = button.getAttribute("data-profile-menu-toggle");
      root.querySelector(`[data-profile-menu='${id}']`)?.classList.toggle("hidden");
    });
  }
  for (const checkbox of root.querySelectorAll("[data-profile-assign]")) {
    checkbox.addEventListener("change", async () => {
      const [extensionId] = checkbox.getAttribute("data-profile-assign").split(":");
      const assignedProfileIds = [...root.querySelectorAll(`[data-profile-assign^='${extensionId}:']:checked`)].map((item) => item.getAttribute("data-profile-assign").split(":")[1]);
      const result = await setExtensionProfiles(extensionId, assignedProfileIds);
      model.extensionNotice = { type: result.ok ? "success" : "error", text: result.ok ? t("action.save") : String(result.data.error) };
      await hydrateExtensionsModel(model);
      await rerender();
    });
  }

  for (const rowEl of root.querySelectorAll(".extensions-table tbody tr[data-extension-id]")) {
    rowEl.addEventListener("click", async (event) => {
      const action = event.target?.closest?.("[data-action]")?.getAttribute?.("data-action");
      if (!action) return;
      const extensionId = rowEl.getAttribute("data-extension-id");
      const item = model.extensionLibraryState?.items?.[extensionId];
      if (!item) return;
      if (action === "remove") {
        const result = await removeExtensionLibraryItem(extensionId);
        model.extensionNotice = { type: result.ok ? "success" : "error", text: result.ok ? t("extensions.removed") : String(result.data.error) };
      }
      if (action === "edit") {
        const payload = await openLibraryModal(root, t, model.profiles ?? [], item);
        if (!payload) return;
        const result = await updateExtensionLibraryItem({
          extensionId,
          displayName: payload.displayName || item.displayName,
          version: payload.version || item.version,
          engineScope: payload.engineScope || item.engineScope,
          storeUrl: payload.storeUrl || item.storeUrl || null,
          logoUrl: payload.logoUrl || item.logoUrl || null
        });
        if (result.ok) {
          await setExtensionProfiles(extensionId, payload.assignedProfileIds);
        }
        model.extensionNotice = { type: result.ok ? "success" : "error", text: result.ok ? t("extensions.edited") : String(result.data.error) };
      }
      await hydrateExtensionsModel(model);
      await rerender();
    });
  }
}
