# Support

## Documentation First

Start with the repository documentation:

- [docs/README.md](docs/README.md)
- [docs/ru/README.md](docs/ru/README.md)
- [docs/eng/README.md](docs/eng/README.md)

Key operator pages:

- UI overview: `docs/*/core-docs/ui.md`
- Network routing: `docs/*/core-docs/network-routing.md`
- DNS and filters: `docs/*/core-docs/dns-and-filters.md`
- Security model: `docs/*/core-docs/security.md`
- Release runbook: `docs/*/release-runbook.md`

## Local Diagnostics

Before asking for help, collect the current local evidence:

```powershell
powershell -ExecutionPolicy Bypass -File .\scripts\local-ci-preflight.ps1
```

If the issue is release or supply-chain related, also run:

```powershell
powershell -ExecutionPolicy Bypass -File .\scripts\security-gates-preflight.ps1
```

```powershell
powershell -ExecutionPolicy Bypass -File .\scripts\vulnerability-gates-preflight.ps1
```

## What To Include In A Support Request

- Cerbena Browser version
- OS version
- browser engine involved (`Wayfern` or `Camoufox`)
- affected profile type and route mode
- exact error text
- steps to reproduce
- relevant logs or screenshots

## Current Channels

Primary project home:

`https://github.com/BerkutSolutions/cerbena-browser`

Until dedicated public channels are published, use repository issues for non-sensitive defects and private contact for security reports.
