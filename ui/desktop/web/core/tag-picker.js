function escapeHtml(value) {
  return String(value ?? "")
    .replaceAll("&", "&amp;")
    .replaceAll("<", "&lt;")
    .replaceAll(">", "&gt;")
    .replaceAll("\"", "&quot;");
}

function normalizeTagValue(value) {
  return String(value ?? "").trim().toLocaleLowerCase();
}

export function uniqueTags(values) {
  const result = [];
  const seen = new Set();
  for (const value of values ?? []) {
    const trimmed = String(value ?? "").trim();
    if (!trimmed) continue;
    const normalized = normalizeTagValue(trimmed);
    if (seen.has(normalized)) continue;
    seen.add(normalized);
    result.push(trimmed);
  }
  return result.sort((left, right) => left.localeCompare(right, undefined, { sensitivity: "base" }));
}

export function collectTagOptions(items, readTags) {
  const result = [];
  for (const item of items ?? []) {
    result.push(...(readTags(item) ?? []));
  }
  return uniqueTags(result);
}

export function tagSummary(tags, fallback) {
  if (!tags.length) return fallback;
  if (tags.length === 1) return tags[0];
  return `${tags[0]} +${tags.length - 1}`;
}

export function buildTagPickerMarkup({
  id,
  selectedTags = [],
  availableTags = [],
  toggleLabel,
  searchPlaceholder,
  emptyLabel,
  allowCreate = true
}) {
  const normalizedSelected = uniqueTags(selectedTags);
  const normalizedAvailable = uniqueTags([...availableTags, ...normalizedSelected]);
  return `
    <div class="dns-dropdown tag-picker" data-tag-picker="${escapeHtml(id)}">
      <button type="button" class="dns-dropdown-toggle tag-picker-toggle" data-tag-picker-toggle="${escapeHtml(id)}">
        <span class="tag-picker-summary" data-tag-picker-summary="${escapeHtml(id)}">${escapeHtml(toggleLabel ?? tagSummary(normalizedSelected, emptyLabel))}</span>
      </button>
      <div class="dns-dropdown-menu hidden tag-picker-menu" data-tag-picker-menu="${escapeHtml(id)}">
        <input
          type="text"
          class="tag-picker-search"
          data-tag-picker-search="${escapeHtml(id)}"
          placeholder="${escapeHtml(searchPlaceholder)}"
          autocomplete="off"
          autocapitalize="off"
          spellcheck="false"
        />
        <div class="tag-picker-selected ${normalizedSelected.length ? "" : "hidden"}" data-tag-picker-selected="${escapeHtml(id)}"></div>
        <button type="button" class="tag-picker-create hidden" data-tag-picker-create="${escapeHtml(id)}"></button>
        <div class="tag-picker-options" data-tag-picker-options="${escapeHtml(id)}">
          ${normalizedAvailable.map((tag) => `
            <label class="tag-picker-option" data-tag-picker-option="${escapeHtml(id)}" data-tag-value="${escapeHtml(tag)}">
              <input type="checkbox" data-tag-picker-checkbox="${escapeHtml(id)}" value="${escapeHtml(tag)}" ${normalizedSelected.some((item) => normalizeTagValue(item) === normalizeTagValue(tag)) ? "checked" : ""} />
              <span>${escapeHtml(tag)}</span>
            </label>
          `).join("") || `<div class="meta">${escapeHtml(emptyLabel)}</div>`}
        </div>
      </div>
    </div>
  `;
}

