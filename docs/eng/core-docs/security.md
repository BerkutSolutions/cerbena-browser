---
title: Security
sidebar_position: 9
---

Cerbena Browser security is built around zero-trust, profile isolation, and deny-by-default network behavior.

## Baseline guarantees

- the project provides backend-centric trust for critical operations rather than UI-only restrictions;
- the project provides profile isolation across data, keys, extensions, network scope, and audit state;
- the project provides a trusted update contour: `auto-update` remains disabled by default, and the updater runs outside the main browser shell with signature and checksum verification when explicitly launched;
- the backend verifies every critical action;
- profile-to-profile access is denied by default;
- route policy and DNS policy are applied in profile scope;
- an audit trail records sensitive operations;
- `auto-update` is disabled by default.

## Key mechanisms

- profile password lock;
- `ephemeral` mode;
- per-profile encryption at rest;
- encrypted storage for sensitive launcher state such as `Sync`, link-routing, and launch-session broker data;
- app-scoped desktop-shell secret protection through Windows DPAPI, with legacy plaintext data migrated into encrypted envelopes after successful readback;
- protected desktop sync snapshots that now capture real profile-scoped restore material instead of demo payloads, with encrypted backup blobs and integrity-checked restore;
- a sync E2E path with random data keys, AEAD key wrapping, and versioned fallback decryption for backward compatibility with earlier envelopes;
- encrypted storage for global shell security state such as startup page, managed certificates, and DNS blocklist registry instead of plaintext `_global-security.json`;
- protected-profile launch posture that requires unlocked state, blocks unsafe `system access` / `KeePassXC` combinations, forbids `maximum` policy launches with active extensions, and disables direct cookie-copy flows into protected profiles;
- a device-posture pipeline that checks host signals before protected launch and can warn, require confirmation, or refuse launch based on profile policy and host risk severity;
- a launch-session broker with a workspace marker file so the launcher does not trust any process solely because it points at the same profile directory;
- kill-switch;
- service and domain filtering;
- panic wipe and selective wipe;
- a panic cleanup control with protected-sites retention policy, exact per-domain retention for Chromium/Firefox-family cookie/history SQLite stores, and a Windows external overlay border for isolated browser windows;
- a standalone `cerbena-updater.exe` with its own window, localized step pipeline, and preview dry-run mode that does not install files;
- verification of `checksums.txt`, the detached `checksums.sig`, and release-asset `SHA-256` before staging or installation handoff;
- fail-closed intake for curated DNS blocklists over safe schemes with optional `SHA-256` pinning;
- safe extraction rules for managed runtime archives that reject traversal paths and archive links.

## Network risks addressed by the product

- bypassing a required VPN route;
- DNS leaks;
- mixing policy state across profiles;
- applying global blocks without visible explanation.

## Related documents

- [Profile isolation](../architecture-docs/profile-isolation.md)
- [Network policy](../architecture-docs/policy-engine.md)
- [Security validation](../operators/security-validation.md)
- [Release gates](../operators/release-gates.md)
