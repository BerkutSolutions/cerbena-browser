---
title: Limitations
sidebar_position: 13
---

## Current limits

- parts of the runtime flow are optimized primarily for Windows hosts, including installer/update paths.
- the `desktop UI` and the `docs site` are built in separate tooling contours and release with different pipelines.
- route runtime depends on managed binaries and container/toolchain availability on the local machine.
- some `VPN`/`proxy` chains are constrained by supported conversion/normalization logic in backend runtime adapters.
- release troubleshooting and preflight diagnostics assume local access to PowerShell scripts in `scripts/*`.

## Deliberate decisions

- `auto-update` is disabled by default;
- the UI does not pretend to be a security boundary;
- release gates favor verification and reproducibility over silent client-side magic.

## Operational constraints you should plan for

1. Managed network dependencies (`openvpn`, `tor`, container helpers) must be present and healthy before profile routing can be considered stable.
2. Profile runtime behavior can differ between dev mode and installed mode because storage roots and process wiring are intentionally isolated.
3. Release validation is intentionally strict: documentation parity, i18n checks, launcher contract tests, and updater trust checks can block packaging.
4. Some diagnostics are designed for local operators first (`scripts/local-ci-preflight.ps1`, `scripts/release.ps1`) and are not intended to be hardwired into GitHub mandatory CI contracts.
