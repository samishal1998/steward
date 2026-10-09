# AppTrail → Steward — Technical transfer for a separate repository

**Template status: NOT COMPLETED.** The existing AppTrail agent must populate this document from the actual code, tests, and accumulated relevant project knowledge before transferring implementation to a new agent.

**Do not substitute the historical PRD for this document. Do not invent schemas or connection details. Do not include real credentials.** Mark each material finding by evidence status and identify the baseline commit.

## 1. Executive orientation for the new agent

State why a separate repository was selected, which functionality genuinely overlaps, which interfaces/structures differ, and which tempting shared abstractions should not be introduced.

Summarize Steward's independent service/route model, explicit setup boundary, read-only Docker-label policy, owned file-backed writes, no-required-daemon constraint, and optional AppTrail relationship. Link the completed repository decision and updated implementation plan.

## 2. Evidence baseline and reading map

| Item | Verified value / source |
|---|---|
| AppTrail repository and immutable commit | TO VERIFY |
| Current stack and toolchain | TO VERIFY |
| Branch or working-tree qualifications | TO VERIFY |
| Provider/client directories | TO VERIFY |
| Configuration schemas and loaders | TO VERIFY |
| Observation and identity schemas | TO VERIFY |
| Authentication/secret-resolution modules | TO VERIFY |
| Fixtures and relevant tests | TO VERIFY |
| CI/release configuration | TO VERIFY |
| Documentation known to be stale | TO VERIFY |

Use immutable repository links where available. Include local path, symbol, and line range as navigation aids. State when line ranges are tied to a particular commit.

## 3. Docker connection and discovery knowledge

Document the **actual AppTrail implementation**, not a generic Docker tutorial.

### 3.1 Connection construction

Record endpoint source/precedence, Unix/TCP/SSH/TLS support actually implemented, Docker context/environment handling, API version negotiation, client library/version, timeouts, rootless/Desktop/remote-daemon behavior, and any supported restricted API proxy.

| Setting | Actual schema/type | Default/required/optional/null behavior | Resolution precedence | Code/test evidence |
|---|---|---|---|---|
| TO VERIFY | TO VERIFY | TO VERIFY | TO VERIFY | TO VERIFY |

Identify permission requirements and which operations are read-only. Explain any distinction between a restricted API endpoint and privileged daemon socket access. State where credentials are referenced, how they are loaded, and what must be redacted; do not copy values.

### 3.2 API operations actually used

| Purpose | Exact method/path or client call | Parameters / API version | Response fields consumed | Pagination/watch/error behavior | Evidence |
|---|---|---|---|---|---|
| Container enumeration | TO VERIFY | TO VERIFY | TO VERIFY | TO VERIFY | TO VERIFY |
| Inspection / ports / networks / labels | TO VERIFY | TO VERIFY | TO VERIFY | TO VERIFY | TO VERIFY |
| Events or polling | TO VERIFY or not implemented | TO VERIFY | TO VERIFY | TO VERIFY | TO VERIFY |
| Swarm/node/service discovery | TO VERIFY or not implemented | TO VERIFY | TO VERIFY | TO VERIFY | TO VERIFY |

Explain event reconnect, snapshot resync, stale observations, container recreation, stable identity, daemon identity, and whether AppTrail loses source provenance during normalization. State which of these are demonstrated by tests.

### 3.3 Sanitized fixtures and gotchas

Provide minimal fixtures for a host-published app, shared-network app, Traefik-labeled app, Caddy-labeled app if supported, and unavailable/remote daemon. Include fixture source/version and parsing commands/tests. Document known failed approaches and workarounds with evidence.

## 4. Caddy connection and configuration knowledge

### 4.1 Implemented reading modes

Separate plain Caddyfile, native JSON/admin API, ordinary Caddy in Docker, and `caddy-docker-proxy` controller deployments. Identify which modes AppTrail actually recognizes and how it proves the distinction.

Record endpoint/path schema, authorization and transport, timeout/retry behavior, config source versus effective runtime config, Caddy/plugin versions tested, and behavior when the API is disabled. The default admin port in documentation does not prove the user's installation exposes it.

### 4.2 Exact interfaces and payloads

Provide actual source symbols and types for client construction, fetching/parsing, matcher extraction, reverse-proxy upstream extraction, and emitted observations. Include required/optional/null semantics and preservation of complex matchers, unknown modules, source locations, and TLS intent.

| Operation | Exact interface / method / path | Input schema | Native response or file fixture | Normalized output | Evidence |
|---|---|---|---|---|---|
| TO VERIFY | TO VERIFY | TO VERIFY | TO VERIFY | TO VERIFY | TO VERIFY |

### 4.3 Knowledge relevant to Steward writes

Distinguish facts already known from discovery work from write/refresh questions not implemented in AppTrail. Do not invent a safe mutation API simply because a read API exists.

Document base-file/controller behavior, imports, required modules, runtime reload authority, disabled admin behavior, certificate storage, mount layout, and anything tested about file-only refresh. Label untested Caddy Docker refresh behavior as an explicit Steward spike. Include preservation and concurrency hazards.

## 5. Traefik connection and configuration knowledge

### 5.1 Implemented source modes

Document whether AppTrail uses Docker labels, startup/dynamic files, diagnostic APIs, or some combination. Identify exact provider IDs, supported Traefik versions/rule syntax, `@docker`/`@file` handling, authentication/base-path configuration, and inaccessible-API behavior.

