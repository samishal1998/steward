# Steward — Testing and acceptance plan

**Status:** required test specification; no test in this document is represented as already executed.  
**Related:** [PRD](01_PRD.md), [architecture](02_ARCHITECTURE.md), [implementation plan](03_IMPLEMENTATION_PLAN.md), [references](07_VERIFIED_REFERENCES.md).

## 1. What must be proved

The first release must demonstrate safe management of both initial proxy engines, not merely valid-looking generated files. The native-service release must demonstrate independent service operation on actual Linux/systemd and macOS/launchd environments. Composition must preserve those independent behaviors.

A passing test records the input, actual engine/controller/native-manager versions, runtime and network context, expected effects, observed effects, and evidence of preserved external resources. Cross-compilation, mocked subprocesses, and a passing container suite are not substitutes for native service-manager tests.

The agent maintains `CAPABILITIES.md` and a test-evidence directory. A capability can be `planned`, `verified`, `blocked`, or `unsupported`; never call it supported solely because an adapter compiles.

## 2. Layers and environments

| Layer | Purpose | Appropriate environment |
|---|---|---|
| Pure unit tests | Validation, matching, identity, rendering, planning, redaction, state transitions. | Any development host/CI runner. |
| Golden/contract tests | Native configuration and API fixtures; CLI JSON/schema contracts. | Hermetic fixtures plus pinned versions. |
| Property and fuzz tests | Unsafe inputs, parser limits, normalization, plan invariants. | Isolated CI jobs with bounded resources. |
| Live proxy tests | Effective routes, reload/watch behavior, coexistence, request path. | Pinned Caddy/Traefik processes and Docker containers. |
| Native OS tests | Service lifecycle, privilege/scope, reboot/logout, installation. | Disposable Linux VMs and actual macOS runners or test machines. |
| Failure injection | Crash consistency, concurrency, failed activation, recovery. | Instrumented adapter/filesystem/process harness and disposable hosts. |
| Release tests | Download/install/upgrade/uninstall and architecture selection. | Clean supported target environments. |

Docker is useful for proxy and networking tests. It does not establish that launchd works on macOS, nor does a minimal Linux container necessarily exercise the same user-manager, login, privilege, and boot behavior as a Linux host. Native scope tests are separate evidence.

### Required dimensions

Cover plain file-backed Caddy, Caddy with the Docker controller, and Traefik with watched files plus existing Docker routes. Include native and containerized proxies; Linux Engine and Docker Desktop networking; user and system scope where supported; APIs accessible, inaccessible, and disabled; preconfigured versus missing integration; owned versus external resources; and non-default config paths.

Do not explode this into every Cartesian combination. Identify supported deployment recipes in M1, run their full acceptance suite, and add targeted pairwise/failure cases for remaining interactions. Every omitted combination must have a documented limitation rather than an implicit support claim.

## 3. Requirement traceability

| Product requirement | Primary test groups |
|---|---|
| FR-01 / FR-02 — detection and explicit bindings | D01–D09, N01–N08 |
| FR-03 — provenance and external resources | D03–D09, O01–O07 |
| FR-04 — route lifecycle | C01–C10, T01–T10 |
| FR-05 / FR-06 — preservation and conflicts | O01–O07, C03–C09, T03–T09, F01–F10 |
| FR-07 / FR-08 — explicit setup and installation | B01–B12 |
| FR-09 — CLI/TUI | U01–U10 |
| FR-10 — apply, verification, recovery | F01–F10, V01–V07 |
| FR-11 / FR-12 — independent native services | S01–S12, L01–L10 |
| FR-13 — composition | X01–X10 |
| FR-14 — optional AppTrail | A01–A08 |
| FR-15 — honest capabilities | D09, B10, S10, L06, U07 |

## 4. Discovery, binding, and ownership tests

### Detection and inventory

| ID | Scenario | Required result |
|---|---|---|
| D01 | Caddy binary installed but Traefik owns the relevant listener. | Do not select Caddy merely because it is installed. |
| D02 | Two proxies, different addresses/IP families/ports. | Show candidates and evidence; require explicit binding when ambiguous. |
| D03 | Ordinary Caddy in Docker versus `caddy-docker-proxy`. | Classify runtime and configuration controller separately. |
| D04 | Docker unavailable, permission denied, or endpoint remote. | Report partial coverage; never interpret failure as an empty inventory. |
| D05 | Caddy API disabled or Traefik API unavailable. | Use approved alternative observations where available; reduce verification confidence honestly. |
| D06 | Custom paths, symlinks, relative imports, alternate service names. | Resolve only approved readable sources; never perform an unbounded filesystem scan. |
| D07 | Complex Caddy matchers or Traefik host/path expressions. | Preserve native expression/provenance; do not invent a simple equivalent. |
| D08 | Saved binding now points at a different container/process/build/config authority. | Invalidate or require reapproval; no mutation using stale identity. |
| D09 | Unsupported platform, manager, controller mode, or version. | Read-only where safe; structured capability error for unsupported writes. |

