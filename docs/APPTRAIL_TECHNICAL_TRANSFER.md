# AppTrail → Steward technical transfer

**Completed source-backed handoff, 2026-10-09.** Selected workspace:
`/home/samimishal/projects/rust/steward`; public repository:
https://github.com/samishal1998/steward. Read the
[accepted repository decision](decisions/repository-placement.md) and
[implementation plan](IMPLEMENTATION_PLAN.md) before implementing adapters.

## 1. Orientation and evidence labels

AppTrail is a Go discovery/dashboard application; Steward is a standalone Rust
CLI/TUI for independent route and native-service management. Similar Docker,
Caddy and Traefik endpoints are knowledge overlap, not a shared production-code
boundary. AppTrail's package `main`, lossy observations and SQLite reconciliation
are not importable Rust management components. One Cargo package with internal
modules is the accepted implementation refinement.

Preserve: exact-host/one-upstream writable route subset; Caddy plain-file and
Caddy Docker controller plus Traefik watched-file management first; macOS/Linux
general native services later; service-only operation without proxy/Docker;
no required daemon; no Apache; explicit setup; Docker labels read-only;
effective API inspection preferred over labels; custom mappings deferred.
Current read code is **not evidence of safe write behavior**. Ownership needs a
receipt, native identity/path and fingerprint. AppTrail integration is optional.

| Label used here | Meaning |
|---|---|
| **Verified from code** | Static inspection of the identified baseline source/tests; not a runtime result |
| **Prior CI evidence** | Previously reported successful baseline workflow; no tests run in this handoff |
| **Verified upstream source** | Pinned upstream source inspected in the prior audit; execution still unverified |
| **Documented upstream** | Official documentation reviewed in the source pack; moving documentation requires recheck |
| **Proposed** | Steward contract/work to implement; not a shipped feature or upstream API |
| **Unknown** | Missing evidence with a concrete verification gate |
| **Historical proposal** | Archived plan/PRD/template, not current implementation evidence |

## 2. Baseline and reading map

**Verified from code:** AppTrail `main` at
`2a3ac36bed33aca03645d45f5612ad0e02c384ed`, local
`/home/samimishal/projects/rust/app-trail`. All source links and line ranges below
are tied to that commit. Do not substitute concurrent working-tree changes.

| Area | Source / navigation | Finding |
|---|---|---|
| Stack | [go.mod][go], `web/package.json`, [main.go][main] L28–40 | Go 1.26.8, stdlib HTTP, SQLite 1.58.0, React 19/TS/Vite embedded web bundle |
| Config/API | [providers.go][providers] L17–133, [main.go][main] L259–283 | Provider JSON and SQL rows, owner-protected HTTP endpoints |
| Transport/secrets | [provider_http.go][http] L16–102 | Explicit endpoint + authorization-file reference; bounded GETs |
| Docker | [providers.go][providers] L264–393 | Container enumeration and labels; no SDK/context negotiation |
| Caddy | [provider_caddy.go][caddy] L11–37, L123–272 | Native admin JSON, partial normalization |
| Traefik | [provider_traefik.go][traefik] L12–38, L44–250 | Three diagnostic GETs, partial rules, app alias grouping |
| Match helpers | [proxy_routes.go][routes] L15–164, L184–228 | Literal Host/Path/PathPrefix, launch-link generation |
| Contracts/identity | [registry.go][registry] L14–53, L62–154, L155–294 | Observations, globally keyed identities, priority-selected app facts |
| Session auth | [auth.go][auth] L20–65, L91–161 | Single-owner account, bcrypt, cookie sessions; not service-control authorization |
| Native lifecycle | [service.go][service], [Linux][linux], [macOS][darwin], [Windows][windows] | AppTrail self-install/control only |
| Tests | [apptrail_test.go][tests], [proxy_providers_test.go][proxytests], [alias_merge_test.go][aliastests], [service_test.go][servicetests] | Inline synthetic payloads; fixture map in §10 |
| CI/release | [checks.yml][checks], [release.yml][release], [install.sh][install] | Go+web tests/builds, AppTrail-specific version/assets |
| Historical context | [archived pack](planning/README.md), [old PRD](planning/reference/Apptrail_PRD.original.md) | File provider/conceptual watch interface are proposals, not baseline contracts |

**Prior CI evidence:** [Checks run 35741675602][ci-run] successful baseline.
Native macOS logs were unavailable during the audit. The smoke script can skip
when no GUI domain exists ([service-smoke.cjs][smoke] L10–13), so green CI does
not prove a particular LaunchAgent runtime scenario executed. **No new tests or
live integrations were performed for this documentation handoff.**

## 3. Exact connection contract

**Verified from code:** `provider` fields are `id`, `name`, `endpoint`, `type`,
`auth_file`, `enabled`, `scanned`, `error`, `count`. The save request only accepts
name/endpoint/enabled/type/auth_file. This is AppTrail JSON, not Steward YAML.

