# Cerbena Browser Documentation

Version baseline: `repository snapshot 2026-05-11`

This directory contains the release-synced bilingual Cerbena Browser documentation set. The Russian and English branches mirror the same product map and are maintained together as one wiki for architecture, UI workflows, networking, DNS filtering, integrations, installer/release operations, and troubleshooting.

## Main entry points

- GitHub repository: `https://github.com/BerkutSolutions/cerbena-browser`
- Docs portal home: `/`
- Russian wiki: `docs/ru/README.md`
- English wiki: `docs/eng/README.md`
- Root product overview: `README.md`
- English root overview: `README.en.md`
- Docusaurus site roots: `/ru/` for RU and `/en/` for EN

## Russian wiki

- Entry point: `docs/ru/README.md`
- Navigator: `docs/ru/navigator.md`
- UI and workflows: `docs/ru/core-docs/ui.md`
- Profiles and lifecycle: `docs/ru/core-docs/profiles.md`
- Identity and fingerprint: `docs/ru/core-docs/identity.md`
- Extensions: `docs/ru/core-docs/extensions.md`
- DNS and filters: `docs/ru/core-docs/dns-and-filters.md`
- Sync and backups: `docs/ru/core-docs/sync-and-backups.md`
- Security model: `docs/ru/core-docs/security.md`
- Architecture: `docs/ru/architecture-docs/architecture.md`
- Release runbook: `docs/ru/release-runbook.md`
- Security validation: `docs/ru/operators/security-validation.md`

## English wiki

- Entry point: `docs/eng/README.md`
- Navigator: `docs/eng/navigator.md`
- UI and workflows: `docs/eng/core-docs/ui.md`
- Profiles and lifecycle: `docs/eng/core-docs/profiles.md`
- Identity and fingerprint: `docs/eng/core-docs/identity.md`
- Extensions: `docs/eng/core-docs/extensions.md`
- DNS and filters: `docs/eng/core-docs/dns-and-filters.md`
- Sync and backups: `docs/eng/core-docs/sync-and-backups.md`
- Security model: `docs/eng/core-docs/security.md`
- Architecture: `docs/eng/architecture-docs/architecture.md`
- Release runbook: `docs/eng/release-runbook.md`
- Security validation: `docs/eng/operators/security-validation.md`

## What this wiki covers

- Real desktop shell sections: `Home`, `Extensions`, `Security`, `Identity`, `DNS`, `Network`, `Traffic`, and `Settings`.
- `Home` as the main operational surface for profile creation, launch, stop, import/export, bulk actions, and dashboard metrics.
- `Settings` as the home for `General`, `Links`, `Sync`, and update controls.
- Profile-modal editing with the left-side section rail and synchronized `Identity`, `VPN`, `DNS`, `Extensions`, `Security`, and `Sync` behavior.
- Zero-trust backend enforcement and profile isolation contracts.
- Managed route runtime and traffic gateway behavior.
- Service filtering, blocklists, domain restrictions, and external-link routing.
- Launcher CLI, local API, MCP, sync controls, installer packaging, and release gates.
- Trusted release delivery with signed checksums, a standalone updater, backward-compatible sync encryption, and desktop-shell secret protection.
- Current hardening and regression safety expectations through stages `TASKS11`-`TASKS17` (decomposition closure, shell resilience, strengthened web quality gates, and release/preflight safety).

## Quality expectations

- RU and EN trees must keep matching page sets.
- Russian wiki pages should remain fully Russian except for approved product and protocol keywords such as `TLS`, `DNS`, `Cloudflare`, and similar technical names.
- Documentation changes are covered by Rust repo tests, desktop web quality gates (`npm test` in `ui/desktop`), and the local preflight script: `scripts/local-ci-preflight.ps1`.
