---
title: Интеграции local API и MCP
sidebar_position: 1
---

Cerbena Browser допускает локальную автоматизацию, но не жертвует ради нее zero-trust-принципами.

## Local API

Подходит для desktop automation и controlled link dispatch.

## MCP

Подходит для agent-driven сценариев, где нужен ограниченный набор инструментов для работы с профилями и политиками.

## Обязательные guardrails

- явный profile scope;
- backend authorization;
- audit events;
- отказ при невалидном контексте.
