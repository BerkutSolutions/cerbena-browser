function escapeHtml(value) {
  return String(value ?? "")
    .replaceAll("&", "&amp;")
    .replaceAll("<", "&lt;")
    .replaceAll(">", "&gt;")
    .replaceAll("\"", "&quot;");
}

export function eyeIcon() {
  return `
    <svg viewBox="0 0 24 24" width="18" height="18" aria-hidden="true" focusable="false">
      <path fill="currentColor" d="M12 5c5.48 0 9.62 4.1 10.82 6.6a.8.8 0 0 1 0 .8C21.62 14.9 17.48 19 12 19S2.38 14.9 1.18 12.4a.8.8 0 0 1 0-.8C2.38 9.1 6.52 5 12 5Zm0 1.6c-4.42 0-7.93 3.16-9.15 5.4 1.22 2.24 4.73 5.4 9.15 5.4s7.93-3.16 9.15-5.4c-1.22-2.24-4.73-5.4-9.15-5.4Zm0 2.15A3.25 3.25 0 1 1 8.75 12 3.25 3.25 0 0 1 12 8.75Zm0 1.6A1.65 1.65 0 1 0 13.65 12 1.65 1.65 0 0 0 12 10.35Z"/>
    </svg>
  `;
}

export function eyeOffIcon() {
  return `
    <svg viewBox="0 0 24 24" width="18" height="18" aria-hidden="true" focusable="false">
      <path fill="currentColor" d="M3.47 2.53 2.53 3.47l3 3C3.6 7.78 2.05 9.77 1.18 11.6a.8.8 0 0 0 0 .8C2.38 14.9 6.52 19 12 19c1.93 0 3.67-.5 5.2-1.31l3.33 3.31.94-.94L3.47 2.53Zm8.53 14.87c-4.42 0-7.93-3.16-9.15-5.4.8-1.48 2.18-3.1 4.06-4.2l1.86 1.85A3.2 3.2 0 0 0 8.75 12 3.25 3.25 0 0 0 12 15.25c.85 0 1.62-.32 2.19-.86l1.84 1.84c-1.18.67-2.56 1.17-4.03 1.17Zm0-10.8c4.42 0 7.93 3.16 9.15 5.4-.52.95-1.3 1.98-2.28 2.9l-1.15-1.15c.54-.57.88-1.35.88-2.15A3.25 3.25 0 0 0 12 8.75c-.8 0-1.58.34-2.15.88L8.28 8.06c1.1-.66 2.39-1.46 3.72-1.46Zm0 3.75a1.65 1.65 0 0 1 1.65 1.65c0 .16-.03.31-.07.46l-2.04-2.04c.15-.04.3-.07.46-.07Z"/>
    </svg>
  `;
}

export function templateStatus(model, template, t) {
  const ping = model.networkPingState?.[template.id];
  if (!ping) return `<span class="badge">${t("network.status.unknown")}</span>`;
  const className = ping.reachable ? "success" : "error";
  const label = ping.reachable
    ? `${escapeHtml(String(ping.latencyMs ?? "-"))} ms`
    : t("network.status.unavailable");
  return `<span class="badge ${className}" title="${escapeHtml(ping.message ?? "")}">${label}</span>`;
}

export function templateChainLabel(template, t, normalizeTemplateNodes) {
  const nodes = normalizeTemplateNodes(template);
  return nodes
    .map((node) => `${t(`network.node.${node.connectionType}`)}:${node.protocol}`)
    .join(" -> ");
}

export function templateRow(template, model, t, normalizeTemplateNodes) {
  return `
    <tr data-template-id="${template.id}">
      <td>${escapeHtml(template.name)}</td>
      <td>${escapeHtml(templateChainLabel(template, t, normalizeTemplateNodes))}</td>
      <td>${templateStatus(model, template, t)}</td>
      <td class="actions network-table-actions-cell">
        <div class="dns-dropdown network-actions-dropdown">
          <button type="button" class="dns-dropdown-toggle network-actions-toggle" data-template-menu-toggle="${template.id}">...</button>
        </div>
      </td>
    </tr>
  `;
}