| Setting | Actual rules/defaults | Evidence |
|---|---|---|
| `name` | Required nonblank, maximum 100 bytes (`len` of Go string) | `saveProvider` L83–86 |
| `type` | Empty -> `docker`; only `docker`, `caddy`, `traefik`; immutable on update | `saveProvider` L87–93, L114–126 |
| `endpoint` | Required valid HTTP(S) base URL or `unix:///absolute/socket`; <=2048; no userinfo/query/fragment; HTTP(S) base path retained | `validEndpoint` L53–66; `saveProvider` L94–105 |
| `auth_file` | Optional empty string; otherwise absolute server-local path, <=4096, no NUL/CR/LF | `saveProvider` L98–101 |
| `enabled` | Request bool: omitted/null decodes false; persisted new schema column defaults 1, but API insertion uses supplied bool | `saveProvider`, `main.go` schema |
| Response metadata | `scanned` epoch seconds; `count` observation count; `error` stores either failure or successful warning | `scan` L181–217 |

Go zero-valued string/bool fields generally treat omission/null as zero; this is
not an explicit versioned nullable configuration contract. `id` is generated for
creation and taken from the URL for update. SQL `kind`/`auth_file` migrate with
defaults `docker`/empty. DB uses `PRAGMA user_version=2`, rejects newer schemas;
provider/observation JSON has no separate published serialization version.

The owner API request decoder requires `application/json`, caps the body at
1 MiB, rejects unknown fields and a second JSON value (`main.go:body` L339–355).
Error responses are `{ "error": string }`; provider warning/error strings are
not typed write-eligibility diagnostics. Public dashboard JSON deliberately
omits private provider/fact inventory; use existing owner authorization for
any read integration instead of treating public cards as source authority.

**Transport, verified from code:** `newProviderClient` rereads `auth_file` once per
new client/scan; regular file <=8 KiB, whole trimmed contents are one complete
HTTP `Authorization` header value, nonempty and without control characters.
No automatic `Bearer` prefix. Rotation applies on the next scan. Credential
values and API error bodies must not enter diagnostics. Never copy credentials
from AppTrail into Steward.

* Explicit HTTP/HTTPS or Unix socket only; HTTP transport `Proxy:nil` does not
  use environment HTTP proxies. Unix requests use synthetic `http://localhost`
  but dial the configured socket path.
* Dial and TLS-handshake timeout **5 seconds** each; per-request **12 seconds**;
  response headers **32 KiB**; response body **8 MiB** (limit+1 detection).
* GET only, `Accept: application/json`; **200 only**, redirects never followed.
  Non-200 produces status diagnostic without exposing the response body.
* Standard HTTPS verification; no custom CA bundle, client certificate/mTLS,
  Docker SSH transport, `tcp://` alias, context or environment negotiation.
* Caller context propagates; scan has **45 seconds** overall. There is no
  explicit retry/backoff or provider watch/cache interface. Clients close idle
  connections after snapshot.

Permission is that of the AppTrail server process: socket access, file read and
network reachability. A restricted API proxy exposing the required GET paths may
work; no particular proxy deployment is certified by these tests. Read-only
requests or a `:ro` socket mount do not make Docker daemon authority read-only.

## 4. Docker discovery and limitations

**Verified from code:** `dockerSnapshot(ctx, provider)` returns
`([]observation, warning string, error)` using:

| Purpose | Actual operation | Fields / handling |
|---|---|---|
| Enumeration | `GET /v1.44/containers/json?all=true` | `Id`, `Names`, `Image`, `State`, `Status`, `Labels`; null array or missing ID rejects snapshot |
| App identity/metadata | Read `apptrail.*`, Compose project/service labels | `apptrail.enable=false` omits; `apptrail.id` explicit identity; metadata priorities |
| Route fallback | Literal `traefik.http.routers.<router>.rule` + `tls`/`tls.*` labels | `apptrail.url` wins; otherwise infer URLs; unsupported expressions increment warnings |
| Ports/networks/inspection | **Not implemented** | No inspect endpoint, published ports, exposed ports or network addresses consumed |
| Events/Swarm/version | **Not implemented** | No events, reconnect/resync, Swarm services/nodes or API version negotiation |

API v1.44 is hardcoded (source comment targets Engine 25+); older/alternative
daemon compatibility is **unknown**. Explicit rootless/Desktop/remote socket or
HTTP(S) endpoints can be configured if reachable; no automatic path/context
discovery exists. TCP via HTTP(S) is distinct from Docker `tcp://` handling.

Native key uses provider ID + Compose project/service or first container name +
router discriminator, not daemon identity. Explicit `apptrail.id` and normalized
URL keys are global. Container recreation can retain dashboard identity via
these keys; the inline test exercises it. The observation does not retain raw
Docker ID, full labels, daemon identity, ports or networks. Steward must retain
those native facts independently when needed; never infer a safe backend address
from AppTrail's launch URL.

There is **no Caddy label parser** in AppTrail. Baseline Steward support for
`lucaslorentz/caddy-docker-proxy` labels remains a planned read-only feature,
not existing AppTrail code. Synthetic Caddy-label and network cases in §10 need
new Steward fixtures; no personal inventory is necessary.

## 5. Caddy: implemented read versus proposed control

