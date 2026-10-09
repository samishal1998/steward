# Steward — Product Requirements Document

**Revision:** 0.1 planning baseline  
**Date:** October 8, 2026  
**Companions:** [Architecture](02_ARCHITECTURE.md) · [Plan](03_IMPLEMENTATION_PLAN.md) · [Agent handoff](04_APPTRAIL_AGENT_HANDOFF.md) · [Tests](05_TESTING_AND_ACCEPTANCE.md)

## 1. Product definition

Steward is a lightweight host-local service and reverse-proxy manager with an agent-friendly CLI and a simple TUI. It translates user intent into the configuration and lifecycle operations required by supported tools without replacing those tools.

The three use cases are equally valid:

| Mode | User intent | Dependencies that must not be required |
|---|---|---|
| Routes only | “Expose this hostname through the proxy already on this machine, forwarding to this endpoint.” | Native-service creation, AppTrail, a Steward daemon. |
| Services only | “Run this command from this directory as a managed background service.” | Docker, Caddy, Traefik, route setup, AppTrail. |
| Combined | “Create/start this service, verify readiness, and expose its endpoint.” | A dashboard, a permanent Steward supervisor, a deployment platform. |

Service-only use must not merely tolerate a missing proxy; it must avoid initializing or probing proxy and Docker integrations unless the user requests discovery involving them.

## 2. Problem and audience

A developer moves between machines with different proxies, operating systems, installation methods, and configuration sources. A coding agent currently has to rediscover whether Caddy or Traefik owns the listener, where authoritative configuration lives, whether the proxy runs natively or in a container, and how a change takes effect. The same friction appears when placing a simple foreground command under systemd or launchd.

Steward provides a consistent interface while retaining enough provider-specific information to operate safely. Its primary users are individual developers, coding agents, and operators of small self-hosted installations—not a central multi-tenant operations team.

Success means the user describes a hostname, endpoint, or command rather than repeatedly teaching an agent the host's infrastructure.

## 3. Product principles

1. **Independence:** services and routes are separate resources and separate command families. Combining them is explicit.
2. **Ownership before mutation:** discovering a resource never grants permission to rewrite it or stop the process behind it.
3. **Explicit setup:** proxy installation, provider enablement, proxy startup-configuration changes, and new access grants require a setup plan and approval. Any restart required by setup is disclosed. Explicit service lifecycle commands are ordinary planned actions under the approved scope, not an extra setup wizard each time.
4. **Honest capability reporting:** ambiguous discovery, unsupported matchers, inaccessible sources, and incomplete verification remain visible; they are never converted into guesses.
5. **Small operational footprint:** the OS supervises applications; the proxy serves traffic; Steward exits after commands and keeps no required background control service.
6. **Evidence over abstraction:** modularity inside Steward is useful; forced sharing with AppTrail is not. The repository audit controls that decision.

## 4. Scope and sequencing

### 4.1 First deliverable: proxy-management release

Support Caddy in ordinary file-backed mode, Caddy with `caddy-docker-proxy`, and Traefik with a watched file provider. Discover existing routes from accessible live configuration, configuration files, and Docker metadata. Create, edit, enable, disable, and remove **Steward-owned** routes. Offer explicit setup to integrate with an existing proxy or install a selected proxy when none exists.

Include CLI, TUI, plans, diagnostics, conflict detection, backups, safe application, and recovery. Basic native lifecycle adapters needed to install/start the proxy itself belong here as infrastructure; they do not imply that the general `service` product surface is already complete.

### 4.2 Second deliverable: native services and composition

Add general service creation and lifecycle management on systemd/Linux and launchd/macOS. Add optional combined service-and-route operations. A service does not need an HTTP port: workers and other foreground commands are valid service-only resources.

The user also wants the concept to extend to init-style systems. OpenRC and SysV-style init require distinct adapters and tests, not a generic “initd” implementation. They remain a documented follow-on milestone rather than an unverified first-release promise. [S18]

### 4.3 Later, independently shippable extensions

Nginx; OpenRC and selected SysV distributions; Windows service/proxy support; richer route features; explicit external-resource migration; AppTrail export/provider integration; an optional local control endpoint or CLI-served web UI. Remote invocation can initially be ordinary SSH to a host with Steward installed. No built-in fleet controller is required.

### 4.4 Non-goals

Steward does not implement an HTTP reverse proxy, Docker replacement, process supervisor, package manager, DNS provider, certificate authority, cluster scheduler, source-code deployment engine, or AppTrail dashboard. It does not build the user's application, guess how to deploy an arbitrary repository, manage Kubernetes, rewrite container labels, or recreate application containers. Apache is expressly out of scope.

## 5. Primary workflows

### 5.1 Route an existing host process

The user specifies a hostname and an upstream such as `http://127.0.0.1:3000`. Steward discovers candidate proxy instances, shows why it selected one, checks its authorized write path, and creates a plan. An unambiguous previously approved binding can be reused. Multiple proxies, conflicting listeners, or inaccessible authoritative configuration require explicit selection or remediation.

