export function wireRouteNetworkSecurityBlocklists(ctx){
  const { overlay, t, blocklistState, globalActiveBlocklistIds, markDirty, renderBlocklistSummary } = ctx;
  const blocklistDropdown = overlay.querySelector(".profile-blocklist-dropdown");
  const blocklistMenu = overlay.querySelector("#profile-blocklists-menu");
  const blocklistSearch = overlay.querySelector("#profile-blocklists-search");
  const blocklistSelectAll = overlay.querySelector("#profile-blocklists-select-all");
  const updateBlocklistSelectAllLabel = () => {
    if (!blocklistSelectAll) return;
    const selectable = [...overlay.querySelectorAll("[data-profile-blocklist-id]")].filter((node) => !node.disabled);
    const allSelected = selectable.length > 0 && selectable.every((node) => node.checked);
    blocklistSelectAll.textContent = allSelected ? t("security.clear") : t("security.all");
  };
  const applyBlocklistSearch = () => {
    const query = String(blocklistSearch?.value ?? "").trim().toLowerCase();
    for (const option of overlay.querySelectorAll("[data-profile-blocklist-option]")) {
      const haystack = option.getAttribute("data-profile-blocklist-option") || "";
      option.classList.toggle("hidden", Boolean(query) && !haystack.includes(query));
    }
  };
  overlay.querySelector("#profile-blocklists-toggle")?.addEventListener("click", (event) => {
    event.preventDefault();
    event.stopPropagation();
    blocklistMenu?.classList.toggle("hidden");
    if (!blocklistMenu?.classList.contains("hidden")) setTimeout(() => blocklistSearch?.focus(), 0);
  });
  blocklistMenu?.addEventListener("click", (event) => event.stopPropagation());
  blocklistSearch?.addEventListener("input", () => applyBlocklistSearch());
  blocklistSelectAll?.addEventListener("click", () => {
    const selectable = [...overlay.querySelectorAll("[data-profile-blocklist-id]")].filter((node) => !node.disabled);
    const allSelected = selectable.length > 0 && selectable.every((node) => node.checked);
    for (const checkbox of selectable) {
      checkbox.checked = !allSelected;
      const id = checkbox.getAttribute("data-profile-blocklist-id");
      if (checkbox.checked) blocklistState.add(id); else blocklistState.delete(id);
    }
    for (const id of globalActiveBlocklistIds) blocklistState.add(id);
    markDirty();
    updateBlocklistSelectAllLabel();
    renderBlocklistSummary();
  });
  for (const checkbox of overlay.querySelectorAll("[data-profile-blocklist-id]")) {
    checkbox.addEventListener("change", () => {
      const id = checkbox.getAttribute("data-profile-blocklist-id");
      if (checkbox.checked) blocklistState.add(id); else blocklistState.delete(id);
      for (const globalId of globalActiveBlocklistIds) blocklistState.add(globalId);
      markDirty();
      updateBlocklistSelectAllLabel();
      renderBlocklistSummary();
    });
  }
  applyBlocklistSearch();
  updateBlocklistSelectAllLabel();
  for (const field of overlay.querySelectorAll("input,select,textarea")) {
    field.addEventListener("change", () => markDirty());
  }


  return { blocklistDropdown, blocklistMenu };
}
