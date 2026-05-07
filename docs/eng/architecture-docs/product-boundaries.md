---
title: Product Boundaries
sidebar_position: 3
---

Cerbena Browser is designed as a standalone browser launcher platform.

## In scope

- desktop launcher and profile lifecycle;
- profile isolation across data, network, extension state, and crypto context;
- adapters for `Chromium`, `Ungoogled Chromium`, and `LibreWolf`;
- per-profile `VPN` / `Proxy` / `TOR` / `DNS`;
- domain and service restrictions;
- local API and `MCP`;
- import/export, password lock, and `ephemeral` mode.

## Outside the baseline contour

- cloud multi-tenant control plane;
- mobile native clients;
- billing and organization management;
- plugin marketplace governance.
