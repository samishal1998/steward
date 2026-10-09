# Steward — Addendum 01: Effective-configuration discovery and Docker label compatibility

**Decision date:** October 9, 2026  
**Status:** Accepted product decisions; implementation behavior remains subject to verification.  
**Scope:** Route discovery, unfamiliar Caddy Docker integrations, and the decision to defer configurable label mappings.  
**Relationship to the planning pack:** Read alongside the existing documents. This addendum does not replace or regenerate them. It clarifies discovery precedence without changing service management, configuration ownership, installation policy, or the AppTrail repository decision.

## 1. Decision

**Steward should prefer inspecting the selected proxy instance’s current configuration over reconstructing its routes from Docker labels.** Known label readers remain useful as secondary evidence and fallbacks. A generic, user-configurable label-to-field mapping system is deferred.

| ID | Agreed decision |
|---|---|
| DISC-01 | Prefer authorized runtime configuration inspection for route discovery when available and sufficiently understood. |
| DISC-02 | Retain the planned built-in discovery support for Traefik’s documented Docker labels and the baseline `lucaslorentz/caddy-docker-proxy` integration. These are not the only possible ways to discover routes. |
| DISC-03 | Do not introduce custom label-field mappings, a mapping language, a mapping editor, or a separate plugin framework in the first release. |
| DISC-04 | An unfamiliar Caddy Docker plugin may still be discoverable through its resulting Caddy configuration, where that configuration uses structures Steward understands. This does not establish management compatibility. |
| DISC-05 | Docker labels remain read-only. Existing managed-file write paths, controller authority, explicit setup, and ownership restrictions remain unchanged. |

The earlier idea of a configurable label-discovery adapter was exploratory, not an additional implementation requirement. This decision supersedes that suggestion only; it does not remove the existing Docker discovery capability.

## 2. Why this approach

The original concern was reasonable: a user might run a Caddy Docker integration whose labels differ from Steward’s baseline integration. Requiring a mapping for each convention would make discovery depend on reconstructing the input language of each controller.

However, Steward’s primary question is **“What routes does this proxy currently report?”**, not **“How would this particular Docker plugin translate these labels?”** Where the proxy exposes usable current configuration, Steward can inspect the result rather than emulate the translator.

There is an important qualification: **the fact that users supply the labels does not, by itself, eliminate the need to understand them when reading them.** The reason to defer mapping is the availability of a better discovery boundary—not that read-only parsing is automatically format-independent.

### Verified basis

| Upstream behavior | Implication for this design |
|---|---|
| Caddy documents `GET /config/[path]` for exporting its current configuration as JSON. [R1] | Use the existing Caddy reader against the selected instance, rather than requiring knowledge of the original label convention. |
| Traefik documents GET endpoints for HTTP routers, services, entrypoints, and dynamic-configuration diagnostics. [R2] | Inspect native routing objects and their status when the API is available and authorized. |
| `lucaslorentz/caddy-docker-proxy` describes generating an in-memory Caddyfile from Docker metadata and applying configuration updates. [R3] | Labels are an input to a controller; Steward can often observe the generated result instead. |
| Traefik’s Docker provider supports default rules and automatic router/service associations. [R4] | A direct mapping of a few label fields is not a complete substitute for the provider’s interpretation. |
| Caddy’s administration endpoint also permits configuration changes; its documentation recommends protecting access. [R1] | Steward’s read-only discovery behavior must not be mistaken for a guarantee that the credentials or endpoint are read-only. |

These facts support the architecture choice. They do not certify every plugin, every proxy release, or a universal route-normalization algorithm.

## 3. Discovery policy

### 3.1 Prefer current configuration, preserve source evidence

Use the following order for determining what the selected proxy currently reports. This is a priority for interpreting evidence, not a requirement to discard lower-priority sources.

| Evidence | Use | Required qualification |
|---|---|---|
| Authorized current configuration from the running proxy | Preferred route inventory and native status where exposed. | Bind it to the correct proxy instance, timestamp it, and retain unsupported portions as unresolved. |
| Known configuration files or previously obtained configuration snapshots | Fallback discovery and source inspection. | Treat as configured or last observed; do not assert that these bytes are currently loaded. |
| Supported Docker labels and metadata | Fallback inference, container correlation, and provenance. | Treat as source intent unless confirmed against current configuration. Unknown conventions remain unsupported. |

For Caddy, inspect the current configuration using the configured admin endpoint and extract only understood route structures. `GET /config/` is the documented whole-configuration read; a narrower supported path may be sufficient. [R1]

