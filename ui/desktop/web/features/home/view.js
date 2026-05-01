import { renderProfilesSection } from "../profiles/view.js";
import { listTrafficEvents } from "../traffic/api.js";

function computeMetricsFromTraffic(events) {
  const profileEvents = events ?? [];
  let dnsBlocked = 0;
  let trackerBlocked = 0;
  let serviceBlocked = 0;

  for (const event of profileEvents) {
    if (event.status !== "blocked") continue;
    const reason = String(event.reason ?? "").toLowerCase();
    const blockedByRule = Boolean(event.blocked_globally) || Boolean(event.blocked_for_profile);
    if (reason.includes("dns")) dnsBlocked += 1;
    if (reason.includes("service")) serviceBlocked += 1;
    if (blockedByRule || reason.includes("tracker")) trackerBlocked += 1;
  }

  return { dnsBlocked, trackerBlocked, serviceBlocked };
}

function buildDashboard(metrics) {
  return {
    metrics: [
      { key: "home.metric.dns_blocked", value: metrics.dnsBlocked },
      { key: "home.metric.tracker_blocked", value: metrics.trackerBlocked },
      { key: "home.metric.service_blocked", value: metrics.serviceBlocked }
    ],
    quick_actions: []
  };
}

export function renderHome(t, model) {
  const dashboard = model.homeDashboard ?? buildDashboard({ dnsBlocked: 0, trackerBlocked: 0, serviceBlocked: 0 });
  const metrics = dashboard?.metrics ?? [];
  const notice = model.homeNotice ? `<p class="notice ${model.homeNotice.type}">${model.homeNotice.text}</p>` : "";

  return `
  <div class="feature-page">
    <div class="feature-page-head">
      <h2>${t("nav.home")}</h2>
    </div>
    ${notice}
    <div class="home-metrics-grid">
      ${metrics.map((m)=>`<div class="panel home-metric-card"><strong>${t(m.key)}</strong><p>${m.value}</p></div>`).join("")}
    </div>
    ${renderProfilesSection(t, model)}
  </div>`;
}

export async function hydrateHomeModel(model) {
  let metrics = { dnsBlocked: 0, trackerBlocked: 0, serviceBlocked: 0 };
  const traffic = await listTrafficEvents();
  if (traffic.ok && Array.isArray(traffic.data)) {
    metrics = computeMetricsFromTraffic(traffic.data);
  }
  model.homeDashboard = buildDashboard(metrics);
}

export function wireHome() {}