**Verified from code:** `caddySnapshot` performs `GET /config/` under the chosen
base path (e.g. base `/admin` yields `/admin/config/`). This reads current native
config; it does not parse a Caddyfile, import files, discover startup authority,
inspect plugin versions or distinguish plain native, ordinary Docker and
Docker-controller deployments. It works from understood output without knowing
the label convention; that does not certify management compatibility.

| Native input | Actual treatment | Loss / warning |
|---|---|---|
| Root object / `apps.http` | Null root rejected; unrelated nonempty root rejected; recognized root without HTTP app returns empty | Empty supported HTTP inventory differs from connection failure |
| `http_port`, `https_port`, servers | Zero ports default 80/443; server names sorted; `listen` must yield single TCP port | Host publication, NAT and reachability not established |
| `match` OR sets, host/path arrays | Host/path alternatives combined with nested inherited matches; empty explicit arrays don't become catch-all | Unknown matcher marked unresolved; expansion <=10,000 |
| Handler `subroute` | Recursive walk, <=32 levels | Only selected nested semantics modeled |
| `reverse_proxy`, `file_server` | Emit resolved or unresolved app observation | Other handlers skipped; no full route/handler round trip |
| `upstreams[].dial` | Backend fact only when exactly one upstream | Pools, transport/TLS options, retries and middleware not preserved |
| Route/handler `@id` | Handler ID preferred; native caddy-id key only for one resolved URL | Otherwise server/normalized URL or synthetic unresolved fingerprint |
| TLS policies/automatic HTTPS/listens | Infer launch scheme using `caddyUsesTLS` | Not proof of certificate issuance, TLS trust or actual routing outcome |

`caddyRoute`, `caddyHandler`, `caddyServer`, `caddyHTTP` are partial Go structs
([caddy][caddy] L11–37). Omitted/null slices/maps generally decode empty; unknown
JSON fields are not retained in the final observation. Unknown matcher raw JSON
exists while parsing but not as a lossless management record. Caddy global
options, certificate storage, filesystem authority and module inventory are not
management contracts in AppTrail.

**Verified upstream source, prior audit:** `caddy-docker-proxy` **v2.13.1**, commit
`2434d713b4906d12f3379ecb8a584e0ef97e6039`:

* [generator/generator.go L72–84][cdp-generator] reads the base file during generation.
* [loader.go L234–275][cdp-loader] regenerates on timer and compares generated
  bytes before adaptation/application.

This improves the earlier pack's moving-branch evidence; it does **not** prove
live base-only edit activation. A changed imported leaf may leave the compared
generated Caddyfile bytes unchanged. **Proposed:** owned route content in an
explicitly authorized base region or wholly owned base; preserve unrelated
bytes. Verify file-only refresh, latency, validation failures, later Docker events
and restart on the pinned build. Never overwrite controller-generated output or
push base-only config into its admin API. An unfamiliar controller may be
inspectable but must remain read-only without an approved write integration.

**Unknown, release gate:** actual Caddy versions/modules, import layout, runtime
reload authority, disabled admin behavior, mounts, certificate paths and full
validation behavior for Steward. Inspect selected startup command/service and
module list; validate the complete candidate with the same environment, then
observe native route identity and preserve unrelated routes through recovery.
`import` is parsing, not a watcher. No automatic API enablement/restart is a
discovery fallback.

## 6. Traefik: actual diagnostic interfaces

**Verified from code:** `traefikSnapshot` performs these three requests in order,
under the configured base path, each with **`?per_page=10001`**:

| Endpoint | Partial native schema consumed |
|---|---|
| `GET /api/http/routers` | `name`, `rule`, `service`, `status`, `provider`, `entryPoints`, `using`, raw `tls` |
| `GET /api/entrypoints` | `name`, `address`, `http.tls` |
| `GET /api/http/services` | `name`, `status`, `serverStatus`, optional `loadBalancer.servers[].url` |

All must return non-null arrays; each <=10,000 records. The transport accepts
absent `X-Next-Page` or values `0`/`1`; values greater than 1 reject pagination
(the implementation rejects any other nonempty string too). There is no paging
loop. Failure of any endpoint retains prior observations because reconcile is
not called. The requests are not an atomic runtime snapshot. AppTrail does
**not** call `/api/rawdata`; that remains an upstream documented diagnostic
option for Steward, not a transferred implementation.

`traefikMatches` ([routes][routes] L87–164) uses Go AST expressions: literal
`Host`, `Path`, `PathPrefix`, parentheses, AND/OR; host alternatives retained;
PathPrefix represented by trailing `*`. Other matchers, negation, computed args,
dynamic paths and unsupported expressions reject parsing. Rule <=64 KiB,
depth <=32, expansion <=10,000. These are AppTrail parser limits, not full
Traefik v2/v3 rule-language compatibility guarantees.

