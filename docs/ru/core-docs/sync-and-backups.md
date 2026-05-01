---
title: Sync, snapshots и restore
sidebar_position: 7
---

Модуль `Sync` в Cerbena Browser отвечает за self-hosted-синхронизацию, snapshots и восстановление состояния профиля.

## Базовые элементы

- `sync controls` на уровне профиля;
- health ping к endpoint;
- список конфликтов;
- snapshots и restore flow;
- модель `E2E encryption`.

## Где это находится в текущем UI

- профиль хранит собственные sync-параметры;
- централизованное управление находится в `Settings > Sync`;
- там же доступны статус, endpoint health, snapshots и restore для выбранного профиля.

## Что есть в desktop-контуре

- сохранение и чтение profile sync controls;
- создание snapshot из desktop UI;
- restore с проверкой целостности;
- backend-обновление статуса здоровья соединения.

## Рекомендации

- включайте `sync` только там, где он действительно нужен;
- храните server URL и ключи отдельно от профилей, если того требует политика;
- проверяйте restore на тестовом профиле до применения в рабочем контуре.
