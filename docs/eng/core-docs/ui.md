---
title: UI and Workflows
sidebar_position: 1
---

The Cerbena Browser desktop UI is a local shell on top of `Tauri` backend commands. It is not treated as a trust boundary, but it mirrors the real state of profiles, network policies, runtime processes, and protection controls.

## Real shell sections

- `Home`: the main operational surface for profile creation, bulk actions, launch, stop, import, export, summary metrics, and status.
- `Extensions`: extension library, import from store links or archives, profile assignment, and engine compatibility.
- `Security`: root certificates and global shell security controls.
- `Identity`: `Automatic` and `Manual` identity modes, generation, and templates.
- `DNS`: DNS modes, policy-level table/editor, blocklists, service catalog, suffix bans, and domain rules.
- `Network`: connection templates, route mode, global VPN policy, and node health checks.
- `Traffic`: gateway decisions and manual domain blocks.
- `Settings`: global shell settings with `General`, `Links`, `Sync`, and update controls.
- `Updater`: a separate `cerbena-updater.exe` window for trusted update flow, preview checks, and safe installation handoff.

## Key UX principles

- Every sensitive action is backed by backend validation.
- Profile data is edited through a modal with a left-side vertical section rail for `General`, `Identity`, `VPN`, `DNS`, `Extensions`, `Security`, `Sync`, and `Advanced`.
- `Home` replaces the old standalone profiles tab and now hosts the main profile lifecycle UI.
- `Settings` centralizes default search/start page, external-link routing, sync operations, and update policy controls.
- When an update is detected, the launcher hands control to the standalone updater instead of performing UI-critical installation inside the main browser shell.
- `Identity` in `Automatic` mode hides manual fields and regenerates a realistic platform-scoped identity on each session.
- `Identity` in `Manual` mode exposes generation and template tools that populate real editable values.
- Network and runtime statuses are not considered valid without backend verification.

## Typical workflow

1. Create a profile in `Home`.
2. Configure `Identity`, `Network`, and `DNS`.
3. Import and assign extensions where needed.
4. Review global `Links`, `Sync`, and update behavior in `Settings`.
5. Launch the profile and watch `Traffic`.

## Where to look when something fails

- `Traffic` for explicit block reasons.
- `DNS` when the issue is related to policy levels, blocklists, denylist, or service restrictions.
- `Settings > Sync` for synchronization, endpoint health, and snapshot issues.
- `Updater` when the issue is related to release validation, trusted download, checksum comparison, or "already up to date" behavior.
- operator/runtime diagnostics for provisioning, installer, engine download, or route-runtime failures.