**Read normalization:** internal provider/`@internal` routers and
`noop@internal` services omitted; nonempty status other than `enabled` increments
warning and skips router. `using` takes precedence over `entryPoints`. Missing
entrypoint, unknown rule or non-single-TCP listener creates unresolved output.
TLS inferred from router/entrypoint raw TLS value (present except null/false).
Service lookup uses exact name then the router provider suffix for unqualified
service names. One load-balancer server yields a backend fact; `UP`/`DOWN` and
service status yield display health. Resolver, priority, middleware, complete
transport and server pools are not preserved.

`groupTraefikAliases` groups by router and normalized URL path, prefers HTTPS,
keeps aliases/entrypoints, and supports legacy card consolidation. That is
dashboard behavior; Steward needs each native route, provider qualification,
raw constraints and native disabled/error/internal status where relevant to
conflict analysis. A hostname is not enough to correlate instances.

**Not implemented in AppTrail:** startup/dynamic file reading, watched-directory
setup, Docker runtime/mount/source authority detection, writes, validation or
reload. API/label discovery cannot establish these. **Unknown:** selected
Traefik version, rule syntax, file/CLI/environment startup authority and mount
recipe. **Proposed gate:** pinned directory watch test covering atomic replace,
empty/final removal, rejected candidate, cross-provider name collisions and
external-route coexistence. Respect the documented parent-directory bind-mount
recipe; do not invent a universal `traefik validate` command or restart on
ordinary dynamic writes.

## 7. Cross-provider contracts, identity and flow

**Verified from code:** actual [registry contracts][registry] L14–53:

```text
fact          = {value: string, priority: int, source: string}
observation   = {source: string, keys: []string, facts: map[string]fact,
                 urls?: []string, alias_group?: bool}
application   = {id, name, url, urls, description, icon, category, health,
                 lifecycle, favorite, hidden, created, seen, sources,
                 active_sources, fields, probe_error?}
override      = optional pointer fields for name/description/url/icon/category/
                favorite/hidden
```

Snapshots are in-process `[]observation` plus warning/error, not the historical
PRD's exported Snapshot/Watch/Diagnostics provider interface. Facts/keys are
application-specific; required validity is enforced by individual parsers,
not a public versioned schema. Pointer overrides treat omitted/null as no
change; empty string or false can be an explicit override where validation
permits it.

`scan` rereads current SQL provider row under a global scan lock, creates a
45-second context, fetches a snapshot, and reconciles only on nil error.
Scheduler scans enabled providers serially immediately and every five minutes;
manual scan endpoint also exists. No Docker event stream or native watch.
Successful warnings still reconcile partial output and update count/time/error;
there is no structured completeness flag suitable for write planning.

`reconcile` transaction marks previous observations in that provider scope absent,
matches global identity keys, persists new facts and commits. Failure rolls back;
a failed fetch does not mark prior apps missing. Successful empty snapshot does.
`apps` computes display facts from active sources, then user overrides; stale
classification follows 24 hours missing, while prior last-known facts/preferences
remain inspectable. SQLite is AppTrail's cache/registry, not Steward's journal.

Identity keys: `explicit:<apptrail.id>`, `native:<provider-id>:<source>`, and
`url:<normalizedURL>`. URL normalization lowercases host, removes default port,
fragment and trailing slash. Global URL matches merge dashboard identities across
providers; conflicting explicit identities reject. Traefik alias groups have
additional merge/prune behavior. Provider/source provenance and display priority
survive as strings; full native objects, engine identity and authority do not.
API URL fact priority 35 beats label-inferred route 30; `apptrail.*` metadata
80, same-origin app manifest metadata 90, and user overrides 100 can win display
selection. **These priorities and
global URL identity cannot authorize route mutation or deletion.**

**Proposed Steward contract:** preserve engine/controller/runtime/network axes,
binding ID, native qualified identity, raw source references/fingerprints,
freshness, structured coverage/errors and unresolved constraints. A failed or
partial source must not become an empty conflict inventory. Separate desired,
observed, owned and activated state; do not feed app cards back into a writer.

## 8. AppTrail integration and native lifecycle boundary

**Verified from code:** registered providers are Docker/Caddy/Traefik only.
There is **no file-inventory provider** at this baseline, despite the old PRD.
The metadata manifest read by enrichment describes an app, not an inventory or
an executable instruction. [enrich.go][enrich] L245–295 consumes same-origin
`/.well-known/apptrail.json` (or a same-origin `manifest` fact override):
`version` must be string `"1"`; optional `id`, `name`, `description`, `icon`,
`category`, and optional/null `health` object containing `url`, `method`,
`expected_status` integer array. Missing health method defaults GET; supported
methods GET/HEAD; empty expected-status list defaults `[200]`; icon/health URLs
must be same-origin. A manifest ID can add an explicit identity if it does not
conflict with another app; it cannot authorize execution.

`POST /api/apps` creates one manual application with
`name`, `url`, `description`, `category`, `icon`; it is not a snapshot import or
delete/freshness contract. Provider APIs and `GET /api/apps` require an owner
session. Single-owner account plus bcrypt/cookie sessions is not a service-control
authorization model. [main.go][main] exposes no service-control endpoint.

* **Stage 0:** no new AppTrail code required in principle. Existing authorized
  providers can observe supported resulting proxy routes. Live identity/alias
  behavior still needs an integration check; neither product requires the other.
