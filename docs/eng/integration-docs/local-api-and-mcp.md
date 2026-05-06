---
title: Local API and MCP Integrations
sidebar_position: 1
---

Cerbena Browser allows local automation without relaxing zero-trust rules.

## Local API

Useful for desktop automation, local profile operations, and controlled link dispatch.

Implementation references:

- API module: `internal/api_local/src/lib.rs`.
- Security checks and guardrails: `internal/api_local/src/security.rs`.
- Home/default browser/panic/search flows: `internal/api_local/src/home.rs`, `default_browser.rs`, `panic.rs`, `search.rs`.
- Integration tests: `internal/api_local/tests/*`.

## MCP

Useful for agent-driven scenarios that need a bounded tool surface for profile and policy operations.

Implementation references:

- MCP module: `internal/api_mcp/src/lib.rs`.
- Test coverage: `internal/api_mcp/tests/api_mcp_tests.rs`.
- Contract linkage and local launcher checks: `cmd/launcher/tests/*`.

## Required guardrails

- explicit profile scope;
- backend authorization;
- audit events;
- fail-closed behavior on invalid context.

## Practical operator checklist

1. Validate Rust tests: `cargo test --workspace`.
2. Validate launcher contracts: `cargo test -p cerbena-launcher`.
3. Validate UI command bindings and localization: `cd ui/desktop && npm test`.
4. Ensure no API/MCP behavior bypasses profile boundary checks in backend code paths.

## Notes for automation consumers

- Prefer profile-bound operations over global mutable actions.
- Expect fail-closed errors when profile binding, authorization, or route constraints are missing.
- Treat UI state as informational only; backend checks are authoritative.
