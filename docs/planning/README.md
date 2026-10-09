# Steward — product and engineering handoff

**Prepared:** October 8, 2026  
**Status:** implementation proposal based on the agreed product direction; repository placement is deliberately undecided.  
**Product name:** Steward. Earlier working names were Trailgate and Wayfinder. The existing dashboard project is **AppTrail**.

Steward is a lightweight CLI/TUI for two independently useful jobs: managing reverse-proxy routes and managing native background services. A small composition layer can start a service and expose it through a proxy. Neither capability requires the other. AppTrail is an optional future consumer, not a runtime dependency.

## Start here

Give the existing AppTrail agent **[04_APPTRAIL_AGENT_HANDOFF.md](04_APPTRAIL_AGENT_HANDOFF.md)** first. Its first task is a source-code audit and a repository decision, not implementation or a shared-code extraction. Then it should follow the PRD, architecture, implementation plan, and acceptance tests.

| Document | Purpose |
|---|---|
| [01_PRD.md](01_PRD.md) | Product goals, scope, workflows, requirements, release boundaries, and acceptance criteria. |
| [02_ARCHITECTURE.md](02_ARCHITECTURE.md) | Module structure, runtime actors, resource model, configuration ownership, transaction design, native adapters, and interaction diagrams. |
| [03_IMPLEMENTATION_PLAN.md](03_IMPLEMENTATION_PLAN.md) | Dependency-ordered work packages, research gates, deliverables, and completion criteria. |
| [04_APPTRAIL_AGENT_HANDOFF.md](04_APPTRAIL_AGENT_HANDOFF.md) | Separate instructions to the existing AppTrail agent, including the mandatory same-repository versus separate-repository investigation. |
| [05_TESTING_AND_ACCEPTANCE.md](05_TESTING_AND_ACCEPTANCE.md) | Unit, integration, native-OS, failure-injection, security, and end-to-end acceptance tests. |
| [06_APPTRAIL_INTEGRATION.md](06_APPTRAIL_INTEGRATION.md) | What the old AppTrail PRD establishes, what remains unknown, and a staged optional integration design. |
| [07_VERIFIED_REFERENCES.md](07_VERIFIED_REFERENCES.md) | Official documentation and upstream source references, research qualifications, and implementation verification gates. |
| [templates/REPOSITORY_DECISION.md](templates/REPOSITORY_DECISION.md) | Evidence-based decision record for the AppTrail agent to complete. |
| [templates/APPTRAIL_TECHNICAL_TRANSFER.md](templates/APPTRAIL_TECHNICAL_TRANSFER.md) | Required source-backed help document when choosing a separate repository. |
| [examples/README.md](examples/README.md) | Proposed YAML contracts and their interpretation. These are design fixtures, not an implemented CLI. |
| [charts/README.md](charts/README.md) | Editable Mermaid chart sources, also embedded in the relevant documents. |
| [reference/Apptrail_PRD.original.md](reference/Apptrail_PRD.original.md) | Unmodified earlier AppTrail draft supplied as historical context; not evidence of the current code. |

## Decision boundaries

**Agreed:** one lightweight Steward product; independent routes and services; Caddy and Traefik first; Docker labels are discovery-only; managed route writes use owned file-backed configuration; bootstrap and disruptive setup are explicit; macOS/Linux first; no required Steward daemon; no Apache adapter; Nginx and additional platforms can follow.

**Proposed implementation defaults:** Rust, one executable with an integrated TUI, a small workspace, YAML/JSON user configuration, a durable local operation journal, and staged delivery of proxy management before general native-service management. These are recommendations, not claims about AppTrail's existing implementation.

**Must be investigated:** actual AppTrail code and schemas; repository placement; genuine reusable functions; exact supported versions; Caddy Docker controller refresh behavior; native launchd behavior on target macOS versions; install artifact availability and provenance.

The receiving agent may refine internal implementation details when evidence warrants it. It must not silently expand scope, weaken ownership/security rules, replace a promised capability with a weaker one, or introduce sharing that requires unnecessary abstractions. Material discrepancies require an evidence-backed proposal to the user before proceeding.

## What this pack does not claim

No AppTrail source-code audit, live proxy experiment, installer execution, or native service-manager test was performed in preparing these documents. Public documentation and selected upstream source were inspected. Proposed Steward commands, schemas, traits, filenames, and status codes are designs for the agent to implement; they are not existing vendor APIs.

The AppTrail repository decision remains open. The transfer document is intentionally a template: the agent with actual repository access must populate it rather than presenting unverified interfaces as facts.

## Reading conventions

`[S01]`-style references point to [verified references](07_VERIFIED_REFERENCES.md). `[A01]` refers to the included historical AppTrail PRD, with the relevant section named in the text. “Must” denotes a product/safety requirement. “Proposed” denotes a design choice that can be changed with a documented rationale.
