---
title: Managed Runtime
sidebar_position: 1
---

Cerbena Browser can provision and use managed runtime binaries for the network contour and for container-backed traffic isolation.

## What this includes

- browser runtimes for `Chromium`, `Ungoogled Chromium`, and `LibreWolf`;
- route backends `sing-box`, `openvpn`, `amneziawg`, and `tor`;
- container helper assets for profile-scoped route isolation;
- local caching, integrity validation, and artifact reuse.


## Operator responsibilities

- verify that binary/runtime provisioning completed successfully;
- track route runtime failures through operator/runtime diagnostics and per-profile runtime logs;
- understand which backend and which isolation strategy were selected for a given profile template;
- verify that container-backed route helpers, dedicated Docker networks, and helper-image cleanup stay aligned with launcher uninstall behavior.
- verify that uninstall/janitor cleanup leaves no launcher-managed residue from retired browser/runtime artifacts and only manages current `chromium`/`ungoogled-chromium`/`librewolf` paths.

## Runtime-specific notes

- The container helper image is launcher-managed and built on first use.
- The current helper revision is `2026-05-02-r5`.
- For `VLESS Reality` and other `uTLS`-dependent transports, the helper image now ships a `sing-box` build compiled with `with_utls`.
- For native-only `AmneziaWG`, Cerbena keeps compatibility inside the per-profile container through `amneziawg-go`, `awg-quick`, and a local `SOCKS` endpoint instead of requiring a host-wide VPN session.