### 5.2 Exact interfaces and schemas

| Operation / parser | Actual source symbol | Inputs and defaults | Native fixture | Output schema / information lost | Evidence |
|---|---|---|---|---|---|
| TO VERIFY | TO VERIFY | TO VERIFY | TO VERIFY | TO VERIFY | TO VERIFY |

List actual API methods/paths used, not endpoints copied from a draft plan. Explain router-to-service relations, entrypoints, schemes, TLS/resolvers, paths, priorities, multiple host matchers, and unresolved expressions. Identify which information is unsuitable for round-trip mutation.

### 5.3 Setup and runtime constraints

Record startup configuration source detection, file versus directory provider mode, watcher/mount behavior, native versus container runtime, and permission boundaries where known. Do not presume AppTrail's read-only implementation can safely enable a provider or restart Traefik.

Include known bad configurations, useful diagnostic responses, and exact tested recovery/refresh recipes. Do not list a validation or write command unless verified from the selected upstream version.

## 6. Cross-provider data contracts

Provide the real current schemas or source excerpts for connection configuration, native response adapters, observations, source provenance, stable identity hints, application records, and error/diagnostic results. Link actual validators/types and the serialization version policy.

| Contract | Exact schema location / symbol | Fields and semantic meaning | Required/optional/null rules | Versioning | Tested examples |
|---|---|---|---|---|---|
| TO VERIFY | TO VERIFY | TO VERIFY | TO VERIFY | TO VERIFY | TO VERIFY |

Explain the full call flow from configuration to client to native facts to observations to reconciliation. Document cache ownership, timeouts, cancellation, retry policy, partial snapshots, stale handling, and deletion detection.

Separate native facts from dashboard-only normalization. State which normalized values are lossy and must not be used by Steward to decide ownership, host equivalence, route deletion, or service mutation.

## 7. Actual AppTrail extension/integration points

Identify the current supported file provider format, application metadata manifest, provider registration mechanism, and API endpoints, if implemented. Compare them explicitly with the old PRD rather than assuming they match.

Record the smallest existing boundary that can consume Steward information, its auth/permission requirements, polling/watch behavior, and identity correlation. Identify which suggested Steward integration stages need new AppTrail code and which need none.

Do not invent a service-control endpoint. Read-only integration and future control are distinct. Include any current AppTrail authentication/authorization limitations that make management actions premature.

## 8. Networking, identity, and permissions

Document native versus container localhost assumptions, host gateway/Desktop behavior, shared Docker networks, published versus exposed ports, remote Docker context, rootless paths, and inaccessible listeners.

Explain what AppTrail merely assumes today versus what Steward must prove before a write. Include system/user identity and privilege boundaries. Do not propose copying a production admin token or mounting a privileged socket as a default shortcut.

## 9. Known failures and lessons

| Symptom | Actual cause / confidence | Affected versions/topology | Verified workaround or rejected approach | Reproduction / evidence |
|---|---|---|---|---|
| TO FILL | TO FILL | TO FILL | TO FILL | TO FILL |

Include relevant accumulated project knowledge even when it lives in discussions rather than code, but label it accordingly and attach a verification task. Exclude unrelated dashboard/theme history.

## 10. Fixture and test pack

For each fixture include purpose, engine/plugin/API version, synthetic or sanitization method, exact file location, expected output, and current test command. Avoid credentials, actual private inventory, certificate keys, or personal environment variables.

Provide at least one reproducible successful discovery path per implemented provider and one error/partial-coverage path. If no runnable test exists, say so and give precise construction steps; do not claim it passed.

| Fixture / test | Command | Dependencies/setup | Expected result | Actually executed? |
|---|---|---|---|---|
| TO FILL | TO FILL | TO FILL | TO FILL | Date/version or no |

## 11. Verified documentation map

| Topic | Primary upstream URL and version | Repository permalink / test | What it supports | Last checked / qualification |
|---|---|---|---|---|
| TO FILL | TO FILL | TO FILL | TO FILL | TO FILL |

Prefer primary project documentation and immutable code references. Distinguish inspected documentation, inspected code, and executed behavior.

## 12. New agent's first session

Provide concrete, copyable steps using actual paths and commands after the audit:

1. Read the repository decision, Steward PRD/architecture, and this transfer.
2. Set up the selected standalone workspace/toolchain without introducing AppTrail runtime dependencies.
3. Load the supplied sanitized fixtures and run the specified baseline tests.
4. Verify the selected Caddy/Traefik versions and complete the blocking refresh/network/setup spikes.
5. Implement the first approved vertical slice with plan/apply/ownership/recovery in place.
6. Record evidence, unsupported behavior, and any material discrepancy before changing scope.

Replace the generic steps above with concrete repository/test commands and expected results. Note any required user access or authorization that is not yet available.

## 13. Unknowns and required decisions

| Unknown | Why it matters | Exact verification step | Blocks which milestone? | Owner |
|---|---|---|---|---|
| TO FILL | TO FILL | TO FILL | TO FILL | TO FILL |

A clearly documented unknown is acceptable. A fabricated interface is not.

## 14. Transfer completion review

The new agent must be able to identify the actual connection and schema contracts, reproduce relevant provider behavior, understand what cannot be reused, and begin Steward without access to the original agent's private conversation history. Confirm that the updated implementation plan and every required fixture/reference accompany this completed document.
