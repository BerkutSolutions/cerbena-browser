---
title: Архитектура
sidebar_position: 2
---

Cerbena Browser разделяет launcher, policy engine, browser adapters, local integrations и docs-stack на явные модули.

## Основные контуры

- `cmd/launcher`: CLI entrypoint и release-удобства.
- `internal/profile`: lifecycle, storage, encryption, wipe, import/export.
- `internal/network_policy`: route, DNS, service filtering, validators.
- `internal/fingerprint`: identity presets и consistency checks.
- `internal/extensions`: библиотека расширений и policy hooks.
- `internal/api_local` и `internal/api_mcp`: локальная automation-поверхность.
- `internal/sync_client`: snapshots, restore и sync-модель.
- `ui/desktop/src-tauri`: desktop backend, runtime orchestration, traffic gateway.
- `ui/desktop/web`: локальная UI-оболочка.

## Почему это важно

- можно усиливать один контур без скрытого влияния на другой;
- security-проверки остаются backend-centric;
- документация и тесты легче привязывать к конкретным контрактам.
