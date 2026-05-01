# Security Policy

## Scope

`Cerbena Browser` is a zero-trust desktop launcher for isolated browser profiles. Security-sensitive areas include:

- profile isolation and storage boundaries
- route runtime and kill-switch enforcement
- DNS policies, blocklists, and service restrictions
- extension install/enforcement flow
- panic cleanup and protected-site retention
- sync, backup, and encrypted restore paths
- release/update integrity

## Supported Line

Security fixes are provided for the current active release line tracked in this repository.

Because the product is still moving quickly, the supported baseline is:

- the current `main` branch
- the most recent tagged release once public releases are available

## Reporting a Vulnerability

Until a dedicated security mailbox is published, report sensitive issues privately through the repository owner or the future GitHub Security Advisory flow for:

`https://github.com/BerkutSolutions/cerbena-browser`

Please include:

- affected component or file path
- impact and realistic exploit path
- reproduction steps or proof-of-concept
- whether profile isolation, DNS, routing, or update integrity is involved

## Disclosure Expectations

- Do not publish exploit details before a fix is available.
- Give maintainers enough time to validate and patch the issue.
- Coordinate on release timing for issues that affect isolation, update integrity, or network enforcement.

## Update Policy

Auto-update is disabled by default. Any future automatic update path must preserve:

- signed artifact verification
- explicit policy opt-in
- safe staging and application behavior
- documented rollback and troubleshooting guidance
