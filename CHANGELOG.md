# Журнал изменений

## 1.1.2

### Ядро
- Исправлен баг в логике updater: после msiexec exitCode=0 статус становился applied_pending_relaunch, но новый процесс всё ещё был 1.1.0, и код снова запускал reuse staged asset → pending apply по кругу.
- Предотвращен бесконечный цикл повторного применения одного и того же staged MSI: при состоянии `applied_pending_relaunch` и старой текущей версии авто-переапплай блокируется с явной ошибкой.
- Усилена post-apply проверка updater-helper: после `msiexec` резолвится фактический `INSTALLDIR` из verbose-лога, проверяется версия `cerbena.exe`, и при несовпадении с целевой версией relaunch останавливается (fail-closed с диагностикой).
