---
id: release-troubleshooting
title: Release Troubleshooting
sidebar_position: 91
---

## Typical reasons a release is blocked

- `cargo test --workspace` fails;
- the `ru` and `eng` docs branches drift apart;
- the Russian wiki contains mixed-language fragments;
- UI smoke or `i18n` validation fails;
- the update signature flow is invalid;
- the required `Wayfern ToS` acknowledgement is missing.

## What to inspect

1. `cmd/launcher/tests/docs_quality_tests.rs`
2. `cmd/launcher/tests/launcher_stability_tests.rs`
3. `ui/desktop/scripts/check-i18n.mjs`
4. `ui/desktop/scripts/ui-smoke-test.mjs`
5. `scripts/local-ci-preflight.ps1`

## Recovery

- fix wiki and `README` divergence;
- recheck runtime and logging regressions;
- rerun the release gates before retrying.
