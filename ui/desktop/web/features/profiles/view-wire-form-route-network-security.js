import { wireRouteNetworkSecurityBlocklists } from "./view-wire-form-route-network-security-blocklists.js";
import { selectRouteNetworkSecurityNodes } from "./view-wire-form-route-network-security-selectors.js";
export function wireRouteNetworkSecurityTab(ctx) {
  const {
    overlay,
    form,
    t,
    model,
    existing,
    dnsDraft,
    globalSecurityRef,
    profileNetworkState,
    loadPolicyPresets,
    summarizePolicyPreset,
    globalBlocklistOptions,
    normalizeProfileRouteMode,
    routeTemplateOptions,
    globalRouteNoticeHtml,
    previewNetworkSandboxSettings,
    renderProfileSandboxFrame,
    applyPolicyPresetToDraft,
    escapeHtml,
    hasAssignedProfileCertificates,
    domainStatusIcon,
    domainStatusLabel,
    buildDomainEntries,
    slugId,
    makeUniqueId,
    pickCertificateFiles,
    setNotice,
    rerender,
    getGlobalSecuritySettings,
    normalizeGlobalSecuritySettings,
    saveGlobalSecuritySettings,
    buildGlobalSecuritySaveRequest,
    markDirty
  } = ctx;
  const {
    profileRouteMode, profileRouteTemplate, profileRouteTemplateRow, profileKillSwitchRow, profileKillSwitchInput,
    profileGlobalRouteNoticeSlot, profileSandboxSlot, profileDnsModeField, profileDnsServersRow, profileDnsTemplateRow,
    passwordLockField, panicFrameEnabledField, panicColorRow, profileEngineField, singlePageModeField,
    defaultSearchRow, singlePageHint, passwordFields, passwordValueField, passwordConfirmField, passwordToggleButton,
    domainTable, domainInput, domainTypeField, domainSearchField, domainFilterField, certificateTable,
    profileCertificateSelectField, profileCertificateEngineGuard
  } = selectRouteNetworkSecurityNodes(overlay);
  let profileSandboxPreview = null;
  let initialSandboxMode = null;
  let draftSandboxMode = null;
  const selectedRouteTemplateFromForm = () => {
    const routeTemplateId = profileRouteTemplate?.value || "";
    return (profileNetworkState.connectionTemplates ?? []).find((item) => item.id === routeTemplateId) ?? null;
  };
  const bindProfileSandboxSelect = () => {
    const select = overlay.querySelector("#profile-sandbox-mode");
    select?.addEventListener("change", async () => {
      markDirty();
      draftSandboxMode = select.value || null;
      await refreshProfileSandboxFrame();
    });
  };
  const refreshProfileSandboxFrame = async () => {
    if (!profileSandboxSlot || !profileRouteMode || !profileRouteTemplate) return;
    const routeMode = normalizeProfileRouteMode(profileRouteMode.value || "direct");
    const selectedTemplate = selectedRouteTemplateFromForm();
    if (routeMode === "direct" || !selectedTemplate) {
      profileSandboxPreview = null;
      initialSandboxMode = null;
      profileSandboxSlot.innerHTML = "";
      return;
    }
    const preferredMode = draftSandboxMode || overlay.querySelector("#profile-sandbox-mode")?.value || profileNetworkState.sandbox?.preferredMode || null;
    const previewResult = await previewNetworkSandboxSettings({
      profileId: existing?.id ?? null,
      routeMode,
      templateId: selectedTemplate.id,
      preferredMode
    });
    if (!previewResult.ok) {
      profileSandboxPreview = null;
      profileSandboxSlot.innerHTML = "";
      return;
    }
    profileSandboxPreview = previewResult.data;
    if (!initialSandboxMode) {
      initialSandboxMode = profileSandboxPreview.sandbox.preferredMode || profileSandboxPreview.sandbox.effectiveMode || profileSandboxPreview.compatibleModes?.[0] || null;
    }
    if (!draftSandboxMode) draftSandboxMode = preferredMode || initialSandboxMode;
    profileSandboxSlot.innerHTML = renderProfileSandboxFrame(profileSandboxPreview, selectedTemplate, draftSandboxMode, t);
    bindProfileSandboxSelect();
  };
  const refreshRouteTemplateOptions = () => {
    if (!profileRouteTemplate || !profileRouteMode) return;
    const routeMode = normalizeProfileRouteMode(profileRouteMode.value || "direct");
    profileRouteMode.value = routeMode;
    const routeIsDirect = routeMode === "direct";
    profileRouteTemplateRow?.classList.toggle("hidden", routeIsDirect);
    profileKillSwitchRow?.classList.toggle("hidden", routeIsDirect);
    if (profileKillSwitchInput) profileKillSwitchInput.disabled = routeIsDirect;
    if (routeIsDirect) {
      profileRouteTemplate.disabled = true;
      profileRouteTemplate.value = "";
      return;
    }
    const currentValue = profileRouteTemplate.value || "";
    profileRouteTemplate.disabled = false;
    profileRouteTemplate.innerHTML = routeTemplateOptions(profileNetworkState.connectionTemplates ?? [], currentValue, routeMode, t);
    if (![...profileRouteTemplate.options].some((option) => option.value === currentValue)) profileRouteTemplate.value = "";
  };
  const refreshGlobalRouteNotice = () => {
    if (!profileGlobalRouteNoticeSlot || !profileRouteMode) return;
    profileGlobalRouteNoticeSlot.innerHTML = globalRouteNoticeHtml(profileNetworkState, profileRouteMode.value || "direct", t);
  };
  refreshRouteTemplateOptions();
  refreshGlobalRouteNotice();
  refreshProfileSandboxFrame().catch(() => {});
  profileRouteMode?.addEventListener("change", async () => {
    markDirty();
    initialSandboxMode = null;
    draftSandboxMode = null;
    refreshRouteTemplateOptions();
    refreshGlobalRouteNotice();
    await refreshProfileSandboxFrame();
  });
  profileRouteTemplate?.addEventListener("change", async () => {
    markDirty();
    initialSandboxMode = null;
    draftSandboxMode = null;
    await refreshProfileSandboxFrame();
  });
  const renderDnsControls = () => {
    const isManual = (profileDnsModeField?.value ?? "system") === "custom";
    if (isManual) {
      const dnsServersField = overlay.querySelector("[name='dnsServers']");
      if (dnsServersField && !String(dnsServersField.value ?? "").trim()) dnsServersField.value = "1.1.1.1,8.8.8.8";
    }
    profileDnsServersRow?.classList.toggle("hidden", !isManual);
    profileDnsTemplateRow?.classList.toggle("hidden", !isManual);
  };
  renderDnsControls();
  profileDnsModeField?.addEventListener("change", () => {
    markDirty();
    renderDnsControls();
  });
  const initialDomainEntries = (() => {
    try { return JSON.parse(domainTable?.dataset?.domains ?? "[]"); } catch { return []; }
  })();
  const allowState = initialDomainEntries.filter((item) => item?.type === "allow" && item?.domain).map((item) => item.domain);
  const denyState = initialDomainEntries.filter((item) => item?.type === "deny" && item?.domain).map((item) => item.domain);
  const certificateState = (() => {
    try {
      const byId = JSON.parse(certificateTable?.dataset?.certificateIds ?? "[]").map((value) => ({ kind: "id", value }));
      const byPath = JSON.parse(certificateTable?.dataset?.certificatePaths ?? "[]").map((value) => ({ kind: "path", value }));
      return [...byId, ...byPath];
    } catch { return []; }
  })();
  const policyPresets = loadPolicyPresets(model.serviceCatalog);
  const blocklistItems = globalBlocklistOptions(globalSecurityRef.current);
  const globalActiveBlocklistIds = new Set(blocklistItems.filter((item) => item.active).map((item) => item.id));
  const blocklistState = new Set(dnsDraft.selectedBlocklists ?? []);
  for (const id of globalActiveBlocklistIds) blocklistState.add(id);
  const domainTableState = (() => {
    try { return JSON.parse(domainTable?.dataset?.domains ?? "[]"); } catch { return buildDomainEntries(allowState, denyState); }
  })();
  const domainUiState = { search: "", filter: "all" };
  const syncDomainArrays = () => {
    allowState.length = 0; denyState.length = 0;
    for (const item of domainTableState) (item.type === "allow" ? allowState : denyState).push(item.domain);
  };
  const renderDomainTable = () => {
    if (!domainTable) return;
    const query = domainUiState.search.trim().toLowerCase();
    const filter = domainUiState.filter;
    const rows = domainTableState
      .filter((item) => (filter === "all" ? true : item.type === filter))
      .filter((item) => (!query ? true : item.domain.toLowerCase().includes(query)))
      .sort((left, right) => left.type !== right.type ? (left.type === "deny" ? -1 : 1) : left.domain.localeCompare(right.domain));
    domainTable.innerHTML = rows.map((item) => `<tr><td class="profile-domain-status"><span class="profile-domain-status-badge profile-domain-status-${item.type}">${domainStatusIcon(item.type)} ${escapeHtml(domainStatusLabel(item.type, t))}</span></td><td>${escapeHtml(item.domain)}</td><td class="actions"><button type="button" data-domain-remove="${item.type}:${escapeHtml(item.domain)}">${t("extensions.remove")}</button></td></tr>`).join("") || `<tr><td colspan="3" class="meta">${t("extensions.empty")}</td></tr>`;
    for (const btn of domainTable.querySelectorAll("[data-domain-remove]")) {
      btn.addEventListener("click", () => {
        const [type, domain] = btn.getAttribute("data-domain-remove").split(":");
        const next = domainTableState.filter((item) => !(item.type === type && item.domain === domain));
        domainTableState.length = 0;
        domainTableState.push(...next);
        syncDomainArrays();
        markDirty();
        renderDomainTable();
      });
    }
  };
  const renderCertificateEngineGuard = () => {
    const engine = profileEngineField?.value ?? "chromium";
    const hasCertificates = hasAssignedProfileCertificates(certificateState);
    const certificatesSupported = engine === "librewolf";
    if (!profileCertificateEngineGuard) return;
    let message = "";
    if (!certificatesSupported && hasCertificates) message = t("profile.security.certificateIsolationWarning");
    else if (!certificatesSupported) message = t("profile.security.certificateIsolationHint");
    else if (hasCertificates) message = t("profile.security.certificateIsolationLibreWolf");
    profileCertificateEngineGuard.innerHTML = message ? `<div class="notice ${certificatesSupported ? "" : "error"}">${escapeHtml(message)}</div>` : "";
  };
  const renderCertificates = () => {
    if (!certificateTable) return;
    certificateTable.innerHTML = certificateState.map((entry) => {
      if (entry.kind === "id") {
        const cert = (globalSecurityRef.current.certificates ?? []).find((item) => item.id === entry.value);
        if (!cert) return "";
        return `<tr><td>${escapeHtml(cert.name)}</td><td class="actions"><button type="button" data-cert-remove="id:${cert.id}">${t("extensions.remove")}</button></td></tr>`;
      }
      const path = String(entry.value ?? "").trim();
      if (!path) return "";
      const name = path.split(/[/\\\\]/).pop()?.replace(/\.(pem|crt|cer)$/i, "") || path;
      return `<tr><td>${escapeHtml(name)}</td><td class="actions"><button type="button" data-cert-remove="path:${escapeHtml(path)}">${t("extensions.remove")}</button></td></tr>`;
    }).join("") || `<tr><td colspan="2" class="meta">${t("security.certificates.empty")}</td></tr>`;
    for (const btn of certificateTable.querySelectorAll("[data-cert-remove]")) {
      btn.addEventListener("click", () => {
        const [kind, ...valueParts] = String(btn.getAttribute("data-cert-remove") ?? "").split(":");
        const value = valueParts.join(":");
        const next = certificateState.filter((item) => !(item.kind === kind && item.value === value));
        certificateState.length = 0;
        certificateState.push(...next);
        markDirty();
        renderCertificates();
        renderCertificateEngineGuard();
      });
    }
  };
  const renderBlocklistSummary = () => {
    for (const checkbox of overlay.querySelectorAll("[data-profile-blocklist-id]")) {
      const id = checkbox.getAttribute("data-profile-blocklist-id");
      checkbox.checked = blocklistState.has(id) || globalActiveBlocklistIds.has(id);
    }
  };
  const renderPolicySummary = () => {
    const summaryEl = overlay.querySelector("#profile-policy-summary");
    if (!summaryEl) return;
    const level = overlay.querySelector("[name='policyLevel']")?.value ?? "normal";
    const summary = summarizePolicyPreset(policyPresets[level]);
    summaryEl.textContent = `${summary.blocklists} ${t("dns.policy.summary.blocklists")} | ${summary.blockedServices} ${t("dns.policy.summary.services")} | ${summary.allowDomains} ${t("dns.policy.summary.allow")} | ${summary.denyDomains} ${t("dns.policy.summary.deny")}`;
  };
  const applyPolicyLevelToModal = () => {
    const level = overlay.querySelector("[name='policyLevel']")?.value ?? "normal";
    const preset = policyPresets[level];
    if (!preset) return;
    applyPolicyPresetToDraft(dnsDraft, preset, model.serviceCatalog);
    blocklistState.clear();
    for (const id of dnsDraft.selectedBlocklists ?? []) blocklistState.add(id);
    for (const id of globalActiveBlocklistIds) blocklistState.add(id);
    if (profileDnsModeField) profileDnsModeField.value = dnsDraft.mode ?? "system";
    if (overlay.querySelector("[name='dnsServers']")) overlay.querySelector("[name='dnsServers']").value = dnsDraft.servers ?? "";
    if (overlay.querySelector("#profile-domain-search")) overlay.querySelector("#profile-domain-search").value = "";
    if (overlay.querySelector("#profile-domain-input")) overlay.querySelector("#profile-domain-input").value = "";
    allowState.length = 0;
    allowState.push(...String(dnsDraft.allowlist ?? "").split(",").map((item) => item.trim()).filter(Boolean));
    denyState.length = 0;
    denyState.push(...String(dnsDraft.denylist ?? "").split(",").map((item) => item.trim()).filter(Boolean));
    domainTableState.length = 0;
    domainTableState.push(...buildDomainEntries(allowState, denyState));
    domainUiState.search = "";
    domainUiState.filter = "all";
    renderDnsControls();
    renderBlocklistSummary();
    renderDomainTable();
    renderPolicySummary();
    markDirty();
  };
  overlay.querySelector("[name='policyLevel']")?.addEventListener("change", () => { markDirty(); renderPolicySummary(); });
  overlay.querySelector("#profile-policy-load")?.addEventListener("click", () => applyPolicyLevelToModal());
  const renderPasswordControls = () => {
    const enabled = Boolean(passwordLockField?.checked);
    passwordFields?.classList.toggle("hidden", !enabled);
    if (passwordValueField) passwordValueField.required = enabled;
    if (passwordConfirmField) passwordConfirmField.required = enabled;
  };
  const renderPanicColorControls = () => panicColorRow?.classList.toggle("hidden", !panicFrameEnabledField?.checked);
  const renderSinglePageControls = () => {
    const engine = profileEngineField?.value ?? "chromium";
    const supported = ["chromium", "wayfern", "ungoogled_chromium", "camoufox"].includes(String(engine).toLowerCase());
    if (singlePageModeField) { if (!supported) singlePageModeField.checked = false; singlePageModeField.disabled = !supported; }
    const active = Boolean(supported && singlePageModeField?.checked);
    if (defaultSearchRow) {
      defaultSearchRow.classList.toggle("hidden", active);
      const select = defaultSearchRow.querySelector("[name='defaultSearchProvider']");
      if (select) select.disabled = active;
    }
    singlePageHint?.classList.toggle("hidden", !active);
  };
  renderSinglePageControls();
  renderCertificateEngineGuard();
  renderPasswordControls();
  renderPanicColorControls();
  renderDomainTable();
  renderCertificates();
  renderBlocklistSummary();
  renderPolicySummary();
  profileEngineField?.addEventListener("change", () => { markDirty(); renderSinglePageControls(); renderCertificateEngineGuard(); });
  singlePageModeField?.addEventListener("change", () => { markDirty(); renderSinglePageControls(); });
  passwordLockField?.addEventListener("change", () => { markDirty(); renderPasswordControls(); });
  panicFrameEnabledField?.addEventListener("change", () => { markDirty(); renderPanicColorControls(); });
  passwordToggleButton?.addEventListener("click", () => {
    const reveal = (passwordValueField?.type ?? "password") === "password";
    if (passwordValueField) passwordValueField.type = reveal ? "text" : "password";
    if (passwordConfirmField) passwordConfirmField.type = reveal ? "text" : "password";
  });
  overlay.querySelector("#profile-domain-add")?.addEventListener("click", () => {
    const domain = String(domainInput?.value ?? "").trim().toLowerCase();
    if (!domain) return;
    const type = domainTypeField?.value === "allow" ? "allow" : "deny";
    if (!/^[a-z0-9.-]+$/i.test(domain)) return;
    const existingIndex = domainTableState.findIndex((item) => item.domain === domain);
    if (existingIndex >= 0) domainTableState[existingIndex] = { domain, type };
    else domainTableState.push({ domain, type });
    syncDomainArrays();
    markDirty();
    if (domainInput) domainInput.value = "";
    renderDomainTable();
  });
  domainSearchField?.addEventListener("input", () => { domainUiState.search = domainSearchField.value; renderDomainTable(); });
  domainFilterField?.addEventListener("change", () => { domainUiState.filter = domainFilterField.value || "all"; renderDomainTable(); });
  overlay.querySelector("#profile-certificate-add")?.addEventListener("click", () => {
    const value = form.profileCertificateSelect.value;
    if (value && !certificateState.some((item) => item.kind === "id" && item.value === value)) {
      certificateState.push({ kind: "id", value });
      markDirty();
      renderCertificates();
      renderCertificateEngineGuard();
    }
  });
  overlay.querySelector("#profile-certificate-pick")?.addEventListener("click", async () => {
    const result = await pickCertificateFiles();
    if (!result.ok) {
      setNotice(model, "error", String(result.data?.error ?? "pick_certificate_files failed"));
      await rerender();
      return;
    }
    const files = Array.isArray(result.data) ? result.data : [];
    if (!files.length) return;
    const existingIds = new Set((globalSecurityRef.current.certificates ?? []).map((item) => String(item.id ?? "")));
    const existingPaths = new Set((globalSecurityRef.current.certificates ?? []).map((item) => String(item.path ?? "").trim().toLowerCase()));
    const addedIds = [];
    for (const filePath of files) {
      const clean = String(filePath ?? "").trim();
      if (!clean) continue;
      if (existingPaths.has(clean.toLowerCase()) || existingIds.has(slugId(clean))) continue;
      const id = makeUniqueId(clean, existingIds);
      existingIds.add(id);
      existingPaths.add(clean.toLowerCase());
      globalSecurityRef.current.certificates.push({ id, name: clean.split(/[/\\]/).pop()?.replace(/\.(pem|crt|cer)$/i, "") || clean, path: clean, issuerName: "", subjectName: "", applyGlobally: false, profileIds: [] });
      addedIds.push(id);
    }
    if (addedIds.length) {
      const saveResult = await saveGlobalSecuritySettings(buildGlobalSecuritySaveRequest(globalSecurityRef.current));
      if (!saveResult.ok) {
        setNotice(model, "error", String(saveResult.data?.error ?? "save_global_security_settings failed"));
        await rerender();
        return;
      }
      const refreshedSecurity = await getGlobalSecuritySettings();
      if (refreshedSecurity.ok) {
        try {
          globalSecurityRef.current = normalizeGlobalSecuritySettings(refreshedSecurity.data);
          if (profileCertificateSelectField) {
            profileCertificateSelectField.innerHTML = `<option value="">${t("security.selectCertificate")}</option>${(globalSecurityRef.current.certificates ?? []).map((item) => `<option value="${item.id}">${escapeHtml(item.name)}</option>`).join("")}`;
          }
        } catch {}
      }
      for (const id of addedIds) {
        if (!certificateState.some((item) => item.kind === "id" && item.value === id)) {
          certificateState.push({ kind: "id", value: id });
        }
      }
    }
    markDirty();
    renderCertificates();
    renderCertificateEngineGuard();
  });
  const { blocklistDropdown, blocklistMenu } = wireRouteNetworkSecurityBlocklists({
    overlay,
    t,
    blocklistState,
    globalActiveBlocklistIds,
    markDirty,
    renderBlocklistSummary
  });
  for (const field of overlay.querySelectorAll("input,select,textarea")) {
    field.addEventListener("change", () => markDirty());
  }
  return {
    allowState,
    denyState,
    certificateState,
    blocklistState,
    blocklistItems,
    initialSandboxMode,
    blocklistDropdown,
    blocklistMenu
  };
}
