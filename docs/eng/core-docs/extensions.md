---
title: Extensions
sidebar_position: 6
---

Cerbena Browser stores extensions in a shared library, but applies them to profiles in an isolated way.

## Supported behaviors

- import from a local file package (`File`) through the system file picker;
- import from an unpacked local folder (`Folder`);
- import from a previously exported archive bundle (`Archive`);
- import from supported store links;
- export the library to a chosen folder either as a link manifest file or as a package archive folder;
- import a previously exported link manifest or archive folder from a chosen directory;
- extract real display name, version, engine scope, and icon from the package;
- assign manual tags to extensions and filter the library by tags without case-sensitive matching;
- assign extensions to selected profiles;
- enable, disable, and remove them.

## Isolation rules

- installation state and local data are per-profile;
- engine compatibility is validated (`Wayfern` vs `LibreWolf`);
- extension policy must not bypass the profile route or DNS policy.

## Workflow

1. Add the extension to the library through a store link or the `Import` menu (`File`, `Folder`, `Archive`).
2. Validate metadata, icon, and engine compatibility.
3. Open the extension modal and review `Sources`, then remove an engine-specific source when needed.
4. Optionally add tags and use the tag filter to narrow the library view.
5. Launch the profile and let the first-launch install flow complete.
6. Export or re-import the library through `File` / `Archive` actions when needed.

## Important reminders

- Firefox-targeted extensions should not be assigned to Chromium profiles and vice versa;
- the UI should show the extension name as the primary identifier instead of a raw id;
- policy enforcement must stay backend-driven.
