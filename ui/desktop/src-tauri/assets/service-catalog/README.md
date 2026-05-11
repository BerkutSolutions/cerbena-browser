# Service Catalog Asset Contract

Canonical path:
- `ui/desktop/src-tauri/assets/service-catalog/v1/catalog.json`

Versioning:
- top-level `version` is a required string;
- current supported value is `"1"`;
- incompatible schema changes must use a new versioned folder (`v2/`, `v3/`, ...).

Schema (`v1`):
- `categories[]`:
  - `id`: stable machine key (`[a-z0-9_+]`);
  - `labels.en` and `labels.ru`: required non-empty localized labels;
  - `services[]`.
- `services[]`:
  - `id`: stable machine key (`[a-z0-9_+]`), unique across the whole catalog;
  - `labels.en` and `labels.ru`: required non-empty localized labels;
  - `domains[]`: domain seeds owned by this service (must be normalized domain-like values, no schemes/paths/wildcards).

Validation invariants:
- non-empty categories and non-empty services per category;
- duplicate category IDs are forbidden;
- duplicate service IDs in category and across categories are forbidden;
- missing RU/EN labels are forbidden;
- malformed or duplicate domains per service are forbidden.

Update rules:
- keep changes data-only in `catalog.json` whenever possible;
- do not change loader/runtime contracts together with catalog content unless a schema/version migration is required;
- for schema changes, land parser/validator support first, then migrate data.
