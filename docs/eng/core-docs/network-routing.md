---
title: Routing and Route Runtime
sidebar_position: 3
---

Cerbena Browser networking is built around per-profile route policy, a local traffic gateway, and a managed route runtime.

## Supported modes

- `direct`
- `proxy`
- `vpn`
- `tor`
- `hybrid`

Global `global VPN` and `block without VPN` settings are also available.

## Connection templates

Templates describe one or more nodes:

- `proxy` (`http`, `socks4`, `socks5`, `shadowsocks`, `vmess`, `vless`, `trojan`);
- `vpn` (`wireguard`, `openvpn`, `amnezia`);
- `tor` (`none`, `obfs4`, `snowflake`, `meek`).

A template can be bound to a single profile or selected as the global default route.

## Route runtime

When a route cannot be handled as a plain direct connection, Cerbena Browser launches a local runtime:

- `sing-box` for chains and `v2ray`-compatible transports;
- `openvpn` for `openvpn`;
- `amneziawg` for native `AmneziaWG`;
- `tor` plus pluggable transports for `TOR`.

The runtime is started and stopped automatically with the profile lifecycle.

## Traffic gateway

The local gateway:

- receives profile traffic;
- applies kill-switch, DNS/service/domain rules, and user blocks;
- writes decision logs shown in `Traffic`;
- can forward through a runtime `SOCKS5` endpoint or directly through a configured proxy.

## Kill-switch

Kill-switch blocks traffic when:

- a required VPN/runtime route is unavailable;
- a `TOR` template is invalid or a required bridge is unreachable;
- global policy requires VPN and no active runtime session exists.

No later policy layer is allowed to weaken this rule.

## What to verify during failures

1. Is the correct template selected?
2. Is the route runtime active?
3. Is global `block without VPN` enabled?
4. Is there an explicit block reason in `Traffic` or operator/runtime diagnostics?
