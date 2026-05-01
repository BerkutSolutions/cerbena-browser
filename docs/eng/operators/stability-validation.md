---
title: Stability Validation
sidebar_position: 2
---

For Cerbena Browser, stability means more than "the app launches". It also means documentation, localization, CLI behavior, and critical launcher flows remain reproducible.

## Primary checks

- `cargo test --workspace`
- `cmd/launcher/tests/docs_quality_tests.rs`
- `cmd/launcher/tests/launcher_stability_tests.rs`
- `cmd/launcher/tests/security_gates_contract_tests.rs`
- `cmd/launcher/tests/vulnerability_gates_contract_tests.rs`
- `cd ui/desktop && npm run i18n:check`
- `cd ui/desktop && npm run test`

## What the tests cover

- existence and parity of the `ru` and `eng` wiki trees;
- absence of stray mixed-language fragments in the Russian wiki;
- stability of baseline CLI profile flows;
- smoke validation of desktop UI registry and i18n contracts.

## Local preflight

The full local validation path is described in `scripts/local-ci-preflight.ps1`. It combines Rust tests, docs build, UI smoke checks, and dedicated `security-gates` / `vulnerability-gates` preflight hooks in one repeatable flow.
