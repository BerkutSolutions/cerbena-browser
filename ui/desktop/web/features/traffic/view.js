import { listTrafficEvents, setTrafficRule } from "./api.js";

const TRAFFIC_POLL_INTERVAL_MS = 5000;

function escapeHtml(value) {
  return String(value ?? "")
    .replaceAll("&", "&amp;")
    .replaceAll("<", "&lt;")
    .replaceAll(">", "&gt;")
    .replaceAll("\"", "&quot;");
}

function formatTime(epoch) {
  const date = new Date(Number(epoch));
  return `${date.toLocaleTimeString()} ${date.toLocaleDateString()}`;
}

function profileName(entry, profiles) {
  return profiles.find((profile) => profile.id === entry.profileId)?.name ?? entry.profileName ?? entry.profileId;
}

function statusLabel(entry, t) {
  if (entry.status === "blocked") return t("traffic.blocked");
  if (entry.status === "error") return t("traffic.error");
  return t("traffic.allowed");
}

function ensureFilters(model) {
  if (!model.trafficFilters) {
    model.trafficFilters = {
      fromTime: "",
      toTime: "",
      requestQuery: "",
      responseQuery: "",
      profileId: "all",
      status: "all"
    };
  }
  return model.trafficFilters;
}

function toEpochMs(value) {
  if (!value) return null;
  const parsed = Date.parse(value);
  if (!Number.isFinite(parsed)) return null;
  return parsed;
}

function applyFilters(entries, filters, profiles) {
  const fromEpoch = toEpochMs(filters.fromTime);
  const toEpoch = toEpochMs(filters.toTime);
  const requestNeedle = String(filters.requestQuery ?? "").trim().toLowerCase();
  const responseNeedle = String(filters.responseQuery ?? "").trim().toLowerCase();
  const statusFilter = String(filters.status ?? "all");
  const profileFilter = String(filters.profileId ?? "all");

  return entries.filter((entry) => {
    const timestamp = Number(entry.timestampEpochMs ?? 0);
    if (fromEpoch != null && timestamp < fromEpoch) return false;
    if (toEpoch != null && timestamp > toEpoch) return false;
    if (requestNeedle && !String(entry.requestHost ?? "").toLowerCase().includes(requestNeedle)) {
      return false;
    }
    const responseText = `${entry.status ?? ""} ${entry.reason ?? ""}`.toLowerCase();
    if (responseNeedle && !responseText.includes(responseNeedle)) {
      return false;
    }
    if (statusFilter !== "all" && String(entry.status ?? "") !== statusFilter) {
      return false;
    }
    if (profileFilter !== "all" && String(entry.profileId ?? "") !== profileFilter) {
      return false;
    }
    return true;
  }).map((entry) => ({
    ...entry,
    _profileName: profileName(entry, profiles)
  }));
}

function row(entry, t) {
  const statusClass = entry.status === "blocked" ? "traffic-row-blocked" : entry.status === "error" ? "traffic-row-error" : "";
  const blockedForProfile = entry.blockedForProfile;
  const blockedGlobally = entry.blockedGlobally;
  return `
    <tr class="${statusClass}" data-domain="${escapeHtml(entry.requestHost)}" data-profile-id="${entry.profileId}">
      <td>${escapeHtml(formatTime(entry.timestampEpochMs))}</td>
      <td>
        <div class="profiles-name">${escapeHtml(entry.requestHost)}</div>
        <div class="meta">${escapeHtml(entry.requestKind)}</div>
      </td>
      <td>
        <div>${statusLabel(entry, t)}</div>
        <div class="meta">${escapeHtml(entry.reason)}</div>
      </td>
      <td>
        <div>${escapeHtml(entry._profileName ?? entry.profileName ?? entry.profileId)}</div>
        <div class="meta">${escapeHtml(entry.route)} | ${escapeHtml(String(entry.latencyMs))} ms</div>
      </td>
      <td class="actions traffic-actions-cell">
        <div class="dns-dropdown traffic-actions-dropdown">
          <button type="button" class="dns-dropdown-toggle traffic-actions-toggle" data-traffic-menu-toggle="${entry.id}">...</button>
          <div class="dns-dropdown-menu hidden traffic-actions-menu" data-traffic-menu="${entry.id}">
            <button type="button" class="dns-dropdown-option" data-traffic-scope="profile" data-traffic-state="${blockedForProfile ? "unblock" : "block"}" data-traffic-domain="${escapeHtml(entry.requestHost)}" data-traffic-profile="${entry.profileId}">
              ${blockedForProfile ? t("traffic.unblockProfile") : t("traffic.blockProfile")}
            </button>
            <button type="button" class="dns-dropdown-option" data-traffic-scope="global" data-traffic-state="${blockedGlobally ? "unblock" : "block"}" data-traffic-domain="${escapeHtml(entry.requestHost)}" data-traffic-profile="${entry.profileId}">
              ${blockedGlobally ? t("traffic.unblockGlobal") : t("traffic.blockGlobal")}
            </button>
          </div>
        </div>
      </td>
    </tr>
  `;
}

