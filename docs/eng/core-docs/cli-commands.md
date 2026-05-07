---
title: CLI Commands
sidebar_position: 10
---

The CLI launcher is intended for baseline local operations and release validation.

## Supported commands

### `init-profile`

Create a profile:

```bash
cargo run -p cerbena-launcher -- init-profile --root <dir> --name <name> --engine chromium
```

### `list-profiles`

List profiles:

```bash
cargo run -p cerbena-launcher -- list-profiles --root <dir>
```

### `build-launch-plan`

Build a launch plan for a profile:

```bash
cargo run -p cerbena-launcher -- build-launch-plan --root <dir> --profile-id <uuid> --binary <path>
```

### `update-apply`

Run the manual update path with signature verification:

```bash
cargo run -p cerbena-launcher -- update-apply --version <semver> --signature <sig>
```

### `desktop updater preview`

Open the standalone secure updater screen in dry-run mode without installing anything:

```bash
cargo run --manifest-path ui/desktop/src-tauri/Cargo.toml -- --updater-preview
```
