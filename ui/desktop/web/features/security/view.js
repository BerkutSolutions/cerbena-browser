import { pickCertificateFiles, saveGlobalSecuritySettings } from "./api.js";
import {
  buildGlobalSecuritySaveRequest,
  ensureGlobalSecurityState,
  hydrateGlobalSecurityState
} from "./shared.js";

function escapeHtml(value) {
  return String(value ?? "")
    .replaceAll("&", "&amp;")
    .replaceAll("<", "&lt;")
    .replaceAll(">", "&gt;")
    .replaceAll('"', "&quot;");
}

function slugId(value) {
  return String(value ?? "")
    .toLowerCase()
    .replace(/[^a-z0-9]+/g, "-")
    .replace(/^-+|-+$/g, "");
}

function makeUniqueId(seed, existingIds) {
  const base = slugId(seed) || "item";
  let candidate = base;
  let suffix = 2;
  while (existingIds.has(candidate)) {
    candidate = `${base}-${suffix}`;
    suffix += 1;
  }
  return candidate;
}

function certificateProfileSummary(cert, profiles, t) {
  if (cert.applyGlobally) return t("security.certificates.allProfiles");
  const names = (cert.profileIds ?? [])
    .map((id) => profiles.find((item) => item.id === id)?.name)
    .filter(Boolean);
  return names.length ? names.join(", ") : t("security.certificates.notAssigned");
}

function certificateProfilesMenu(cert, profiles) {
  return `
    <div class="dns-dropdown-menu hidden" id="cert-menu-${cert.id}">
      ${profiles.map((profile) => `
        <label class="dns-blocklist-option">
          <input type="checkbox" data-cert-profile="${cert.id}:${profile.id}" ${(cert.profileIds ?? []).includes(profile.id) ? "checked" : ""} ${cert.applyGlobally ? "disabled" : ""} />
          <span>${escapeHtml(profile.name)}</span>
        </label>
      `).join("")}
    </div>
  `;
}

async function persistSecurity(model, t, rerender) {
  const result = await saveGlobalSecuritySettings(buildGlobalSecuritySaveRequest(ensureGlobalSecurityState(model)));
  model.securityNotice = {
    type: result.ok ? "success" : "error",
    text: result.ok ? t("action.save") : String(result.data.error)
  };
  if (result.ok) {
    model.securityLoaded = false;
    await hydrateGlobalSecurityState(model);
  }
  await rerender();
}

export function renderSecurity(t, model) {
  const state = ensureGlobalSecurityState(model);
  const notice = model.securityNotice ? `<p class="notice ${model.securityNotice.type}">${model.securityNotice.text}</p>` : "";
  return `
  <div class="feature-page">
    <div class="dns-section-head">
      <div>
        <h2>${t("nav.security")}</h2>
        <p class="meta">${t("security.certificates.hint")}</p>
      </div>
      <div class="top-actions">
        <button id="sec-cert-add">${t("security.certificates.add")}</button>
      </div>
    </div>
    ${notice}

    <div class="panel security-table-frame">
      <table class="extensions-table">
        <thead><tr><th>${t("extensions.name")}</th><th>${t("security.profiles")}</th><th>${t("extensions.actions")}</th></tr></thead>
        <tbody>
          ${(state.certificates ?? []).map((cert) => `
            <tr>
              <td>${escapeHtml(cert.name)}</td>
              <td>
                <div class="dns-dropdown">
                  <button type="button" class="dns-dropdown-toggle" data-cert-menu-toggle="${cert.id}">${escapeHtml(certificateProfileSummary(cert, model.profiles ?? [], t))}</button>
                  ${certificateProfilesMenu(cert, model.profiles ?? [])}
                </div>
                <label class="checkbox-inline"><input type="checkbox" data-cert-global="${cert.id}" ${cert.applyGlobally ? "checked" : ""}/> <span>${t("security.certificates.global")}</span></label>
              </td>
              <td class="actions"><button type="button" data-cert-remove="${cert.id}">${t("extensions.remove")}</button></td>
            </tr>
          `).join("") || `<tr><td colspan="3" class="meta">${t("security.certificates.empty")}</td></tr>`}
        </tbody>
      </table>
    </div>
  </div>`;
}

export function wireSecurity(root, model, rerender, t) {
  if (!model.securityLoaded) {
    hydrateGlobalSecurityState(model).then(async () => {
      await rerender();
    });
    return;
  }

  root.querySelector("#sec-cert-add")?.addEventListener("click", async () => {
    const picked = await pickCertificateFiles();
    if (!picked.ok) {
      model.securityNotice = {
        type: "error",
        text: String(picked.data?.error ?? "pick_certificate_files failed")
      };
      await rerender();
      return;
    }
    const state = ensureGlobalSecurityState(model);
    const existingIds = new Set((state.certificates ?? []).map((item) => String(item.id ?? "")));
    const existingPaths = new Set((state.certificates ?? []).map((item) => String(item.path ?? "").trim().toLowerCase()));
    for (const rawPath of (Array.isArray(picked.data) ? picked.data : [])) {
      const clean = String(rawPath ?? "").trim();
      if (!clean || existingPaths.has(clean.toLowerCase()) || existingIds.has(slugId(clean))) continue;
      existingPaths.add(clean.toLowerCase());
      state.certificates.push({
        id: makeUniqueId(clean, existingIds),
        name: clean.split(/[/\\]/).pop()?.replace(/\.(pem|crt|cer)$/i, "") || clean,
        path: clean,
        issuerName: "",
        subjectName: "",
        applyGlobally: false,
        profileIds: []
      });
      existingIds.add(state.certificates.at(-1)?.id ?? "");
    }
    await persistSecurity(model, t, rerender);
  });

  for (const button of root.querySelectorAll("[data-cert-remove]")) {
    button.addEventListener("click", async () => {
      const id = button.getAttribute("data-cert-remove");
      const state = ensureGlobalSecurityState(model);
      state.certificates = (state.certificates ?? []).filter((item) => item.id !== id);
      await persistSecurity(model, t, rerender);
    });
  }

  for (const checkbox of root.querySelectorAll("[data-cert-global]")) {
    checkbox.addEventListener("change", async () => {
      const id = checkbox.getAttribute("data-cert-global");
      const state = ensureGlobalSecurityState(model);
      state.certificates = (state.certificates ?? []).map((item) => item.id === id ? { ...item, applyGlobally: checkbox.checked, profileIds: checkbox.checked ? [] : item.profileIds } : item);
      await persistSecurity(model, t, rerender);
    });
  }

  for (const button of root.querySelectorAll("[data-cert-menu-toggle]")) {
    button.addEventListener("click", () => {
      root.querySelector(`#cert-menu-${button.getAttribute("data-cert-menu-toggle")}`)?.classList.toggle("hidden");
    });
  }

  for (const checkbox of root.querySelectorAll("[data-cert-profile]")) {
    checkbox.addEventListener("change", async () => {
      const [certId, profileId] = checkbox.getAttribute("data-cert-profile").split(":");
      const state = ensureGlobalSecurityState(model);
      state.certificates = (state.certificates ?? []).map((item) => {
        if (item.id !== certId) return item;
        const next = new Set(item.profileIds ?? []);
        if (checkbox.checked) next.add(profileId);
        else next.delete(profileId);
        return { ...item, profileIds: [...next] };
      });
      await persistSecurity(model, t, rerender);
    });
  }
}
