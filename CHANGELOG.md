# Журнал изменений

## 1.1.5

Тестовая версия

### security

- Linux-профили Chromium/LibreWolf получили fail-safe guardrails по sandbox readiness: добавлены диагностика причин fallback, предупреждающая модалка при запуске без песочницы и Linux-only status tool в `Settings -> Tool status`.
- Для Linux runtime добавлены проверки и диагностика AppArmor/userns-предпосылок; fallback в `--no-sandbox` сохранён только как режим совместимости, приоритет — запуск с системной песочницей.
- Сохранён Windows security baseline: Linux-изменения изолированы в platform-specific слоях без ослабления текущего MSI/update trust-path.

### core

- Зафиксирован контракт первой Debian/Linux slice для `.deb` c отдельным platform-layer подходом.
- Вынесены Windows/Linux platform-specific интеграции desktop shell в отдельные модули (dialog pickers, certificate metadata, release signature verification, secret-store derivation).
- Linux auto-apply update path переведён в fail-closed модель вместо попыток использования Windows-only helper flow.
- Для Linux runtime обновлены engine download/launch пути: исправлены executable-биты, добавлена более устойчивая подготовка Chromium helpers и Linux-ветка загрузки LibreWolf.
- Улучшен process tracking для корректного сброса состояния профиля после завершения браузерного процесса.

### ui

- Добавлен Linux-only инструмент `linux-browser-sandbox` в `Settings -> Tool status` с action `Configure` и пошаговой инструкцией по безопасной настройке AppArmor/userns.
- Для Linux Docker в `Tool status` добавлен CLI guide (вместо Desktop-only страницы), включая пост-установочные шаги для доступа к `docker.sock`.
- Улучшена локализация RU/EN для новых Linux runtime/sandbox/Docker сценариев.

### release / ci

- Добавлен отдельный Linux Tauri config `ui/desktop/src-tauri/tauri.linux.conf.json` и `ui/desktop -> npm run build:deb` для сборки `.deb` без изменения основного Windows `tauri.conf.json`.
- Release metadata/publish flow расширены additively: опциональный Debian-артефакт из `build/linux/<version>/` попадает в `release-manifest.json`, `checksums.txt`, `checksums.sig` и GitHub Release upload без ослабления обязательного Windows MSI контракта.
- Подготовлен GitHub Actions workflow для автоматической сборки desktop bundles (`msi + deb`) и публикации релизных артефактов с signed metadata.

### docs / ops

- Обновлены RU/EN release runbook, trust, troubleshooting и operator release gates для Debian `.deb` install/uninstall smoke-проверок и manual-download support boundary.
- Добавлены operator-only helper scripts для Linux-transfer копии проекта (без `node_modules`/`target`/`build`) и загрузки на Ubuntu VM по `SSH/SFTP`.
- Desktop npm tooling для Linux readiness отвязан от `powershell` (`style:sync`/`dev:web:stop`), Windows-specific поведение сохранено в отдельных ветках.
- Снижен шум Linux-сборки за счёт platform-gating Windows-only участков (`install_registration`, `keepassxc_bridge`, `network_runtime`, `route_runtime`) и cleanup неактуальных предупреждений.
