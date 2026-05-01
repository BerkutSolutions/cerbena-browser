---
title: Security Validation
sidebar_position: 4
---

This page is the operator-facing exit gate for the `TASKS4` hardening block behind `U14-2`.

It exists so Cerbena Browser does not over-claim security features based only on implementation notes or UI behavior.

## Validation matrix

| Area | Implemented controls | Automated evidence | Notes |
|---|---|---|---|
| Workspace/session isolation | launch-session broker, workspace fingerprint, session marker, trusted attach/reuse only | `ui/desktop/src-tauri/src/launch_sessions.rs` tests, `cargo test --workspace` | Negative coverage includes tampered marker and engine mismatch rejection. |
| Sensitive state at rest | encrypted launcher stores for sync, link routing, launch sessions, global security registry | `ui/desktop/src-tauri/src/sensitive_store.rs` tests, `ui/desktop/src-tauri/src/launcher_commands.rs` tests | Legacy plaintext migration is supported for readback only. |
| Protected-profile launch guards | unlock enforcement, system-access / KeePassXC rejection, extension limits for `maximum` policy | `ui/desktop/src-tauri/src/profile_security.rs` tests, `cargo test --workspace` | UI is not the security boundary; backend guards decide. |
| Cookie and sensitive-surface protection | cookie copy requires correct unlock state and rejects protected-profile participation | `ui/desktop/src-tauri/src/profile_commands.rs` tests | Unlocked non-protected profiles can still copy cookies by design. |
| Device posture reactions | backend host-signal scan with `allow / warn / confirm / refuse` | `ui/desktop/src-tauri/src/device_posture.rs` tests | Posture is advisory/hard-fail by policy, not anti-malware. |
| Protected snapshots | encrypted profile-scoped snapshot create/restore with integrity verification | `ui/desktop/src-tauri/src/sync_snapshots.rs` tests, sync command tests | Replaced placeholder `btoa("snapshot")` path. |
| Panic cleanup and protected sites | tracked-process stop, selective wipe targets, protected-sites path merge, exact Chromium/Firefox SQLite pruning, external overlay signaling | `internal/profile/src/storage.rs` tests, `internal/profile/tests/profile_manager_tests.rs`, `ui/desktop/src-tauri/src/launcher_commands.rs` tests | Exact retention now covers cookie/history SQLite stores for Chromium and Firefox-family engines. |

## Required release checks

Minimum release validation for this hardening block:

- `cargo test --workspace`
- `cd ui/desktop && npm test`
- `npm run docs:build`
- docs quality tests and launcher stability tests
- manual operator review of residual risks below before any security-positioned release note

## Residual risk and honest limits

Cerbena Browser now enforces a stronger launcher boundary, but it does not claim:

- host compromise immunity;
- kernel-level isolation similar to a hypervisor;
- perfect anti-injection guarantees once a hostile process already controls the user session;
- perfect forensic erasure guarantees for every browser-managed artifact outside the explicitly covered cookie/history SQLite stores.

## Operator expectations

- Treat panic cleanup as strong launcher-managed cleanup, not as forensic-grade secure erase.
- Treat device posture as a launch policy input, not as an endpoint detection product.
- Treat encrypted launcher state as protection against casual/local at-rest exposure, while still protecting the host and user account.
- Protected-site retention now applies exactly to Chromium/Firefox-family cookie and history SQLite stores; do not extend that claim to passwords, bookmark databases, or arbitrary extension-owned storage.

## Related pages

- [Security](../core-docs/security.md)
- [Release Gates](./release-gates.md)
- [Stability Validation](./stability-validation.md)
