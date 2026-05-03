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
- `Network`: connection templates, route mode, global VPN policy, global traffic-isolation policy, and node health checks.
- `Traffic`: gateway decisions, manual domain blocks, and route diagnostics.
- `Settings`: global shell settings with `General`, `Links`, `Sync`, and update controls.
- `Updater`: a separate `cerbena-updater.exe` window for trusted update flow, preview checks, and safe installation handoff.

## Built-in defaults and links

- `Home` starts with six built-in profiles: `Chromium Default`, `Firefox Default`, `Chromium Private Memory`, `Firefox Private Memory`, `Discord`, and `Telegram`.
- `Discord` and `Telegram` are strict `Wayfern` app-window profiles with fixed allowlists and without a free address bar.
- The profile modal `General` tab also supports a `Single-page` checkbox for custom strict `Wayfern` app-window profiles. When enabled, the start page stays mandatory and the default search-provider selector is hidden.
- `Settings > Links` supports routing for `HTTP`, `HTTPS`, `FTP`, `MAILTO`, `IRC`, `MMS`, `NEWS`, `NNTP`, `SMS`, `SMSTO`, `SNEWS`, `TEL`, `URN`, `WEBCAL`, `TG`, `Discord`, `Slack`, `Zoom`, plus file associations for `.htm`, `.html`, `.shtml`, `.mht`, `.mhtml`, `.pdf`, `.svg`, `.xht`, `.xhtml`, and `.xhy`.
- Only `HTTP` and `HTTPS` inherit the global default profile automatically. Other supported types require an explicit binding.

## Key UX principles

- Every sensitive action is backed by backend validation.
- Profile data is edited through a modal with a left-side vertical rail for `General`, `Identity`, `VPN`, `DNS`, `Extensions`, `Security`, `Sync`, and `Advanced`.
- The main `Network` screen owns global route templates and the global `Сетевая изоляция профилей` frame. That frame only appears when global VPN is enabled, because only then does the launcher have an active global route to evaluate.
- The profile modal `VPN` tab owns profile-specific route selection and the `Сетевая изоляция профиля` frame. That frame is route-aware and only offers isolation modes that are actually compatible with the selected template.
- `Traffic` is now a denser operational table: the request column is intentionally wider, the result column is more compact, and decision/action overlays render above the rest of the UI instead of being clipped by table rows.
- Long profile launches show a temporary progress modal so users can see when launcher is preparing the profile, route runtime, container sandbox, helper image, or browser engine.
- Network and runtime statuses are not considered valid without backend verification.

## Typical workflow

1. Create a profile in `Home`.
2. Open the profile modal and configure `Identity`, `VPN`, `DNS`, `Extensions`, and `Security`.
3. If needed, configure the global default route and global isolation policy in `Network`.
4. Review `Settings` for default search/start page, links, sync, and update behavior.
5. Launch the profile and inspect `Traffic` if a route is blocked or slow.

## Where to look when something fails

- `Traffic` for explicit block reasons, route latency, and kill-switch decisions.
- `Network` when the issue is related to templates, isolation strategy, container capacity, or helper availability.
- `DNS` when the issue is related to policy levels, blocklists, denylist, or service restrictions.
- `Settings > Sync` for synchronization, endpoint health, and snapshot issues.
- `Updater` when the issue is related to release validation, trusted download, checksum comparison, or `already up to date` behavior.
- operator/runtime diagnostics for provisioning, installer, engine download, or route-runtime failures.
