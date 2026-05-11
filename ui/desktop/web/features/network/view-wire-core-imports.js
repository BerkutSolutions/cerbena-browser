export function registerNetworkImportAndTemplateHandlers(ctx){
  const {
    root, model, rerender, t,
    syncTemplateDraft, defaultTemplateDraft, askInputModal, networkImportUtils,
    formatNetworkError, templateRequest, testConnectionTemplateRequest,
    refreshTemplatePings, deleteConnectionTemplate, pingConnectionTemplate,
    normalizeTemplateNodes, closeFloatingTemplateMenusGuard, eyeIcon, eyeOffIcon
  } = ctx;
  for (const button of root.querySelectorAll("[data-action='import-link']")) {
    button.addEventListener("click", async () => {
      syncTemplateDraft(root, model);
      const nodeId = button.getAttribute("data-node-id");
      const link = await askInputModal(t, {
        title: t("network.importLink"),
        label: t("network.importLinkPrompt"),
        defaultValue: ""
      });
      if (!link) return;
      const draft = model.networkTemplateDraft ?? defaultTemplateDraft();
      const node = draft.nodes.find((item) => item.nodeId === nodeId);
      if (!node) return;
      try {
        const parsed = networkImportUtils.parseV2RayLink(link, node);
        networkImportUtils.applyImportedNode(model, nodeId, parsed.node, parsed.name || "");
        model.networkNotice = { type: "success", text: t("network.importApplied") };
      } catch (error) {
        model.networkNotice = { type: "error", text: formatNetworkError(error, t) };
      }
      await rerender();
    });
  }

  for (const button of root.querySelectorAll("[data-action='import-amnezia-key']")) {
    button.addEventListener("click", async () => {
      syncTemplateDraft(root, model);
      const nodeId = button.getAttribute("data-node-id");
      const key = await askInputModal(t, {
        title: t("network.importVpnKey"),
        label: t("network.importAmneziaPrompt"),
        defaultValue: "",
        multiline: true
      });
      if (!key) return;
      const draft = model.networkTemplateDraft ?? defaultTemplateDraft();
      const node = draft.nodes.find((item) => item.nodeId === nodeId);
      if (!node) return;
      try {
        const nextNode = networkImportUtils.parseAmneziaInput(key, node);
        networkImportUtils.applyImportedNode(model, nodeId, nextNode, "");
        model.networkNotice = { type: "success", text: t("network.importApplied") };
      } catch (error) {
        model.networkNotice = { type: "error", text: formatNetworkError(error, t) };
      }
      await rerender();
    });
  }

  for (const button of root.querySelectorAll("[data-action='toggle-password-visibility']")) {
    button.addEventListener("click", () => {
      const wrapper = button.closest(".input-icon-field");
      const input = wrapper?.querySelector("input[data-node-field='password']");
      if (!input) return;
      const visible = input.type === "text";
      input.type = visible ? "password" : "text";
      const nextVisible = !visible;
      button.setAttribute("data-password-visible", nextVisible ? "true" : "false");
      const label = t(nextVisible ? "profile.security.hidePassword" : "profile.security.showPassword");
      button.setAttribute("aria-label", label);
      button.setAttribute("title", label);
      button.classList.toggle("active", nextVisible);
      button.innerHTML = nextVisible ? eyeIcon() : eyeOffIcon();
    });
  }

  let floatingTemplateMenu = null;
  const closeAllTemplateMenus = () => {
    floatingTemplateMenu?.remove();
    floatingTemplateMenu = null;
    for (const row of root.querySelectorAll("[data-template-id].menu-open")) {
      row.classList.remove("menu-open");
    }
  };
  const positionTemplateMenu = (menu, toggle) => {
    if (!menu || !toggle) return;
    const rect = toggle.getBoundingClientRect();
    const width = Math.min(360, Math.max(280, rect.width));
    const viewportWidth = window.innerWidth || document.documentElement.clientWidth || 0;
    const left = Math.max(12, Math.min(rect.right - width, viewportWidth - width - 12));
    menu.style.left = `${left}px`;
    menu.style.top = `${Math.round(rect.bottom + 8)}px`;
    menu.style.width = `${width}px`;
  };
  root.addEventListener("click", (event) => {
    const target = event.target;
    if (target?.closest?.(".network-floating-menu")) return;
    if (target?.closest?.("[data-template-menu-toggle]")) return;
    closeAllTemplateMenus();
  });

  const handleTemplateAction = async (template, action) => {
    closeAllTemplateMenus();
    if (!template || !action) return;
    if (action === "ping") {
      const ping = await pingConnectionTemplate(template.id);
      model.networkNotice = {
        type: ping.ok && ping.data.reachable ? "success" : "error",
        text: ping.ok ? ping.data.message : formatNetworkError(ping.data.error, t)
      };
      if (ping.ok) {
        model.networkPingState = {
          ...(model.networkPingState ?? {}),
          [template.id]: ping.data
        };
      }
      await rerender();
      return;
    }
    if (action === "edit") {
      model.networkTemplateDraft = {
        templateId: template.id,
        name: template.name,
        nodes: normalizeTemplateNodes(template)
      };
      model.networkNodeTestState = {};
      await rerender();
      return;
    }
    if (action === "set-default") {
      const current = model.networkGlobalRoute ?? {};
      if (!current.globalVpnEnabled) {
        model.networkNotice = { type: "error", text: t("network.defaultTemplateRequiresGlobal") };
        await rerender();
        return;
      }
      await saveGlobalSettings({
        globalVpnEnabled: true,
        blockWithoutVpn: Boolean(current.blockWithoutVpn),
        defaultTemplateId: template.id
      });
      return;
    }
    if (action === "delete") {
      const result = await deleteConnectionTemplate(template.id);
      model.networkNotice = {
        type: result.ok ? "success" : "error",
        text: result.ok ? t("network.templateDeleted") : formatNetworkError(result.data.error, t)
      };
      if (result.ok) {
        model.networkLoaded = false;
        await hydrateNetworkModel(model);
      }
      await rerender();
    }
  };

  const buildTemplateMenu = (template) => {
    const globalRoute = model.networkGlobalRoute ?? {};
    const globalVpnEnabled = Boolean(globalRoute.globalVpnEnabled);
    const blockWithoutVpn = Boolean(globalRoute.blockWithoutVpn);
    const isDefault = (globalRoute.defaultTemplateId ?? "") === template.id;
    const menu = document.createElement("div");
    menu.className = "dns-dropdown-menu network-actions-menu network-floating-menu";
    menu.innerHTML = `
      <button type="button" class="dns-dropdown-option" data-action="ping">${t("network.testConnection")}</button>
      <button type="button" class="dns-dropdown-option" data-action="edit">${t("extensions.edit")}</button>
      <button type="button" class="dns-dropdown-option" data-action="delete">${t("extensions.remove")}</button>
      ${globalVpnEnabled ? `<button type="button" class="dns-dropdown-option" data-action="set-default">
        ${isDefault ? t("network.defaultTemplateActive") : t("network.defaultTemplateSet")}
      </button>` : ""}
    `;
    for (const actionButton of menu.querySelectorAll("[data-action]")) {
      actionButton.addEventListener("click", async (event) => {
        event.preventDefault();
        event.stopPropagation();
        await handleTemplateAction(template, actionButton.getAttribute("data-action"));
      });
    }
    menu.addEventListener("click", (event) => event.stopPropagation());
    return menu;
  };

  for (const row of root.querySelectorAll("[data-template-id]")) {
    const templateId = row.getAttribute("data-template-id");
    const menuToggle = row.querySelector("[data-template-menu-toggle]");
    menuToggle?.addEventListener("click", (event) => {
      event.preventDefault();
      event.stopPropagation();
      const template = (model.networkTemplates ?? []).find((item) => item.id === templateId);
      const shouldOpen = !floatingTemplateMenu || row.classList.contains("menu-open") === false;
      closeAllTemplateMenus();
      if (shouldOpen && template) {
        row.classList.add("menu-open");
        floatingTemplateMenu = buildTemplateMenu(template);
        document.body.appendChild(floatingTemplateMenu);
        positionTemplateMenu(floatingTemplateMenu, menuToggle);
      }
    });
  }
}
