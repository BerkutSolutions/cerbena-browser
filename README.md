# Berkut Solutions - Cerbena Browser

<p align="center">
  <img src="static/img/logo.png" alt="Cerbena Browser logo" width="220">
</p>

[English version](README.en.md)

[GitHub](https://github.com/BerkutSolutions/cerbena-browser)

`Cerbena Browser` - настольная платформа защищенного браузинга с zero-trust-подходом, полной изоляцией профилей, детерминированным policy engine для маршрутизации, DNS и сервисных ограничений, а также управляемой средой запуска для `Wayfern` и `Camoufox`.

## О продукте

Cerbena Browser не является "надстройкой над обычным браузером". Это отдельный launcher и runtime-контур, который управляет:

- изолированными профилями для `Wayfern` и `Camoufox`;
- профильно-специфичными маршрутами `direct` / `proxy` / `vpn` / `tor` / `hybrid`;
- DNS-политиками, blocklists, сервисными ограничениями и черными списками доменов;
- библиотекой расширений, политиками их назначения и автоустановкой в профиль;
- шаблонами личности и полным ручным fingerprint-редактором;
- panic cleanup, пользовательскими сертификатами, локальным API, MCP и audit-контуром;
- зашифрованными sync/backup-сценариями и локальными release/preflight-проверками.

## Ключевые возможности

- Полная изоляция профилей: данные, кэш, ключи, расширения и сетевые политики разделены.
- Zero-trust backend enforcement: UI не является границей доверия.
- Kill-switch при обязательном VPN-маршруте.
- Глобальная и профильная DNS-фильтрация с редактируемыми уровнями политики.
- Реалистичные шаблоны личности для Windows, macOS, Linux, iOS и Android.
- Panic frame и экстренная очистка профиля с управляемым retention.
- Библиотека расширений с назначением профилям и автоустановкой по движку.
- Ручные и управляемые release/preflight-скрипты, локальные security/vulnerability gates.
- Windows installer wizard с ярлыками, uninstall-записью и деинсталлятором.

## Скриншоты

### Главная

![Главная](static/img/screen-1.png)

### Профиль и идентичность

![Профиль и личность](static/img/screen-2.png)

### DNS-политики и фильтры

![DNS](static/img/screen-3.png)

### Сетевые шаблоны и маршрутизация

![Сеть](static/img/screen-4.png)

### Расширения и безопасность

![Расширения и безопасность](static/img/screen-5.png)

## Технологический стек

- Desktop shell: `Tauri 2` + `Rust`
- Frontend: локальный `web UI`
- Workspace: `Cargo` multi-crate
- Документация: `Docusaurus`
- Движки: `Wayfern`, `Camoufox`
- Managed runtime: `sing-box`, `openvpn`, `amneziawg`, `tor`

## Быстрый старт

### Требования

- `Rust` toolchain
- `Node.js` LTS + `npm`
- Windows как основная целевая desktop-платформа

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

### Release-проверка и упаковка

```powershell
powershell -ExecutionPolicy Bypass -File .\scripts\release.ps1 -Mode package -CompactOutput
```

### Сборка установщика

```powershell
powershell -ExecutionPolicy Bypass -File .\scripts\build-installer.ps1
```

### Что публиковать в GitHub Releases

В GitHub Releases рекомендуется выкладывать:

- `cerbena-browser-setup-<version>.exe` как основной Windows installer;
- при необходимости `cerbena-windows-x64.zip` как переносимый release bundle;
- `checksums.txt` и `release-manifest.json` как сопроводительные release-артефакты.

Installer `.exe` собирается локально через `scripts/build-installer.ps1` и предназначен для публикации в релизах как основной способ установки продукта.

## Документация

- Индекс документации: [docs/README.md](docs/README.md)
- Русская wiki: [docs/ru/README.md](docs/ru/README.md)
- Английская wiki: [docs/eng/README.md](docs/eng/README.md)
- UI и сценарии работы: [docs/ru/core-docs/ui.md](docs/ru/core-docs/ui.md)
- Сеть и маршрутизация: [docs/ru/core-docs/network-routing.md](docs/ru/core-docs/network-routing.md)
- DNS и фильтры: [docs/ru/core-docs/dns-and-filters.md](docs/ru/core-docs/dns-and-filters.md)
- Безопасность: [docs/ru/core-docs/security.md](docs/ru/core-docs/security.md)
- Release runbook: [docs/ru/release-runbook.md](docs/ru/release-runbook.md)

## Полезные файлы

- Вклад разработчиков: [CONTRIBUTING.md](CONTRIBUTING.md)
- Политика безопасности: [SECURITY.md](SECURITY.md)
- Каналы поддержки: [SUPPORT.md](SUPPORT.md)
- История изменений: [CHANGELOG.md](CHANGELOG.md)
