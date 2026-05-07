# Internal Module Boundaries

This folder is intentionally split into focused modules.

- `profile` - Profile lifecycle and metadata management.
- `crypto` - Per-profile key management and encryption services.
- `network_policy` - Route/DNS/domain/service policy evaluation.
- `dns` - DNS resolution strategy and blocklist integration.
- `fingerprint` - Auto/manual identity payload generation and checks.
- `extensions` - Extension lifecycle and per-profile policy controls.
- `import_export` - Profile archive import/export and migration paths.
- `audit` - Audit event model, persistence, and query helpers.
- `api_local` - Local API endpoints and scoped authorization.
- `api_mcp` - MCP tool bindings and permission envelopes.
- `engine_chromium` - Chromium adapter implementation.
- `engine_librewolf` - LibreWolf adapter implementation.
- `sync_client` - Encrypted sync and backup client flows.
