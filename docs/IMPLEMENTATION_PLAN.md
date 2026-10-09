# Steward implementation plan

**Current execution plan, 2026-10-09.** Repository decision accepted; implementation
and compatibility evidence must be recorded as work lands. Local workspace:
`/home/samimishal/projects/rust/steward`; repository:
https://github.com/samishal1998/steward. This plan supersedes the archived plan's
open placement and speculative four-crate layout, preserving its full scope.

Read [repository decision](decisions/repository-placement.md),
[technical transfer](APPTRAIL_TECHNICAL_TRANSFER.md), [capabilities](CAPABILITIES.md),
[progress](PROGRESS.md), then the [archived PRD](planning/01_PRD.md),
[architecture](planning/02_ARCHITECTURE.md), [acceptance plan](planning/05_TESTING_AND_ACCEPTANCE.md)
and [effective-discovery addendum](planning/08_DISCOVERY_ADDENDUM.md).

## 1. Fixed product boundaries

* One lightweight Rust CLI/TUI; one Cargo package with internal modules to start.
  No shared AppTrail production code, DB, web build, credentials or runtime.
* Route and service capabilities are independently useful. No mandatory Steward
  daemon or homegrown process supervisor. Native managers supervise services.
* Route management first: Caddy plain file-backed, baseline Caddy Docker
  controller, Traefik watched files. Both engines stay in proxy-release scope.
  General native Linux/macOS services, composition and optional AppTrail follow.
* Initial writable route: exact hostname, one explicit HTTP(S) upstream origin,
  selected binding/listener, desired enabled state and explicit TLS policy;
  preserve request path/query. Reject credentials, fragments and implicit path
  rewriting. Complex external rules stay visible but are not editable.
* Effective authorized API inspection preferred; files/known labels are qualified
  fallback or provenance. Docker labels read-only; known Traefik and baseline
  Caddy Docker label readers remain planned. Custom label mappings deferred.
* Explicit setup for provider enablement, install, privileged edits, mounts,
  listener/network changes or restart/recreation. No discovery-triggered setup.
* Exact-host conflicts plus conservative wildcard/catch-all/complex overlap
  checks; partial inventory never proves absence of a conflict. No Apache.

## 2. First runnable vertical slice: inventory and route planning

**Initial Caddy/Traefik portion implemented; read-only.** Actual commands and schemas
are in [CLI_CONTRACT.md](CLI_CONTRACT.md). Synthetic fixture/mock/CLI tests are
recorded in progress. Docker/known-label/file evidence, native binding detection,
and the live compatibility spikes remain open. This is a partial milestone,
not the complete product; no adapter mutation is enabled.

| Work package | Concrete implementation | Acceptance / evidence |
|---|---|---|
| CLI/config | Version/help, YAML/JSON schema v1, explicit scope/endpoints/binding IDs; duplicate/unknown field and unsupported-version rejection | Document actual `--help`, parse failures and schema compatibility policy |
| Domain facts | Separate specs, observations and bindings; engine/controller/runtime/network axes; native identity, timestamp, source, structured coverage and diagnostics | Same hostname on two bindings stays two resources; labels cannot establish ownership |
| Transport | Bounded HTTP(S)/Unix GETs; auth-file refs and redaction; no redirect/implicit context changes | Auth rotation/error, body/header/time bounds; unauthorized API not empty inventory |
| Caddy inventory | Current `/config/` native JSON with raw references/unsupported coverage; no inferred controller authority | Literal route plus unknown matcher/handler yields visibly partial inventory |
| Traefik inventory | Router/entrypoint/service diagnostic arrays, native qualified IDs/status and unresolved rules | Pagination failure, disabled/internal status, aliases without dashboard identity collapse |
| Docker evidence | Explicit endpoint container/known-label reads only; API inspection wins observed state over label intent | Label/runtime disagreement preserved; inaccessible Docker does not block usable proxy-only inventory |
| Binding inspection | `proxy list`, route list/inspect, `doctor`; explicit selection of candidate instance | Multiple proxies, stale bindings, inaccessible source and topology ambiguity produce actionable diagnostics |
| Planner | `plan` reports exact-host route diff, owned/external/unknown resources, prerequisites, collisions, source fingerprints and unsupported apply | Deterministic equivalent-input plan; no setup/writes/reload; unresolved overlap rejects mutation eligibility |

