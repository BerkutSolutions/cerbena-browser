# Журнал изменений

## 1.0.4 — Изоляция трафика, контейнерный runtime и polish UI

### Тесты и release gates

- Добавлены отдельные Rust-regression тесты `traffic_isolation_*` для launcher-side выбора режимов `isolated`, `compatibility-native`, `container` и `blocked`, чтобы поломки в профильной изоляции трафика ловились до release.
- `scripts/local-ci-preflight.ps1` теперь выносит проверку traffic isolation в отдельный шаг `Traffic isolation regression tests`, а GitHub workflow `security-regression-gate.yml` запускает тот же фильтр `cargo test traffic_isolation`.
- Локальный Docker vulnerability sandbox больше не обрывает `trivy` слишком рано: для файлового сканирования увеличен внешний таймаут и добавлен внутренний `--timeout 10m`, чтобы release `check` стабильно доходил до конца на прогретом кеше.
- Installer cleanup доведен до полного удаления launcher-owned state: uninstall теперь явно вычищает профили, engine/network runtimes, updater staging, extension packages, DPAPI secret envelope, сохранённые JSON stores и Docker-артефакты контейнерной изоляции, а контрактные тесты проверяют это покрытие для обоих installer-путей.
- `scripts/release.ps1` теперь при `publish` и `full` не только пушит git-тег, но и создает или обновляет GitHub Release через `gh`, прикладывая `checksums.txt`, `checksums.sig`, `release-manifest.json`, portable bundle, standalone updater и installer, чтобы trusted updater не ломался на пустых release assets.

### UI

- Во вкладке `VPN` переработана модель изоляции: глобальный фрейм `Сетевая изоляция профилей` теперь показывается только для глобального VPN и глобального маршрута по умолчанию, а во вкладке `VPN` модалки профиля появился отдельный фрейм `Сетевая изоляция профиля`, который сразу подстраивается под выбранный шаблон и предлагает только совместимые режимы.
- Dropdown-меню действий в таблицах и всплывающие подсказки переполнения тегов вынесены в отдельные overlay-слои поверх интерфейса, поэтому они больше не прячутся за строками и кнопками.
- Для долгого запуска профиля добавлена компактная progress-модалка с этапами подготовки профиля, сети, контейнера, импорта runtime-конфига и старта браузера.
- Во вкладке `Трафик` скорректирована сетка таблицы: колонки `Время` и особенно `Запрос` стали шире, а `Ответ` теперь занимает меньше места и лучше соответствует своему содержимому.

### Безопасность

- Launcher переведен на явные стратегии сетевой изоляции `isolated`, `compatibility-native`, `container` и `blocked`, чтобы profile-scoped маршруты не уходили в неявный system-wide fallback.
- Обычный путь для профильных маршрутов `Amnezia` больше не должен поднимать persistent `AmneziaWG` tunnel/service на уровне всей Windows: стандартный трафик идет через локальный `traffic gateway` и userspace runtime.
- Kill-switch теперь стабильно блокирует трафик, если требуемый route runtime реально недоступен, а gateway пишет понятную причину блокировки в журнал `Трафик`.
- Для `Camoufox` отключен встроенный cursor beacon `userChrome.decoration.cursor`, а стартовая страница по умолчанию нормализуется до `https://duckduckgo.com`.

### VPN и маршрутизация

- Добавлен полноценный `network sandbox` store с автоматической миграцией legacy-профилей в явные сетевые стратегии и с централизованным launcher-side lifecycle для profile-scoped network stacks.
- Контейнерная изоляция доведена до рабочего состояния: Cerbena поднимает profile-scoped helper-контейнеры и отдельные Docker-managed сети, чтобы маршруты работали внутри изолированной среды профиля, а не через сетевой стек всей системы.
- Внутри container-runtime теперь поддерживаются совместимые шаблоны `proxy`, `V2Ray/XRay`, userspace `WireGuard/Amnezia`, single-node `OpenVPN`, а также `TOR` с `obfs4`, `snowflake` и `meek_lite`.
- Для native-only `AmneziaWG` launcher поднимает `amneziawg-go`, `awg-quick` и локальный `SOCKS`-шлюз внутри profile-scoped helper-контейнера, чтобы сохранить нативную совместимость без перенаправления всего трафика хоста.
- Container helper-образ обновлен до ревизии `2026-05-02-r5`, а встроенный `sing-box` теперь собирается с `with_utls`, поэтому container-backed `VLESS Reality` больше не падает из-за отсутствия `uTLS`.
- В `traffic gateway` исправлены узкие места для `CONNECT`-туннелей: TLS-байты больше не теряются между браузером и runtime, handshake-таймауты не висят на уже установленном мосте, а принятые Windows-сокеты переводятся обратно в blocking-режим перед долгоживущим bridge.

### Профили

- Для `Camoufox` восстановлена корректная инициализация поисковых систем: launcher очищает устаревшие search-кэши профиля, публикует Firefox-совместимый каталог движков через enterprise policy и добавляет классические search-plugin файлы в `distribution/searchplugins/common`.
- Профильный выбор поисковика остается изолированным через `user.js`, поэтому default search больше не ломает `SearchService` и не оставляет пустой список движков.
- Профильный запуск `Wayfern` и `Camoufox` теперь стабильнее синхронизирован с network stack lifecycle, поэтому контейнерные маршруты не должны преждевременно падать или терять интернет сразу после старта.

### Очистка и uninstall

- Launcher, janitor и uninstall теперь дополнительно вычищают legacy-хвосты `AmneziaWGTunnel$awg-*`, `network_sandbox_store`, managed runtime-артефакты, profile-scoped helper-контейнеры, Docker-сети и helper-образ для контейнерной изоляции.
- Cleanup-поток приведен к единой модели ownership: launcher отслеживает, какие gateway/runtime/container-артефакты были созданы для профиля, и освобождает их при stop, exit, crash-recovery и uninstall.

### Release и quality gates

- В локальный `scripts/local-ci-preflight.ps1` добавлены обязательные шаги `docker-runtime-preflight` и `vulnerability-gates`, чтобы повседневная предварительная проверка сразу валидировала доступность Docker runtime, source-contract контейнерного sandbox и основные security scanners.
- `scripts/release.ps1` теперь считает Docker runtime preflight и vulnerability gates частью режимов `check`, `package` и `full`, но не меняет publish-поток сверх уже существующих release-проверок.

### Документация

- Документация обновлена под релиз `1.0.4`: описаны явные стратегии изоляции, новая модель глобальной и профильной сетевой изоляции, container-backed route runtime, kill-switch, cleanup и актуальные UI-сценарии для сети и трафика.
