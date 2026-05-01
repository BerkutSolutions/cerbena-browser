---
title: Интеграция KeePassXC
sidebar_position: 3
---

Cerbena Browser поддерживает `KeePassXC` через официальное расширение `KeePassXC-Browser` и локальный `native messaging`-`bridge`, который используют браузеры на базе Chromium.

## Как это работает

1. Оператор включает `Разрешить интеграцию KeePassXC` в настройках безопасности профиля.
2. Launcher подключает расширение `KeePassXC-Browser` только для этого профиля.
3. При запуске профиля backend подготавливает `native messaging`-`manifest` для `org.keepassxc.keepassxc_browser`.
4. `Manifest` указывает на локальный бинарник `keepassxc-proxy.exe`, который обычно установлен по пути `C:\Program Files\KeePassXC\keepassxc-proxy.exe`.
5. Расширение браузера подключается к `native`-`host` через `stdio`, а дальнейшее подтверждение выполняет `KeePassXC`.

## Почему изоляция не нарушается

- Интеграция включается явно и отдельно для каждого профиля. По умолчанию она не активна.
- `Bridge` разрешает только `origin` расширения `KeePassXC-Browser`, перечисленный в `allowed_origins` `manifest`-файла.
- Launcher по-прежнему хранит файлы расширения внутри `profile-scoped runtime`-каталога.
- Защищенные профили остаются под backend-правилами безопасности и могут по-прежнему отклонять опасные сочетания настроек.
- `Bridge` не открывает сайтам произвольный доступ к локальному API. Сайт общается только с расширением, а расширение общается только с разрешенным `native`-`host`.

## Путь соединения

- Веб-страница -> `content/background`-скрипты `KeePassXC-Browser`
- Расширение -> `native messaging` `host` `org.keepassxc.keepassxc_browser`
- `Native host` -> `keepassxc-proxy.exe`
- Proxy -> локальный `KeePassXC`

Это означает, что сайт не получает прямой доступ к системе. Локальный переход ограничен только разрешенным расширением и установленным `bridge` `KeePassXC`.

## Эксплуатационные замечания

- Если для профиля не включен флаг `Разрешить интеграцию KeePassXC`, launcher не разрешит `bridge` для этого профиля.
- Если включен `Запретить запуск расширений`, доступным остается только явно разрешенный путь `KeePassXC`.
- Если `Разрешить расширениям доступ к системе` выключен, остальные расширения с системной интеграцией остаются заблокированными backend-фильтрацией при запуске.

## Диагностика

- `Access to the specified native messaging host is forbidden` обычно означает, что `origin` расширения в браузере не совпадает со списком `allowed_origins` в `manifest`.
- `KeePassXC proxy executable was not found` означает, что `KeePassXC` отсутствует или установлен в нестандартный путь, который launcher не смог определить.
- Для диагностики откройте `Diagnostics -> Logs` и ищите строки с `[keepassxc-bridge]`, где видно регистрацию `manifest` и определение `runtime-origin`.
