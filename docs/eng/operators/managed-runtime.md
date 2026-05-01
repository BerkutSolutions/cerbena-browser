---
title: Managed Runtime
sidebar_position: 1
---

Cerbena Browser can provision and use managed runtime binaries for the network contour.

## What this includes

- browser runtimes for `Wayfern` and `Camoufox`;
- route backends `sing-box`, `openvpn`, `amneziawg`, and `tor`;
- local caching, integrity validation, and artifact reuse.

## Operator responsibilities

- verify binary provisioning completed successfully;
- track route runtime failures through operator/runtime diagnostics and per-profile runtime logs;
- understand which backend is selected for a given profile template.
