# Steward — Editable architecture and interaction charts

The diagrams are embedded as Mermaid code blocks in the main Markdown documents. Matching `.mmd` source files are included here for editing or rendering in a Mermaid-compatible tool. They are diagram sources, not pre-rendered image attachments.

| Source | What it shows | Authoritative document |
|---|---|---|
| [01-components.mmd](01-components.mmd) | One executable; independent route/service modules; native adapters; optional AppTrail edge. | [Architecture §1](../02_ARCHITECTURE.md) |
| [02-resources.mmd](02-resources.mmd) | Hosts, proxies, routes, services/endpoints, operations, and optional AppTrail correlations. | [Architecture §4](../02_ARCHITECTURE.md) |
| [03-route-apply.mmd](03-route-apply.mmd) | Plan, approve, publish, activate, verify, compensate, and commit. | [Architecture §6](../02_ARCHITECTURE.md) |
| [04-service-and-route.mmd](04-service-and-route.mmd) | Start service, check readiness, expose route, handle partial failure. | [Architecture §10](../02_ARCHITECTURE.md) |
| [05-bootstrap.mmd](05-bootstrap.mmd) | Explicit install/integration setup, approval, verification, and recovery. | [Architecture §11](../02_ARCHITECTURE.md) |
| [06-roadmap.mmd](06-roadmap.mmd) | Repository audit, compatibility spikes, proxy release, native services, composition, optional integration. | [Implementation plan](../03_IMPLEMENTATION_PLAN.md) |
| [07-apptrail-integration.mmd](07-apptrail-integration.mmd) | Runtime authorities, discovery/export, and optional authorized dashboard control. | [AppTrail integration §3](../06_APPTRAIL_INTEGRATION.md) |

## Plain-text overview

```text
Human / coding agent
        |
   Steward CLI / TUI             AppTrail (optional)
        |                         | observations/UI
   Plan + ownership + journal <--+ optional approved request
        |
        +-- service module --> systemd / launchd --> application
        |
        +-- route module ----> owned proxy input --> Caddy / Traefik
        |                                                  |
        +-- composition -----> readiness then route         +--> application

After Steward exits, the native manager and proxy continue running.
Service-only use does not initialize the route, Docker, or AppTrail integrations.
```

Arrows describe calls or data flow, not separately deployed microservices. The component diagrams are conceptual implementation designs; they do not claim current AppTrail code or an already available management API.
