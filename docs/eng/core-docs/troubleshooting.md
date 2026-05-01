---
title: Troubleshooting
sidebar_position: 12
---

## The profile does not launch

- verify `Wayfern ToS` acknowledgement;
- verify the binary path exists;
- check operator/runtime diagnostics for provisioning or startup failures.

## Traffic is blocked unexpectedly

- open `Traffic` and inspect the reason;
- verify kill-switch and global `block without VPN`;
- inspect `selected_services`, blocklists, and domain deny rules.

## Route runtime does not start

- determine which backend is required: `sing-box`, `openvpn`, `amneziawg`, or `tor`;
- check whether managed provisioning completed;
- inspect per-profile runtime logs.

## Sync behaves unexpectedly

- open `Settings > Sync`;
- inspect the current status and endpoint health;
- confirm snapshots exist before attempting restore.

## Localization looks broken

- run `cd ui/desktop && npm run i18n:check`;
- verify `ru` / `en` key parity;
- check for mojibake in Russian strings.
