import { handleProfileLaunchActionImpl, handleProfileStopActionImpl } from "./view-wire-launch-stop.js";

export function wireProfileListInteractionsImpl(root, model, rerender, t, deps) {
  const {
    escapeHtml,
    selectionState,
    ensureProfilesViewState,
    handleProfileLogsAction,
    deleteProfile,
    openProfileModal,
    setNotice,
    hydrateProfilesModel
  } = deps;

  if (!(model.profileActionPendingIds instanceof Set)) {
    model.profileActionPendingIds = new Set();
  }
  let floatingTagTooltip = document.body.querySelector("#profiles-floating-tag-tooltip");
  if (!floatingTagTooltip) {
    floatingTagTooltip = document.createElement("div");
    floatingTagTooltip.id = "profiles-floating-tag-tooltip";
    floatingTagTooltip.className = "profiles-floating-tooltip hidden";
    document.body.appendChild(floatingTagTooltip);
  }
  const hideFloatingTagTooltip = () => {
    floatingTagTooltip._activeTrigger = null;
    floatingTagTooltip.classList.add("hidden");
    floatingTagTooltip.innerHTML = "";
  };
  const positionFloatingTagTooltip = (trigger) => {
    const rect = trigger.getBoundingClientRect();
    const margin = 8;
    floatingTagTooltip.style.left = `${Math.max(12, Math.min(rect.left, window.innerWidth - floatingTagTooltip.offsetWidth - 12))}px`;
    let top = rect.bottom + margin;
    if (top + floatingTagTooltip.offsetHeight > window.innerHeight - 12) {
      top = Math.max(12, rect.top - floatingTagTooltip.offsetHeight - margin);
    }
    floatingTagTooltip.style.top = `${top}px`;
  };
  const showFloatingTagTooltip = (trigger) => {
    const raw = trigger.getAttribute("data-tag-tooltip-tags") || "";
    const tags = raw.split("\n").map((value) => value.trim()).filter(Boolean);
    if (!tags.length) {
      hideFloatingTagTooltip();
      return;
    }
    floatingTagTooltip.innerHTML = tags
      .map((tag) => `<span class="profiles-tag">${escapeHtml(tag)}</span>`)
      .join("");
    floatingTagTooltip.classList.remove("hidden");
    floatingTagTooltip._activeTrigger = trigger;
    positionFloatingTagTooltip(trigger);
  };
  if (!floatingTagTooltip.dataset.bound) {
    window.addEventListener("scroll", () => {
      if (floatingTagTooltip._activeTrigger) positionFloatingTagTooltip(floatingTagTooltip._activeTrigger);
    }, { passive: true });
    window.addEventListener("resize", () => {
      if (floatingTagTooltip._activeTrigger) positionFloatingTagTooltip(floatingTagTooltip._activeTrigger);
    });
    document.addEventListener("pointerdown", (event) => {
      if (
        floatingTagTooltip._activeTrigger
        && !event.target?.closest?.("[data-tag-tooltip-trigger]")
        && !event.target?.closest?.("#profiles-floating-tag-tooltip")
      ) {
        hideFloatingTagTooltip();
      }
    });
    floatingTagTooltip.dataset.bound = "true";
  }

  for (const header of root.querySelectorAll("[data-profile-sort]")) {
    header.addEventListener("click", () => {
      const sortState = ensureProfilesViewState(model);
      const key = header.getAttribute("data-profile-sort");
      if (sortState.sortKey === key) {
        sortState.sortDirection = sortState.sortDirection === "asc" ? "desc" : "asc";
      } else {
        sortState.sortKey = key;
        sortState.sortDirection = "asc";
      }
      rerender();
    });
  }

  root.querySelector("#profiles-select-all")?.addEventListener("change", (event) => {
    model.selectedProfileIds = event.target.checked ? model.profiles.map((profile) => profile.id) : [];
    rerender();
  });

  for (const checkbox of root.querySelectorAll(".profile-select")) {
    checkbox.addEventListener("change", (event) => {
      const profileId = checkbox.getAttribute("data-select-id");
      const selectedIds = new Set(selectionState(model));
      if (event.target.checked) selectedIds.add(profileId);
      else selectedIds.delete(profileId);
      model.selectedProfileIds = [...selectedIds];
      rerender();
    });
  }

  for (const trigger of root.querySelectorAll("[data-tag-tooltip-trigger]")) {
    trigger.addEventListener("mouseenter", () => showFloatingTagTooltip(trigger));
    trigger.addEventListener("focus", () => showFloatingTagTooltip(trigger));
    trigger.addEventListener("mouseleave", () => {
      if (floatingTagTooltip._activeTrigger === trigger) hideFloatingTagTooltip();
    });
    trigger.addEventListener("blur", () => {
      if (floatingTagTooltip._activeTrigger === trigger) hideFloatingTagTooltip();
    });
  }

  for (const row of root.querySelectorAll(".profiles-row")) {
    row.addEventListener("click", async (event) => {
      const action = event.target?.closest?.("[data-action]")?.getAttribute?.("data-action");
      if (!action) return;
      const profileId = row.getAttribute("data-profile-id");
      const profile = model.profiles.find((item) => item.id === profileId);
      if (!profile) return;
      if ((action === "launch" || action === "stop") && model.profileActionPendingIds.has(profileId)) return;

      if (action === "launch") {
        await handleProfileLaunchActionImpl(root, model, rerender, t, profile, deps);
      }

      if (action === "stop") {
        await handleProfileStopActionImpl(model, t, profileId, deps);
      }

      if (action === "logs") {
        return handleProfileLogsAction(profile);
      }

      if (action === "edit") {
        return openProfileModal(root, model, rerender, t, profile);
      }

      if (action === "delete") {
        const confirmed = await askConfirmPrompt(t, t("profile.delete.title"), t("profile.delete.confirm"));
        if (confirmed) {
          const result = await deleteProfile(profileId);
          setNotice(model, result.ok ? "success" : "error", result.ok ? t("profile.notice.deleted") : String(result.data.error));
        }
      }

      await hydrateProfilesModel(model);
      rerender();
    });
  }
}
