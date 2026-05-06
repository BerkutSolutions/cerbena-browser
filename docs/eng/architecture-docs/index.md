---
title: Architecture Section
sidebar_position: 1
---

The architecture branch connects the `.work` contracts to the real repository code and explains why Cerbena Browser is organized as isolated modules instead of a monolith.

Start with:

1. [Architecture](architecture.md)
2. [Product boundaries](product-boundaries.md)
3. [Profile isolation](profile-isolation.md)
4. [Network policy](policy-engine.md)
5. [Zero-trust and backend enforcement](zero-trust.md)

## Where architecture lives in code

- Desktop shell and command surface: `ui/desktop/src-tauri/src/main.rs`, `commands.rs`, `launcher_commands.rs`.
- Profile domain and isolation internals: `internal/profile/src/*`.
- Engine lifecycle and launch contracts: `internal/engine/src/*`.
- Network policy, DNS policy, and route modes: `internal/network_policy/src/*`, `ui/desktop/src-tauri/src/network_commands.rs`, `route_runtime.rs`.
- Extension policy and import flows: `internal/extensions/src/*`, `ui/desktop/src-tauri/src/extensions_commands.rs`.
- Local API and MCP backend modules: `internal/api_local/src/*`, `internal/api_mcp/src/*`.
- Sync and encrypted snapshots: `internal/sync_client/src/*`, `ui/desktop/src-tauri/src/sync_commands.rs`, `sync_snapshots.rs`.

## Architectural invariants used by the project

1. UI is not a trust boundary: sensitive decisions are validated in backend command handlers.
2. Profile-scoped state: launcher data is persisted via `state.rs` and profile internals, not a shared cross-profile secret store.
3. Fail-closed networking: required route/runtime failures block traffic instead of falling back silently.
4. Release contract before convenience: updater/release logic is validated through contract tests and local preflight gates.

## Validation checkpoints

- Rust workspace checks: `cargo test --workspace`.
- Launcher contract tests: `cmd/launcher/tests/*`.
- Desktop checks: `ui/desktop/scripts/check-i18n.mjs`, `ui/desktop/scripts/ui-smoke-test.mjs`.
- Release/operator checks: `scripts/local-ci-preflight.ps1`, `scripts/release.ps1`.
