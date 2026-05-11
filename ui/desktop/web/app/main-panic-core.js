export function createPanicUi(deps) {
  const { selectedProfile, shouldRenderPanicControls, fireIcon, fireHeroIcon, panicFrameStyleAttr, callCommand, panicWipeProfile, launchProfile, updateProfile, sleep } = deps;
function panicFrameMode() {
  return typeof window !== "undefined" ? String(window.__PANIC_FRAME_MODE ?? "controls").trim() : "controls";
}

  function renderPanicFloatingButton(i18n, model) {
  if (!shouldRenderPanicControls(model)) return "";
  return `
    <button id="panic-frame-trigger" class="panic-frame-trigger" title="${i18n.t("panicFrame.button")}">
      <span class="panic-frame-trigger-icon">${fireIcon()}</span>
    </button>
  `;
}

  function renderPanicModal(i18n, model) {
  const profile = selectedProfile(model);
  if (!profile || !model.panicUi?.open) return "";
  const sites = [...(model.panicUi.sites ?? profile.panic_protected_sites ?? [])];
  const query = String(model.panicUi.search ?? "").trim().toLowerCase();
  const filteredSites = sites.filter((item) => !query || item.toLowerCase().includes(query));
  if (model.panicUi.mode === "config") {
    return `
      <div class="profiles-modal-overlay" id="panic-config-overlay">
        <div class="profiles-modal-window panic-config-window">
          <div class="action-modal">
            <div class="row-between">
              <h3>${i18n.t("panicFrame.sitesTitle")}</h3>
              <input id="panic-sites-search" value="${String(model.panicUi.search ?? "").replaceAll("\"", "&quot;")}" placeholder="${i18n.t("panicFrame.searchPlaceholder")}" />
            </div>
            <div class="panic-sites-list" id="panic-sites-list">
              ${filteredSites.length ? filteredSites.map((site) => `
                <div class="panic-site-row">
                  <span>${site}</span>
                  <button type="button" data-panic-site-remove="${site}">${i18n.t("action.delete")}</button>
                </div>
              `).join("") : `<p class="meta">${i18n.t("panicFrame.sitesEmpty")}</p>`}
            </div>
            <div class="grid-two">
              <input id="panic-sites-add-input" placeholder="example.com" />
              <button type="button" id="panic-sites-add">${i18n.t("panicFrame.addSite")}</button>
            </div>
            <div class="modal-actions">
              <button type="button" id="panic-sites-clear">${i18n.t("panicFrame.clearSites")}</button>
              <button type="button" id="panic-sites-cancel">${i18n.t("action.cancel")}</button>
              <button type="button" id="panic-sites-save">${i18n.t("action.save")}</button>
            </div>
          </div>
        </div>
      </div>
    `;
  }
  return `
    <div class="profiles-modal-overlay" id="panic-modal-overlay">
      <div class="profiles-modal-window panic-modal-window">
        <div class="action-modal">
          <h3>${i18n.t("panicFrame.menuTitle")}</h3>
          <p class="meta">${i18n.t("panicFrame.subtitle")}</p>
          <div class="panic-fire-hero">${fireHeroIcon()}</div>
          <h4>${i18n.t("panicFrame.cleanupTitle")}</h4>
          <p>${i18n.t("panicFrame.cleanupBody")}</p>
          <div class="panic-protected-summary">
            <div>
              <strong>${i18n.t("panicFrame.protectedSummary")}</strong>
              <p class="meta">${sites.length ? i18n.t("panicFrame.protectedSitesCount").replace("{count}", String(sites.length)) : i18n.t("panicFrame.protectedSitesNone")}</p>
            </div>
            <button type="button" id="panic-config-open">${i18n.t("panicFrame.configure")}</button>
          </div>
          ${model.panicUi.notice ? `<p class="notice ${model.panicUi.notice.type}">${model.panicUi.notice.text}</p>` : ""}
          <div class="modal-actions">
            <button type="button" id="panic-modal-cancel">${i18n.t("action.cancel")}</button>
            <button type="button" class="danger" id="panic-modal-clean">${i18n.t("panicFrame.clean")}</button>
          </div>
        </div>
      </div>
    </div>
  `;
}

  function panicMenuState(model, profile) {
  return model.panicUi ?? {
    open: true,
    mode: "panel",
    sites: [...(profile?.panic_protected_sites ?? [])],
    search: "",
    notice: null
  };
}

  function renderPanicMenu(i18n, model) {
  const profile = selectedProfile(model);
  if (!profile) return "";
  const panicUi = panicMenuState(model, profile);
  const sites = [...(panicUi.sites ?? profile.panic_protected_sites ?? [])];
  const query = String(panicUi.search ?? "").trim().toLowerCase();
  const filteredSites = sites.filter((item) => !query || item.toLowerCase().includes(query));
  if (panicUi.mode === "config") {
    return `
      <div class="panic-context-menu-shell">
        <div class="panic-context-menu panic-context-menu--config">
          <div class="panic-context-menu-header">
            <h3>${i18n.t("panicFrame.sitesTitle")}</h3>
            <button type="button" class="panic-context-close" id="panic-menu-close" aria-label="${i18n.t("action.cancel")}">×</button>
          </div>
          <div class="panic-context-search-row">
            <input id="panic-sites-search" value="${String(panicUi.search ?? "").replaceAll("\"", "&quot;")}" placeholder="${i18n.t("panicFrame.searchPlaceholder")}" />
          </div>
          <div class="panic-sites-list" id="panic-sites-list">
            ${filteredSites.length ? filteredSites.map((site) => `
              <div class="panic-site-row">
                <span>${site}</span>
                <button type="button" data-panic-site-remove="${site}">${i18n.t("action.delete")}</button>
              </div>
            `).join("") : `<p class="meta">${i18n.t("panicFrame.sitesEmpty")}</p>`}
          </div>
          <div class="grid-two">
            <input id="panic-sites-add-input" placeholder="example.com" />
            <button type="button" id="panic-sites-add">${i18n.t("panicFrame.addSite")}</button>
          </div>
          ${panicUi.notice ? `<p class="notice ${panicUi.notice.type}">${panicUi.notice.text}</p>` : ""}
          <div class="modal-actions panic-context-actions">
            <button type="button" id="panic-sites-clear">${i18n.t("panicFrame.clearSites")}</button>
            <button type="button" id="panic-sites-cancel">${i18n.t("action.cancel")}</button>
            <button type="button" id="panic-sites-save">${i18n.t("action.save")}</button>
          </div>
        </div>
      </div>
    `;
  }
  return `
      <div class="panic-context-menu-shell">
      <div class="panic-context-menu">
        <div class="panic-context-menu-header">
          <h3>${i18n.t("panicFrame.menuTitle")}</h3>
          <button type="button" class="panic-context-close" id="panic-menu-close" aria-label="${i18n.t("action.cancel")}">×</button>
        </div>
        <div class="panic-fire-hero">${fireHeroIcon()}</div>
        <h4>${i18n.t("panicFrame.cleanupTitle")}</h4>
        <p>${i18n.t("panicFrame.cleanupBody")}</p>
        <div class="panic-protected-summary">
          <div>
            <strong>${i18n.t("panicFrame.protectedSummary")}</strong>
            <p class="meta">${sites.length ? i18n.t("panicFrame.protectedSitesCount").replace("{count}", String(sites.length)) : i18n.t("panicFrame.protectedSitesNone")}</p>
          </div>
          <button type="button" id="panic-config-open">${i18n.t("panicFrame.configure")}</button>
        </div>
        ${panicUi.notice ? `<p class="notice ${panicUi.notice.type}">${panicUi.notice.text}</p>` : ""}
        <div class="modal-actions panic-context-actions">
          <button type="button" id="panic-modal-cancel">${i18n.t("action.cancel")}</button>
          <button type="button" class="danger" id="panic-modal-clean">${i18n.t("panicFrame.clean")}</button>
        </div>
      </div>
    </div>
  `;
}

  function renderPanicOverlay(i18n, model) {
  const profile = selectedProfile(model);
  const mode = panicFrameMode();
  if (mode === "border") {
    return `
      <div class="panic-overlay-shell ${profile ? "" : "panic-overlay-shell--empty"}" ${panicFrameStyleAttr(profile)}>
        <div class="panic-overlay-border"></div>
      </div>
    `;
  }
  if (mode === "label") {
    return `
      <div class="panic-overlay-shell ${profile ? "" : "panic-overlay-shell--empty"} panic-overlay-shell--label" ${panicFrameStyleAttr(profile)}>
        <div class="panic-overlay-label-shell">
          <div class="panic-overlay-titlebar" aria-hidden="true">
            <strong>${profile?.name ?? "Cerbena"}</strong>
          </div>
        </div>
      </div>
    `;
  }
  if (mode === "menu") {
    return `
      <div class="panic-overlay-shell ${profile ? "" : "panic-overlay-shell--empty"} panic-overlay-shell--menu" ${panicFrameStyleAttr(profile)}>
        ${renderPanicMenu(i18n, model)}
      </div>
    `;
  }
  return `
    <div class="panic-overlay-shell ${profile ? "" : "panic-overlay-shell--empty"} panic-overlay-shell--controls" ${panicFrameStyleAttr(profile)}>
      <div class="panic-overlay-actions">
        <button id="panic-frame-trigger" class="panic-frame-trigger panic-frame-trigger--overlay" title="${i18n.t("panicFrame.button")}">
          <span class="panic-frame-trigger-icon">${fireIcon()}</span>
        </button>
      </div>
    </div>
  `;
}


function wirePanicInteractions(root, model, rerender, i18n, state) {
  const mode = panicFrameMode();
  if (mode === "border" || mode === "label") {
    return;
  }
  if (mode === "controls") {
    root.querySelector("#panic-frame-trigger")?.addEventListener("click", async () => {
      const profile = selectedProfile(model);
      if (!profile) return;
      await callCommand("panic_frame_show_menu", {
        request: { profileId: profile.id }
      });
    });
    return;
  }
  const closePanicMenu = async () => {
    const profile = selectedProfile(model);
    if (!profile) return;
    model.panicUi = null;
    await callCommand("panic_frame_hide_menu", {
      request: { profileId: profile.id }
    });
  };
  const applyPanicLaunchNotice = (type, text) => {
    if (state.currentFeature === "home") {
      model.homeNotice = { type, text };
      return;
    }
    model.profileNotice = { type, text };
  };
  if (mode === "menu" && !model.panicUi) {
    const profile = selectedProfile(model);
    model.panicUi = panicMenuState(model, profile);
  }
  if (mode === "menu") {
    root.querySelector("#panic-menu-close")?.addEventListener("click", closePanicMenu);
  }
  root.querySelector("#panic-frame-trigger")?.addEventListener("click", async () => {
    const profile = selectedProfile(model);
    if (!profile) return;
    model.panicUi = {
      open: true,
      mode: "panel",
      sites: [...(profile.panic_protected_sites ?? [])],
      search: "",
      notice: null
    };
    await rerender();
  });

  root.querySelector("#panic-modal-cancel")?.addEventListener("click", async () => {
    if (mode === "menu") {
      await closePanicMenu();
      return;
    }
    model.panicUi = null;
    await rerender();
  });

  root.querySelector("#panic-config-open")?.addEventListener("click", async () => {
    model.panicUi = {
      ...(model.panicUi ?? { sites: [] }),
      open: true,
      mode: "config",
      search: ""
    };
    await rerender();
  });

  root.querySelector("#panic-modal-clean")?.addEventListener("click", async () => {
    const profile = selectedProfile(model);
    if (!profile) return;
    const result = await panicWipeProfile({
      profileId: profile.id,
      mode: "full",
      retainPaths: [],
      confirmPhrase: "ERASE_NOW"
    });
    model.panicUi = null;
    applyPanicLaunchNotice(result.ok ? "success" : "error", result.ok ? i18n.t("home.panicDone") : String(result.data.error));
    if (result.ok) {
      await sleep(1200);
      const relaunchResult = await launchProfile(profile.id);
      if (!relaunchResult.ok) {
        applyPanicLaunchNotice("error", String(relaunchResult.data.error));
      }
    }
    if (mode === "menu") {
      await closePanicMenu();
      await rerender();
      return;
    }
    await rerender();
  });

  root.querySelector("#panic-sites-cancel")?.addEventListener("click", async () => {
    model.panicUi = {
      ...(model.panicUi ?? {}),
      open: true,
      mode: "panel",
      search: "",
      notice: null
    };
    await rerender();
  });

  root.querySelector("#panic-sites-search")?.addEventListener("input", async (event) => {
    if (!model.panicUi) return;
    model.panicUi.search = event.target.value;
    await rerender();
  });

  root.querySelector("#panic-sites-add")?.addEventListener("click", async () => {
    const input = root.querySelector("#panic-sites-add-input");
    const value = String(input?.value ?? "").trim().toLowerCase();
    if (!value || !/^[a-z0-9.-]+$/i.test(value)) {
      return;
    }
    if (!model.panicUi) return;
    const next = new Set(model.panicUi.sites ?? []);
    next.add(value);
    model.panicUi.sites = [...next].sort();
    model.panicUi.search = "";
    await rerender();
  });

  root.querySelector("#panic-sites-clear")?.addEventListener("click", async () => {
    if (!model.panicUi) return;
    model.panicUi.sites = [];
    model.panicUi.search = "";
    await rerender();
  });

  for (const button of root.querySelectorAll("[data-panic-site-remove]")) {
    button.addEventListener("click", async () => {
      if (!model.panicUi) return;
      const target = button.getAttribute("data-panic-site-remove");
      model.panicUi.sites = (model.panicUi.sites ?? []).filter((item) => item !== target);
      await rerender();
    });
  }

  root.querySelector("#panic-sites-save")?.addEventListener("click", async () => {
    const profile = selectedProfile(model);
    if (!profile || !model.panicUi) return;
    const result = await updateProfile({
      profileId: profile.id,
      panicProtectedSites: model.panicUi.sites ?? [],
      expectedUpdatedAt: profile.updated_at
    });
    if (!result.ok) {
      model.panicUi.notice = { type: "error", text: String(result.data.error) };
      await rerender();
      return;
    }
    model.panicUi = {
      open: true,
      mode: "panel",
      sites: [...(model.panicUi.sites ?? [])],
      search: "",
      notice: { type: "success", text: i18n.t("panicFrame.saved") }
    };
    await rerender();
  });
}


  return {
    panicFrameMode,
    renderPanicFloatingButton,
    renderPanicModal,
    renderPanicOverlay,
    wirePanicInteractions
  };
}