* **Stage 1, proposed:** versioned sanitized Steward export + new AppTrail import
  provider, with host/resource identity, explicit correlation, freshness,
  completeness and duplicate handling. This needs new AppTrail code.
* **Later control, proposed:** narrowly authorized local CLI or protected remote
  boundary using Steward plan/ownership/journal semantics; separate approval
  design needed. No shared DB/credentials or management commands derived from
  public metadata, labels, display overrides or manifests.

**Native knowledge, verified from code:** `serviceConfig` is only
`Executable, Addr, Data, Origin`; executable is AppTrail itself; args are fixed
`-addr/-data/-origin`. Linux fixed `apptrail.service` under user systemd;
install writes the unit, daemon-reloads, enables and restarts. macOS fixed
`io.github.samishal1998.apptrail` LaunchAgent in `gui/<uid>`; install writes plist,
bootouts then enable/bootstrap/kickstart; no GUI domain can prevent activation.
Windows fixed `Apptrail` self-service uses SCM, automatic startup under
`LocalService`, executable/data ACL grants and recovery actions, not general
service specs. These Windows details are transfer context; Steward Windows is
later scope, not current support.

Unix ownership checks regular-file + marker text, with temp/rename writes;
no durable operation journal, ownership fingerprints or drift-guarded recovery.
`runCommand` uses unbounded `CombinedOutput`, no context timeout; unsuitable as
Steward's executor. Escape/rendering tests are useful lessons, not proof of an
arbitrary-service manager. Steward must separately implement argv-only bounded
execution, native status, scope, lifecycle policy and journaled ownership.
Do not promise launchd/systemd parity without on-target tests.

## 9. Networking, failures and operational lessons

AppTrail derives launch URLs from proxy listeners/label TLS assumptions. It does
not resolve published port mappings or prove proxy-context upstream reachability.
Host loopback, container loopback, Desktop VM and remote daemon host are different.
Rootless socket paths and remote endpoints require explicit configuration.
**Documented upstream:** Desktop `host.docker.internal` and Linux `host-gateway`
are topology-specific setup, not proof that a loopback-only app is reachable.
No automatic broad bind, host-network mode, network attachment, port publishing
or TLS-verification bypass is authorized. Steward must prove the selected
network recipe before applying a route; unresolved remote-host intent blocks it.

| Symptom | Cause / evidence status | Lesson / verification |
|---|---|---|
| Auth fails after rotation | Whole header file reread; wrong value produces 401, source test | Correct the authorized header file; do not log values/bodies; rerun scan |
| Proxy returns unrelated JSON | Caddy root/Traefik array validation, source tests | Keep prior facts on failed scan; diagnose wrong base path/API |
| Redirected API access fails | Redirect deliberately refused, source test | Configure final authorized endpoint; never leak auth to redirect target |
| Missing apps after truncated Traefik response | Rejected `X-Next-Page`, source test retains prior observations | Never reconcile partial pagination as complete |
| Wildcard/Method/HostRegexp yields unresolved | Partial matcher semantics, source tests | Retain unsupported constraints and block overlapping writes, not broaden rules |
| Alias/internal cards pollute dashboard | Prior alias cleanup tests in baseline | Dashboard cleanup is not infrastructure route deletion |
| LaunchAgent job green without lifecycle evidence | Smoke test can skip missing GUI domain, code verified | Obtain native logs from logged-in target and test stop under KeepAlive |
| Imported Caddy edit not seen by controller | Generated-byte comparison risk, upstream source; live result unknown | Test direct managed base-content change and later events/restarts |
| File appears but Traefik route not active | Mount/watch/provider hypothesis; not tested here | Verify native activation, atomic replacement, final removal on pinned topology |
| Steward update accidentally resolves AppTrail | Existing installer hardcoded product/repo, code verified | Independent Steward resolver/asset/version tests |

## 10. Fixture and test transfer

AppTrail provider fixtures are inline Go `httptest` responses, not a reusable
fixture directory. **No code or fixture files were copied by this documentation
task.** No AppTrail `LICENSE` was found in the audited root; inspect permission
and attribution before copying existing implementation/test text. Prefer new
synthetic upstream-input fixtures with provenance notes. Use `.example.test`,
loopback, synthetic IDs and no real credentials; upstream/API versions below
describe the shape, not executed engine compatibility.

### Concrete synthetic-fixture map for the implementation agent

Proposed destinations are relative to Steward; not created or claimed runnable
here. Preserve the source test permalink in each fixture's metadata/readme.

