---
title: Изоляция профилей
sidebar_position: 4
---

Компрометация одного профиля не должна раскрывать другой профиль.

## Домены изоляции

- filesystem;
- cryptographic context;
- network policy;
- extensions;
- cache и session data.

## Запрещенные операции

- чтение чужого profile path вне import/export flow;
- переиспользование key references;
- переиспользование extension storage;
- применение route/DNS rules одного профиля к другому.

## Обязательные проверки

- backend должен резолвить target profile перед mutable-действием;
- отсутствующий profile context приводит к отказу;
- cleanup-процедуры должны быть идемпотентны.
