---
id: release-runbook
title: Руководство по релизу
sidebar_position: 90
---

## Базовый цикл

1. Запустить `cargo test --workspace`.
2. Запустить проверки рабочего стола:
   - проверка документации и текстовой целостности;
   - `i18n` проверка;
   - `npm test` в `ui/desktop` (проверка линтером, модульные, сценарные и дымовые проверки);
   - локальная предпроверка.
3. Подготовить материалы подписи релиза и переменные окружения оператора.
4. Собрать релизные артефакты и установщик.
5. Проверить `README`, RU/EN wiki, `CHANGELOG.md` и матрицу трассировки требований.

Перед упаковкой обязательно прочитайте:

- [Модель доверия релиза](./release-trust.md)

## Команды

### Подготовка материалов подписи

```powershell
powershell -ExecutionPolicy Bypass -File .\scripts\new-release-signing-material.ps1
```

### Быстрая предпроверка

```powershell
powershell -ExecutionPolicy Bypass -File .\scripts\local-ci-preflight.ps1 -CompactOutput
```

Локальная предпроверка выполняет договорные проверки рабочей области, релизные контракты, проверки безопасности и проверки уязвимостей, а также валидацию потока рабочего стола.

### Полная упаковка релиза

```powershell
powershell -ExecutionPolicy Bypass -File .\scripts\release.ps1 -Mode package -CompactOutput
```

Автоматизированный путь публикации доступен через:

- `.github/workflows/release-desktop-bundles.yml` (`msi + deb + signed metadata`).

Перед упаковкой должны быть заданы:

- `CERBENA_RELEASE_SIGNING_PRIVATE_KEY_XML` или `CERBENA_RELEASE_SIGNING_PRIVATE_KEY_PATH`
- `CERBENA_AUTHENTICODE_PFX_PATH`
- `CERBENA_AUTHENTICODE_PFX_PASSWORD`
- опционально `CERBENA_AUTHENTICODE_TIMESTAMP_URL`

## Что публиковать в релизе

В GitHub Release рекомендуется прикладывать:

- `cerbena-browser-<version>.msi` как основной артефакт установки и обновления для Windows;
- `cerbena-browser-setup-<version>.exe` как резервный совместимый установщик;
- `cerbena-browser_<version>_amd64.deb` как опциональный пакет формата `.deb` для совместимых дистрибутивов;
- `cerbena-windows-x64.zip` как переносимую сборку;
- `cerbena-updater.exe` как автономный модуль обновления;
- `checksums.txt`, `checksums.sig`, `release-manifest.json`.

## Перед выпуском

- проверить ручной сценарий обновления, отдельную подпись контрольных сумм и `Authenticode` подписи;
- проверить поток обновления через `MSI` (загрузка, передача в `msiexec`, перезапуск, обработка отмен и сбоев);
- проверить, что в `sync`, контуре маршрутизации, шлюзе трафика и потоке установки нет известных регрессий;
- убедиться, что документация отражает актуальные UI-потоки, скрипты релиза и ограничения самоподписанного доверия.

## После релиза

- обновить `CHANGELOG.md`;
- зафиксировать результаты контрольных проверок качества;
- при необходимости обновить `.work/REQUIREMENTS_TRACEABILITY.md`.
