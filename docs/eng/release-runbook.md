---
id: release-runbook
title: Release Runbook
sidebar_position: 90
---

## Baseline cycle

1. Run `cargo test --workspace`.
2. Run documentation checks, `i18n`, UI smoke checks, and local preflight.
3. Build release artifacts and the installer.
4. Validate `README`, the bilingual wiki, `CHANGELOG.md`, and requirement traceability.

## Commands

### Fast preflight

```powershell
powershell -ExecutionPolicy Bypass -File .\scripts\local-ci-preflight.ps1 -CompactOutput
```

The local preflight now runs the Docker runtime contract plus security and vulnerability gates in one pass.

### Full release packaging

```powershell
powershell -ExecutionPolicy Bypass -File .\scripts\release.ps1 -Mode package -CompactOutput
```

### Build the Windows installer

```powershell
powershell -ExecutionPolicy Bypass -File .\scripts\build-installer.ps1
```

## What should be attached to a release

GitHub Releases should normally publish:

- `cerbena-browser-setup-<version>.exe` as the primary Windows installer;
- `cerbena-windows-x64.zip` as the portable release bundle;
- `cerbena-updater.exe` as the standalone updater executable;
- `checksums.txt`;
- `checksums.sig`;
- `release-manifest.json`.

The installer `.exe` is built locally and should be attached to the release as the primary installation artifact for end users.

## Before shipping

- confirm the `Wayfern` flow does not require a missing `ToS` acknowledgement;
- validate the manual update flow and artifact signatures;
- confirm that the standalone updater reports the correct `version is current` state for an up-to-date build and that `preview` mode completes without installation;
- verify that `sync`, `route runtime`, traffic gateway, and installer flow have no known regressions;
- ensure the documentation reflects the current UI, release scripts, and installation path.

## Rollback

1. Stop the active instance.
2. restore the previous signed artifact.
3. validate profile metadata consistency.
4. rerun the key smoke checks.

## After release

- update `CHANGELOG.md`;
- record the validated quality-gate set;
- refresh traceability evidence when required;
- attach the installer `.exe` to the GitHub release entry.
