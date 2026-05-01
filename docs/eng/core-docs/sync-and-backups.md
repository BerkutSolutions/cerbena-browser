---
title: Sync, Snapshots, and Restore
sidebar_position: 7
---

The Cerbena Browser `Sync` module covers self-hosted synchronization, snapshots, and profile recovery.

## Core elements

- profile-level `sync controls`;
- endpoint health ping;
- conflict tracking;
- snapshots and restore flow;
- an `E2E encryption` model.

## Where it lives in the current UI

- each profile stores its own sync settings;
- centralized operations now live in `Settings > Sync`;
- that tab exposes status, endpoint health, snapshots, and restore actions for the selected profile.

## What exists in the desktop contour

- save and load profile sync controls;
- create snapshots from the desktop UI;
- run restore with integrity validation;
- update connection health state through the backend.

## Recommendations

- enable `sync` only where it is operationally required;
- keep server URL and secrets outside profiles when policy demands it;
- validate restore on a test profile before applying it to production-like data.
