import { BLOCKLIST_PRESETS, CATEGORY_LABEL_KEYS, serviceLabel } from "./catalog.js";
import { summarizePolicyPreset } from "./policy-store.js";

export function summarizePolicyDetails(preset, t) {
  const summary = summarizePolicyPreset(preset);
  return `${summary.blocklists} ${t("dns.policy.summary.blocklists")} | ${summary.blockedServices} ${t("dns.policy.summary.services")} | ${summary.denyDomains} ${t("dns.policy.summary.deny")}`;
}

export function serviceNamesFromPreset(preset) {
  const names = [];
  for (const [categoryKey, services] of Object.entries(preset?.blockedServices ?? {})) {
    for (const [serviceKey, blocked] of Object.entries(services ?? {})) {
      if (blocked) {
        names.push({
          categoryKey,
          label: serviceLabel(serviceKey)
        });
      }
    }
  }
  return names.sort((left, right) => left.label.localeCompare(right.label));
}

export function blocklistCatalogItems(globalSecurity) {
  const globalItems = (globalSecurity?.blocklists ?? []).map((item) => ({
    id: item.id,
    label: item.name ?? item.id,
    domains: item.domains ?? []
  }));
  const knownIds = new Set(globalItems.map((item) => item.id));
  for (const preset of BLOCKLIST_PRESETS) {
    if (!knownIds.has(preset.id)) {
      globalItems.push({
        id: preset.id,
        label: preset.label,
        domains: preset.domains ?? []
      });
    }
  }
  return globalItems;
}

function serviceRow(categoryKey, serviceKey, blocked) {
  return `
    <label class="dns-service-row">
      <input
        type="checkbox"
        data-category="${categoryKey}"
        data-service="${serviceKey}"
        data-action="toggle-service"
        ${blocked ? "checked" : ""}
      />
      <span>${serviceLabel(serviceKey)}</span>
    </label>
  `;
}

export function categoryCard(category, draft, search, t) {
  const services = Object.keys(category.services ?? {}).filter((serviceKey) => {
    const needle = search.trim().toLowerCase();
    return !needle || serviceLabel(serviceKey).toLowerCase().includes(needle);
  });
  if (!services.length) return "";

  const allBlocked = services.every((serviceKey) => draft.blockedServices?.[category.category]?.[serviceKey]);
  return `
    <section class="dns-category-card">
      <div class="dns-category-head">
        <div>
          <h3>${t(CATEGORY_LABEL_KEYS[category.category] ?? "dns.serviceCatalog")}</h3>
          <p class="meta">${services.length} ${t("dns.servicesCount")}</p>
        </div>
        <button data-category="${category.category}" data-action="toggle-category">
          ${allBlocked ? t("dns.unblockAll") : t("dns.blockAll")}
        </button>
      </div>
      <div class="dns-services-grid">
        ${services.map((serviceKey) => serviceRow(category.category, serviceKey, Boolean(draft.blockedServices?.[category.category]?.[serviceKey]))).join("")}
      </div>
    </section>
  `;
}
