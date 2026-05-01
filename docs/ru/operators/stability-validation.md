---
title: Проверки стабильности
sidebar_position: 2
---

Для Cerbena Browser стабильность означает не только "приложение запускается", но и то, что документация, локализация, CLI и критичные launcher-сценарии остаются воспроизводимыми.

## Основные проверки

- `cargo test --workspace`
- `cmd/launcher/tests/docs_quality_tests.rs`
- `cmd/launcher/tests/launcher_stability_tests.rs`
- `cmd/launcher/tests/security_gates_contract_tests.rs`
- `cmd/launcher/tests/vulnerability_gates_contract_tests.rs`
- `cd ui/desktop && npm run i18n:check`
- `cd ui/desktop && npm run test`

## Что покрывают тесты

- наличие и синхронность `ru` / `eng` веток wiki;
- отсутствие лишних английских фрагментов в русской wiki;
- стабильность базовых CLI-сценариев профиля;
- smoke-проверку desktop UI registry и i18n-контрактов.

## Локальный preflight

Полная локальная проверка описана в `scripts/local-ci-preflight.ps1`. Скрипт объединяет Rust-тесты, docs build, UI smoke-проверки и отдельные шаги `security-gates` / `vulnerability-gates` в один поток.
