---
title: Architecture
sidebar_position: 2
---

Cerbena Browser separates launcher logic, policy evaluation, browser adapters, local integrations, and the docs stack into explicit modules.

## Main contours

- `cmd/launcher`: CLI entrypoint and release-oriented helpers.
- `internal/profile`: lifecycle, storage, encryption, wipe, import/export.
- `internal/network_policy`: routing, DNS, service filtering, and validators.
- `internal/fingerprint`: identity presets and consistency checks.
- `internal/extensions`: extension library and policy hooks.
- `internal/api_local` and `internal/api_mcp`: local automation surfaces.
- `internal/sync_client`: snapshots, restore, and sync model.
- `ui/desktop/src-tauri`: desktop backend, runtime orchestration, and traffic gateway.
- `ui/desktop/web`: local frontend shell.

## Why this matters

- one contour can be hardened without hidden side effects in another;
- security checks remain backend-centric;
- documentation and tests map more cleanly to concrete contracts.
