import { listTrafficEventsPage, setTrafficRule } from "./api.js";

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

function reasonLabel(entry, t) {
  if (entry.reason === "allowed") return "";
  if (entry.reason === "dns_unresolved") return t("traffic.reason.dnsUnresolved");
  if (entry.reason === "upstream_timeout") return t("traffic.reason.upstreamTimeout");
  return entry.reason;
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

function ensurePagination(model) {
  if (!model.trafficPagination) {
    model.trafficPagination = {
      page: 1,
      pageSize: 25,
      totalItems: 0,
      totalPages: 1
    };
  }
  return model.trafficPagination;
}

function toEpochMs(value) {
  if (!value) return null;
  const parsed = Date.parse(value);
  if (!Number.isFinite(parsed)) return null;
  return parsed;
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
      </td>
      <td>
        <div>${statusLabel(entry, t)}</div>
        ${reasonLabel(entry, t) ? `<div class="meta">${escapeHtml(reasonLabel(entry, t))}</div>` : ""}
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

function paginationFrame(model, t) {
  const pagination = ensurePagination(model);
  const pageSize = Number(pagination.pageSize) || 25;
  const options = [25, 50, 100]
    .map((size) => `<option value="${size}" ${size === pageSize ? "selected" : ""}>${size}</option>`)
    .join("");
  const currentPage = Math.max(1, Number(pagination.page) || 1);
  const totalPages = Math.max(1, Number(pagination.totalPages) || 1);
  const visiblePages = [];
  const windowEnd = Math.min(10, totalPages);
  for (let page = 1; page <= windowEnd; page += 1) {
    visiblePages.push(page);
  }
  const pageButtons = visiblePages
    .map((page) => `<button type="button" class="${page === currentPage ? "is-active" : ""}" data-traffic-page="${page}">${page}</button>`)
    .join("");
  const hasTail = totalPages > 10;
  const tailMarkup = hasTail
    ? `<span class="traffic-pagination-ellipsis">...</span><button type="button" class="${totalPages === currentPage ? "is-active" : ""}" data-traffic-page="${totalPages}">${totalPages}</button>`
    : "";
  return `
    <div class="traffic-pagination">
      <div class="traffic-pagination-left">
        <label>${t("traffic.pageSize")}
          <select id="traffic-page-size">${options}</select>
        </label>
      </div>
      <div class="traffic-pagination-right">
        ${pageButtons}${tailMarkup}
      </div>
    </div>
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
  const displayEntries = entries.map((entry) => ({
    ...entry,
    _profileName: profileName(entry, profiles)
  }));
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
          <colgroup>
            <col class="traffic-col-time" />
            <col class="traffic-col-request" />
            <col class="traffic-col-result" />
            <col class="traffic-col-profile" />
            <col class="traffic-col-actions" />
          </colgroup>
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
            ${displayEntries.length ? displayEntries.map((entry) => row(entry, t)).join("") : `<tr><td colspan="5" class="meta">${t("traffic.empty")}</td></tr>`}
          </tbody>
        </table>
      </div>
      ${paginationFrame(model, t)}
    </div>
  `;
}

export async function hydrateTrafficModel(model) {
  const filters = ensureFilters(model);
  const pagination = ensurePagination(model);
  const result = await listTrafficEventsPage({
    page: pagination.page,
    pageSize: pagination.pageSize,
    fromEpochMs: toEpochMs(filters.fromTime),
    toEpochMs: toEpochMs(filters.toTime),
    requestQuery: filters.requestQuery,
    responseQuery: filters.responseQuery,
    profileId: filters.profileId,
    status: filters.status
  });
  if (!result.ok) {
    model.trafficState = { entries: [] };
    return;
  }
  model.trafficState = { entries: result.data.entries ?? [] };
  pagination.page = Number(result.data.page ?? 1) || 1;
  pagination.pageSize = Number(result.data.pageSize ?? pagination.pageSize) || pagination.pageSize;
  pagination.totalItems = Number(result.data.totalItems ?? 0) || 0;
  pagination.totalPages = Number(result.data.totalPages ?? 1) || 1;
}

export function wireTraffic(root, model, rerender, t) {
  const refreshTraffic = async () => {
    await hydrateTrafficModel(model);
    await rerender({ refreshProfiles: false, refreshFeature: false });
  };
  if (!model.trafficPoller) {
    model.trafficPoller = setInterval(async () => {
      await hydrateTrafficModel(model);
      await rerender({ refreshProfiles: false, refreshFeature: false });
    }, TRAFFIC_POLL_INTERVAL_MS);
  }
  root.querySelector("#traffic-refresh")?.addEventListener("click", refreshTraffic);

  const updateFilters = async () => {
    const filters = ensureFilters(model);
    const pagination = ensurePagination(model);
    filters.fromTime = root.querySelector("#traffic-filter-from")?.value ?? "";
    filters.toTime = root.querySelector("#traffic-filter-to")?.value ?? "";
    filters.requestQuery = root.querySelector("#traffic-filter-request")?.value ?? "";
    filters.responseQuery = root.querySelector("#traffic-filter-response")?.value ?? "";
    filters.profileId = root.querySelector("#traffic-filter-profile")?.value ?? "all";
    filters.status = root.querySelector("#traffic-filter-status")?.value ?? "all";
    pagination.page = 1;
    await refreshTraffic();
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

  root.querySelector("#traffic-page-size")?.addEventListener("change", async () => {
    const pagination = ensurePagination(model);
    const pageSize = Number(root.querySelector("#traffic-page-size")?.value ?? 25);
    pagination.pageSize = [25, 50, 100].includes(pageSize) ? pageSize : 25;
    pagination.page = 1;
    await refreshTraffic();
  });

  for (const button of root.querySelectorAll("[data-traffic-page]")) {
    button.addEventListener("click", async () => {
      const page = Number(button.getAttribute("data-traffic-page") ?? 1);
      if (!Number.isFinite(page) || page < 1) return;
      const pagination = ensurePagination(model);
      if (pagination.page === page) return;
      pagination.page = page;
      await refreshTraffic();
    });
  }
}