function filtersFrame(model, t, profiles) {
  const filters = ensureFilters(model);
  const profileOptions = [`<option value="all">${t("traffic.filter.allProfiles")}</option>`];
  for (const profile of profiles) {
    profileOptions.push(`<option value="${profile.id}" ${filters.profileId === profile.id ? "selected" : ""}>${escapeHtml(profile.name)}</option>`);
  }
  return `
    <div class="panel traffic-filters-frame">
      <div class="traffic-filters-grid">
        <label>${t("traffic.filter.fromTime")}
          <input type="datetime-local" id="traffic-filter-from" value="${escapeHtml(filters.fromTime)}" />
        </label>
        <label>${t("traffic.filter.toTime")}
          <input type="datetime-local" id="traffic-filter-to" value="${escapeHtml(filters.toTime)}" />
        </label>
        <label>${t("traffic.filter.request")}
          <input id="traffic-filter-request" value="${escapeHtml(filters.requestQuery)}" />
        </label>
        <label>${t("traffic.filter.response")}
          <input id="traffic-filter-response" value="${escapeHtml(filters.responseQuery)}" />
        </label>
        <label>${t("traffic.filter.profile")}
          <select id="traffic-filter-profile">${profileOptions.join("")}</select>
        </label>
        <label>${t("traffic.filter.status")}
          <select id="traffic-filter-status">
            <option value="all" ${filters.status === "all" ? "selected" : ""}>${t("traffic.filter.allStatuses")}</option>
            <option value="processed" ${filters.status === "processed" ? "selected" : ""}>${t("traffic.allowed")}</option>
            <option value="blocked" ${filters.status === "blocked" ? "selected" : ""}>${t("traffic.blocked")}</option>
            <option value="error" ${filters.status === "error" ? "selected" : ""}>${t("traffic.error")}</option>
          </select>
        </label>
      </div>
    </div>
  `;
}

export function renderTraffic(t, model) {
  const state = model.trafficState ?? { entries: [] };
  const notice = model.trafficNotice ? `<p class="notice ${model.trafficNotice.type}">${model.trafficNotice.text}</p>` : "";
  const entries = state.entries ?? [];
  const profiles = model.profiles ?? [];
  const filteredEntries = applyFilters(entries, ensureFilters(model), profiles);
  return `
    <div class="feature-page">
      <div class="feature-page-head row-between">
        <div>
          <h2>${t("nav.traffic")}</h2>
        </div>
        <div class="top-actions">
          <button id="traffic-refresh">${t("traffic.refresh")}</button>
        </div>
      </div>
      ${notice}
      ${filtersFrame(model, t, profiles)}
      <div class="panel">
        <table class="extensions-table traffic-table">
          <thead>
            <tr>
              <th>${t("traffic.time")}</th>
              <th>${t("traffic.request")}</th>
              <th>${t("traffic.result")}</th>
              <th>${t("traffic.profile")}</th>
              <th></th>
            </tr>
          </thead>
          <tbody>
            ${filteredEntries.length ? filteredEntries.map((entry) => row(entry, t)).join("") : `<tr><td colspan="5" class="meta">${entries.length ? t("traffic.filteredEmpty") : t("traffic.empty")}</td></tr>`}
          </tbody>
        </table>
      </div>
    </div>
  `;
}

export async function hydrateTrafficModel(model) {
  const result = await listTrafficEvents();
  model.trafficState = { entries: result.ok ? result.data : [] };
  ensureFilters(model);
}

export function wireTraffic(root, model, rerender, t) {
  const refreshTraffic = async () => {
    await hydrateTrafficModel(model);
    await rerender();
  };
  if (!model.trafficPoller) {
    model.trafficPoller = setInterval(async () => {
      await hydrateTrafficModel(model);
      await rerender();
    }, TRAFFIC_POLL_INTERVAL_MS);
  }
  root.querySelector("#traffic-refresh")?.addEventListener("click", refreshTraffic);

  const updateFilters = async () => {
    const filters = ensureFilters(model);
    filters.fromTime = root.querySelector("#traffic-filter-from")?.value ?? "";
    filters.toTime = root.querySelector("#traffic-filter-to")?.value ?? "";
    filters.requestQuery = root.querySelector("#traffic-filter-request")?.value ?? "";
    filters.responseQuery = root.querySelector("#traffic-filter-response")?.value ?? "";
    filters.profileId = root.querySelector("#traffic-filter-profile")?.value ?? "all";
    filters.status = root.querySelector("#traffic-filter-status")?.value ?? "all";
    await rerender();
  };

  for (const selector of [
    "#traffic-filter-from",
    "#traffic-filter-to",
    "#traffic-filter-request",
    "#traffic-filter-response",
    "#traffic-filter-profile",
    "#traffic-filter-status"
  ]) {
    root.querySelector(selector)?.addEventListener("input", updateFilters);
    root.querySelector(selector)?.addEventListener("change", updateFilters);
  }

  for (const button of root.querySelectorAll("[data-traffic-menu-toggle]")) {
    button.addEventListener("click", () => {
      const id = button.getAttribute("data-traffic-menu-toggle");
      root.querySelector(`[data-traffic-menu='${id}']`)?.classList.toggle("hidden");
    });
  }
  for (const button of root.querySelectorAll("[data-traffic-scope]")) {
    button.addEventListener("click", async () => {
      const scope = button.getAttribute("data-traffic-scope");
      const blocked = button.getAttribute("data-traffic-state") === "block";
      const domain = button.getAttribute("data-traffic-domain");
      const profileId = button.getAttribute("data-traffic-profile");
      const result = await setTrafficRule(scope === "profile" ? profileId : null, domain, blocked);
      model.trafficNotice = { type: result.ok ? "success" : "error", text: result.ok ? t("action.save") : String(result.data.error) };
      await refreshTraffic();
    });
  }
}
