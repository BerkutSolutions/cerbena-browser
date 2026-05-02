# Журнал изменений

## 1.0.6-2 — reboot-финализация для trusted updater

### Тесты и release gates

- Релиз `1.0.6-2` выпущен как follow-up к `1.0.6-1`, чтобы проверить уже полный apply/relaunch сценарий после успешного secure-update staging.

### Апдейтер

- После успешной проверки и staging updater теперь завершает поток в явном состоянии `Готово к перезапуску` / `Ready to restart`, а финальная кнопка меняется с обычного `Close` на действие перезапуска и применения обновления.
- Если новой версии нет или secure-update завершился ошибкой, финальная кнопка по-прежнему просто закрывает окно без попытки apply-path.
- Закрытие updater-окна через кнопку и системный close-event теперь запускает `pending_apply_on_exit`, zip-helper применяет staged bundle и затем автоматически перезапускает `cerbena.exe`, а update-store очищается после успешного перехода на новую версию.

## 1.0.6-1 — live-проверка trusted updater после transport-fix

### Тесты и release gates

- Релиз `1.0.6-1` выпущен как прямой smoke-check после ручной установки `1.0.6`, чтобы проверить, что trusted updater снова видит hotfix-релизы и открывает standalone update flow без fallback-сценариев.

### Апдейтер

- Версионная линия `1.0.6-1` использует уже исправленный verifier transport из `1.0.6`, поэтому должна обновляться с установленной `1.0.6` через обычные кнопки `Проверить сейчас` и auto-start при включенном чекбоксе.

## 1.0.6 — transport-fix для secure updater

### Тесты и release gates

- Релиз `1.0.6` выпущен как исправление живого updater-regression из `1.0.5`, где подпись релиза ломалась не на GitHub assets, а на локальной передаче checksum payload в PowerShell verifier.

### Апдейтер

- Проверка `checksums.sig` больше не передает base64 payload через хвостовые аргументы `powershell -Command`, потому что в установленной среде такой вызов не гарантирует попадание значений в `$args`; verifier теперь получает checksum/signature через явные environment variables.
- Добавлен regression-тест на transport-путь PowerShell verifier, чтобы secure updater больше не зависал и не падал на этапе `Security validation` из-за пустых аргументов.
- Логика `Проверить сейчас` и автозапуска updater при включенном чекбоксе сохранена: после ручной установки этой версии дальнейшие релизы снова должны открываться через trusted updater без ручного обхода.

## 1.0.5 — автостарт апдейтера и проверка живого обновления

### Тесты и release gates

- Релиз `1.0.5` выпущен как отдельная проверка живого trusted-update пути поверх установленной `1.0.4-1`, чтобы можно было валидировать ручной и автоматический сценарии обновления через GitHub Release.

### Апдейтер

- При включенном флажке автоматической подготовки обновлений launcher теперь не ждёт первого 15-минутного scheduler tick: проверка новой версии выполняется уже при запуске приложения и при наличии нового релиза сразу открывает standalone updater.
- Ручная кнопка `Проверить сейчас` сохраняет прежний behaviour для карточки состояния, но при обнаружении новой версии сразу переводит управление в полноценное окно secure updater вместо тихой записи статуса в локальный store.

## 1.0.4-1 — hotfix апдейтера и публикации релиза

### Тесты и release gates

- `scripts/release.ps1` теперь автоматически собирает тело GitHub Release из соответствующей секции `CHANGELOG.md`, создает `v{версия}` при первом publish и обновляет описание релиза при повторной публикации вместе с trust-assets.
- `cmd/launcher/tests/release_pipeline_contract_tests.rs` дополнительно фиксирует контракт на `gh release create/edit`, `--notes-file` и публикацию release assets из changelog-driven потока.

### Апдейтер

- Ручная кнопка `Проверить сейчас` теперь не ограничивается обновлением локального status-store: при наличии нового релиза она сразу запускает standalone secure updater flow, чтобы пользователь видел полноценное окно проверки и handoff-пайплайн.
- Сравнение версий в updater теперь считает hotfix-сборки вида `1.0.4-1` новее `1.0.4`, поэтому установленная `1.0.4` корректно предлагает обновление на этот релиз.
- Проверка `checksums.sig` стала устойчивее к вариантам переводов строк в `checksums.txt`, а release artifacts теперь подписывают canonical LF-представление checksum-манифеста, чтобы trusted update не ломался на newline-разночтениях между publish и verify.

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