Implementation boundaries: `cli`, `config`, `domain`, `transport`, `discovery`
(Caddy/Traefik/Docker), and `plan` internal modules; add real abstractions only
where actual consumers need them. Fixture filenames and upstream contracts are
in [transfer §10](APPTRAIL_TECHNICAL_TRANSFER.md#10-fixture-and-test-transfer).
Reauthor synthetic fixtures; review AppTrail rights before vendoring source.

### Runnable offline proof

From `/home/samimishal/projects/rust/steward`, the implemented fixture workflow is:

```sh
cargo run -- --help
cargo run -- --version
# These profiles intentionally read synthetic snapshots, not live proxies.
cargo run -- --config examples/read-only.yaml proxy list --json
cargo run -- --config examples/read-only.yaml route list --json
cargo run -- --config examples/read-only.yaml doctor --json
cargo run -- --config examples/read-only.yaml plan --file examples/routes-only.yaml --json
cargo test --locked -j 1 -- --test-threads=1
cargo fmt --check
cargo clippy --locked -j 1 --all-targets -- -D warnings
```

The example/fixture files are independently authored and covered by actual tests,
not copied from historical examples as though they were implemented syntax. The
tests prove native inventory/freshness/coverage, conservative collisions, no false
ownership and apply-unavailable behavior. Offline observations are runtime-unconfirmed.
The archive's `route add --yes` commands are not enabled. See `PROGRESS.md` for the
execution record; the original documentation-only handoff did not execute tests.

### Implemented command corrections

The binding profile is selected with `--config examples/read-only.yaml`. Route
intent is separate: `--config examples/read-only.yaml plan --file examples/routes-only.yaml --json`.
`proxy list`, `proxy inspect ID`, `route list`, `route inspect --proxy ID NATIVE_ID`,
and `doctor` are available. No-argument invocation prints help; the TUI remains
planned. `apply` and `service list` explicitly reject before proxy/config initialization.
The actual fixture files above now exist. They are synthetic and runtime-unconfirmed.

## 3. Compatibility gates before writes

Experiments use disposable pinned environments, never production as an implicit
test harness. Record version/digest, OS, topology, exact commands, synthetic
inputs, observed result, failure case and immutable source reference.

| Gate | Required experiment | Blocks |
|---|---|---|
| Plain Caddy | Identify complete startup authority/modules/imports; validate full candidate in same environment, graceful activation and guarded restoration; unrelated routes preserved | Plain Caddy apply |
| Caddy Docker controller | Pin v2.13.1/source `2434d713…`; managed base-content change without Docker event, timing, labels preserved, later events/restart, invalid content and rollback; verify controller/server mode | Caddy Docker apply/release |
| Traefik files | Pin engine/rule syntax; inspect startup file/CLI/env source and directory vs filename mode; parent-dir bind mount; atomic replacement and final-route removal activate; provider errors/coexistence | Traefik apply/release |
| Network/TLS | Probe from chosen proxy network context; Linux/Desktop/remote distinctions, explicit host or shared-network recipe; upstream HTTPS verification and route-specific response | Affected route apply |
| Native bootstrap | Target systemd/launchd scope, argv/env/path and registration/lifecycle; preserve existing ownership and permission boundaries | Proxy install/control |
| Artifact provenance | Available architecture-specific engine/module builds, integrity/authenticity policy, notices and extraction safety | Installer/release |

Upstream source verifies that the Caddy Docker base is read during generation
and timer regeneration compares generated bytes; **live activation remains
unverified**. Do not solve an imported-file refresh gap by overwriting generated
runtime output, inventing Docker events or recreating application containers.
Do not invent a Traefik universal validation command. Material compatibility
failure requires an evidence-backed design/scope decision, not silent removal
of a promised mode.

## 4. Journaled mutation foundation

Before a production adapter can write, implement and verify:

1. Stable operation ID, scope-specific lock, durable append/state transitions,
   restricted backups and receipts; distinguish prepared/written/activation
   uncertain/verified/compensated/drift-blocked states.
2. Ownership receipt + exact native ID/path + fingerprint; external resources
   read-only unless a separate explicit setup/adoption plan authorizes input edits.
3. Recheck plan version and all relevant input digests immediately before apply;
   `--yes` cannot override stale plan or unresolved overlap.
4. Safe staging/atomic publication, regular-file/symlink/race protections,
   permissions, same-filesystem rules and crash-durable storage behavior.
5. Executable+argv bounded executor, timeout/cancellation, redacted bounded logs,
   explicit privileges and native status; no AppTrail `CombinedOutput` reuse.
6. Recovery/compensation only for owned or explicitly authorized inputs while
   current bytes still match the operation's published fingerprint. Drift blocks
   restoration rather than overwriting a newer manual edit.
7. Separate source publication, controller acceptance, native route presence,
   upstream health, HTTP routing, DNS and TLS trust. A file or generic HTTP 200
   alone is not apply success.

Meaningful tests inject failure/crash at each boundary, two competing writers,
manual drift and unsupported rule overlap. Fake adapters prove the operation
engine, not live proxy compatibility. Partial inventory can permit unrelated
service work but cannot bypass a missing route-conflict proof.

## 5. Full dependency-ordered roadmap

| Milestone | Deliverable / completion gate | Current evidence status |
|---|---|---|
| M0 | Approved placement, completed transfer and current docs | Completed documentation; audited baseline and prior CI qualified |
| M1–M3 first slice | Contracts, read-only inventory/bindings/route planning and synthetic fixture proof | Initial Caddy/Traefik CLI implemented; Docker/file evidence and automatic candidate detection pending |
| M1 compatibility | Complete per-mode experiments above | Open live gates |
| M2 operation engine | Journal, locks, receipts, fingerprints, bounded executor and drift-safe recovery | Proposed; required before writes |
| M4 plain Caddy | Owned aggregate import, full validation/activation and add/update/enable/disable/remove; external/global options/modules preserved | Planned, gate-dependent |
| M4 Caddy Docker | Approved managed base contribution; no competing effective writer; later events/restart/recovery verified | Planned, gate-dependent |
| M5 Traefik | Stable owned aggregate YAML names/references, watched directory publication, native verification and input compensation; Docker and manual routes preserved | Planned, gate-dependent |
| M6 explicit bootstrap | Existing-install integration and clean-host Caddy/Traefik setup; scope/listeners/TLS/storage/artifact choices; install/upgrade/detach/uninstall preserve user data/certificates | Planned; no automatic Docker install/relabeling or package-manager takeover |
| M7 proxy release + TUI | Routes/Proxies/Operations views, source-aware actions, preview/recovery; supported-version matrix, live failure tests, standalone install/uninstall | Planned; all three initial proxy modes required |
| M8 general native services | Linux systemd/macOS launchd list/inspect/add/edit/start/stop/restart/enable/disable/remove and bounded logs; real scope/logout/reboot tests | Planned; service-only works without proxy/Docker/AppTrail |
| M9 composition | Explicit service endpoint dependencies; service-first readiness-gated up, route-first down; retain-service default, owned-new-resource cleanup policy, guarded cascade | Planned; preexisting resources never deleted by compensation |
| M10 optional AppTrail | Stage 0 existing discovery; later versioned export/new importer and authorized control proposal | Stage 0 code boundary verified, live interaction unknown; later work proposed |
| Later adapters/platforms | Nginx, other native/platform adapters with explicit support evidence | Deferred; Apache excluded, custom mappings deferred |

Native bootstrap code may arrive for proxy setup before M8; it must use native
managers and remain distinct from completed general-service support. No background
Steward process is required. TUI calls core use cases rather than shelling out
to the CLI. Independent routes-only and services-only views/workflows remain.

## 6. Release and verification policy

Dedicated `samishal1998/steward`, `vX.Y.Z`, `steward_vX.Y.Z_<os>_<arch>.tar.gz`
assets plus checksums/product-version checks; Rust-only release CI independent
of AppTrail Go/web jobs. Test resolver isolation from newer AppTrail releases
and rejection of wrong product/version/architecture/checksum. Planned targets:
Linux x86_64/arm64, macOS arm64/x86_64. Builds alone do not establish runtime
support; publish exact exercised combinations and non-skipped logs. Windows later.

No license selected. Review dependency/artifact notices and AppTrail source/fixture
copy permissions before distribution. Maintain capability matrix and receipts/runbooks.

Checks as code lands: targeted Rust tests for actual behavior, `cargo fmt --check`,
`cargo clippy --all-targets -- -D warnings`, then the relevant live/OS/install gate.
See the [archived testing plan](planning/05_TESTING_AND_ACCEPTANCE.md) and addendum
acceptance cases for preservation, security, recovery and independent workflows.
Steward checks now have their own local execution record. Baseline AppTrail
Checks success must not be reused as Steward or specific native-manager proof.

## 7. Recording progress and discrepancies

For each completed implementation task record command, result, commit/version,
topology and next gate in [PROGRESS.md](PROGRESS.md); update
[CAPABILITIES.md](CAPABILITIES.md) only to the demonstrated evidence level.
Keep ownership/write eligibility separate from read capability. Unknowns and
their owners are in [transfer §13](APPTRAIL_TECHNICAL_TRANSFER.md#13-unknowns-and-required-verification).
If evidence contradicts a product assumption, report exact behavior, impact and
smallest options; continue independent work without baking in the discrepancy.
