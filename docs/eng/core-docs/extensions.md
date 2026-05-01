---
title: Extensions
sidebar_position: 6
---

Cerbena Browser stores extensions in a shared library, but applies them to profiles in an isolated way.

## Supported behaviors

- import from a local archive through the system file picker;
- import from supported store links;
- export the library to a chosen folder either as a link manifest file or as a package archive folder;
- import a previously exported link manifest or archive folder from a chosen directory;
- extract real display name, version, engine scope, and icon from the package;
- assign manual tags to extensions and filter the library by tags without case-sensitive matching;
- assign extensions to selected profiles;
- enable, disable, and remove them.

## Isolation rules

- installation state and local data are per-profile;
- engine compatibility is validated (`Wayfern` vs `Camoufox`);
- extension policy must not bypass the profile route or DNS policy.

## Workflow

1. Add the extension to the library through a link or archive.
2. Validate metadata, icon, and engine compatibility.
3. Optionally add tags and use the tag filter to narrow the library view.
4. Assign it to a profile.
5. Launch the profile and let the first-launch install flow complete.
6. Export or re-import the library through the folder-based `File` / `Archive` actions when needed.

## Important reminders

- Firefox-targeted extensions should not be assigned to Chromium profiles and vice versa;
- the UI should show the extension name as the primary identifier instead of a raw id;
- policy enforcement must stay backend-driven.
