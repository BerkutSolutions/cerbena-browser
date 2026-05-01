---
title: Network Policy
sidebar_position: 5
---

The Cerbena Browser policy engine must return deterministic and explainable decisions.

## Evaluation order

1. hard constraints
2. route policy
3. DNS policy
4. domain/service policy
5. exceptions

## Hard constraints

- deny on missing profile context;
- deny on unavailable required `VPN`;
- deny on broken `TOR-only` assumptions;
- deny on invalid or unloaded policy bundle.

## Explainability

Every decision should include:

- final action;
- selected route;
- matched rules;
- reason code;
- effective DNS;
- conflict-resolution trace.
