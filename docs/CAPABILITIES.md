# Steward capabilities and evidence

**Updated: 2026-10-09.** The first read-only Rust slice is implemented and tested
on the development Linux x86_64 host. It is not the proxy-management release.
Consult [actual CLI contracts](CLI_CONTRACT.md) and [execution evidence](PROGRESS.md).

Statuses: **verified from code** = AppTrail baseline/static source; **prior CI** =
existing baseline run; **verified upstream source** = pinned controller source;
**proposed/planned** = Steward work; **unknown** = live/support evidence absent.
**Verified by test** means the current Steward synthetic/mock or CLI tests ran
as recorded in progress; it does not imply live proxy/native-manager compatibility.
See [transfer](APPTRAIL_TECHNICAL_TRANSFER.md),
[plan](IMPLEMENTATION_PLAN.md) and [progress](PROGRESS.md).

| Capability | Evidence available | Steward status / support gate |
|---|---|---|
| Standalone Rust repository | Approved decision; local workspace and public repo supplied | Accepted; one Cargo package/internal modules; no shared production code |
| Read-only inventory and route planning | Rust model/config/readers/planner and CLI tests | Implemented; deterministic non-applicable previews, versioned JSON errors and partial results |
| Caddy current JSON inspection | Steward `/config/` synthetic and mock transport tests | Implemented with native references and partial matcher/handler diagnostics; no live engine certification |
| Plain Caddy owned-file route writes | Official input/API docs, no AppTrail write adapter | Unknown live compatibility; full authority/validation/recovery gates before apply |
| Caddy Docker controller managed-base writes | v2.13.1, commit `2434d713b4906d12f3379ecb8a584e0ef97e6039`: base read + timer/generated-byte comparison verified upstream source | Live file-only activation, events/restart and external-route preservation unverified; release gate |
| Unfamiliar Caddy Docker plugin inspection | Effective output may use understood native structures | Conditional planned read coverage only; no management certification |
| Traefik current API inspection | Steward three-endpoint mock tests and native snapshots | Implemented; native status/qualified identities, aliases not merged; no `/api/rawdata` or engine certification |
| Traefik watched-file writes | Official directory/watch model; no AppTrail writer | Unknown pinned-version/mount activation; atomic replace/final removal/coexistence gates |
| Docker explicit endpoint/known labels | AppTrail v1.44 container list and Traefik labels verified from code | Planned read-only; no context negotiation or write authority implied |
| Baseline Caddy Docker labels | Not implemented in AppTrail | Planned built-in read-only reader; not removed from product scope |
| Custom label mappings | Accepted discovery addendum | Deferred; no mapping language/editor/plugin framework in first release |
| Exact-host one-upstream routes | Route-only manifest validation and conservative conflict tests | Read-only preview implemented; all writes remain unavailable |
| Ownership, journal, crash recovery | Required design; AppTrail marker-only lifecycle insufficient | Planned mandatory foundation before production writes |
| Explicit install/setup/upgrade/uninstall | Required product scope | Planned; no automatic discovery-triggered setup; preserve certificates/user data |
| Linux general native services | AppTrail fixed self-service code exists, not general service API | Planned later systemd user/system support; live scope/lifecycle tests required |
| macOS general native services | Fixed AppTrail LaunchAgent code and skip-capable smoke script | Planned later launchd support; native logs unavailable in audit, no execution claim |
| Independent service-only operation | CLI test invokes unavailable service path with deliberately invalid config | Placeholder initializes no proxy/Docker; general service capability remains planned |
| CLI/TUI and composition | CLI input/result implementation plus full roadmap | Read-only CLI implemented; TUI, services and composition planned |
| AppTrail Stage 0 discovery | Existing Caddy/Traefik read boundaries verified from code | Requires no new provider in principle; live resulting-route identity test pending |
| AppTrail versioned export/import/control | No file-inventory provider or service-control endpoint at baseline | Later new code/contracts; no shared DB/credentials |
| Nginx/additional native/platform adapters | Later product scope | Deferred; Apache excluded; Windows later |

## Version/platform claims

| Component | Known evidence | Qualification |
|---|---|---|
| AppTrail | `main` `2a3ac36bed33aca03645d45f5612ad0e02c384ed`; Go 1.26.8 | Transfer baseline only, not a Steward runtime dependency |
| AppTrail CI | Checks run [35741675602](https://github.com/samishal1998/apptrail/actions/runs/35741675602) successful | Prior CI; macOS native logs unavailable; green skip-capable job does not prove lifecycle execution |
| Docker | API v1.44 request hardcoded in baseline | Engine 25+ source intent; no new live daemon test/older negotiation proof |
| Caddy | Partial Caddy v2-shaped JSON source/fixtures | Exact supported engine/modules/install modes not certified |
| caddy-docker-proxy | v2.13.1 pinned upstream source | Source inspection, not live activation compatibility |
| Traefik | Diagnostic HTTP arrays + literal rule fixtures | Exact engine version/rule syntax support still to pin and exercise |
| Steward Linux x86_64 | Rust 1.98.0 local tests/build | Read-only fixture/mock execution verified; live proxy/service compatibility pending |
| Steward Linux arm64 | Planned release target | Build and native/topology execution gates pending |
| Steward macOS CI host | Rust 1.98 fixture/mock/CLI tests, formatting and Clippy passed in run 37978578690 | Read-only slice verified on hosted macOS; live proxy and native-manager gates pending |
| Steward macOS arm64/x86_64 distribution | Planned release targets | Installer and complete on-target proxy/service acceptance still pending |

## Required status distinctions

Inventory must expose source, freshness, coverage and native status. Effective API
observations take precedence over inferred label intent, while discrepancies
remain visible. Discovery does not grant ownership or control. Writes additionally
require known configuration authority, receipt/fingerprint, sufficient conflict
coverage, tested activation and journaled recovery. Report publication, controller
acceptance, native activation, health, DNS and TLS readiness separately.

Promotion to supported requires exact command/result, version/digest, OS/runtime/
network topology and failure/recovery evidence in `PROGRESS.md` or a linked spike
report. Do not advertise a platform from cross-compilation alone.
