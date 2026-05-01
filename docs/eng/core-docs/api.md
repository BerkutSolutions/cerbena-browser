---
title: Local API and MCP
sidebar_position: 8
---

Cerbena Browser exposes local automation surfaces, but all of them remain subject to zero-trust validation.

## Local API

Primary uses:

- open links through the default profile;
- automate launcher workflows;
- integrate with external desktop tools and local agents.

## MCP

The `MCP` layer exposes tools for profiles and policy workflows, but:

- scope is constrained by backend contracts;
- sensitive actions require authorization;
- critical operations are audited.

## Guardrails

- never rely on UI checks alone;
- never execute profile operations without explicit profile context;
- never widen scope outside backend authorization.

See [Local API and MCP integrations](../integration-docs/local-api-and-mcp.md) for integration-focused guidance.
