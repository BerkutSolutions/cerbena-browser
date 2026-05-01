# Журнал изменений

## 1.0.0 — Первый релиз

### Core

- Выпущена первая публичная версия `Cerbena Browser` с изолированными профилями, zero-trust backend enforcement и локальной desktop-оболочкой.
- Реализованы профильно-специфичные маршрутные режимы `direct`, `proxy`, `vpn`, `tor`, `hybrid` с управляемым runtime-контуром.
- Добавлены sync/backup-сценарии, локальный API, MCP и аудит критичных операций.
- Автообновление по умолчанию отключено и включается только явно через настройки.

### UI

- Основной lifecycle UI профилей перенесен на `Home`; отдельная вкладка `Profiles` убрана.
- Добавлены плавные анимации модалок, обновленные иконки бокового меню, сохранение выбранного языка и отображение версии `v1.0.0`.
- Переработаны вкладки `DNS`, `Security`, `Extensions`, `Traffic`, `Network` и `Settings` под актуальную структуру продукта.
- Добавлены платформенные шаблоны личности, редактор имени личности, policy-level UX для DNS и улучшенные уведомления о загрузке движков.

### Network и DNS

- Реализованы DNS blocklists, suffix denylist, сервисные ограничения и редактируемые уровни политики.
- Добавлена глобальная VPN-политика, kill-switch и live gateway enforcement.
- Поддержаны route runtime и managed provisioning для `sing-box`, `openvpn`, `amneziawg` и `tor`.

### Extensions и Identity

- Реализована библиотека расширений с назначением профилям и автоустановкой по движку.
- Добавлены реалистичные identity templates для Windows, macOS, Linux, iOS и Android.
- Доработаны автоматический и ручной режимы fingerprint-управления, валидация согласованности и привязка шаблонов к реальным данным.

### Release

- Добавлены локальные preflight/release-скрипты и контуры security/vulnerability gates.
- Сборка Windows installer вынесена в `scripts/build-installer.ps1`; installer публикуется как основной release-артефакт.
- Fallback-installer переработан в wizard-установщик с нормальной иконкой, ярлыками, uninstall-записью и деинсталлятором.
- Исправлены installer-flow, раскладка payload, создание ярлыков `Cerbena Browser.lnk`, установка `cerbena.exe` и обработка uninstall при открытом приложении.
- Release-скрипт `scripts/release.ps1` теперь умеет автоматически инициализировать git-репозиторий, создавать ветку `main`, добавлять `origin` `https://github.com/BerkutSolutions/cerbena-browser.git` и выполнять первый push с bootstrap-коммитом `v1.0.0`, если репозиторий или remote еще не настроены.

### Docs

- Обновлены `README.md`, `README.en.md`, wiki entrypoints и release runbook под текущее состояние UI, release-потока и installer-публикации.
- В корневой `README` добавлены актуальные скриншоты интерфейса из `static/img`.
