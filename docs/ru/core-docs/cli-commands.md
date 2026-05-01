---
title: CLI-команды
sidebar_position: 10
---

CLI launcher предназначен для базовых локальных операций и релизных проверок.

## Поддерживаемые команды

### `init-profile`

Создает профиль:

```bash
cargo run -p cerbena-launcher -- init-profile --root <dir> --name <name> --engine wayfern
```

### `list-profiles`

Показывает список профилей:

```bash
cargo run -p cerbena-launcher -- list-profiles --root <dir>
```

### `ack-wayfern-tos`

Подтверждает обязательные правила для `Wayfern`:

```bash
cargo run -p cerbena-launcher -- ack-wayfern-tos --root <dir> --profile-id <uuid>
```

### `build-launch-plan`

Строит план запуска для профиля:

```bash
cargo run -p cerbena-launcher -- build-launch-plan --root <dir> --profile-id <uuid> --binary <path>
```

### `update-apply`

Запускает ручной поток обновления с проверкой подписи:

```bash
cargo run -p cerbena-launcher -- update-apply --version <semver> --signature <sig>
```

### `desktop updater preview`

Открывает самостоятельный экран защищённого апдейтера в режиме сухого прогона без установки файлов:

```bash
cargo run --manifest-path ui/desktop/src-tauri/Cargo.toml -- --updater-preview
```