### Ownership and collision rules

| ID | Scenario | Required result |
|---|---|---|
| O01 | External route uses a Steward-looking name/comment. | Remains external without a matching ownership receipt and fingerprint. |
| O02 | Existing Docker route shares the requested hostname. | Reject/flag conflict; do not add labels or a competing file route. |
| O03 | Same host appears under a wildcard, catch-all, or complex matcher. | Conservative conflict report, with source and uncertainty. |
| O04 | External resource selected for edit/disable/delete. | Explain origin; do not offer a fake successful file override. |
| O05 | Managed route is disabled or removed. | Remove only owned routing input; preserve upstream process, DNS, certificate state, and unrelated sites. |
| O06 | Managed file edited externally since last receipt. | Report drift and block stale apply; no unconditional restoration. |
| O07 | Adopt/migrate flow is not implemented or not authorized. | Return a concrete limitation and source instructions, not implicit takeover. |

Fixtures must include hostname case, trailing dot handling, IDNs, invalid hostnames, IPv4/IPv6 authorities, explicit ports, TLS policy mismatches, and duplicate IDs. Write normalization must have one documented contract; discovery must retain original values for explanation.

## 5. Caddy acceptance

| ID | Scenario | Required result |
|---|---|---|
| C01 | Add/update/disable/enable/remove one exact-host route in ordinary file mode. | Correct effective route and request behavior for every operation. |
| C02 | Existing Caddyfile lacks the approved import. | Ordinary route command returns `setup.required` without changing the file or process. |
| C03 | Existing global options, manual sites, imports, and extra modules. | Whole authoritative configuration remains usable; unrelated routes survive. |
| C04 | Invalid candidate or missing provisioned dependency. | Distinguish parse/adapt/provision/activation errors; preserve or recover previous owned input safely. |
| C05 | Reload generated route fragment alone is attempted internally. | Test fails: only the complete authoritative configuration can be applied in plain mode. |
| C06 | Caddy Docker controller: managed base contribution changes with no Docker event. | Requested route appears through a proved controller refresh path within the recorded budget. |
| C07 | Later real Docker event and later controller restart after C06. | Managed route and existing label routes remain present; no competing writer. |
| C08 | Controller drops/rejects an invalid generated site while other sites load. | Steward detects missing requested route; not success merely because a reload occurred. |
| C09 | Rollback while Docker-derived state changed since planning. | Restore only safe owned input, then let the controller regenerate current full state. Never replay a stale full snapshot. |
| C10 | API-managed/JSON-managed external installation. | Read-only unless a separately verified writer/persistence contract exists. |

Record engine version, plugin version/commit, controller mode, polling configuration, and mounted paths. File-only refresh and unchanged-generated-content behavior must be tested on the selected release; observations of a moving upstream branch are not the acceptance evidence. [S04–S06]

Include ordinary Caddy validation when the running process holds resources a separate validation process also wants. The implementation must not misclassify a runtime resource conflict as malformed syntax or silently skip all validation. [S03]

## 6. Traefik acceptance

| ID | Scenario | Required result |
|---|---|---|
| T01 | Owned aggregate changes under a watched directory. | Add/update/disable/enable/remove converges without process restart. |
| T02 | File provider absent, watch disabled, or required mount missing. | `setup.required`; no spontaneous restart/recreation. |
| T03 | Existing Docker and external file routes. | Preserve all external definitions and their behavior. |
| T04 | Router/service names collide across providers. | Correct provider-qualified identity; no accidental dependency on an unrelated resource. |
| T05 | Atomic replacement through the selected container mount. | Watch still receives changes; test parent-directory mounting explicitly. |
| T06 | Remove the final managed route. | No stale native route remains; generated empty representation is valid for the pinned version. |
| T07 | Invalid route, missing service/resolver/entrypoint, or watch failure. | Requested activation is not claimed; diagnostics identify the failing layer. |
| T08 | Startup settings originate from arguments or environment, not the discovered YAML. | Setup edits the actual authorized source, or stops with instructions; no invented merge precedence. |
| T09 | Concurrent external file update and managed update/compensation. | Preserve external update and do not introduce a priority override. |
| T10 | Diagnostic API unavailable but file integration usable. | Report precisely what was verified and what remains unknown. |

