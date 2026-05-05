# Журнал изменений

## 1.0.26

Тестовая версия по проверке работоспособности обновлений

### Обновления

- Усилено сквозное логирование апдейтера: вкладка `Logs` теперь читает persisted runtime log из файла, а `ui/desktop/src-tauri/src/update_commands.rs` и helper-скрипты применения `.msi`/`.zip` пишут туда шаги handoff/apply/relaunch, чтобы финальные события не терялись после закрытия окна апдейтера.
- Reworked local updater e2e reliability and observability: `scripts/local-updater-e2e.ps1` now defaults to a single attempt (`PublishedRetryCount=0`), emits explicit per-attempt timeout/remaining-budget diagnostics, and prevents hidden long retry tails near global deadline.
- Hardened published updater e2e fail-fast path in `scripts/published-updater-e2e.ps1`: capped MSI helper timeout (`max 120000ms`), added periodic `applying diagnostics` (msiexec match count + staged MSI log existence/size), and aligned hard-stall abort messaging/time budget for faster, clearer failures.
- Expanded MSI builder telemetry in `scripts/build-installer.ps1` and snapshot build telemetry in `scripts/local-updater-e2e.ps1` with per-stage elapsed timers so long phases are observable instead of silent.