| Proposed fixture / purpose | Source case / immutable permalink | Construction and expected result |
|---|---|---|
| `tests/fixtures/docker/containers-recreated.json` | [TestDiscoveryIdentityAndFailedScans L131–199][docker-test] | Docker v1.44 list with Compose project/service, synthetic ID changing between scans, literal Traefik Host+PathPrefix/TLS; dashboard identity survives recreation; Steward retains binding-scoped native facts |
| `tests/fixtures/docker/list-failed.json` + mock response | [same test][docker-test] | Mock HTTP 500 then `[]`; failure retains prior facts, successful empty snapshot records absence; unavailable endpoint is not empty |
| `tests/fixtures/caddy/config-nested.json` | [TestCaddyDiscoveryAndAuthorization L71–177][caddy-test] | Caddy v2-shaped JSON: nested host/path reverse proxy, plain file_server, wildcard; two launchable app URLs and one unresolved route in AppTrail; Steward keeps unsupported/native details |
| `tests/fixtures/caddy/config-auth-failure.json` + mock auth rotation | [same test][caddy-test] | Synthetic header supplied only in test memory/temp file; rotate, then wrong header -> 401; no credential/error-body leak; unrelated root rejected; empty HTTP servers is successful empty |
| `tests/fixtures/traefik/routers.json`, `entrypoints.json`, `services.json` | [TestTraefikDiscoveryMergingAndTruncation L179–244][traefik-test] | HTTP diagnostic arrays: enabled `@file` aliases + path, `@docker`, HostRegexp, disabled; 8443/8080 entries; single-server backend/UP. AppTrail emits two app groups + unresolved and omits disabled; Steward preserves disabled status separately |
| `tests/fixtures/traefik/pagination.json` + mock header | [same test][traefik-test] | All requests require `per_page=10001`; header `X-Next-Page: 2` fails entire snapshot without deleting prior observations |
| `tests/fixtures/traefik/internal-aliases.json` | [TestTraefikInternalRoutersAndHostAliases L11–119][alias-test] | Internal/noop routers + two app routers, four aliases on one; AppTrail -> two groups, no internal warnings; preserve raw identities for Steward conflict analysis |
| `tests/fixtures/rules/constraints.json` | [TestProviderRedirectsAndRouteConstraints L246–284][constraint-test] | Unsupported Method rejects; incompatible exact/path-prefix intersection yields none; empty Caddy host array never broadens to catch-all; UDP/Unix listener not single TCP |
| `tests/fixtures/transport/redirect.json` + mock servers | [same test][constraint-test] | 302 from authenticated endpoint never reaches target; non-200 is diagnostic only |
| `tests/fixtures/docker/host-published.json` | **New synthetic fixture**, not covered by existing Docker fields | Add list Ports/NetworkSettings for host-published app, with explicit endpoint/bind context; AppTrail ignores these; Steward must qualify host reachability instead of guessing |
| `tests/fixtures/docker/shared-network.json` | **New synthetic fixture**, no baseline network parser | Shared-network app+proxy, network ID/alias and unpublished port; demonstrate required network evidence, not automatic attachment |
| `tests/fixtures/docker/caddy-labels.json` | **New synthetic fixture**, no AppTrail Caddy label parser | Minimal `caddy`/`caddy.reverse_proxy` intent paired with understood Caddy current JSON; use baseline caddy-docker-proxy convention; unknown-label variant discovered via native JSON, never grants write capability |
| `tests/fixtures/docker/remote-unavailable.json` | New topology case + [failed-scan test][docker-test] | Remote daemon address/host differ from CLI machine; inaccessible GET yields failure; no inferred local host upstream or silent Docker context selection |

### Future reproduction commands (not executed)

Run from an authorized disposable AppTrail checkout of the identified baseline,
with Go 1.26.8 and Node 24. The `go:embed web/dist/*` directive requires building
the web bundle **before** Go tests. Setup below may download dependencies; it is
provided for later reproduction and was not performed in this handoff.

```sh
# Working directory: /home/samimishal/projects/rust/app-trail
npm ci --prefix web
npm run build --prefix web
go test ./... -run '^TestDiscoveryIdentityAndFailedScans$' -count=1
go test ./... -run '^TestCaddyDiscoveryAndAuthorization$' -count=1
go test ./... -run '^TestTraefikDiscoveryMergingAndTruncation$' -count=1
go test ./... -run '^TestTraefikInternalRoutersAndHostAliases$' -count=1
go test ./... -run '^TestProviderRedirectsAndRouteConstraints$' -count=1
go test ./... -run '^TestServiceConfigurationAndOwnership$' -count=1
go test -race ./...
go vet ./...
```

Expected result: Go `ok` for the selected tests; assertion expectations are in
the fixture map. These are synthetic parsing/registry tests, not live proxy or
native service integration tests. Successful prior CI is recorded separately
in §2. New Steward fixtures need meaningful Rust assertions for source/coverage,
native identity and planning rejection, not app-card grouping equivalence.
Planned Steward checks: `cargo test`, `cargo fmt --check`,
`cargo clippy --all-targets -- -D warnings`; no Rust test result is claimed here.

## 11. Upstream documentation map

The source pack reviewed official references on 2026-10-08/09; versionless pages
can change. This task adds no new live/compatibility evidence.

