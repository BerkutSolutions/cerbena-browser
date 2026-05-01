---
title: Extensions
sidebar_position: 6
---

Cerbena Browser stores extensions in a shared library, but applies them to profiles in an isolated way.

## Supported behaviors

- import from a local archive through the system file picker;
- import from supported store links;
- extract real display name, version, engine scope, and icon from the package;
- assign extensions to selected profiles;
- enable, disable, and remove them.

## Isolation rules

- installation state and local data are per-profile;
- engine compatibility is validated (`Wayfern` vs `Camoufox`);
- extension policy must not bypass the profile route or DNS policy.

## Workflow

1. Add the extension to the library through a link or archive.
2. Validate metadata, icon, and engine compatibility.
3. Assign it to a profile.
4. Launch the profile and let the first-launch install flow complete.
5. Disable, reinstall, or remove the extension when needed.

## Important reminders

- Firefox-targeted extensions should not be assigned to Chromium profiles and vice versa;
- the UI should show the extension name as the primary identifier instead of a raw id;
- policy enforcement must stay backend-driven.
