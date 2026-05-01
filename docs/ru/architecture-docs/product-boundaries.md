---
title: Границы продукта
sidebar_position: 3
---

Cerbena Browser проектируется как standalone browser launcher platform.

## В scope

- desktop launcher и profile lifecycle;
- изоляция профилей по данным, сети, extension state и crypto context;
- adapters для `Wayfern` и `Camoufox`;
- per-profile `VPN` / `Proxy` / `TOR` / `DNS`;
- domain и service restrictions;
- local API и `MCP`;
- import/export, password lock и `ephemeral`-режим.

## Вне scope базового контура

- облачный multi-tenant control plane;
- мобильные native-клиенты;
- billing и organization management;
- marketplace governance.
