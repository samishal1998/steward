---
title: Architecture & roadmap
description: Steward's independent route/service model, implemented management alpha, and deployment evidence boundaries.
---

## One executable, separate capabilities

Steward is one standalone Rust package. CLI and TUI use the same application logic.
The OS supervises applications; there is no required Steward daemon or AppTrail
server/database. Route and service commands remain independently useful, while
composition is explicit.

## Implemented in v0.2.0-alpha.2

- Caddy/Traefik native inventory, qualified file snapshots and discovery candidates.
- Direct HTTP/TLS route lifecycle with explicit JSON/Caddyfile/controller/directory authority.
- Saved managed plans with input revalidation and no implicit pruning.
- Writer locks, ownership receipts, operation history, guarded recovery and rollback.
- Proxy artifact installation, setup, upgrade and uninstall through native managers.
- systemd/launchd service lifecycle, enablement, status and bounded logs.
- Service-first/readiness-gated `up` and route-first `down` with explicit retention.
- SSH/container execution targets, dynamic completion and an interactive TUI.
- Versioned redacted export for the optional AppTrail reader.

Inventory does not grant ownership. Native configuration, route activation,
application readiness, DNS reachability and certificate trust are separate facts.

## Verified boundaries

Source acceptance uses real user-scope systemd/launchd workflows on Linux x86_64
and macOS ARM64, with Caddy 2.11.7 and Traefik 3.7.14. Tests include source drift,
writer contention, failed activation, interruption, rollback and composition.
Linux additionally exercises Caddy Docker-controller 2.13.1 timer/event/restart
behavior, a container target and strict-key SSH delegation.

The release matrix separately builds and executes all four downloadable native
targets, checks their extracted bundles, and exercises route/TLS/TUI behavior.
These checks establish the documented scenarios. Public ACME/DNS, system-scope
runtime, logout/reboot and additional custom modules/topologies need on-target
acceptance rather than inference from a successful build.

## Remaining roadmap

| Area | Next work |
| --- | --- |
| Deployment coverage | Public certificate issuance/DNS, system scope, boot/logout, additional proxy/controller/network combinations |
| Broader route semantics | Extend management beyond the explicitly supported exact-host, one-upstream definitions with preservation evidence |
| Additional adapters | Nginx and other native managers/platforms remain deferred |
| Windows | Deferred for Steward; the independent AppTrail importer retains AppTrail's existing platform matrix |
| Label configuration | Known labels are read-only; custom mapping languages remain deferred |
| AppTrail control | The current export/import boundary is read-only; privileged dashboard control is a separate design |

Apache remains outside the agreed scope.

## Source-backed records

The original product pack is preserved in [docs/planning](https://github.com/samishal1998/steward/tree/main/docs/planning).
Its historical examples are design inputs, not the current executable contract.
Use these records for implemented behavior and actual evidence:

- [CLI contract](https://github.com/samishal1998/steward/blob/main/docs/CLI_CONTRACT.md)
- [Feature completion and verification](https://github.com/samishal1998/steward/blob/main/docs/FEATURE_COMPLETION.md)
- [Capability matrix](https://github.com/samishal1998/steward/blob/main/docs/CAPABILITIES.md)
- [Execution history](https://github.com/samishal1998/steward/blob/main/docs/PROGRESS.md)
