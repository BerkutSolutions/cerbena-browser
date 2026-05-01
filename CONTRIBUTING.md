# Contributing to Cerbena Browser

Thanks for contributing to `Cerbena Browser`.

## Ground Rules

- Follow the repository contracts under `.work/`.
- Keep zero-trust backend enforcement intact. UI-only checks are not enough.
- Preserve profile isolation across data, keys, cache, extensions, and network policy.
- Keep all new user-facing strings localized in both `ru` and `en`.
- Avoid new runtime dependencies on external CDNs or remotely hosted frontend assets.

## Before Opening a Change

1. Sync your branch with the current default branch.
2. Read `.work/PROMPT.md` and the linked architecture contracts.
3. Confirm the task stays inside the current product stage unless explicitly expanded.

## Development Workflow

```bash
cargo test --workspace
```

```bash
cd ui/desktop
npm install
npm test
```

```bash
npm install
npm run docs:build
```

For the full local gate:

```powershell
powershell -ExecutionPolicy Bypass -File .\scripts\local-ci-preflight.ps1
```

## Code and Review Expectations

- Prefer small, reviewable commits.
- Update `CHANGELOG.md` when behavior changes.
- Update `.work/REQUIREMENTS_TRACEABILITY.md` when a requirement mapping changes.
- Add or update tests for security-sensitive, routing, DNS, extension, and release-path changes.
- Document any residual risks or blocked checks in the final change summary.

## Pull Request Checklist

- [ ] `cargo test --workspace` passed
- [ ] `ui/desktop/src-tauri` tests passed when Rust desktop code changed
- [ ] `ui/desktop` UI checks passed when frontend changed
- [ ] `docs:build` passed when docs changed
- [ ] RU/EN i18n coverage updated
- [ ] `CHANGELOG.md` updated
- [ ] `.work/REQUIREMENTS_TRACEABILITY.md` updated when applicable
