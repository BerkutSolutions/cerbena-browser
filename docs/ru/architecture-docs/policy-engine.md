---
title: Политика сети
sidebar_position: 5
---

Policy engine Cerbena Browser должен принимать детерминированные и объяснимые решения.

## Порядок вычисления

1. hard constraints
2. route policy
3. DNS policy
4. domain/service policy
5. exceptions

## Hard constraints

- deny при отсутствии profile context;
- deny при недоступном обязательном `VPN`;
- deny при нарушении `TOR-only`;
- deny при невалидном или незагруженном policy bundle.

## Explainability

Каждое решение должно иметь:

- итоговое действие;
- выбранный маршрут;
- matched rules;
- reason code;
- effective DNS;
- след конфликтных переопределений.
