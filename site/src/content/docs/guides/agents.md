---
title: Using Steward with agents
description: Published machine-readable commands, scope, result semantics and recovery instructions.
---

Point your coding agent at these public, plain-text endpoints:

- [llms.txt](../../llms.txt): short index and operational boundaries.
- [llms-full.txt](../../llms-full.txt): the full agent command guide.

The full guide is published directly from the repository's `docs/AGENT_GUIDE.md`, so the public instructions and source reference stay together.

## A useful prompt

> Read https://samishal1998.github.io/steward/llms-full.txt. Check my installed Steward version, then list connected proxies and native routes with `--json`. Help me add an HTTP route for my already running app using an explicitly registered local proxy. Report the operation result and its verification.

The agent guide covers API-only connections, writable file authority, direct route lifecycle, ownership/drift, exit codes, partial inventories, and interrupted operation recovery. It does not turn a read-only connection into write authority.
