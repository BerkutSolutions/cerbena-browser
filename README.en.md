# Berkut Solutions - Cerbena Browser

<p align="center">
  <img src="static/img/logo.png" alt="Cerbena Browser logo" width="220">
</p>

[Russian version](README.md)

[GitHub](https://github.com/BerkutSolutions/cerbena-browser)
[Wiki](https://berkutsolutions.github.io/cerbena-browser/)

`Cerbena Browser` is a secure desktop browsing platform built around zero-trust enforcement, strong profile isolation, explicit traffic-isolation strategies, and a launcher-managed runtime boundary for `Wayfern` and `Camoufox`.

## Product Overview

Cerbena Browser is not a thin profile manager layered on top of a regular browser. It is a dedicated launcher and runtime boundary that manages:

- isolated profiles for `Wayfern` and `Camoufox`;
- per-profile route modes: `direct`, `proxy`, `vpn`, `tor`, and `hybrid`;
- explicit isolation strategies: `isolated`, `compatibility-native`, `container`, and `blocked`;
- DNS policies, blocklists, service restrictions, and domain blacklists;
- extension library assignment, install policy, and engine-aware auto-installation;
- identity templates and a full manual fingerprint editor;
- panic cleanup, custom certificates, local API, MCP, and audit flows;
- encrypted sync/backup workflows and local release/preflight validation.

The project provides:

- profile isolation across data, keys, extensions, cache, and network policy;
- a fail-closed kill-switch whenever a required VPN/runtime route is unavailable;
- container-backed routing for compatible templates so traffic can stay inside a profile-scoped sandbox instead of modifying host-wide networking;
- encrypted storage for sensitive launcher and desktop-shell state, including migration of legacy plaintext material into protected formats;
- end-to-end protection for sync payloads while preserving backward compatibility for previously created data;
- a trusted update path with a separate `cerbena-updater.exe`, `checksums.sig` signature validation, `SHA-256` verification, and safe installation handoff.

## Core Capabilities

- Full profile isolation across data, cache, keys, extensions, and network policy.
- Zero-trust backend enforcement: the UI is never a trust boundary.
- Explicit routing strategies with a container-backed route sandbox.
- Kill-switch behavior when a required VPN route is unavailable.
- Global and per-profile DNS filtering with editable policy levels.
- Realistic identity templates for Windows, macOS, Linux, iOS, and Android.
- Panic frame and emergency cleanup with managed retention controls.
- Extension library with profile assignment and engine-specific auto-install.
- Local release/preflight scripts plus security and vulnerability gates.
- Windows installer wizard with shortcuts, uninstall registration, and uninstaller flow that also removes launcher-managed network/container residue.

## Screenshots

### Home

![Home](static/img/screen-1.png)

### Extensions

![Extensions](static/img/screen-6.png)

### Profile and identity editing

![Profile and identity](static/img/screen-2.png)

### DNS policies and filters

![DNS](static/img/screen-3.png)

### Network templates and routing

![Network](static/img/screen-4.png)

### Extensions and security

![Extensions and security](static/img/screen-5.png)

## Technology Stack

- Desktop shell: `Tauri 2` + `Rust`
- Frontend: local `web UI`
- Workspace: `Cargo` multi-crate
- Documentation: `Docusaurus`
- Browser engines: `Wayfern`, `Camoufox`
- Managed runtime: `sing-box`, `openvpn`, `amneziawg`, `tor`, Docker-managed container helpers

## Quick Start

### Requirements

- `Rust` toolchain
- `Node.js` LTS + `npm`
- Windows as the primary desktop target
- `Docker Desktop` if you want to use container-backed traffic isolation

### Verification

```bash
cargo test --workspace
```

```bash
cd ui/desktop
npm ci
npm test
```

```bash
npm ci
npm run docs:build
```

### Run the desktop UI

```bash
cd ui/desktop
npm run dev
```

## Release and installer

### Local preflight

```powershell
powershell -ExecutionPolicy Bypass -File .\scripts\local-ci-preflight.ps1 -CompactOutput
```

This preflight now includes the Docker runtime gate, security gate, and vulnerability gate by default.

### Release validation and packaging

```powershell
powershell -ExecutionPolicy Bypass -File .\scripts\release.ps1 -Mode package -CompactOutput
```

### Build the installer

```powershell
powershell -ExecutionPolicy Bypass -File .\scripts\build-installer.ps1
```

### What to upload to GitHub Releases

GitHub Releases should typically include:

- `cerbena-browser-setup-<version>.exe` as the primary Windows installer;
- `cerbena-windows-x64.zip` as the portable release bundle when needed;
- `cerbena-updater.exe` as the standalone updater executable;
- `checksums.txt`, `checksums.sig`, and `release-manifest.json` as trusted release metadata artifacts.

The installer `.exe` is produced locally by `scripts/build-installer.ps1` and is intended to be the main installation asset attached to releases. The same pipeline is expected to clean launcher-managed containers, Docker networks, helper images, managed runtimes, and legacy route-service residue during uninstall.

## Documentation

- Documentation index: [docs/README.md](docs/README.md)
- Russian wiki: [docs/ru/README.md](docs/ru/README.md)
- English wiki: [docs/eng/README.md](docs/eng/README.md)
- UI and workflows: [docs/eng/core-docs/ui.md](docs/eng/core-docs/ui.md)
- Network and routing: [docs/eng/core-docs/network-routing.md](docs/eng/core-docs/network-routing.md)
- DNS and filters: [docs/eng/core-docs/dns-and-filters.md](docs/eng/core-docs/dns-and-filters.md)
- Security: [docs/eng/core-docs/security.md](docs/eng/core-docs/security.md)
- Release runbook: [docs/eng/release-runbook.md](docs/eng/release-runbook.md)

## Helpful Files

- Contribution guide: [CONTRIBUTING.md](CONTRIBUTING.md)
- Security policy: [SECURITY.md](SECURITY.md)
- Support channels: [SUPPORT.md](SUPPORT.md)
- Change history: [CHANGELOG.md](CHANGELOG.md)