Steward checks the target from the relevant network context, validates the candidate, applies it through the provider-specific path, and reports configuration activation separately from application health, DNS, and TLS readiness. It does not start the upstream application or modify DNS.

### 5.2 Route a Docker application without editing Docker

Docker discovery supplies route labels, published ports, and network context. New Steward routes still use the file-backed write path, targeting a stable address reachable from the proxy. Ordinary container labels require container recreation to change; Swarm service labels have different semantics. Steward intentionally treats both as read-only discovery in this product. [S08]

No application container is recreated or relabeled. No network is attached to an application silently. A stale container IP is not accepted as a durable auto-updating endpoint. When a stable reachable target cannot be established, the route plan is blocked and explains why.

### 5.3 First run with no supported proxy

Service-only commands continue normally. Entering Routes shows a setup action; an explicit interactive setup wizard asks whether to install Caddy or Traefik, where it should run, and what scope, listeners, TLS policy, and Docker access to use.

For Caddy with Docker support, setup installs a verified Caddy build that includes the Docker extension; merely installing stock Caddy is not enough for that capability. For Traefik, setup prepares a watched route directory and, when Docker is available and approved, Docker discovery. Missing Docker is not a reason to install Docker automatically or break file-only proxy use. [S04, S23]

Non-interactive commands never prompt indefinitely. They return a structured setup requirement and the information needed for the next explicit command.

### 5.4 Existing proxy needs integration setup

A route command must not silently turn on a missing provider or change startup arguments. It returns a setup plan identifying the authoritative source, proposed edits, expected restart/recreation, backup, and rollback scope.

Traefik distinguishes startup configuration from hot-reloaded routing configuration. An integration that changes startup settings is not an ordinary route update. [S01]

If adding a bind mount requires recreating the **proxy container**, that is a separately disclosed setup operation. The prohibition on recreating application containers still applies.

### 5.5 Manage a background service without routing

The user supplies an executable/argument vector, working directory, scope, startup policy, and restart policy. Steward renders a native definition, registers it through the selected manager, starts it only when requested, and reads actual state back.

No proxy package is downloaded, no Docker socket is opened, and no route configuration is created. A stopped or disabled proxy must not cause this operation to fail.

### 5.6 Create a service and expose it

One combined request produces one ordered operation: preflight both capabilities, prepare artifacts, register/start the service, verify its endpoint, publish the route, and verify the result. A readiness failure must not create the public route.

A route failure after a new service starts produces a precise partial result. Default proposed policy: remove the new route change, retain the successfully started service for inspection, and report a non-success combined outcome. A stricter cleanup policy can remove only resources created by that operation. Pre-existing resources are never deleted as compensation.

### 5.7 Inspect an existing route or service

The TUI shows ownership, origin, effective state, capabilities, and the reason an action is unavailable. Discovered Docker routes remain visible even though editing and disabling them through Steward is blocked.

For an external service, the service milestone may offer explicit authorization for lifecycle controls without taking ownership of its definition. That is not a generic “adopt and rewrite” operation. External route migration is a separate, later workflow with source removal and conflict verification.

## 6. Functional requirements

| ID | Requirement | Acceptance evidence |
|---|---|---|
| FR-01 | Detect proxy engine, controller/config source, runtime, listeners, scope, and evidence separately. | Distinguish native Caddy, Docker Caddy, Docker-controller Caddy, and Traefik combinations. |
| FR-02 | Register/reuse explicit host bindings and list all known candidate instances. | Ambiguous discovery never chooses an arbitrary proxy. |
| FR-03 | Inventory managed and external routes with source provenance and unresolved expressions. | A label-defined route cannot be mistaken for a writable file route. |
| FR-04 | Manage exact-host HTTP reverse-proxy routes to one explicit upstream. | Create/update/disable/enable/remove is idempotent on both initial engines. |
| FR-05 | Preserve all unowned configuration. | Regression fixtures and live checks prove unrelated routes survive. |
| FR-06 | Detect exact-host collisions and conservatively flag wildcard, catch-all, or complex-rule overlaps. | No silent route-priority override or guessed equivalence. |
| FR-07 | Preview and approve setup independently of ordinary operation. | Missing-provider route request writes nothing. |
| FR-08 | Install the selected proxy through a verified, recoverable installation plan. | Clean-machine install and interrupted-install tests. |
| FR-09 | Support a TUI and a stable machine-readable CLI contract. | The same application functions power both. |
| FR-10 | Provide journaled apply, conflict checks, verification, and recovery. | Crash and concurrent-edit fault tests pass. |
| FR-11 | Manage native foreground services independently. | Service-only operation succeeds with proxy/Docker adapters disabled. |
| FR-12 | Separate running state, startup policy, registration state, and restart policy. | Start/stop does not silently change startup policy. |
| FR-13 | Compose service and route operations with explicit partial-failure behavior. | Readiness and rollback tests prove ordering. |
| FR-14 | Make AppTrail optional and preserve its independent data ownership. | Both apps remain usable when the other is absent. |
| FR-15 | Preserve portability without inventing identical native capabilities. | Unsupported features return structured capability errors. |

