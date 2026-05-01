# UI Desktop Launcher

This folder contains the desktop launcher shell for Cerbena Browser based on `Tauri 2`. It hosts the Windows desktop UI, the Tauri command bridge, the local web frontend, and packaging hooks used by the release and installer scripts.

GitHub: `https://github.com/BerkutSolutions/cerbena-browser`

## Commands

- `npm run style:sync` - copy root `styles/base.css` into `web/styles/base.css`
- `npm run i18n:check` - verify RU/EN key parity and feature module presence
- `npm run dev` - run style sync + i18n check + `tauri dev`
- `npm run build` - run style sync + i18n check + `tauri build`
- `npm test` - run the desktop validation flow, including localization and UI smoke coverage
- `cargo run --manifest-path src-tauri/Cargo.toml -- --updater-preview` - open the standalone secure updater screen in dry-run mode

## Structure

- `src-tauri/` - Rust host, command bridge, window lifecycle
- `web/` - modular frontend by feature domains
- `scripts/` - developer checks and sync scripts
- `web/assets/brand/` - launcher branding assets reused by desktop UI and installer flow

## Security and update behavior

- the desktop shell keeps sensitive launcher state in encrypted stores instead of plaintext shell files;
- legacy sensitive data is migrated forward into the protected store format after successful readback;
- the project provides a separate `cerbena-updater.exe` so update UX and installation handoff can run outside the main browser shell;
- the updater verifies `checksums.txt`, `checksums.sig`, and artifact `SHA-256` before staging or installation;
- updater preview mode exercises discovery, comparison, safety validation, download, checksum, and install handoff stages without writing installed binaries.

## Current shell scope

- `Home` contains the main profile lifecycle surface, metrics, import/export, and bulk actions
- modal profile editor covers `Identity`, `VPN`, `DNS`, `Extensions`, `Security`, `Sync`, and `Advanced`
- `Settings` centralizes `General`, `Links`, `Sync`, and update controls
- installer/release scripts consume the desktop release build as `cerbena.exe`

## Release integration

- `scripts/generate-release-artifacts.ps1` packages the desktop release bundle
- `scripts/build-installer.ps1` builds the Windows installer wizard
- GitHub Releases should publish the generated installer `.exe`, the standalone `cerbena-updater.exe`, and the portable `.zip` bundle