For Traefik, use the configured API location and the relevant GET endpoints, including `/api/http/routers`, `/api/http/services`, `/api/entrypoints`, and `/api/rawdata` where necessary. Respect the deployment’s base path and authentication. The raw-data endpoint includes errors and status; its entries must not all be presented as working routes. [R2]

**“Effective configuration” in this addendum means current configuration/status reported by the selected proxy. It does not mean that every route is reachable, every certificate is valid, or every possible request outcome has been evaluated.** Keep the original plan’s separate activation, health, DNS, and TLS verification levels.

### 3.2 No automatic changes to make discovery work

Use existing authorized access. If access is disabled, unreachable, or denied, report the reason and use available supported fallbacks. Do not interpret failed access as an empty route inventory.

Discovery must not enable an API, publish an admin port, weaken authentication, mount a Docker socket, change plugin settings, reload a proxy, or restart a container. Any required access/setup change remains a separate explicit operation under the original approval rules.

A usable proxy inventory should not require Docker access solely to identify routes. Docker access may still be needed for optional provenance or upstream-network context; its absence must be reported precisely rather than disguised as a proxy failure.

### 3.3 Keep observed state and source intent separate

When a label or file disagrees with current proxy configuration, retain both observations and report the discrepancy. Do not overwrite the runtime observation with inferred label values or assume the external controller has finished applying a change.

Correlate observations only when evidence supports it. A matching hostname alone is not enough to merge different proxy instances or distinct routes. Preserve native identifiers, scope, and source references; do not invent a Docker container association when the generated configuration does not expose one.

A partial inventory must remain visibly partial. If unresolved configuration prevents the planner from assessing a potential conflict for a proposed route, that mutation must be blocked with a diagnostic. Partial discovery need not disable unrelated service-only operations or operations for which the existing safety checks can establish sufficient evidence.

## 4. Handling unfamiliar Caddy plugins

**Use the existing Caddy discovery adapter first. Do not create a new top-level provider merely because the labels have a different spelling.**

| Situation | Steward behavior |
|---|---|
| Unknown label convention; current Caddy configuration is readable and understood | Discover the supported routes without requiring a label mapping. Report the originating controller as unknown unless separately identified. |
| Unknown convention; only some resulting route structures are understood | Show the understood information and an explicit partial-discovery warning. Preserve references to unresolved portions. |
| Unknown convention; current configuration cannot be read, but a supported config file is available | Offer file-derived observations without claiming runtime confirmation. |
| Unknown convention; only unknown labels are available | Report that route inference is unsupported. Do not guess hostnames, upstreams, or controller behavior. |
| Routes can be inspected, but no safe managed-file integration is known | Allow inspection only; do not enable writes, reloads, or plugin installation on that basis. |

Native configuration inspection is not universal semantic support. The reader must not simplify unrecognized matchers, nested behavior, or handler configurations into a misleading hostname-to-port route. Unknown behavior should be surfaced, not silently dropped from conflict assessment.

The baseline Caddy Docker integration remains `lucaslorentz/caddy-docker-proxy`, as already named in the planning pack. This addendum makes no popularity ranking and does not assume that every Caddy Docker deployment uses it. Its existing version/mode and managed-file activation tests still apply. [P1, P2]

## 5. Discovery does not grant control

Keep three separate questions in the adapter and UI:

| Question | Evidence required |
|---|---|
| Can Steward inspect this route? | A supported, authorized source and sufficient parsing coverage. |
| Does Steward own its definition? | The existing ownership receipt, native identity/path, and fingerprint checks. |
| Can Steward safely apply changes through this controller? | An explicitly approved and tested configuration integration for that binding. |

A route read through an admin API is not automatically Steward-owned. A route generated from external labels remains externally controlled unless an explicit source-migration flow establishes otherwise. Being able to see it must not enable edit, disable, or delete actions that the original ownership model forbids.

This addendum changes **where discovery prefers to read**, not **where management is allowed to write**. In particular, it does not authorize overwriting a Caddy controller’s generated configuration through the admin API. Existing controller-aware managed-file publication and activation remain the write path. [P1]

## 6. Implementation delta and agent handoff

Implement this within the existing discovery work, not as a new subsystem. Reuse the planned `ProxyDiscovery`, `ProxyBinding`, and `RouteObservation` responsibilities; these names describe Steward’s proposed internal architecture, not external SDK interfaces. Existing observation fields already cover source, status, freshness, ownership, and unresolved data. [P1]

The implementing agent should:

