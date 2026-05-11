# Text Integrity Gate

Purpose:
- detect tracked text artifacts with encoding corruption or mojibake signatures before they reach CI/release flows.

Entrypoint:
- `node scripts/check-text-integrity.mjs`
- npm alias: `npm run text:check`

Scope source:
- `scripts/text-integrity.config.json`

Configuration rules:
- `text_roots`: explicit tracked roots/files that are scanned as text.
- `exclude_prefixes`: deterministic skips (build caches, dependency dirs, generated targets).
- `binary_extensions`: hard binary boundary to avoid false positives.
- `mojibake_patterns`: suspicious signature set.
- `mojibake_allowlist_patterns`: narrow path allowlist for exceptional files.

Allowlist process:
1. Prefer fixing source data over allowlisting.
2. If allowlist is required, add a path-scoped regex in `mojibake_allowlist_patterns`.
3. Keep allowlist entries minimal and reviewable; broad wildcard entries are forbidden.
4. Mention every new allowlist entry in `CHANGELOG.md` and `.work/REQUIREMENTS_TRACEABILITY.md`.
