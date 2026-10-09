---
title: Architecture & roadmap
description: Steward's independent route/service model, ownership boundary, and staged delivery.
---

## One executable, separate capabilities

Steward is a standalone Rust project. The CLI and planned TUI use the same application logic. Route management and native-service management are independently useful; composition is explicit. There is no required Steward daemon or Apptrail server/database.

The current implementation is one Cargo package with internal modules. It avoids a generic shared provider framework and cross-language runtime coupling. The [repository decision](https://github.com/samishal1998/steward/blob/main/docs/decisions/repository-placement.md) documents why Apptrail’s dashboard observations are not Steward’s infrastructure authority.

## Current read-only alpha

Implemented:

- Explicit Caddy/Traefik binding profiles and native API inventory.
- Native identity, status, freshness, fingerprints, and partial coverage.
- Offline synthetic snapshots with honest qualification.
- Exact-host route validation and deterministic conflict previews.
- Versioned JSON results and bounded transport/parser/evidence handling.

Inventory success does not grant ownership or make a proxy safely writable. Native reported configuration, route activation, upstream health, DNS, and TLS readiness are separate evidence levels.

## Before enabling route writes

The next milestones add an ownership receipt/fingerprint model, scoped writer locks, durable journals, input staging, revalidation, activation observation, and drift-aware compensation.

Compatibility experiments must prove:

- **Ordinary Caddy:** preserve full authoritative input, validate/reload complete configuration, preserve external sites and modules, and recover owned changes.
- **Caddy Docker controller:** managed base-only edits activate without invented Docker events; label routes survive later events/restarts; the controller remains the effective writer.
- **Traefik files:** correct startup authority, watched-directory integration, parent-directory mounts, atomic replacement, final-route removal, and coexistence with external/Docker routes.

Both proxy families remain in scope for the management release. A readable API is not permission to overwrite controller-generated configuration.

## Delivery roadmap

| Stage | Capability |
| --- | --- |
| Available alpha | Read-only inventory and route preview |
| Next foundation | Compatibility experiments, ownership, journaled apply/recovery |
| Proxy-management release | Caddy ordinary/controller modes, Traefik watched files, explicit setup/install, CLI/TUI |
| Native services | General foreground commands through Linux systemd/macOS launchd, independently of proxies/Docker |
| Composition | Readiness-gated service-first publication and route-first shutdown with partial outcomes |
| Optional integration | Existing Apptrail discovery first; later versioned export/read/control contracts if useful |

Docker labels stay read-only. Known label/file fallback discovery is planned; custom mapping languages are deferred. Apache is excluded. Other managers/platforms are later adapters, not approximations of systemd/launchd semantics.

## Source-backed handoff

The full accepted pack is preserved in [docs/planning](https://github.com/samishal1998/steward/tree/main/docs/planning). Its old examples are design inputs, not proof of implemented commands. Current contracts, capability status, and actual test records are outside that archive:

- [Technical transfer](https://github.com/samishal1998/steward/blob/main/docs/APPTRAIL_TECHNICAL_TRANSFER.md)
- [Implementation plan](https://github.com/samishal1998/steward/blob/main/docs/IMPLEMENTATION_PLAN.md)
- [Capability matrix](https://github.com/samishal1998/steward/blob/main/docs/CAPABILITIES.md)
- [Execution evidence](https://github.com/samishal1998/steward/blob/main/docs/PROGRESS.md)
