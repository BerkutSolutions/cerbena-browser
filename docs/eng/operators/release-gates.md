---
title: Release Gates
sidebar_position: 3
---

A release should not rely on a successful build alone.

## Minimum gates

- `cargo test --workspace`
- docs quality tests
- `npm run docs:build`
- `cd ui/desktop && npm test`
- `powershell -ExecutionPolicy Bypass -File scripts/security-gates-preflight.ps1`
- `powershell -ExecutionPolicy Bypass -File scripts/vulnerability-gates-preflight.ps1`
- validation of `README.md`, `README.en.md`, `docs/ru`, and `docs/eng`
- review of [Security validation](./security-validation.md) when a release note claims `TASKS4` / `U14-2` hardening coverage

## Why this gate set matters

- it prevents drift between code and wiki;
- it prevents broken localization from leaking into releases;
- it guards baseline launcher workflows;
- it keeps `TASKS4` security claims tied to explicit residual-risk review instead of narrative-only confidence;
- it makes release cycles more predictable.
