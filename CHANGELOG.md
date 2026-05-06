# Журнал изменений

## 1.0.29

### UI
- Профили Firefox-family теперь настраиваются и отображаются только через `LibreWolf`: удалены выбор `Camoufox`, отдельный статус-инструмент и связанные подсказки/диагностика.
- Вкладка `Расширения` теперь показывает отдельные Chromium/LibreWolf build-варианты одного расширения в общем hybrid-элементе и позволяет удалять вариант по конкретному движку в модалке `Источники`.
- Меню `Импорт` в `Расширениях` унифицировано до действий `Файл`, `Папка`, `Архив`; локальный архив вынесен под `Файл`, а старые отдельные верхние кнопки убраны.
- Карточка расширения по ЛКМ снова всегда открывает модалку; внутри модалки добавлена layout-структура с левым rail-блоком и правыми фреймами `Источники`, `Профили`, `Теги`, синхронизированными по стилю с `Settings`.

### Безопасность
- Удалены активные и compatibility-only пути `Camoufox` из Rust-моделей, launcher runtime, bootstrap state и сессионной диагностики; проект теперь работает только в режиме `LibreWolf` без миграционных переписывателей persisted metadata/registry значений.
- Пользовательские корневые сертификаты для Firefox-family теперь материализуются в изолированное профильное хранилище `policy/librewolf-certificates`, проходят fail-closed валидацию и очищаются при stop/delete/crash cleanup без системного импорта.
- KeePassXC native messaging для `LibreWolf` теперь использует Firefox-совместимый manifest с `allowed_extensions` и launcher-регистрацию host keys под `Mozilla`/`LibreWolf`, сохраняя профильную диагностику и не ломая существующий `Wayfern` путь.
- Launcher теперь умеет импортировать unpacked extension folders, валидирует их `manifest.json` и для Firefox-family профилей репакует их в managed `.xpi` под `LibreWolf`.

### Документация
- Зафиксированы T8-1/T8-7, обновлены traceability и backlog под `LibreWolf` как единственный поддерживаемый Firefox-family runtime без обратной совместимости для `Camoufox`.
- README и operator runtime runbook теперь явно фиксируют `LibreWolf`-only cutover, а release/preflight-поверхности и launcher docs-quality тесты не допускают возврат `Camoufox` в живые docs/scripts.
