---
title: Limitations
sidebar_position: 13
---

## Current limits

- parts of the runtime flow are optimized primarily for Windows hosts.
- the `desktop UI` and the `docs site` are built in separate tooling contours.
- route runtime depends on managed binaries being available locally.
- some `VPN`/`proxy` chains are limited by the supported backend conversions.

## Deliberate decisions

- `auto-update` is disabled by default;
- the UI does not pretend to be a security boundary;
- release gates favor verification and reproducibility over silent client-side magic.
