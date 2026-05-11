export function renderProfileModalHtml(t, profile, dnsDraft, globalSecurity, model, networkState, syncOverview, identityPreset, deps) {
  const {
    mergedProfileExtensions = () => ({ enabled: [], disabled: [] }),
    profileSecurityFlags = () => ({ allowSystemAccess: false, allowKeepassxc: false, disableExtensionsLaunch: false }),
    certificateEntriesForProfile = () => [],
    loadPolicyPresets = () => ({}),
    summarizePolicyPreset = () => ({ blocklists: 0, blockedServices: 0, allowDomains: 0, denyDomains: 0 }),
    listIdentityTemplates = () => [],
    listIdentityPlatforms = () => [],
    listIdentityTemplatePlatforms = () => [],
    inferIdentityUiState = () => ({ mode: "real", templatePlatform: "desktop", templateKey: "", autoPlatform: "windows" }),
    buildRealPreset = () => ({}),
    routeTemplateOptions = () => "",
    globalRouteNoticeHtml = () => "",
    buildTagPickerMarkup = () => "",
    collectProfileTags = () => [],
    profileTags = () => [],
    dnsTemplateOptions = () => "",
    globalBlocklistOptions = () => [],
    buildDomainEntries = () => [],
    eyeIcon = () => "",
    extensionLibraryOptions = () => "",
    option = (value, label, selected) => `<option value="${value}" ${selected ? "selected" : ""}>${label}</option>`,
    escapeHtml = (value) => String(value ?? ""),
    DOMAIN_OPTIONS = [],
    templateSummaryLabel = () => "",
    templateDropdownOptionsHtml = () => "",
    templateInputValue = () => "",
    searchOptions = () => "",
    normalizeProfileRouteMode = (value) => value ?? "direct"
  } = deps;

  const isRunning = profile?.state === "running";
  const searchDefault = profile?.default_search_provider ?? "duckduckgo";
  const singlePageMode = Boolean((profile?.tags ?? []).some((tag) => tag === "locked-app:custom"));
  const currentPolicy = profile?.tags?.find((x) => x.startsWith("policy:"))?.replace("policy:", "") ?? "normal";
  const ext = mergedProfileExtensions(model, profile);
  const securityFlags = profileSecurityFlags(profile);
  const selectedCertificates = certificateEntriesForProfile(profile, globalSecurity);
  const selectedCertIds = selectedCertificates.filter((item) => item.kind === "id").map((item) => item.value);
  const selectedCertPaths = selectedCertificates.filter((item) => item.kind === "path").map((item) => item.value);
  const selectedBlocklists = dnsDraft?.selectedBlocklists ?? [];
  const allowDomains = dnsDraft?.allowlist ? dnsDraft.allowlist.split(",").map((v) => v.trim()).filter(Boolean) : [];
  const denyDomains = dnsDraft?.denylist ? dnsDraft.denylist.split(",").map((v) => v.trim()).filter(Boolean) : [];
  const policyPresets = loadPolicyPresets(model.serviceCatalog);
  const policySummary = summarizePolicyPreset(policyPresets[currentPolicy]);
  const certificateOptions = (globalSecurity?.certificates ?? []).map((item) => `<option value="${item.id}">${escapeHtml(item.name)}</option>`).join("");
  const routeTemplates = networkState?.connectionTemplates ?? [];
  const selectedRouteMode = normalizeProfileRouteMode(profile ? (networkState?.payload?.route_mode ?? "direct") : "direct");
  const routeIsDirect = selectedRouteMode === "direct";
  const selectedRouteTemplateId = profile ? (networkState?.selectedTemplateId ?? "") : "";
  const routeTemplateList = routeTemplateOptions(routeTemplates, selectedRouteTemplateId, selectedRouteMode, t);
  const routeKillSwitchEnabled = profile ? (networkState?.payload?.kill_switch_enabled ?? true) : true;
  const syncServerValue = syncOverview?.controls?.server?.server_url ?? "";
  const syncKeyValue = syncOverview?.controls?.server?.key_id ?? "";
  const syncEnabled = Boolean(syncOverview?.controls?.server?.sync_enabled);
  const resolvedIdentityPreset = identityPreset ?? buildRealPreset();
  const identityState = inferIdentityUiState(resolvedIdentityPreset);
  const identityTemplates = listIdentityTemplates(t);
  const identityPlatforms = listIdentityPlatforms(t);
  const identityTemplatePlatforms = listIdentityTemplatePlatforms(t);
  const filteredIdentityTemplates = listIdentityTemplates(t, { platformFamilies: [identityState.templatePlatform] });
  const isIdentityAuto = identityState.mode === "auto";
  const isIdentityReal = identityState.mode === "real";
  return `
  <div class="profiles-modal-overlay" id="profile-modal-overlay">
    <div class="profiles-modal-window profile-modal">
      <div class="profile-modal-layout">
        <div class="tab-header profile-modal-rail">
          <button type="button" data-tab="general" class="active">${t("profile.tab.general")}</button>
          <button type="button" data-tab="identity">${t("profile.tab.identity")}</button>
          <button type="button" data-tab="vpn">${t("profile.tab.vpn")}</button>
          <button type="button" data-tab="dns">${t("profile.tab.dns")}</button>
          <button type="button" data-tab="extensions">${t("profile.tab.extensions")}</button>
          <button type="button" data-tab="security">${t("profile.tab.security")}</button>
          <button type="button" data-tab="sync">${t("profile.tab.sync")}</button>
          <button type="button" data-tab="advanced">${t("profile.tab.advanced")}</button>
        </div>

        <div class="profile-modal-main">
        ${isRunning ? `<p class="warning">${t("profile.runtime.runningWarning")}</p>` : ""}

        <form id="profile-form" data-profile-id="${profile?.id ?? ""}" class="profile-modal-form">
          <div class="tab-pane" data-pane="general">
            <div class="grid-two profile-modal-grid">
              <label>${t("profile.field.name")}<input name="name" value="${profile?.name ?? ""}" required /></label>
              <label>${t("profile.field.engine")}<select name="engine" id="profile-engine">${option("chromium", "Chromium", profile?.engine === "chromium")}${option("ungoogled-chromium", "Ungoogled Chromium", profile?.engine === "ungoogled-chromium")}${option("firefox-esr", "Firefox ESR", profile?.engine === "firefox-esr")}${option("librewolf", "LibreWolf", profile?.engine === "librewolf")}</select></label>
              <label class="profile-modal-span-2 profile-description-field">${t("profile.field.description")}<textarea name="description" rows="4">${escapeHtml(profile?.description ?? "")}</textarea></label>
              <label class="profile-modal-span-2">${t("profile.field.tags")}
                ${buildTagPickerMarkup({
                  id: "profile-tags",
                  selectedTags: profileTags(profile ?? { tags: [] }) ?? [],
                  availableTags: collectProfileTags(model.profiles),
                  emptyLabel: t("profile.tags.empty"),
                  searchPlaceholder: t("profile.tags.search"),
                  createLabel: (value) => t("profile.tags.create").replace("{tag}", value)
                })}
              </label>
              <label>${t("profile.field.defaultStartPage")}<input name="defaultStartPage" value="${profile?.default_start_page ?? ""}" /></label>
              <label class="checkbox-inline">
                <input type="checkbox" name="singlePageMode" id="profile-single-page-mode" ${singlePageMode ? "checked" : ""} />
                <span>${t("profile.field.singlePage")}</span>
              </label>
              <label id="profile-default-search-row" class="${singlePageMode ? "hidden" : ""}">${t("profile.field.defaultSearch")}<select name="defaultSearchProvider" ${singlePageMode ? "disabled" : ""}>${searchOptions(searchDefault)}</select></label>
              <p class="meta profile-modal-span-2 ${singlePageMode ? "" : "hidden"}" id="profile-single-page-hint">${t("profile.field.singlePageHint")}</p>
              <label class="checkbox-inline profile-modal-span-2">
                <input type="checkbox" name="panicFrameEnabled" ${profile?.panic_frame_enabled ? "checked" : ""} />
                <span>${t("profile.field.panicFrame")}</span>
              </label>
              <label class="profile-modal-span-2 ${profile?.panic_frame_enabled ? "" : "hidden"}" id="profile-panic-color-row">${t("profile.field.panicFrameColor")}<input type="color" name="panicFrameColor" value="${escapeHtml(profile?.panic_frame_color ?? "#ff8652")}" /></label>
            </div>
          </div>

          <div class="tab-pane hidden" data-pane="identity">
            <div class="grid-two">
              <label>${t("profile.field.identityMode")}
                <select name="identityMode" id="profile-identity-mode" class="identity-mode-select">
                  <option value="real" ${isIdentityReal ? "selected" : ""}>${t("identity.mode.real")}</option>
                  <option value="auto" ${isIdentityAuto ? "selected" : ""}>${t("identity.mode.auto")}</option>
                  <option value="manual" ${!isIdentityAuto && !isIdentityReal ? "selected" : ""}>${t("identity.mode.manual")}</option>
                </select>
              </label>
              <label id="profile-identity-platform-row" class="${isIdentityAuto ? "" : "hidden"}">${t("profile.field.platformTarget")}
                <select name="platformTarget" id="profile-platform-target">
                  ${identityPlatforms.map((item) => `<option value="${item.key}" ${item.key === identityState.autoPlatform ? "selected" : ""}>${escapeHtml(item.label)}</option>`).join("")}
                </select>
              </label>
            </div>
            <div class="security-frame ${isIdentityAuto || isIdentityReal ? "hidden" : ""}" id="profile-identity-template-row">
              <label>${t("identity.field.platformTemplate")}
                <select id="profile-identity-template-platform">
                  ${identityTemplatePlatforms.map((item) => `<option value="${item.key}" ${item.key === identityState.templatePlatform ? "selected" : ""}>${escapeHtml(item.label)}</option>`).join("")}
                </select>
              </label>
              <label>${t("identity.field.displayName")}
                <input type="text" name="identityDisplayName" id="profile-identity-display-name" value="${escapeHtml(resolvedIdentityPreset?.display_name ?? templateSummaryLabel(t, identityTemplates, identityState.templateKey))}" />
              </label>
              <h4>${t("profile.identity.template")}</h4>
              <div class="dns-dropdown profile-identity-template-dropdown">
                <button type="button" class="dns-dropdown-toggle" id="profile-identity-template-toggle">
                  <span id="profile-identity-template-summary">${escapeHtml(templateSummaryLabel(t, identityTemplates, identityState.templateKey))}</span>
                </button>
                <input type="hidden" name="identityTemplate" value="${escapeHtml(templateInputValue(identityState))}" />
                <div class="dns-dropdown-menu hidden" id="profile-identity-template-menu">
                  <input id="profile-identity-template-search" placeholder="${t("profile.identity.templateSearch")}" />
                  <div id="profile-identity-template-options">
                    ${templateDropdownOptionsHtml(t, filteredIdentityTemplates, identityState.templateKey)}
                  </div>
                </div>
              </div>
            </div>
            <p class="meta ${isIdentityReal ? "" : "hidden"}" id="profile-identity-real-hint">${t("identity.realHint")}</p>
            <p class="meta ${isIdentityAuto ? "" : "hidden"}" id="profile-identity-auto-hint">${t("identity.autoHint")}</p>
            <div id="profile-identity-state" data-preset="${escapeHtml(JSON.stringify(resolvedIdentityPreset))}" data-ui="${escapeHtml(JSON.stringify(identityState))}"></div>
            <div id="profile-identity-templates" data-templates="${escapeHtml(JSON.stringify(identityTemplates.map((item) => ({ key: item.key, label: item.label, autoPlatform: item.autoPlatform, platformFamily: item.platformFamily }))))}"></div>
            <div id="profile-identity-platforms" data-platforms="${escapeHtml(JSON.stringify(identityPlatforms))}"></div>
          </div>

          <div class="tab-pane hidden" data-pane="vpn">
            <div class="grid-two profile-modal-grid">
              <label>${t("profile.field.routeMode")}
                <select name="profileRouteMode" id="profile-route-mode">
                  <option value="direct" ${selectedRouteMode === "direct" ? "selected" : ""}>${t("network.mode.direct")}</option>
                  <option value="proxy" ${selectedRouteMode === "proxy" ? "selected" : ""}>${t("network.mode.proxy")}</option>
                  <option value="vpn" ${selectedRouteMode === "vpn" ? "selected" : ""}>${t("network.mode.vpn")}</option>
                  <option value="tor" ${selectedRouteMode === "tor" ? "selected" : ""}>${t("network.mode.tor")}</option>
                </select>
              </label>
              <label id="profile-route-template-row" class="${routeIsDirect ? "hidden" : ""}">${t("network.routeTemplate")}
                <select name="profileRouteTemplateId" id="profile-route-template" ${selectedRouteMode === "direct" ? "disabled" : ""}>
                  ${routeTemplateList}
                </select>
              </label>
              <label class="checkbox-inline ${routeIsDirect ? "hidden" : ""}" id="profile-kill-switch-row">
                <input type="checkbox" name="profileKillSwitch" ${routeKillSwitchEnabled ? "checked" : ""} ${routeIsDirect ? "disabled" : ""}/>
                <span>${t("network.killSwitch")}</span>
              </label>
            </div>
            <div id="profile-global-route-notice-slot">
              ${globalRouteNoticeHtml(networkState, selectedRouteMode, t)}
            </div>
            <div id="profile-sandbox-frame-slot"></div>
          </div>

          <div class="tab-pane hidden profile-pane-plain" data-pane="dns">
            <div class="security-frame">
              <div class="grid-two">
                <label>${t("profile.field.dnsMode")}<select name="dnsMode" id="profile-dns-mode"><option value="system" ${(dnsDraft?.mode ?? "system") === "system" ? "selected" : ""}>${t("dns.system")}</option><option value="custom" ${(dnsDraft?.mode ?? "system") === "custom" ? "selected" : ""}>${t("dns.custom")}</option></select></label>
                <label id="profile-dns-servers-row">${t("profile.field.dnsServers")}<input name="dnsServers" placeholder="1.1.1.1,8.8.8.8" value="${escapeHtml(dnsDraft?.servers ?? "")}" /></label>
                <label id="profile-dns-template-row">${t("dns.template.current")}<select name="dnsTemplateId">${dnsTemplateOptions(profile, t)}</select></label>
              </div>
            </div>
            <div class="security-frame">
              <h4>${t("profile.policy")}</h4>
              <label><select name="policyLevel"><option value="light" ${currentPolicy === "light" ? "selected" : ""}>light</option><option value="normal" ${currentPolicy === "normal" ? "selected" : ""}>normal</option><option value="high" ${currentPolicy === "high" ? "selected" : ""}>high</option><option value="maximum" ${currentPolicy === "maximum" ? "selected" : ""}>maximum</option></select></label>
              <p class="meta" id="profile-policy-summary">${escapeHtml(`${policySummary.blocklists} ${t("dns.policy.summary.blocklists")} • ${policySummary.blockedServices} ${t("dns.policy.summary.services")} • ${policySummary.allowDomains} ${t("dns.policy.summary.allow")} • ${policySummary.denyDomains} ${t("dns.policy.summary.deny")}`)}</p>
              <div class="top-actions"><button type="button" id="profile-policy-load">${t("dns.policy.load")}</button></div>
            </div>
            <div class="security-frame">
              <h4>${t("profile.dns.blocklists")}</h4>
              <div class="dns-dropdown profile-blocklist-dropdown">
                <button type="button" class="dns-dropdown-toggle" id="profile-blocklists-toggle">
                  <span id="profile-blocklists-summary">${t("profile.dns.selectBlocklists")}</span>
                </button>
                <div class="dns-dropdown-menu hidden profile-blocklists-menu" id="profile-blocklists-menu">
                  <input id="profile-blocklists-search" placeholder="${t("dns.searchPlaceholder")}" />
                  <div class="top-actions">
                    <button type="button" id="profile-blocklists-select-all">${t("security.all")}</button>
                  </div>
                  <div id="profile-blocklists-options">
                    ${globalBlocklistOptions(globalSecurity).map((item) => `
                      <label class="dns-blocklist-option" data-profile-blocklist-option="${escapeHtml((item.label ?? item.id).toLowerCase())}">
                        <input type="checkbox" data-profile-blocklist-id="${item.id}" ${selectedBlocklists.includes(item.id) || item.active ? "checked" : ""} ${item.active ? "disabled" : ""} />
                        <span>${escapeHtml(item.label)}</span>
                        ${item.active ? `<span class="meta">${t("security.active")}</span>` : ""}
                      </label>
                    `).join("")}
                  </div>
                </div>
              </div>
            </div>
            <div class="security-frame">
              <div class="row-between">
                <div>
                  <h4>${t("profile.security.domains")}</h4>
                  <p class="meta">${t("profile.security.domainsHint")}</p>
                </div>
              </div>
              <div class="grid-two profile-modal-grid">
                <label>${t("profile.security.domainInput")}<input name="domainEntry" id="profile-domain-input" list="profile-domain-suggestions" placeholder="example.com" /></label>
                <label>${t("profile.security.domainStatus")}<select name="domainEntryType" id="profile-domain-type"><option value="deny">${t("profile.security.domainBlocked")}</option><option value="allow">${t("profile.security.domainAllowed")}</option></select></label>
                <label>${t("profile.security.domainSearch")}<input name="domainSearch" id="profile-domain-search" placeholder="${t("profile.security.domainSearch")}" /></label>
                <label>${t("profile.security.domainFilter")}<select name="domainFilter" id="profile-domain-filter"><option value="all">${t("profile.security.domainFilterAll")}</option><option value="deny">${t("profile.security.domainBlocked")}</option><option value="allow">${t("profile.security.domainAllowed")}</option></select></label>
              </div>
              <div class="top-actions">
                <button type="button" id="profile-domain-add">${t("profile.security.domainAdd")}</button>
              </div>
              <datalist id="profile-domain-suggestions">
                ${DOMAIN_OPTIONS.map((value) => `<option value="${value}"></option>`).join("")}
              </datalist>
              <table class="extensions-table">
                <thead><tr><th>${t("profile.security.domainStatus")}</th><th>${t("security.domain")}</th><th>${t("extensions.actions")}</th></tr></thead>
                <tbody id="profile-domain-table" data-domains="${escapeHtml(JSON.stringify(buildDomainEntries(allowDomains, denyDomains)))}"></tbody>
              </table>
            </div>
          </div>

          <div class="tab-pane hidden" data-pane="extensions">
            <div class="grid-two profile-extension-toolbar">
              <label>${t("profile.field.extensionIds")}
                <select name="extensionSelect">
                  <option value="">${t("security.selectExtension")}</option>
                  ${extensionLibraryOptions(model, ext, profile)}
                </select>
              </label>
              <label class="profile-toolbar-action">&nbsp;<button type="button" class="profile-toolbar-button" id="profile-extension-add">${t("extensions.add")}</button></label>
            </div>
            <table class="extensions-table">
              <thead><tr><th>${t("extensions.name")}</th><th>${t("extensions.status")}</th><th>${t("extensions.actions")}</th></tr></thead>
              <tbody id="profile-extensions-table" data-enabled="${escapeHtml(JSON.stringify(ext.enabled))}" data-disabled="${escapeHtml(JSON.stringify(ext.disabled))}"></tbody>
            </table>
          </div>

          <div class="tab-pane hidden" data-pane="security">
            <div class="security-toggle-list">
              <label class="checkbox-inline security-toggle-row"><input type="checkbox" name="passwordLock" ${profile?.password_lock_enabled ? "checked" : ""}/> <span>${t("profile.field.passwordLock")}</span></label>
              <div class="grid-two profile-modal-grid hidden" id="profile-password-fields">
                <label>${t("profile.security.password")} 
                  <span class="profile-password-field">
                    <input type="password" name="profilePassword" autocomplete="new-password" />
                    <button type="button" class="profile-password-toggle" id="profile-password-toggle" aria-label="${t("profile.security.showPassword")}">${eyeIcon()}</button>
                  </span>
                </label>
                <label>${t("profile.security.passwordConfirm")}<input type="password" name="profilePasswordConfirm" autocomplete="new-password" /></label>
              </div>
              <label class="checkbox-inline security-toggle-row"><input type="checkbox" name="ephemeral" ${profile?.ephemeral_mode ? "checked" : ""}/> <span>${t("profile.field.ephemeral")}</span></label>
              <label class="checkbox-inline security-toggle-row"><input type="checkbox" name="disableExtensionsLaunch" ${securityFlags.disableExtensionsLaunch ? "checked" : ""}/> <span>${t("profile.security.disableExtensionsLaunch")}</span></label>
              <label class="checkbox-inline security-toggle-row"><input type="checkbox" name="allowSystemAccess" ${securityFlags.allowSystemAccess ? "checked" : ""}/> <span>${t("profile.security.allowSystemAccess")}</span></label>
              <label class="checkbox-inline security-toggle-row"><input type="checkbox" name="allowKeepassxc" ${securityFlags.allowKeepassxc ? "checked" : ""}/> <span>${t("profile.security.allowKeepassxc")}</span></label>
            </div>
            <div class="security-frame">
              <h4>${t("security.certificates.customTitle")}</h4>
              <p class="meta">${t("security.certificates.hint")}</p>
              <div id="profile-certificate-engine-guard"></div>
              <div class="grid-two profile-certificates-toolbar">
                <label>${t("security.certificates.profile")}<select name="profileCertificateSelect"><option value="">${t("security.selectCertificate")}</option>${certificateOptions}</select></label>
                <label class="profile-toolbar-action">&nbsp;<button type="button" class="profile-toolbar-button" id="profile-certificate-add">${t("security.certificates.add")}</button></label>
                <label class="profile-modal-span-2 profile-toolbar-action">&nbsp;<button type="button" class="profile-toolbar-button profile-toolbar-button-wide" id="profile-certificate-pick">${t("security.certificates.pickFiles")}</button></label>
              </div>
              <table class="extensions-table">
                <thead><tr><th>${t("extensions.name")}</th><th>${t("extensions.actions")}</th></tr></thead>
                <tbody
                  id="profile-certificates-table"
                  data-certificate-ids="${escapeHtml(JSON.stringify(selectedCertIds))}"
                  data-certificate-paths="${escapeHtml(JSON.stringify(selectedCertPaths))}"
                ></tbody>
              </table>
            </div>
          </div>

          <div class="tab-pane hidden" data-pane="sync">
            <div class="grid-two">
              <label>${t("profile.sync.server")}<input name="syncServer" value="${escapeHtml(syncServerValue)}" placeholder="https://sync.example" /></label>
              <label>${t("profile.sync.key")}<input name="syncKey" value="${escapeHtml(syncKeyValue)}" placeholder="generated-key-id" /></label>
              <label class="checkbox-inline"><input name="syncEnabled" type="checkbox" ${syncEnabled ? "checked" : ""}/> ${t("sync.enabled")}</label>
            </div>
          </div>

          <div class="tab-pane hidden" data-pane="advanced">
            <label>${t("profile.advanced.launchHook")}<input name="launchHook" placeholder="https://hook.example/start" /></label>
          </div>

          <footer class="modal-actions">
            <button type="button" id="profile-cancel">${t("action.cancel")}</button>
            <button type="submit">${t("action.save")}</button>
          </footer>
        </form>
        </div>
      </div>
    </div>
  </div>`;
}
