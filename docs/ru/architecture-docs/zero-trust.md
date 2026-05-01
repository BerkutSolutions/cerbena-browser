---
title: Zero-trust и backend enforcement
sidebar_position: 6
---

Cerbena Browser исходит из того, что `UI` может ошибаться, отставать по состоянию или быть обойден.

## Следствия

- backend повторно валидирует каждое чувствительное действие;
- authorization и scope checks обязательны для local API и `MCP`;
- launch, wipe, sync, network policy и extension operations не должны доверять только клиентской логике;
- audit trail обязателен для критичных операций.

## Что это дает

- защиту от "UI-only security";
- воспроизводимое поведение при автоматизации;
- предсказуемые release gates и traceability.
