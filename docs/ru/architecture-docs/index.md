---
title: Раздел архитектуры
sidebar_position: 1
---

Архитектурный раздел связывает контракты из `.work` с текущей реализацией в репозитории и объясняет, почему Cerbena Browser организован как набор изолированных модулей, а не как монолит.

Начните с:

1. [Архитектура](architecture.md)
2. [Границы продукта](product-boundaries.md)
3. [Изоляция профилей](profile-isolation.md)
4. [Сетевая политика](policy-engine.md)
5. [Zero-trust и backend enforcement](zero-trust.md)

## Где архитектура отражена в коде

- Оболочка рабочего стола и командная поверхность: `ui/desktop/src-tauri/src/main.rs`, `commands.rs`, `launcher_commands.rs`.
- Домен профилей и изоляция: `internal/profile/src/*`.
- Жизненный цикл движков и контракты запуска: `internal/engine/src/*`.
- Сетевая политика, DNS-политика и режимы маршрутизации: `internal/network_policy/src/*`, `ui/desktop/src-tauri/src/network_commands.rs`, `route_runtime.rs`.
- Политики расширений и импорта: `internal/extensions/src/*`, `ui/desktop/src-tauri/src/extensions_commands.rs`.
- Локальный API и MCP backend-модули: `internal/api_local/src/*`, `internal/api_mcp/src/*`.
- Sync и зашифрованные snapshots: `internal/sync_client/src/*`, `ui/desktop/src-tauri/src/sync_commands.rs`, `sync_snapshots.rs`.

## Архитектурные инварианты проекта

1. UI не является границей доверия: чувствительные решения проверяются в серверных обработчиках команд.
2. Состояние изолируется по профилям: приложение сохраняет данные в профильных модулях состояния, а не в едином общем секрете.
3. Принцип запрета по умолчанию в сети: при сбое обязательного маршрута или рантайма трафик блокируется, а не уходит в тихий резервный режим.
4. Контракт релиза выше удобства: потоки обновления и выпуска проверяются контрактными тестами и локальными предпроверками.

## Контрольные точки валидации

- Проверки Rust рабочей области: `cargo test --all`.
- Контрактные тесты launcher: `cmd/launcher/tests/*`.
- Проверки качества веб-части рабочего стола (через `npm test` в `ui/desktop`):
  - проверка целостности текста;
  - `i18n` проверка;
  - проверка линтером (`lint:web`);
  - модульные тесты (`test:unit:web`);
  - сценарные тесты (`test:scenario:web`);
  - UI smoke-тесты.
- Проверки релиза и оператора: `scripts/local-ci-preflight.ps1`, `scripts/release.ps1`.
