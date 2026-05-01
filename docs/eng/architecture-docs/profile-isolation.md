---
title: Profile Isolation
sidebar_position: 4
---

A compromise in one profile must not expose another profile.

## Isolation domains

- filesystem;
- cryptographic context;
- network policy;
- extensions;
- cache and session data.

## Forbidden operations

- reading another profile path outside the import/export flow;
- reusing key references;
- reusing extension storage;
- applying route or DNS rules from one profile to another.

## Required checks

- the backend must resolve the target profile before mutable actions;
- missing profile context must fail closed;
- cleanup routines must remain idempotent.
