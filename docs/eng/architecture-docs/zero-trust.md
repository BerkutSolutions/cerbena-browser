---
title: Zero-Trust and Backend Enforcement
sidebar_position: 6
---

Cerbena Browser assumes that the `UI` can be stale, incomplete, or bypassed.

## Consequences

- the backend re-validates every sensitive action;
- authorization and scope checks are mandatory for local API and `MCP`;
- launch, wipe, sync, network policy, and extension operations must not trust presentation logic alone;
- audit trail is required for critical operations.

## What this provides

- protection against UI-only security;
- predictable behavior under automation;
- clearer release gates and traceability.
