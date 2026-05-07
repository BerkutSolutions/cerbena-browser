---
id: release-trust
title: Release Trust Model
sidebar_position: 91
---

## Purpose

Cerbena currently uses an **internal self-signed release trust model** for a solo-maintained Windows release flow.
It exists to:

- prove artifact integrity and origin for published releases;
- enforce release verification in the updater and release scripts;
- keep public verification material separate from private signing material.

This model is **not equivalent to public Windows trust**. A valid self-signed signature does not guarantee:

- no `SmartScreen` warning;
- no browser or download warning;
- public reputation equivalent to an `OV/EV` code-signing certificate.

## Authoritative trust boundary

For the current stage, Cerbena release trust is defined as:

1. `checksums.txt` is signed with a detached release signature.
2. The public key used to verify that signature is committed in the repository:
   - `config/release/release-signing-public-key.xml`
3. The updater accepts a release asset only after:
   - successful `checksums.sig` verification;
   - successful `SHA-256` match for the selected asset.
4. Windows executables are additionally signed with an operator-owned self-signed `Authenticode` `PFX`.
5. Release generation fails if a required signature is missing or fails verification.

## Windows artifacts that must be signed

Cerbena uses a hard-fail policy for every published Windows artifact that participates in install, update, or launch.

Required `Authenticode` coverage:

- main app `cerbena.exe`;
- standalone updater `cerbena-updater.exe`;
- launcher `cerbena-launcher.exe`;
- any `*.exe`, `*.dll`, `*.sys`, or `*.msi` shipped in the public release bundle;
- any `*.exe` or `*.msi` published as a Windows installer artifact.

Required detached release-signature coverage:

- `checksums.txt`;
- `checksums.sig`.

`release-manifest.json`, archives such as `cerbena-windows-x64.zip`, and optional Linux packages such as `cerbena-browser_<version>_amd64.deb` are trusted through the signed checksum contract.

## Helper and sidecar policy

Unsigned helper binaries are not allowed.

If a new Windows sidecar binary is published in the release bundle or installer output and matches one of:

- `*.exe`
- `*.dll`
- `*.sys`
- `*.msi`

then release verification must fail unless that file is signed by the expected release certificate.
There is no transitional warning mode for these files.

## Public vs private material

Only public verification material may be committed:

- `config/release/release-signing-public-key.xml`

Private signing material must be injected only on the operator machine:

- `CERBENA_RELEASE_SIGNING_PRIVATE_KEY_XML` or `CERBENA_RELEASE_SIGNING_PRIVATE_KEY_PATH`
- `CERBENA_AUTHENTICODE_PFX_PATH`
- `CERBENA_AUTHENTICODE_PFX_PASSWORD`
- optionally `CERBENA_AUTHENTICODE_TIMESTAMP_URL`

## Operator bootstrap and local verification

Bootstrap new self-signed material on the operator machine with:

```powershell
powershell -ExecutionPolicy Bypass -File .\scripts\new-release-signing-material.ps1
```

Minimum operator flow:

1. generate the keypair and self-signed `PFX`;
2. move the private outputs out of the workspace into secure local storage;
3. commit only the replacement public XML when rotating trust;
4. set `CERBENA_RELEASE_SIGNING_PRIVATE_KEY_PATH`, `CERBENA_AUTHENTICODE_PFX_PATH`, and `CERBENA_AUTHENTICODE_PFX_PASSWORD` before packaging;
5. build artifacts only from that operator-controlled machine.

Local verification after packaging:

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

The committed public XML is enough to verify detached release signatures from a fresh clone, but it does not grant signing ability.

## Rotation and future migration

Rotation is bootstrapped with:

- `scripts/new-release-signing-material.ps1`

The script generates:

- a new private XML key for `checksums.sig`;
- a new public XML key;
- a new self-signed `Authenticode PFX`;
- a public `PEM` certificate.

After rotation:

1. move private outputs out of the workspace into secure operator storage;
2. replace `config/release/release-signing-public-key.xml`;
3. ship future releases only with the new matching private material;
4. keep documentation honest about Windows reputation limits for self-signed artifacts.

Migration to a future `OV/EV` certificate should preserve the same model:

- updater checksum verification stays mandatory;
- Authenticode strengthens the Windows-native trust surface but does not replace detached release verification.

## Trust limitations and future public signing

The current self-signed model is intentionally honest about its limits:

- it proves operator-controlled integrity for Cerbena release tooling and updater verification;
- it does not grant SmartScreen reputation;
- it does not remove browser download warnings by itself;
- it should not be documented as equivalent to a public `OV/EV` certificate.

Future migration to a public code-signing certificate must keep:

- the same detached checksum-signature contract for the updater;
- the same release-manifest and checksum publication model;
- operator-visible verification and rollback steps that remain valid even if Authenticode trust changes.
