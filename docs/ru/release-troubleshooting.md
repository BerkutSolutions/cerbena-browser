---
id: release-troubleshooting
title: Диагностика релиза
sidebar_position: 91
---

## Типовые причины срыва релиза

- не проходит `cargo test --workspace`;
- документационные ветки `ru` и `eng` расходятся;
- русская wiki содержит смешанные языковые фрагменты;
- UI smoke или `i18n`-проверка падает;
- невалиден сценарий проверки подписи обновления;
- `Wayfern ToS` не подтвержден для нужного сценария.

## Что проверять

1. `cmd/launcher/tests/docs_quality_tests.rs`
2. `cmd/launcher/tests/launcher_stability_tests.rs`
3. `ui/desktop/scripts/check-i18n.mjs`
4. `ui/desktop/scripts/ui-smoke-test.mjs`
5. `scripts/local-ci-preflight.ps1`

## Восстановление

- исправить расхождения в wiki и `README`;
- перепроверить регрессии runtime и журналирования;
- заново прогнать контрольные проверки релиза перед повторной попыткой.