export function wireTagPicker(root, config) {
  const {
    id,
    state,
    emptyLabel,
    searchPlaceholder,
    createLabel,
    allowCreate = true,
    onChange
  } = config;
  const wrapper = root.querySelector(`[data-tag-picker='${id}']`);
  if (!wrapper) return null;
  const toggle = wrapper.querySelector(`[data-tag-picker-toggle='${id}']`);
  const menu = wrapper.querySelector(`[data-tag-picker-menu='${id}']`);
  const searchInput = wrapper.querySelector(`[data-tag-picker-search='${id}']`);
  const summary = wrapper.querySelector(`[data-tag-picker-summary='${id}']`);
  const selectedContainer = wrapper.querySelector(`[data-tag-picker-selected='${id}']`);
  const createButton = wrapper.querySelector(`[data-tag-picker-create='${id}']`);
  const optionsContainer = wrapper.querySelector(`[data-tag-picker-options='${id}']`);

  const ensureState = () => {
    state.selected = uniqueTags(state.selected ?? []);
    state.available = uniqueTags([...(state.available ?? []), ...state.selected]);
  };

  const isSelected = (tag) => (state.selected ?? []).some((item) => normalizeTagValue(item) === normalizeTagValue(tag));

  const setSelected = (tag, checked) => {
    const normalized = normalizeTagValue(tag);
    const canonical = (state.available ?? []).find((item) => normalizeTagValue(item) === normalized) ?? String(tag).trim();
    if (!canonical) return;
    if (checked) {
      if (!isSelected(canonical)) {
        state.selected = [...(state.selected ?? []), canonical];
      }
      if (!(state.available ?? []).some((item) => normalizeTagValue(item) === normalized)) {
        state.available = [...(state.available ?? []), canonical];
      }
    } else {
      state.selected = (state.selected ?? []).filter((item) => normalizeTagValue(item) !== normalized);
    }
    ensureState();
    render();
    onChange?.(state.selected);
  };

  const renderSelected = () => {
    const selected = uniqueTags(state.selected ?? []);
    if (!selected.length) {
      selectedContainer?.classList.add("hidden");
      if (selectedContainer) selectedContainer.innerHTML = "";
      return;
    }
    selectedContainer?.classList.remove("hidden");
    if (selectedContainer) {
      selectedContainer.innerHTML = selected.map((tag) => `
        <span class="profiles-tag">
          ${escapeHtml(tag)}
          <button type="button" class="profiles-tag-remove" data-tag-picker-remove="${escapeHtml(id)}" data-tag-value="${escapeHtml(tag)}" aria-label="remove">x</button>
        </span>
      `).join("");
      for (const button of selectedContainer.querySelectorAll(`[data-tag-picker-remove='${id}']`)) {
        button.addEventListener("click", () => {
          setSelected(button.getAttribute("data-tag-value"), false);
          searchInput?.focus();
        });
      }
    }
  };

  const renderOptions = () => {
    const query = String(searchInput?.value ?? "").trim().toLocaleLowerCase();
    const options = uniqueTags(state.available ?? []);
    const visible = options.filter((tag) => !query || tag.toLocaleLowerCase().includes(query));
    if (optionsContainer) {
      optionsContainer.innerHTML = visible.map((tag) => `
        <label class="tag-picker-option" data-tag-picker-option="${escapeHtml(id)}" data-tag-value="${escapeHtml(tag)}">
          <input type="checkbox" data-tag-picker-checkbox="${escapeHtml(id)}" value="${escapeHtml(tag)}" ${isSelected(tag) ? "checked" : ""} />
          <span>${escapeHtml(tag)}</span>
        </label>
      `).join("") || `<div class="meta">${escapeHtml(emptyLabel)}</div>`;
      for (const checkbox of optionsContainer.querySelectorAll(`[data-tag-picker-checkbox='${id}']`)) {
        checkbox.addEventListener("change", () => {
          setSelected(checkbox.value, checkbox.checked);
          searchInput?.focus();
        });
      }
    }
    const canCreate = allowCreate
      && Boolean(query)
      && !options.some((tag) => normalizeTagValue(tag) === normalizeTagValue(query));
    if (createButton) {
      createButton.classList.toggle("hidden", !canCreate);
      createButton.textContent = canCreate
        ? (createLabel ? createLabel(query) : query)
        : "";
    }
  };

  const renderSummary = () => {
    if (summary) {
      summary.textContent = tagSummary(uniqueTags(state.selected ?? []), emptyLabel);
    }
  };

  const render = () => {
    ensureState();
    renderSummary();
    renderSelected();
    renderOptions();
  };

  const createCurrentTag = () => {
    const value = String(searchInput?.value ?? "").trim();
    if (!value) return;
    setSelected(value, true);
    if (searchInput) {
      searchInput.value = "";
    }
    render();
    searchInput?.focus();
  };

  toggle?.addEventListener("click", (event) => {
    event.preventDefault();
    event.stopPropagation();
    menu?.classList.toggle("hidden");
    if (!menu?.classList.contains("hidden")) {
      searchInput?.focus();
      searchInput?.select();
    }
  });
  menu?.addEventListener("click", (event) => {
    event.stopPropagation();
  });
  searchInput?.addEventListener("input", () => {
    renderOptions();
  });
  searchInput?.addEventListener("keydown", (event) => {
    if (event.key !== "Enter") return;
    if (!allowCreate) return;
    event.preventDefault();
    createCurrentTag();
  });
  createButton?.addEventListener("mousedown", (event) => {
    event.preventDefault();
  });
  createButton?.addEventListener("click", () => {
    createCurrentTag();
  });

  render();

  return {
    close() {
      menu?.classList.add("hidden");
    },
    rerender(nextAvailable, nextSelected = state.selected) {
      state.available = uniqueTags(nextAvailable ?? []);
      state.selected = uniqueTags(nextSelected ?? []);
      render();
    },
    focus() {
      searchInput?.focus();
    }
  };
}
