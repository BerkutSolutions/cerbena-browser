---
title: Routing and Route Runtime
sidebar_position: 3
---

Cerbena Browser networking is built around per-profile route policy, a local traffic gateway, and a launcher-managed route runtime.

## Supported route modes

- `direct`
- `proxy`
- `vpn`
- `tor`
- `hybrid`

Global `global VPN` and `block without VPN` controls are also available.

## Connection templates

Templates describe one or more nodes:

- `proxy` (`http`, `socks4`, `socks5`, `shadowsocks`, `vmess`, `vless`, `trojan`);
- `vpn` (`wireguard`, `openvpn`, `amnezia`);
- `tor` (`none`, `obfs4`, `snowflake`, `meek`).

A template can be attached to a single profile or selected as the global default route.

## Isolation strategies

Cerbena resolves route execution through one of four explicit strategies:

- `isolated` — the route stays inside the local gateway and a userspace runtime.
- `compatibility-native` — a legacy/native-compatible path is allowed when the template cannot run safely in pure userspace.
- `container` — the route runs inside a profile-scoped container sandbox.
- `blocked` — launch is denied because the requested route and isolation policy are incompatible.

Cerbena does not silently fall back between those strategies anymore. The launcher stores the selected policy in the network sandbox store and can auto-migrate upgraded legacy profiles into an explicit compatibility mode when needed.

## Global versus per-profile isolation UI

The launcher separates two different decisions:

- the `Network` screen owns the global default route and the `Сетевая изоляция профилей` policy for global VPN mode;
- the profile modal `VPN` tab owns the profile-specific template selection and the `Сетевая изоляция профиля` frame for that exact route.

If global VPN is disabled, the global isolation frame is hidden because there is no active global route to evaluate there.

## Route runtime

When a route cannot be handled as a plain direct connection, Cerbena launches a managed local runtime:

- `sing-box` for chained routes, `v2ray`-compatible transports, and userspace `wireguard`/`amnezia` paths;
- `openvpn` for `openvpn`;
- `tor` plus pluggable transports for `TOR`.

The runtime starts and stops with the profile lifecycle. The normal per-profile path must not create a persistent system-wide tunnel: profile traffic goes through the local gateway and its runtime upstream, while launcher startup and uninstall also clean up legacy `AmneziaWG` residue from older installs.

## Container-backed route isolation

The `container` strategy uses a launcher-managed helper image and a dedicated Docker network per profile.

Launcher behavior in this mode:

- probes `Docker Desktop` before launch;
- enforces a cap on active sandbox slots;
- reserves a dedicated managed Docker network per profile;
- builds the helper image on first use;
- starts a profile-scoped helper runtime inside the sandbox;
- removes managed containers, networks, and helper images during cleanup and uninstall.

Supported container-backed route families now include:

- compatible `proxy` templates;
- `V2Ray/XRay` templates;
- userspace `WireGuard` and `Amnezia`;
- single-node `OpenVPN`;
- `TOR` with `obfs4`, `snowflake`, and `meek_lite`-compatible bridge flows.

For native-only `AmneziaWG` templates, Cerbena still keeps the tunnel inside the profile-scoped container by launching `amneziawg-go`, `awg-quick`, and a local `SOCKS` gateway there instead of switching the whole host into a system-wide route.

The current helper image revision is `2026-05-02-r5`. Its embedded `sing-box` is built with `with_utls`, so container-backed `VLESS Reality` routes can run without the earlier `uTLS` failure.

## Traffic gateway

The local gateway:

- receives profile traffic;
- applies kill-switch, DNS/service/domain rules, and user blocks;
- writes decision logs shown in `Traffic`;
- forwards through a runtime `SOCKS5` endpoint or directly through a configured proxy.

Recent fixes in this path matter for reliability:

- buffered TLS bytes are preserved for `CONNECT` tunnels;
- short handshake timeouts are cleared before long-lived bridging;
- Windows sockets are returned to blocking mode before the long-lived bridge starts.

Those fixes are what prevent container-backed routes from dying with `ERR_CONNECTION_CLOSED` or Windows `os error 10035` after an allowed `CONNECT` decision.

## Kill-switch

Kill-switch blocks traffic when:

- a required VPN/runtime route is unavailable;
- a `TOR` template is invalid or a required bridge is unreachable;
- global policy requires VPN and no active runtime session exists.

No later policy layer is allowed to weaken this rule.

## Cleanup expectations

When launcher-managed network/runtime behavior changes, the same delivery must update cleanup. For the current route stack that means uninstall or janitor flows must remove:

- profile-scoped local gateway listeners;
- route runtime directories and config residue;
- launcher-owned profile data, updater staging, extension packages, DPAPI secret envelope, and persisted JSON stores under the local app state root;
- legacy `AmneziaWGTunnel$awg-*` Windows services when owned by Cerbena;
- network sandbox store artifacts;
- helper containers, Docker networks, and helper images created for container-backed isolation.

## What to verify during failures

1. Is the correct template selected?
2. Is the resolved strategy `isolated`, `compatibility-native`, `container`, or `blocked`?
3. Is the route runtime active?
4. Is global `block without VPN` enabled?
5. Is there an explicit block reason in `Traffic` or in operator/runtime diagnostics?
