# Berkut Solutions - Cerbena Browser

<p align="center">
  <img src="static/img/logo.png" alt="Cerbena Browser logo" width="220">
</p>

[English version](README.en.md)

[GitHub](https://github.com/BerkutSolutions/cerbena-browser)
[Wiki](https://berkutsolutions.github.io/cerbena-browser/)

`Cerbena Browser` — это настольная платформа защищенного браузинга с zero-trust-подходом, сильной изоляцией профилей, явными стратегиями сетевой изоляции и управляемым launcher/runtime-контуром для `Wayfern` и `Camoufox`.

## О продукте

Cerbena Browser — это не надстройка над обычным браузером. Это отдельный launcher и runtime-контур, который управляет:

- изолированными профилями для `Wayfern` и `Camoufox`;
- профильными режимами маршрутизации `direct`, `proxy`, `vpn`, `tor` и `hybrid`;
- явными стратегиями сетевой изоляции `isolated`, `compatibility-native`, `container` и `blocked`;
- DNS-политиками, blocklists, сервисными ограничениями и доменными блокировками;
- библиотекой расширений, политиками назначения и auto-install по движку;
- шаблонами личности и полным ручным редактором fingerprint-параметров;
- panic cleanup, пользовательскими сертификатами, локальным API, MCP и audit-сценариями;
- зашифрованными sync/backup-потоками и локальными release/preflight-проверками.

Проект обеспечивает:

- изоляцию профилей по данным, ключам, расширениям, cache и сетевой политике;
- fail-closed kill-switch, если обязательный VPN/runtime-маршрут недоступен;
- контейнерную изоляцию совместимых маршрутов, чтобы трафик оставался внутри profile-scoped sandbox-среды, а не менял сетевой стек всего хоста;
- зашифрованное хранение чувствительного состояния launcher и desktop-shell, включая миграцию legacy-данных в защищенный формат;
- сквозную защиту sync-полезной нагрузки с сохранением обратной совместимости для уже созданных данных;
- доверенный поток обновления через отдельный `cerbena-updater.exe`, проверку `checksums.sig`, сверку `SHA-256` и безопасную передачу на установку.

## Ключевые возможности

- Полная изоляция профилей: данные, cache, ключи, расширения и сетевая политика разделены.
- Zero-trust backend enforcement: UI не является границей доверия.
- Явные стратегии маршрутизации с container-backed сетевой изоляцией.
- Kill-switch при обязательном VPN-маршруте.
- Глобальная и профильная DNS-фильтрация с редактируемыми уровнями политики.
- Реалистичные шаблоны личности для Windows, macOS, Linux, iOS и Android.
- Panic frame и экстренная очистка профиля с управляемым retention.
- Библиотека расширений с назначением профилям и auto-install по движку.
- Локальные release/preflight-скрипты и security/vulnerability-проверки.
- Windows installer wizard с ярлыками, uninstall-регистрацией и деинсталлятором, который дополнительно удаляет launcher-managed сетевые и контейнерные следы.

## Скриншоты

### Главная

![Главная](static/img/screen-1.png)

### Расширения

![Расширения](static/img/screen-6.png)

### Профиль

![Профиль и личность](static/img/screen-2.png)

### Личность

![DNS](static/img/screen-3.png)

### DNS

![Сеть](static/img/screen-4.png)

### VPN и маршрутизация

![Расширения и безопасность](static/img/screen-5.png)

## Технологический стек

- Desktop shell: `Tauri 2` + `Rust`
- Frontend: локальный `web UI`
- Workspace: `Cargo` multi-crate
- Документация: `Docusaurus`
- Движки: `Wayfern`, `Camoufox`
- Managed runtime: `sing-box`, `openvpn`, `amneziawg`, `tor`, Docker-managed helper-контейнеры

## Быстрый старт

### Требования

- `Rust` toolchain
- `Node.js` LTS + `npm`
- Windows как основная desktop-платформа
- `Docker Desktop`, если нужен container-backed режим сетевой изоляции

### Проверки

```bash
cargo test --workspace
```

```bash
cd ui/desktop
npm ci
npm test
```

```bash
npm ci
npm run docs:build
```

### Запуск desktop UI

```bash
cd ui/desktop
npm run dev
```

## Release и installer

### Локальный preflight

```powershell
powershell -ExecutionPolicy Bypass -File .\scripts\local-ci-preflight.ps1 -CompactOutput
```

Локальный preflight теперь по умолчанию включает проверку Docker runtime, security gates и vulnerability gates.

### Release-проверка и упаковка

```powershell
powershell -ExecutionPolicy Bypass -File .\scripts\release.ps1 -Mode package -CompactOutput
```

### Сборка установщика

```powershell
powershell -ExecutionPolicy Bypass -File .\scripts\build-installer.ps1
```

### Что публиковать в GitHub Releases

В GitHub Releases обычно выкладываются:

- `cerbena-browser-setup-<version>.exe` как основной Windows installer;
- `cerbena-windows-x64.zip` как переносимый release bundle, если он нужен;
- `cerbena-updater.exe` как отдельный standalone-апдейтер;
- `checksums.txt`, `checksums.sig` и `release-manifest.json` как артефакты доверенной поставки.

Installer `.exe` собирается локально через `scripts/build-installer.ps1` и рассчитан на публикацию в релизах как основной способ установки. Та же pipeline должна удалять launcher-managed контейнеры, Docker-сети, helper-образ, managed runtime-артефакты и legacy-хвосты route-runtime при uninstall.

## Документация

- Индекс документации: [docs/README.md](docs/README.md)
- Русская wiki: [docs/ru/README.md](docs/ru/README.md)
- Английская wiki: [docs/eng/README.md](docs/eng/README.md)
- UI и рабочие сценарии: [docs/ru/core-docs/ui.md](docs/ru/core-docs/ui.md)
- Сеть и маршрутизация: [docs/ru/core-docs/network-routing.md](docs/ru/core-docs/network-routing.md)
- DNS и фильтры: [docs/ru/core-docs/dns-and-filters.md](docs/ru/core-docs/dns-and-filters.md)
- Безопасность: [docs/ru/core-docs/security.md](docs/ru/core-docs/security.md)
- Release runbook: [docs/ru/release-runbook.md](docs/ru/release-runbook.md)

## Полезные файлы

- Вклад разработчиков: [CONTRIBUTING.md](CONTRIBUTING.md)
- Политика безопасности: [SECURITY.md](SECURITY.md)
- Каналы поддержки: [SUPPORT.md](SUPPORT.md)
- История изменений: [CHANGELOG.md](CHANGELOG.md)
