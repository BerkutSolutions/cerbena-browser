---
title: Identity and Fingerprint
sidebar_position: 5
---

The `Identity` module controls how a profile appears to websites and remote services.

## Modes

- `Automatic`: generate a realistic profile for the selected platform without exposing every manual field.
- `Manual`: edit the fields explicitly with generator and template assistance.

## What the current UI does

- In `Automatic` mode, the manual generate button and templates are hidden.
- In `Automatic` mode, identity is regenerated per profile session.
- In `Manual` mode, a single `Generate` action and template selector populate real editable values.
- Additional platform presets are available for `Debian`, `Ubuntu`, and `Windows 8`.

## Field groups

- `UA`, platform, version, brand, and vendor;
- hardware: CPU threads, touch points, device memory;
- screen/window: width, height, DPR, inner/outer sizes, coordinates;
- locale, timezone, and geolocation;
- `WebGL`, `Canvas`, and fonts;
- audio and battery.

## Consistency validator

Before save and launch, the backend validates:

- platform and `UA` alignment;
- realistic screen/window combinations;
- locale/timezone/geo consistency;
- JSON and numeric field correctness;
- obvious conflicts against active network policy.

## When to use `Automatic`

- standard day-to-day profiles;
- scenarios where a realistic baseline matters more than field-level control;
- faster onboarding for new profiles.

## When to use `Manual`

- research and compatibility testing;
- migration of an existing profile model;
- precise emulation of a target browser or platform shape.
