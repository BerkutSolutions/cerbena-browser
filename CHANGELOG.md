# Журнал изменений

## 1.0.1 — Стабилизация релиза

### Core

- Исправлен Windows runtime-flow установленного приложения: убраны лишние shell-вызовы при запуске браузеров, из-за которых вспыхивали `cmd` и `powershell`, а запуск мог подвисать.
- Стабилизирован трекинг процессов профиля для Wayfern и Camoufox без циклического PowerShell-поллинга.
- Исправлен flapping-тест `snapshots_retention_and_quarantine_work` в sync-контуре: поведение retention/quarantine стало детерминированным и совпадает с release-contract ожиданиями.

### UI

- Версия продукта обновлена до `1.0.1` во всех пользовательских точках: desktop shell, docs-home, settings/update surface и mock/update fallback state.
- Обновлены служебные версии в extension/default metadata и policy-extension manifest, чтобы UI и runtime больше не расходились по release baseline.

### Docs

- Документация и Docusaurus release-site переведены на `1.0.1`.
- GitHub Pages сборка исправлена для project-site base path `/cerbena-browser/`, чтобы опубликованный сайт визуально совпадал с локальной сборкой.
- Убран workflow-guard, из-за которого `docs-pages / deploy` пропускал фактический деплой после успешного build.

### Release

- Workspace, desktop package, Tauri config и release metadata обновлены до `1.0.1`.
- Release/update user-agent строки синхронизированы с текущей версией пакета.
- Полный release-flow подготовлен к выпуску `v1.0.1`.
