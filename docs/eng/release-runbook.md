---
id: release-runbook
title: Release Runbook
sidebar_position: 90
---

## Baseline cycle

1. Run `cargo test --workspace`.
2. Run documentation checks, `i18n`, UI smoke checks, and local preflight.
3. Prepare operator release-signing material and environment variables.
4. Build release artifacts and the installer.
4. Validate `README`, the bilingual wiki, `CHANGELOG.md`, and requirement traceability.

Read this before packaging:

- [Release Trust Model](./release-trust.md)

## Commands

### Bootstrap self-signed signing material

```powershell
powershell -ExecutionPolicy Bypass -File .\scripts\new-release-signing-material.ps1
```

The script generates an operator-local bundle containing:

- the private XML key for detached checksum signatures;
- the public XML key to commit into `config/release/release-signing-public-key.xml` during rotation;
- a self-signed `Authenticode PFX`;
- a public `PEM` certificate;
- the `PFX` password.

### Fast preflight

```powershell
powershell -ExecutionPolicy Bypass -File .\scripts\local-ci-preflight.ps1 -CompactOutput
```

The local preflight now runs the Docker runtime contract plus security and vulnerability gates in one pass.

### Full release packaging

```powershell
powershell -ExecutionPolicy Bypass -File .\scripts\release.ps1 -Mode package -CompactOutput
```

Before packaging, set:

- `CERBENA_RELEASE_SIGNING_PRIVATE_KEY_XML` or `CERBENA_RELEASE_SIGNING_PRIVATE_KEY_PATH`
- `CERBENA_AUTHENTICODE_PFX_PATH`
- `CERBENA_AUTHENTICODE_PFX_PASSWORD`
- optionally `CERBENA_AUTHENTICODE_TIMESTAMP_URL`

### Build the Windows installer

```powershell
powershell -ExecutionPolicy Bypass -File .\scripts\build-installer.ps1
```

### Verify detached and Windows signatures locally

```powershell
. .\scripts\release-signing.ps1
Verify-ReleaseChecksumSignature `
  -ChecksumsPath .\build\release\<version>\checksums.txt `
  -SignaturePath .\build\release\<version>\checksums.sig

Verify-WindowsArtifacts @(
  '.\build\release\<version>\staging\cerbena-windows-x64',
  '.\build\installer\<version>\output\cerbena-browser-<version>.msi',
  '.\build\installer\<version>\output\cerbena-browser-setup-<version>.exe'
)
```

## What should be attached to a release

GitHub Releases should normally publish:

- `cerbena-browser-<version>.msi` as the primary Windows installer and updater handoff artifact;
- `cerbena-browser-setup-<version>.exe` as the manual/fallback installer path;
- `cerbena-windows-x64.zip` as the portable release bundle;
- `cerbena-updater.exe` as the standalone updater executable;
- `checksums.txt`;
- `checksums.sig`;
- `release-manifest.json`.

The `MSI` is built locally, signed with the same Authenticode material as the rest of the Windows release, and should be attached to the release as the primary installation artifact for end users. The `.exe` installer remains a compatibility fallback for manual recovery and environments where `msiexec` is not the intended path.

Updater ownership for Windows is now explicit:

- installed Windows builds prefer the signed `MSI` artifact;
- portable Windows builds keep the signed `.zip` artifact as the primary update path;
- when the updater selects `MSI`, it does not extract an inner payload and does not trust a post-download repack step;
- instead, the verified `MSI` is handed directly to `msiexec`, and relaunch/cleanup state is recorded in the app update store for auditability.

## Before shipping

- validate the manual update flow, detached signatures, and `Authenticode` signatures;
- validate the MSI-first update flow from an installed Windows build, including verified download, `msiexec` handoff, relaunch, and cancellation/failure recovery behavior;
- confirm that the standalone updater reports the correct `version is current` state for an up-to-date build and that `preview` mode completes without installation;
- verify that `sync`, `route runtime`, traffic gateway, and installer flow have no known regressions;
- ensure the documentation reflects the current UI, release scripts, installation path, and self-signed trust limitations.

## Rollback

1. Stop the active instance.
2. restore the previous signed artifact.
3. validate profile metadata consistency.
4. rerun the key smoke checks.

## MSI recovery policy

If a verified `MSI` update fails after download, use the following policy:

1. Treat the downloaded package as untrusted for install completion until `msiexec` exits with success.
2. Keep the previous installed binaries as the authoritative runnable state until the `MSI` transaction finishes.
3. If `msiexec` reports cancellation (`1602`), mark the update as canceled, clear pending apply state, and retry only after a fresh operator decision.
4. If `msiexec` reports another active installer transaction (`1618`), wait for the competing transaction to finish, then rerun the update.
5. If the update store shows `applied_pending_relaunch` but the current running version already matches the staged version, reconcile the stale handoff state back to `up_to_date`.

Operator recovery for publication or trust incidents:

- broken MSI publication: remove the bad GitHub release asset, republish a newly signed `MSI`, and regenerate `checksums.txt`, `checksums.sig`, and `release-manifest.json` together;
- rotated or revoked self-signed material: rotate the keypair, commit the new public XML, rebuild all release artifacts with the new private material, and do not reuse the previous signing secret;
- checksum or signature mismatch: stop distribution immediately, rebuild from a clean workspace, and do not relabel the existing corrupted artifacts as trusted.

## After release

- update `CHANGELOG.md`;
- record the validated quality-gate set;
- refresh traceability evidence when required;
- attach the `.msi` and fallback installer `.exe` to the GitHub release entry.
