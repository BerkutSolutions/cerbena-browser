# English wiki

This page belongs to the current Cerbena Browser documentation branch and serves as a human-readable entry point outside the live Docusaurus navigation tree.

## Quick links

- GitHub repository: `https://github.com/BerkutSolutions/cerbena-browser`
- Docs portal home: `/`
- Russian live branch: `/ru/`
- English live branch: `/en/`

## What the English wiki contains

- product overview and reading routes;
- documentation for the real desktop UI sections;
- documents for profiles, networking, DNS, filters, extensions, `Links`, `Sync`, and updates;
- material for trusted update delivery, signed checksums, the standalone updater, and protected desktop-shell state;
- architecture contracts for profile isolation and zero-trust;
- operator material for managed runtime, quality gates, and stability;
- release runbook, installer packaging, and troubleshooting.

## Where to start

1. Read the [main index](index.md).
2. Open the [Navigator](navigator.md) when you need a role-based reading path.
3. For the interface, go to [UI and workflows](core-docs/ui.md).
4. For the network boundary, read [Routing and route runtime](core-docs/network-routing.md) and [DNS, blocklists, and service filters](core-docs/dns-and-filters.md).
5. For the architecture baseline, use [Architecture](architecture-docs/architecture.md), [Profile isolation](architecture-docs/profile-isolation.md), and [Zero-trust](architecture-docs/zero-trust.md).

## Main branches

- Architecture: `docs/eng/architecture-docs/*`
- Core documents: `docs/eng/core-docs/*`
- Operator material: `docs/eng/operators/*`
- Integrations and tooling: `docs/eng/integration-docs/*`
- Release and recovery: `docs/eng/release-runbook.md`, `docs/eng/release-troubleshooting.md`

## Synchronization rule

- The Russian and English branches must keep the same page set.
- Technical keywords such as `TLS`, `DNS`, `Cloudflare`, `Wayfern`, `LibreWolf`, and `MCP` are preserved consistently across both branches.
- Every new user-facing capability should be reflected in both `docs/ru` and `docs/eng`.
