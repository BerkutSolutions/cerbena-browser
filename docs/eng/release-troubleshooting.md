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
- updater/restart handoff does not complete in expected timing windows.
- required release artifacts (`.msi`, setup bundle, checksums/signature manifest) are incomplete or mismatched.

## What to inspect

1. `cmd/launcher/tests/docs_quality_tests.rs`
2. `cmd/launcher/tests/launcher_stability_tests.rs`
3. `ui/desktop/scripts/check-i18n.mjs`
4. `ui/desktop/scripts/ui-smoke-test.mjs`
5. `scripts/local-ci-preflight.ps1`
6. `scripts/release.ps1`
7. `scripts/published-updater-e2e.ps1`
8. `scripts/local-updater-e2e.ps1`
9. `ui/desktop/src-tauri/src/update_commands.rs`

## Recovery

- fix wiki and `README` divergence;
- recheck runtime and logging regressions;
- rerun the release gates before retrying.

## Fast diagnosis flow

1. Run `cargo test --workspace` and stop on first regression in contract tests.
2. Run `scripts/local-ci-preflight.ps1 -CompactOutput` and capture failing stage output.
3. If update flow fails, inspect updater-related diagnostics in:
   - `scripts/published-updater-e2e.ps1` step logs;
   - launcher runtime logs exposed by the desktop diagnostics screen;
   - backend updater sequence in `update_commands.rs`.
4. Rebuild installer artifacts via `scripts/build-installer.ps1` and validate expected filenames/checksum bundle.
5. Re-run preflight and updater e2e after fixes to verify the same scenario passes end-to-end.