Use live diagnostic APIs only when already enabled/authorized. Do not enable an insecure dashboard to improve test convenience in a production recipe. Do not build tests around a fictional generic `traefik validate` command. Watch/mount and startup-provider semantics follow the selected version's actual documentation. [S01, S07, S11]

## 7. Network, DNS, TLS, and verification

| ID | Scenario | Required result |
|---|---|---|
| N01 | Native proxy targets a host-loopback service. | Verify chosen address family and port, not merely DNS resolution. |
| N02 | Container proxy receives `localhost` upstream input. | Detect namespace ambiguity; never treat container loopback as host loopback. |
| N03 | Docker Desktop host access. | Use a proved host-access recipe and test the actual application bind/interface. |
| N04 | Linux Engine host-gateway path. | Require existing/approved mapping; test reachability, firewall, and bind address. |
| N05 | Proxy and application share a Docker network. | Select stable reachable service alias and internal port, not an arbitrary ephemeral address. |
| N06 | Docker API points to another machine. | Do not substitute the local host for the remote daemon's host. Unsupported writes fail explicitly. |
| N07 | Public DNS points elsewhere, or HTTPS issuance is still pending. | Separate routing activation from DNS/TLS readiness; never claim a public working URL without evidence. |
| N08 | Upstream HTTPS validation fails. | Retain verification; no automatic insecure transport fallback. |

Public ACME integration tests should use a staging/test environment and controlled DNS rather than production issuance on every CI run. Local route tests should avoid relying on public DNS or permanent trust-store changes. Local CA trust installation is an explicit separate action. [S20, S21]

| ID | Verification distinction | Required result |
|---|---|---|
| V01 | File published but controller did not observe it. | `pending`, `uncertain`, or failure according to the timeout contract; not applied. |
| V02 | Controller loaded something but requested route is absent. | Failure with route-specific evidence. |
| V03 | Route present, upstream unavailable from proxy context. | Routing and upstream states remain distinct. |
| V04 | Host-side probe succeeds but proxy-context probe is unavailable. | Do not claim proxy-to-upstream reachability. |
| V05 | Probe returns 401/403 for an intentionally authenticated app. | Distinguish configured expected response from health failure; do not assume every non-200 means unrouted. |
| V06 | Client disconnects during a mutation. | Journal remains recoverable; do not abandon unsafe half-written configuration. |
| V07 | Verification deadline expires after possible activation. | Receipt records uncertain outcome and recovery guidance; no blind duplicate retry. |

## 8. Explicit setup and installation

| ID | Scenario | Required result |
|---|---|---|
| B01 | No proxy; ordinary route command. | No downloads/installation; describe the explicit setup path. |
| B02 | Interactive setup on a clean supported host. | Offer Caddy/Traefik choices with runtime, scope, Docker, TLS, storage, and disruption preview. |
| B03 | Noninteractive setup missing required selections. | Structured validation failure, never a hanging prompt. |
| B04 | Docker absent or unauthorized. | Do not install Docker or start a Docker-dependent controller silently. |
| B05 | Caddy Docker build chosen. | Installed binary actually contains required extension/version; not a detached plugin file. |
| B06 | Existing package-manager proxy with extra modules and certificate data. | Preserve package ownership/modules/data; no blind binary replacement. |
| B07 | Bad digest/signature, wrong architecture, or malicious archive path. | Reject before installation; keep current executable/config intact. |
| B08 | Crash during download, extraction, registration, or startup. | Recoverable receipt and scoped cleanup; no unidentified partial installation. |
| B09 | Existing service restart or proxy-container recreation required. | Explicit setup approval; never recreate application containers. |
| B10 | Platform/version artifact unavailable. | State unavailable capability and reason; do not invent a download URL. |
| B11 | Upgrade or uninstall. | Explicit operation; retain certificates and external definitions by default. |
| B12 | AppTrail publishes a newer unrelated release in a shared repo. | Steward installer still resolves the intended component/channel/architecture. |

## 9. Native-service independence and systemd

Run native-manager cases on a real supported systemd host/VM, in both user and system scopes that Steward claims to support. Capture manager version, login/session setup, permissions, and whether linger was configured beforehand.