1. Make the source priority in section 3 explicit in the existing Caddy and Traefik readers. Keep supported label parsing available for fallback and enrichment, with deterministic diagnostics for unsupported patterns.
2. Surface source type, freshness, coverage, native status, and ownership in existing CLI/TUI inspection. Do not add a label-mapping screen or expose a new configuration language.
3. Add the focused tests below and record supported engine/plugin versions. Revalidate upstream documentation and actual responses before claiming compatibility; no live proxy tests are claimed by this document.

For the AppTrail agent, apply the original reuse test to concrete discovery functions and fixtures. An equivalent Caddy JSON reader may be a reuse candidate; merely calling Caddy or Docker is not sufficient. Do not force AppTrail’s application model and Steward’s route model into one shared interface.

If the repositories are separated, the already-required technical transfer should include actual endpoint/auth setup, sanitized current-configuration samples, supported parsing coverage, fallback behavior, and any real unknown-plugin cases found during the audit. No additional integration service, shared database, or permanent Steward daemon is introduced. [P3]

## 7. Focused acceptance cases

These supplement the existing testing plan rather than replace it.

| Case | Required result |
|---|---|
| Standard Traefik Docker routes with authorized API access | Current native routing objects are used; labels may enrich provenance without creating duplicate entries. |
| Unknown Caddy label convention producing understood native HTTP configuration | Routes are discovered without a custom mapping or a new plugin adapter. |
| Admin/API access denied or unavailable | Clear access diagnostic and qualified fallback; no setup changes or false empty inventory. |
| Labels/files describe a change not present in current configuration | Intent and observed state remain distinct; no false activation success. |
| Native configuration contains unsupported behavior | Partial coverage is visible; the unsupported portion is not flattened into a simple route. |
| Current configuration contains a reported error or disabled route | Preserve native status; do not label the route healthy or active merely because it appears in a response. |
| Discovery succeeds for an externally controlled Caddy instance | External-route mutation and unverified controller-management actions remain unavailable. |
| Proposed route overlaps an unresolved area of configuration | Block the unsafe mutation and identify the missing evidence. |
| Same hostname appears on different instances or distinct native routes | Preserve separate identities unless explicit evidence justifies correlation. |
| Service-only usage | No proxy or Docker discovery is initialized solely because this addendum exists. |

Use fixtures to test unknown label conventions and known native output without claiming compatibility with an untested real plugin. A claim about a particular third-party plugin requires a version-pinned integration test for that plugin.

## 8. When to revisit configurable mappings

Reopen this decision only after a real deployment demonstrates that authorized current-configuration inspection and existing supported fallbacks cannot supply required discovery information.

Bring a concrete label sample, the missing fields, the reason native configuration cannot resolve them, and the smallest proposed remedy. Evaluate a narrow reader improvement before proposing a generic mapper. Any mapping feature would remain read-only and require a new scope decision; it is not a hidden first-release deliverable.

**Bottom line:** prefer the proxy’s observable configuration, retain known label readers as supporting evidence, and defer custom mappings until a demonstrated gap justifies them.

## 9. References and document relationships

Primary sources were checked on October 9, 2026. Versionless documentation and upstream branches can change. Documentation review establishes the described API/design basis, not execution-tested compatibility.

| Reference | Source |
|---|---|
| R1 | [Caddy administration API](https://caddyserver.com/docs/api), especially current-configuration reads and endpoint protection. |
| R2 | [Traefik API and dashboard documentation](https://doc.traefik.io/traefik/reference/install-configuration/api-dashboard/), especially security, endpoint paths, base path, and raw-data status. |
| R3 | [lucaslorentz/caddy-docker-proxy upstream documentation](https://github.com/lucaslorentz/caddy-docker-proxy), especially configuration generation and deployment modes. |
| R4 | [Traefik Docker routing documentation](https://doc.traefik.io/traefik/reference/routing-configuration/other-providers/docker/), especially automatic router/service construction and default rules. |
| P1 | Existing [architecture](02_ARCHITECTURE.md), sections 2, 4, 6.1, and 7: controller identity, observations, ownership, verification, and apply paths. |
| P2 | Existing [verified references](07_VERIFIED_REFERENCES.md), particularly S04–S06, S10–S11, S24, and the compatibility gates. |
| P3 | Existing [AppTrail agent handoff](04_APPTRAIL_AGENT_HANDOFF.md) and [future integration document](06_APPTRAIL_INTEGRATION.md). Their audit and integration requirements remain in force. |

Store this file beside the original planning documents so the relative links resolve. No original document or planning ZIP is modified by this addendum.
