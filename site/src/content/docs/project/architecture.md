---
title: Architecture & roadmap
description: Steward's independent route/service model, ownership boundary, and staged delivery.
---

## One executable, separate capabilities

Steward is a standalone Rust project. The CLI and planned TUI use the same application logic. Route management and native-service management are independently useful; composition is explicit. There is no required Steward daemon or Apptrail server/database.

The current implementation is one Cargo package with internal modules. It avoids a generic shared provider framework and cross-language runtime coupling. The [repository decision](https://github.com/samishal1998/steward/blob/main/docs/decisions/repository-placement.md) documents why Apptrail’s dashboard observations are not Steward’s infrastructure authority.

## Current direct-route alpha

Implemented:

- Explicit Caddy/Traefik binding profiles and native API inventory.
- Native identity, status, freshness, fingerprints, and partial coverage.
- Offline synthetic snapshots with honest qualification.
- Exact-host route validation and deterministic conflict previews.
- Versioned JSON results and bounded transport/parser/evidence handling.
- Stored API connections without YAML and completions for five shells.
- Receipt-owned HTTP route add/update/disable/enable/remove on explicit local
  Caddy JSON and Traefik watched-directory authority.
- State/input locks, a bounded durable operation journal, native/routing checks,
  failed-activation compensation and interrupted-operation recovery.

Inventory success does not grant ownership or make a proxy safely writable. Native reported configuration, route activation, upstream health, DNS, and TLS readiness are separate evidence levels.

## Beyond the first writable slice

The initial operation path implements those safeguards for local HTTP-only file-backed routes. Its journal retains one latest operation; broader audit history and authority/setup modes are later refinements. Live checks use disposable Caddy 2.11.7 and Traefik 3.7.14 processes. This does not complete the full proxy-management milestone.

Compatibility experiments must prove:

- **Ordinary Caddy:** preserve full authoritative input, validate/reload complete configuration, preserve external sites and modules, and recover owned changes.
- **Caddy Docker controller:** managed base-only edits activate without invented Docker events; label routes survive later events/restarts; the controller remains the effective writer.
- **Traefik files:** correct startup authority, watched-directory integration, parent-directory mounts, atomic replacement, final-route removal, and coexistence with external/Docker routes.

Both proxy families remain in scope for the management release. A readable API is not permission to overwrite controller-generated configuration.

## Delivery roadmap

| Stage | Capability |
| --- | --- |
| Available alpha | Native inventory/previews, direct local HTTP routes, receipts/recovery, shell completion |
| Next foundation | Broader authority/controller/TLS compatibility, historical operations and manifest apply |
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
