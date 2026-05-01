---
id: index
title: Русская wiki
sidebar_position: 1
---

Cerbena Browser - это самостоятельная платформа контролируемого браузинга с изолированными профилями, строгим сетевым policy engine и desktop-контуром на `Tauri 2` + `Rust`.

Эта wiki описывает не абстрактную архитектуру, а фактический контур текущего репозитория: `Profiles`, `Identity`, `Network`, `DNS`, `Extensions`, `Security`, `Traffic`, `Settings`, релизный поток и локальные интеграции.

## Для кого эта документация

- Для инженеров, которые настраивают профили, маршруты, blocklists и DNS-политики.
- Для security-команд, которым нужны zero-trust и изоляция профилей.
- Для разработчиков, которые поддерживают launcher, сайт документации и локальные интеграции.

## Что важно понимать сразу

- `UI` не является границей доверия.
- Профильные данные, ключи, сеть, cache и состояние расширений изолированы.
- `Kill-switch` блокирует трафик, если обязательный runtime-маршрут недоступен.
- `Auto-update` по умолчанию выключен.
- `Settings` теперь объединяет глобальные настройки оболочки, маршрутизацию ссылок и операции `Sync`.
- Документация поддерживается синхронно для `ru` и `eng`.

## Основные ветки wiki

- [Навигатор](navigator.md)
- [UI и рабочие сценарии](core-docs/ui.md)
- [Профили и жизненный цикл](core-docs/profiles.md)
- [Маршрутизация и route runtime](core-docs/network-routing.md)
- [DNS, blocklists и сервисные фильтры](core-docs/dns-and-filters.md)
- [Личность и fingerprint](core-docs/identity.md)
- [Расширения](core-docs/extensions.md)
- [Sync, snapshots и restore](core-docs/sync-and-backups.md)
- [Локальный API и MCP](core-docs/api.md)
- [Безопасность](core-docs/security.md)
- [Архитектура](architecture-docs/architecture.md)
- [Руководство по релизу](release-runbook.md)

## Рекомендуемые маршруты чтения

### Быстрое знакомство

1. [Навигатор](navigator.md)
2. [UI и рабочие сценарии](core-docs/ui.md)
3. [Архитектура](architecture-docs/architecture.md)

### Сеть и ограничения

1. [Маршрутизация и route runtime](core-docs/network-routing.md)
2. [DNS, blocklists и сервисные фильтры](core-docs/dns-and-filters.md)
3. [Политика сети](architecture-docs/policy-engine.md)

### Release и поддержка

1. [Проверки стабильности](operators/stability-validation.md)
2. [Release gates](operators/release-gates.md)
3. [Руководство по релизу](release-runbook.md)
4. [Диагностика релиза](release-troubleshooting.md)