## 7. Supported route model

The initial writable subset is deliberately small: one exact hostname, one upstream HTTP or HTTPS origin, a selected proxy binding/listener policy, desired enabled state, and an explicit TLS policy. Preserve the incoming path/query by default. URL credentials, fragments, and implicit upstream path rewriting are rejected in the first writable subset.

Existing path routes, regex rules, redirects, middleware chains, load-balancing pools, and non-HTTP routes may appear as external observations. Their discovery does not imply edit support. HTTPS upstream verification is on by default; private CA or server-name settings require an explicit supported adapter option.

Public TLS, local/internal TLS, and HTTP-only are distinct policies. Caddy and Traefik do not have identical certificate configuration models. Missing DNS, an unavailable certificate resolver, or an untrusted local CA cannot be reported as successful public HTTPS. Certificate storage remains owned by the proxy; uninstall does not delete it by default. [S20, S21]

“Disable route” means remove Steward's matching route from active proxy configuration while retaining its desired definition. It does not stop its upstream, delete a certificate, remove DNS, or prove the hostname is inaccessible through another route or listener.

## 8. Native service model

A managed service has a stable Steward ID, executable/arguments, working directory, manager/scope, startup intent, restart intent, environment policy, and optional readiness definition. A port is optional. The application stays a foreground process; systemd or launchd supervises it. [S13, S16]

V1 service creation targets standard long-running foreground commands. No shell interpolation occurs unless the user explicitly supplies a shell executable as part of the command. The setup resolves executables rather than relying on an interactive shell, Node version-manager initialization, or a user's current working directory.

Service edit may require native reload/re-registration and restart. The plan states the effect before application. Native reload support is a capability, not an alias for restart. Service removal preserves application directories, data, logs, and credentials unless a separate explicitly authorized deletion is added later.

## 9. CLI and TUI requirements

Proposed command families are `steward route`, `steward service`, `steward proxy`, `steward plan`, `steward apply`, `steward doctor`, and `steward operation`. `steward up` is thin composition, not a deployment engine. The exact proposed command grammar is recorded in the architecture and example guide.

Invoking `steward` on an interactive terminal opens the TUI. Without a terminal, it prints concise help/status instead of attempting interactive rendering. `--json` makes stdout machine-readable; progress and diagnostics go to stderr. Agent mode never launches an editor, waits for a password, or accepts an installation implicitly.

The TUI uses Routes, Services, Proxies, and Operations views. Routes show hostname, target, proxy, source, ownership, desired state, activation, and health. Services show manager, scope, running/startup state, attached routes, and recent failure. A combined operation is one expandable job, not an unstructured sequence of unrelated notifications.

## 10. Safety and non-functional requirements

Applying a manifest changes only the resource IDs it explicitly supplies. Empty or absent service/route collections do not remove existing resources. Deletion is an explicit, previewable action; implicit pruning is excluded from initial apply behavior.


Every mutation has a stable operation ID, a bounded timeout, a meaningful result, and a persisted receipt. Apply checks the version/hash of each relevant input again immediately before mutation. Recovery cannot blindly overwrite newer manual changes. The system must distinguish rejection before apply, failure after a write, uncertain application, successful compensation, and compensation blocked by drift.

The minimum privilege required by the selected action is requested explicitly. The whole TUI should not run elevated simply to list resources. Docker daemon access is security-sensitive even when Steward only sends reads; a read-only filesystem mount of a socket is not an API authorization policy. [S09]

No secrets in generated public metadata, route labels, default diagnostics, or AppTrail exports. Validation/probing is bounded and cannot turn discovered URLs or configuration expressions into arbitrary command execution. Private-network probes are useful for this product but require an explicit host/network policy; credential-bearing redirects and cloud metadata endpoints are blocked by default.

Proposed performance targets are a responsive TUI during I/O and sub-second local cached inventory for a small host. Live convergence times are adapter-specific; a provider polling interval must not be hidden behind a universal two-second success promise. Measure targets on declared reference hosts before turning them into release commitments.

## 11. Acceptance of the product

The proxy release is complete only when both initial proxy families work in their documented native/container arrangements, external routes survive changes, installation/setup is explicit, recovery is tested, and the TUI/CLI share behavior.

The service release is complete only when both Linux/systemd and macOS/launchd are exercised on actual target operating systems, services remain running after Steward exits, service-only use has no proxy dependency, and combined operations report and recover partial failure correctly.

Repository placement and future integration must follow the dedicated AppTrail handoff. Calling the same upstream APIs is never, by itself, a reason to share crates or merge products.
