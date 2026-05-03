---
title: Profiles and Lifecycle
sidebar_position: 2
---

A Cerbena Browser profile is an isolated browser-state container with its own data, policies, extensions, and cryptographic context.

## What a profile stores

- name, description, tags, and engine (`Wayfern` or `Camoufox`);
- start page and search provider, or a strict single-page app-window mode;
- `Identity`, `Network`, `DNS`, `Extensions`, `Security`, and `Sync` settings;
- runtime state, audit trail, and cache;
- optional password lock and `ephemeral` mode.

New profiles default to `https://duckduckgo.com`.

Built-in profiles now include:

- `Chromium Default`
- `Firefox Default`
- `Chromium Private Memory`
- `Firefox Private Memory`
- `Discord` - a strict `Wayfern` app-window limited to Discord domains
- `Telegram` - a strict `Wayfern` app-window limited to Telegram domains

## Isolation properties

- each profile uses its own `profiles/{profile_uuid}/` root;
- data, cache, and extensions are not shared across profiles;
- keys and secrets must not be reused across profiles;
- wipe/import/export operations always stay inside the selected profile scope.

## Main operations

- create and edit;
- duplicate, import, and export;
- launch and stop;
- lock and unlock;
- selective wipe and panic wipe.

## What matters in the current profile modal

- sections are presented as a left-side vertical rail;
- `Identity` follows the same `Automatic` / `Manual` rules as the standalone page;
- `DNS` hides manual fields in system mode and reveals them in manual mode;
- `Extensions` uses human-readable names instead of raw ids;
- `Security` merges allow/deny domain management into one searchable section and exposes password lock as an explicit password/confirm flow.

## Special modes

- `Password lock`: requires explicit unlock before launch.
- `Ephemeral`: clears volatile data when the profile closes.
- `Private memory profile`: a built-in variant for private sessions.
- `Single-page`: available for `Wayfern` profiles; launches a strict app window without a free address bar, hides the default search provider, and restricts navigation to the start-page domain.

## Practical guidance

- separate work, research, and test profiles;
- do not mix route policies for unrelated scenarios inside one profile;
- use different profiles for distinct extension sets and trust models.