| ID | Scenario | Required result |
|---|---|---|
| S01 | Service-only command on host with no proxy or Docker. | Succeeds without initializing/probing proxy, Docker, or AppTrail dependencies. |
| S02 | Register service definition. | No implicit route, no implicit start, no undocumented startup change. |
| S03 | Start service, then Steward exits. | Native manager supervises the foreground executable; no Steward process required. |
| S04 | Start/stop while startup policy remains enabled. | Current running state changes independently; stop is not instantly undone by restart policy. |
| S05 | Enable/disable, with and without explicit `--now`. | Contract and backend side effects are visible; no hidden runtime change. |
| S06 | Definition changes while process is running. | Manager reload and any application restart are distinguished and planned. |
| S07 | User logout or host reboot. | Actual behavior matches documented startup/lifetime capability; linger is not enabled silently. |
| S08 | Application crashes under supported restart policy. | Observed manager behavior matches policy; no homegrown monitor loop. |
| S09 | Arguments contain spaces, quotes, dollar signs, percent signs, Unicode, or shell metacharacters. | Correct argv reaches process; no unintended expansion or shell execution. |
| S10 | Requested application reload unsupported. | Capability error, not a substitute restart. |
| S11 | Service removal or app-directory deletion failure. | Remove only owned definition/registration; never delete application data as part of service removal. |
| S12 | Missing executable, permission error, bad workdir, log failure, or startup timeout. | Structured diagnostics, retained evidence, and no false running status. |

Write a small fixture executable that records its argv, environment, working directory, UID, signals, and lifecycle events to a protected test directory. Use it to prove behavior instead of inferring it from generated text. Verify the selected systemd unit syntax against the installed manager. [S13–S15, S17]

## 10. launchd-specific acceptance

Modern command syntax and state behavior must be checked on the target macOS release using its native manual pages and experiments. The archived Apple design material is background, not a substitute for this verification. [S16, S27]

| ID | Scenario | Required result |
|---|---|---|
| L01 | Current user versus authorized system domain. | Correct plist location, manager domain, ownership, and identity. |
| L02 | SSH/headless context without the assumed GUI session. | Discover actual available domain; do not invent a `gui` target. |
| L03 | Plist registered, loaded, enabled, running, exited. | States are represented separately and read from native evidence. |
| L04 | Stop a KeepAlive-managed process. | Remains stopped according to requested runtime intent; preserve declared next-login/boot policy where supported, report native side effects. |
| L05 | Edit loaded plist. | Re-registration/restart required by the native manager is explicit; rewriting disk alone is not considered applied. |
| L06 | systemd-only restart/backoff/environment feature requested. | Unsupported or documented mapping, never pretend identical semantics. |
| L07 | Log out/in and reboot as applicable. | Startup behavior and user/system lifetime match the declared support contract. |
| L08 | Absolute executable from a version-managed development environment. | Runs without relying on interactive shell initialization. |
| L09 | Hostile arguments, workdir, env, and log paths. | Correct plist encoding/argv behavior, protected permissions, no shell interpretation. |
| L10 | Remove owned job; unrelated external jobs remain. | Correct target domain and scoped cleanup; no writes under `/System/Library`. |

A launchd adapter is not complete when only plist rendering works. Registration, control, state inspection, persistence, and failure handling are part of the deliverable.

## 11. Combined service-and-route operations

| ID | Scenario | Required result |
|---|---|---|
| X01 | New service and route with healthy endpoint. | Preflight both, create/start service, pass readiness, publish route, report both outcomes. |
| X02 | Readiness fails. | No new route published; service state and diagnostics explicit. |
| X03 | Route activation fails after service starts. | Partial non-success result; safe route compensation; service retained by default with durable ownership/desired-state receipt. |
| X04 | Explicit cleanup-on-failure for newly created resources. | Remove only new owned resources; never erase a pre-existing service or unrelated route. |
| X05 | Routes-only disable. | Service remains unchanged. |
| X06 | Service-only stop with attached route. | Warn about exposure/upstream effect; do not edit route automatically. |
| X07 | Combined down. | Disable owned route and verify before stopping service, with partial outcomes represented. |
| X08 | Remove service with attached routes. | Block until explicit detach/cascade plan approved. |
| X09 | Change explicitly linked endpoint port. | Proposed route change follows the explicit link; similarly numbered unlinked routes stay untouched. |
| X10 | Crash/reboot after successful initial composition. | No claim of ongoing readiness gating: native service/proxy behavior is reported; no hidden Steward controller. |

Test shared-resource and scope boundaries. A user-scoped service with a system-scoped proxy must not accidentally bypass authorization because a combined command contains both. Initial unsupported cross-scope transactions must be rejected before mutation.

## 12. Crash consistency and concurrency

