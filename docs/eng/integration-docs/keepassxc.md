---
title: KeePassXC Integration
sidebar_position: 3
---

Cerbena Browser supports `KeePassXC` through the official `KeePassXC-Browser` extension and the local native messaging bridge used by Chromium-based browsers.

## How it works

1. The operator enables `Allow KeePassXC integration` in the profile security settings.
2. The launcher includes the `KeePassXC-Browser` extension only for that profile.
3. On profile launch, the backend prepares a native messaging manifest for `org.keepassxc.keepassxc_browser`.
4. The manifest points to the local `keepassxc-proxy.exe` binary, usually installed at `C:\Program Files\KeePassXC\keepassxc-proxy.exe`.
5. The browser extension connects to the native host through `stdio`, and `KeePassXC` handles the approval flow.

## Why isolation is preserved

- Integration is explicit and per-profile. It is not enabled globally by default.
- The bridge only allows the `KeePassXC-Browser` extension origin listed in the manifest `allowed_origins`.
- The launcher still keeps extension files inside the profile-scoped runtime directory.
- Protected profiles remain subject to backend security rules and can still reject unsafe combinations.
- The bridge does not expose arbitrary local API access to web pages. The website talks only to the extension, and the extension talks only to the approved native host.

## Connection path

- Web page -> `KeePassXC-Browser` content/background scripts
- Extension -> native messaging host `org.keepassxc.keepassxc_browser`
- Native host -> `keepassxc-proxy.exe`
- Proxy -> local `KeePassXC`

This means the website never receives direct host-system access. The native hop is limited to the approved extension and the installed `KeePassXC` bridge.

## Operational notes

- If the profile does not have `Allow KeePassXC integration`, the launcher will not allow the bridge for that profile.
- If `Block extension launch` is enabled, only the explicitly allowed `KeePassXC` path remains available.
- If `Allow extensions system access` is disabled, other system-integrating extensions remain blocked by backend launch filtering.

## Troubleshooting

- `Access to the specified native messaging host is forbidden` usually means the browser extension origin does not match the manifest `allowed_origins`.
- `KeePassXC proxy executable was not found` means `KeePassXC` is missing or installed in a non-standard location that the launcher could not resolve.
- Use `Diagnostics -> Logs` and search for `[keepassxc-bridge]` entries to inspect manifest registration and runtime origin detection.
