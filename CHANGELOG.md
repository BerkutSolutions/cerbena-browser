# Журнал изменений

## 1.0.8 — фикс нативной совместимости Amnezia и выравнивание VPN UI

### Сетевой runtime

- Путь установки нативной совместимости `AmneziaWG` теперь запускает administrative-extract для MSI через скрытый PowerShell `Start-Process`, а не через прежний прямой вызов `msiexec`, что убирает видимое окно `Windows Installer` и исправляет `msiexec administrative extract failed (code Some(1639))` во время подготовки шлюза и runtime.
- Дополнительные launcher-side проверки runtime и route helper-процессы теперь тоже используют скрытый запуск Windows-подпроцессов, поэтому при старте профиля и поднятии маршрута должно остаться меньше мигающих консолей.

### VPN UI

- Во вкладке `VPN` модалки профиля теперь показываются те же уведомления network sandbox, что и в глобальном фрейме `Сетевая изоляция профилей`: предупреждения о `compatibility-native`, system-wide рисках, container-изоляции, blocked-route hints и локализованные причины выбора стратегии.
- Смена режима изоляции профиля в модалке теперь сразу обновляет preview и связанные предупреждения, поэтому подсказки следуют за выбранным режимом, а не остаются в старом состоянии.
- Исправлена регрессия состояния модалки, из-за которой `Предпочитаемый режим изоляции` визуально отпрыгивал обратно к ранее сохранённому значению после каждого preview-refresh, хотя предупреждения уже были пересчитаны для нового выбора.

### UI профилей

- В DNS-модалке профиля триггер выбора блок-листов теперь снова является стабильной кнопкой `Выбрать блок-листы`, а не подставляет в себя список текущих пресетов; фактический выбор отображается только внутри меню с чекбоксами.

### Установщик и локальная проверка

- Локальная проверка установленной сборки подтвердила, что fallback uninstaller теперь действительно полностью удаляет браузер и принадлежащие ему runtime-артефакты; это состояние зафиксировано в локальных `.work`-заметках для будущих AI-сессий.

## Local verification — startup/shutdown UX and uninstall reconciliation

### Производительность и launcher UX

- В desktop shell добавлены lifecycle-overlay состояния `Подготовка Cerbena` и `Завершение работы Cerbena`, которые показывают реальные этапы bootstrap/shutdown вместо немого зависания UI.
- Startup janitor убран с синхронного `setup`-пути, а shutdown cleanup больше не выполняется дважды через `window_close` и `CloseRequested`, поэтому окно должно быстрее появляться и заметно быстрее закрываться.
- Windows subprocess cleanup для `taskkill` и launcher-side `docker` probe/cleanup теперь запускается в скрытом режиме, чтобы на старте и при выходе не мигали консольные окна.
- Если в текущей сессии launcher не поднимал ни профильные процессы, ни network stack, shutdown больше не пытается делать тяжёлый process/runtime cleanup “на всякий случай”, поэтому быстрый запуск и сразу закрытие не должны зависать.
- Стартовая lifecycle-модалка убрана: shell снова появляется сразу, без короткого промежуточного окна инициализации, которое только подчёркивало позднюю догрузку CSS.

### Установка и uninstall

- Launcher теперь при старте восстанавливает uninstall-регистрацию в `HKCU\\Software\\Microsoft\\Windows\\CurrentVersion\\Uninstall\\Cerbena Browser`, синхронизируя текущие `DisplayVersion`, install-root, icon и uninstall-команды с установленным `Cerbena Browser Uninstall.exe` после ZIP-обновлений.
- Fallback uninstaller больше не считает собственный `Cerbena Browser Uninstall.exe` “запущенным браузером”, поэтому подтверждение закрытия должно реально завершать оставшиеся процессы Cerbena вместо ложного self-match.

### Апдейтер и CI

- Автоматическая подготовка обновлений теперь по умолчанию включена для новых `app_update_store`, чтобы свежая установка не стартовала с выключенным чекбоксом auto-update.
- GitHub workflow и launcher contract-тесты больше не зависят от локального `scripts/release.ps1`: release-artifact контракт в CI проверяется через Rust contract tests, smoke preflight на GitHub не запускает Docker-local preflight, а `docker-runtime-preflight.ps1` мягко пропускает managed-network probe в runner-окружениях без `bridge` plugin.

### Сетевой runtime

- MSI-based network runtime extraction для `OpenVPN` и `AmneziaWG` теперь запускается с корректным hidden `msiexec` administrative-extract path и без всплывающего Windows Installer help-окна, а launcher-side `docker`, `tasklist` и service probes тоже переведены в скрытый режим, чтобы поднятие шлюза и закрытие браузера не мигали консольными окнами.