| ID | Injection point | Required result |
|---|---|---|
| F01 | Two Steward writers target the same scope/integration. | Serialize or reject; no lost update. |
| F02 | External writer changes a source after plan creation. | Fingerprint mismatch blocks apply. |
| F03 | Crash after journal preparation, before publication. | Recovery identifies no native change and discards safe staging only. |
| F04 | Crash after file replacement, before activation acknowledgement. | Recovery inspects actual source/runtime; does not assume success or failure. |
| F05 | Crash after activation, before desired-state/receipt commit. | Reconcile actual effect and durable intent; no duplicate creation. |
| F06 | Disk full/fsync failure/permission change during commit. | Non-success receipt and recovery path; no silent journal loss. |
| F07 | External edit after Steward write but before compensation. | Refuse destructive restore of externally changed content. |
| F08 | Cancellation/deadline during reload/watch/restart. | Classify uncertain side effects and preserve operation evidence. |
| F09 | Cross-filesystem rename, symlink swap, path replacement. | Reject unsafe publication; verify target ownership/path again. |
| F10 | Recovery itself crashes or is retried. | Idempotent recovery and preserved history. |

The journal and native configuration do not form a distributed transaction. Tests must prove this limitation is handled explicitly, not hidden behind a database or filesystem transaction claim.

## 13. CLI, TUI, and agent usability

| ID | Scenario | Required result |
|---|---|---|
| U01 | JSON mode on success, error, partial, and uncertain outcomes. | One valid versioned result envelope; logs on stderr; non-success exit for failed/partial mutation. |
| U02 | Noninteractive stdin/stdout. | No TUI or blocking prompt; missing approval is actionable failure. |
| U03 | `--yes` on stale/setup-requiring ordinary operation. | Does not bypass source changes, privilege checks, or setup boundaries. |
| U04 | Dry run with tracked external files/processes. | No external mutation and no service/proxy activation. |
| U05 | Repeat identical desired state, then apply a subset/empty capability collection. | Identical input is a no-op; omitted resources stay unchanged; no implicit prune/reload/restart. |
| U06 | TUI edit/enable/disable versus equivalent CLI command. | Same validation, plan, ownership rules, and operation engine. |
| U07 | External or unsupported resource selected. | Unavailable actions clearly explained, not merely failing after execution. |
| U08 | Slow/unreachable provider during TUI use. | Responsive input/cancellation; partial discovery visible. |
| U09 | Long hostnames, Unicode, small terminal, monochrome output. | Usable layout; status not dependent on color alone. |
| U10 | Logs/config include credentials or control characters. | Secret redaction and safe terminal/JSON rendering. |

## 14. Security and adversarial fixtures

At minimum test command and rule injection through hostnames, paths, labels, service IDs, argv, and provider output; path traversal in archives and config includes; file permission and symlink races; untrusted plan modification; malformed/deep/oversized YAML/JSON; duplicate keys; denied Docker socket access; redirects toward cloud-metadata/link-local destinations; excessive probe responses; TLS failures; accidental secrets in errors/backups/export; and a stale privileged plan replayed against a different host or scope.

Discovery data must not authorize mutations or become executable commands. A read-only mount of a Docker socket is not a method-level Docker API policy. Privileged operations must revalidate source paths, identity, and approved effects. [S09]

Keep fixtures synthetic or scrubbed. Do not commit real tokens, private keys, certificate private material, personal host inventories, or full production process environments.

## 15. AppTrail optional-integration tests

These gates apply when the optional integration is implemented, not before its existence is claimed.

| ID | Scenario | Required result |
|---|---|---|
| A01 | AppTrail absent/stopped; Steward used. | Steward operation remains functional. |
| A02 | Steward absent/stopped; AppTrail uses its existing providers. | Existing AppTrail behavior remains functional. |
| A03 | Snapshot schema/version/host/source identity mismatches. | Reject or mark unsupported; no guessed mapping. |
| A04 | Same route appears in Steward and existing proxy observations. | Correlate through verified identity rules without duplicate app or forced false merge. |
| A05 | Partial/failed/stale snapshot. | Do not interpret absent entries as proven deletion. |
| A06 | User hides/renames an app in AppTrail. | Does not disable service/route or overwrite Steward desired state. |
| A07 | Optional management action unauthorized or replayed. | Denied before apply; no shell/privilege bypass. |
| A08 | Products build in same repo or separately. | No mandatory shared database/UI dependency; component release selection works. |

## 16. Completion evidence

Each milestone closes with a report linking requirement IDs to actual automated tests and native test transcripts, listing exact versions, failures, skipped cases, unsupported combinations, and reproducible commands. A release candidate includes clean-host install/uninstall evidence, rollback/recovery evidence, and an honest supported-version/platform table.

Do not publish a green summary that hides skipped Caddy Docker refresh tests, untested launchd control, or missing container-to-host reachability. Those are central product capabilities. A material failure returns to the user with the observed evidence and a proposed alternative rather than being silently narrowed out of scope.