| Topic | Primary source | What is established / qualification |
|---|---|---|
| Docker API | [Engine v1.44](https://docs.docker.com/reference/api/engine/version/v1.44/) + [providers.go][providers] | Exact AppTrail list path; SDK/context/Swarm behavior absent |
| Docker authority/network | [daemon access](https://docs.docker.com/engine/security/protect-access/), [Desktop networking](https://docs.docker.com/desktop/features/networking/) | Transport privilege and context distinctions; topology tests pending |
| Caddy current config | [admin API](https://caddyserver.com/docs/api), [Caddyfile import](https://caddyserver.com/docs/caddyfile/directives/import), [CLI](https://caddyserver.com/docs/command-line) | Read/current versus input authority; import not watcher; full validation/reload requires target proof |
| Caddy Docker controller | [v2.13.1 README][cdp-readme], [generator][cdp-generator], [loader][cdp-loader] | Pinned base read/timer/byte comparison verified upstream source; live activation unverified |
| Traefik | [API](https://doc.traefik.io/traefik/reference/install-configuration/api-dashboard/), [file provider](https://doc.traefik.io/traefik/reference/install-configuration/providers/others/file/), [Docker provider](https://doc.traefik.io/traefik/reference/install-configuration/providers/docker/) | Diagnostic reads and dynamic source/watch model; no universal mutation/validation API |
| systemd | [official manuals](https://www.freedesktop.org/software/systemd/man/latest/systemctl.html), [source-pack references](planning/07_VERIFIED_REFERENCES.md#3-native-service-manager-references) | Native lifecycle/scope; installed-version compatibility must be checked |
| launchd | [Apple guidance](https://support.apple.com/guide/terminal/script-management-with-launchd-apdc6c1077b-5d5d-4d35-9c19-60f2397b2369/mac) + on-target `man launchctl`, `man launchd.plist` | Scope/lifecycle concepts; modern command semantics need native evidence |
| Release selection | [GitHub releases API](https://docs.github.com/en/rest/releases/releases) + [install.sh][install] | Latest is repository-scoped; dedicated Steward repo/asset namespace required |

## 12. First implementation session

1. In `/home/samimishal/projects/rust/steward`, read the completed decision,
   this transfer, [plan](IMPLEMENTATION_PLAN.md), [capabilities](CAPABILITIES.md)
   and archived PRD/architecture/addendum. Historical commands/examples are not
   an implemented CLI contract.
2. Use one Rust package/internal modules; establish dependency/license and
   configuration/result schema choices. Do not import Go, AppTrail SQLite,
   frontend, server or credentials. Consult actual `--help` as code lands.
3. Independently author the synthetic fixture map above; run future Rust checks
   and record exact commands/results/commit. If reproducing AppTrail tests,
   rebuild `web/dist` first in an authorized disposable checkout.
4. Deliver the first runnable inventory + route-planning slice described in
   the plan. Preserve native evidence, unresolved coverage and source precedence;
   all commands in this slice are read-only. Document unsupported apply/service
   operations rather than simulating success.
5. Pin proxy/controller/native versions and perform the blocking disposable
   compatibility experiments. Journal/receipts, fingerprint rechecks, bounded
   executor and crash recovery must exist before any production adapter writes.
6. Update evidence/status documents; carry full route/service/setup/TUI/composition
   milestones forward. Do not call this first slice the complete product.

## 13. Unknowns and required verification

| Unknown | Exact next verification | Blocks / owner |
|---|---|---|
| Live Caddy Docker base-only activation | Pinned v2.13.1 disposable controller: edit managed base content without Docker events; measure activation; verify external routes after events/restart and rejection/recovery | Caddy Docker writes/release; proxy implementer |
| Plain Caddy full input authority/modules | Inspect startup source/module versions, validate complete imported candidate in same context, graceful reload and rollback while external routes survive | Plain Caddy writes; proxy implementer |
| Traefik source/watch/mount behavior | Pin version/startup source, parent-directory mount; test atomic replacement, final deletion, provider diagnostics and external routes | Traefik writes; proxy implementer |
| Host/container/remote mapping | From selected proxy context prove upstream DNS/address/interface/TLS and native route-specific response | Affected route apply; networking spike owner |
| Native macOS runtime | Logged-in disposable macOS domain; capture non-skipped install/start/stop/KeepAlive/restart/enable/disable/uninstall logs | macOS advertised native support; native implementer |
| Native Linux general-service semantics | Pin systemd version; test user/system scope, login/linger, argv/environment/log limits and lifecycle/recovery | General native services; native implementer |
| Artifact/module provenance, license | Inventory Steward dependencies and binary sources; inspect AppTrail reuse rights before copying; ask user for new repo licensing decision | Copying/release distribution; maintainer |
| Later AppTrail export contract | Design versioned importer and test duplicate/freshness/partial semantics with no shared credentials/DB | Optional Stage 1 integration; integration owner |
| Exact CLI/config freeze | Match first-slice implementation help and examples; reject unsupported versions and document schema evolution | Stable CLI contract; Rust implementer |

## 14. Completion review

Repository topology is selected and approved; actual connection, native schema,
normalization and auth/lifecycle boundaries are recorded with immutable sources.
The inline-fixture map provides reproduction and new synthetic construction
instructions without copying credentials or claiming missing coverage. Completed
implementation/capability/progress documents accompany this transfer; the full
source pack is preserved under `docs/planning/` with its historical template
status. Remaining runtime and licensing unknowns have explicit gates, not blank
prompts. No new tests were performed and no write safety is claimed.

[go]: https://github.com/samishal1998/apptrail/blob/2a3ac36bed33aca03645d45f5612ad0e02c384ed/go.mod
[main]: https://github.com/samishal1998/apptrail/blob/2a3ac36bed33aca03645d45f5612ad0e02c384ed/main.go
[providers]: https://github.com/samishal1998/apptrail/blob/2a3ac36bed33aca03645d45f5612ad0e02c384ed/providers.go
[http]: https://github.com/samishal1998/apptrail/blob/2a3ac36bed33aca03645d45f5612ad0e02c384ed/provider_http.go
[caddy]: https://github.com/samishal1998/apptrail/blob/2a3ac36bed33aca03645d45f5612ad0e02c384ed/provider_caddy.go
[traefik]: https://github.com/samishal1998/apptrail/blob/2a3ac36bed33aca03645d45f5612ad0e02c384ed/provider_traefik.go
[routes]: https://github.com/samishal1998/apptrail/blob/2a3ac36bed33aca03645d45f5612ad0e02c384ed/proxy_routes.go
[registry]: https://github.com/samishal1998/apptrail/blob/2a3ac36bed33aca03645d45f5612ad0e02c384ed/registry.go
[auth]: https://github.com/samishal1998/apptrail/blob/2a3ac36bed33aca03645d45f5612ad0e02c384ed/auth.go
[enrich]: https://github.com/samishal1998/apptrail/blob/2a3ac36bed33aca03645d45f5612ad0e02c384ed/enrich.go
[service]: https://github.com/samishal1998/apptrail/blob/2a3ac36bed33aca03645d45f5612ad0e02c384ed/service.go
[linux]: https://github.com/samishal1998/apptrail/blob/2a3ac36bed33aca03645d45f5612ad0e02c384ed/service_linux.go
[darwin]: https://github.com/samishal1998/apptrail/blob/2a3ac36bed33aca03645d45f5612ad0e02c384ed/service_darwin.go
[windows]: https://github.com/samishal1998/apptrail/blob/2a3ac36bed33aca03645d45f5612ad0e02c384ed/service_windows.go
[tests]: https://github.com/samishal1998/apptrail/blob/2a3ac36bed33aca03645d45f5612ad0e02c384ed/apptrail_test.go
[proxytests]: https://github.com/samishal1998/apptrail/blob/2a3ac36bed33aca03645d45f5612ad0e02c384ed/proxy_providers_test.go
[aliastests]: https://github.com/samishal1998/apptrail/blob/2a3ac36bed33aca03645d45f5612ad0e02c384ed/alias_merge_test.go
[servicetests]: https://github.com/samishal1998/apptrail/blob/2a3ac36bed33aca03645d45f5612ad0e02c384ed/service_test.go
[checks]: https://github.com/samishal1998/apptrail/blob/2a3ac36bed33aca03645d45f5612ad0e02c384ed/.github/workflows/checks.yml
[release]: https://github.com/samishal1998/apptrail/blob/2a3ac36bed33aca03645d45f5612ad0e02c384ed/.github/workflows/release.yml
[install]: https://github.com/samishal1998/apptrail/blob/2a3ac36bed33aca03645d45f5612ad0e02c384ed/install.sh
[smoke]: https://github.com/samishal1998/apptrail/blob/2a3ac36bed33aca03645d45f5612ad0e02c384ed/scripts/service-smoke.cjs
[ci-run]: https://github.com/samishal1998/apptrail/actions/runs/35741675602
[docker-test]: https://github.com/samishal1998/apptrail/blob/2a3ac36bed33aca03645d45f5612ad0e02c384ed/apptrail_test.go#L131-L199
[caddy-test]: https://github.com/samishal1998/apptrail/blob/2a3ac36bed33aca03645d45f5612ad0e02c384ed/proxy_providers_test.go#L71-L177
[traefik-test]: https://github.com/samishal1998/apptrail/blob/2a3ac36bed33aca03645d45f5612ad0e02c384ed/proxy_providers_test.go#L179-L244
[alias-test]: https://github.com/samishal1998/apptrail/blob/2a3ac36bed33aca03645d45f5612ad0e02c384ed/alias_merge_test.go#L11-L119
[constraint-test]: https://github.com/samishal1998/apptrail/blob/2a3ac36bed33aca03645d45f5612ad0e02c384ed/proxy_providers_test.go#L246-L284
[cdp-readme]: https://github.com/lucaslorentz/caddy-docker-proxy/blob/2434d713b4906d12f3379ecb8a584e0ef97e6039/README.md
[cdp-generator]: https://github.com/lucaslorentz/caddy-docker-proxy/blob/2434d713b4906d12f3379ecb8a584e0ef97e6039/generator/generator.go#L72-L84
[cdp-loader]: https://github.com/lucaslorentz/caddy-docker-proxy/blob/2434d713b4906d12f3379ecb8a584e0ef97e6039/loader.go#L234-L275
