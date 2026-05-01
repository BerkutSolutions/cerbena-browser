---
id: index
title: English Wiki
sidebar_position: 1
---

Cerbena Browser is a standalone secure-browsing platform with isolated profiles, a strict network policy engine, and a desktop launcher built on `Tauri 2` + `Rust`.

This wiki documents the real repository surface rather than an abstract architecture only: `Profiles`, `Identity`, `Network`, `DNS`, `Extensions`, `Security`, `Traffic`, `Settings`, the release flow, and local integrations.

## Who this documentation is for

- Engineers who configure profiles, routes, blocklists, and DNS policies.
- Security teams that need zero-trust and profile isolation guarantees.
- Developers who maintain the launcher, the docs site, and local integration surfaces.

## What matters immediately

- The `UI` is not a trust boundary.
- Profile data, keys, network state, cache, and extension state are isolated.
- `Kill-switch` blocks traffic when a required VPN/runtime route is unavailable.
- `Auto-update` is disabled by default.
- `Settings` is now the shared home for `General`, `Links`, and `Sync`.
- Documentation is maintained as synchronized `ru` and `eng` branches.

## Main wiki branches

- [Navigator](navigator.md)
- [UI and workflows](core-docs/ui.md)
- [Profiles and lifecycle](core-docs/profiles.md)
- [Routing and route runtime](core-docs/network-routing.md)
- [DNS, blocklists, and service filters](core-docs/dns-and-filters.md)
- [Identity and fingerprint](core-docs/identity.md)
- [Extensions](core-docs/extensions.md)
- [Sync, snapshots, and restore](core-docs/sync-and-backups.md)
- [Local API and MCP](core-docs/api.md)
- [Security](core-docs/security.md)
- [Architecture](architecture-docs/architecture.md)
- [Release runbook](release-runbook.md)

## Recommended reading paths

### Quick orientation

1. [Navigator](navigator.md)
2. [UI and workflows](core-docs/ui.md)
3. [Architecture](architecture-docs/architecture.md)

### Networking and restrictions

1. [Routing and route runtime](core-docs/network-routing.md)
2. [DNS, blocklists, and service filters](core-docs/dns-and-filters.md)
3. [Network policy](architecture-docs/policy-engine.md)

### Release and support

1. [Stability validation](operators/stability-validation.md)
2. [Release gates](operators/release-gates.md)
3. [Release runbook](release-runbook.md)
4. [Release troubleshooting](release-troubleshooting.md)
