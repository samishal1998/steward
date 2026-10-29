# Steward capabilities and evidence

## Current management release — v0.2.0-alpha.3

Version `0.2.0-alpha.3` packages the previously verified management expansion, including
checked apply, native TLS/file/controller modes, proxy/native-service lifecycle,
targets, composition, history/rollback, adoption, dynamic completion, TUI and a
versioned AppTrail reader/export pair. See [feature coverage and actual execution
evidence](FEATURE_COMPLETION.md). Linux/macOS native user-scope checks passed;
public ACME/DNS and logout/reboot are not inferred from local lifecycle tests.
Nginx, Windows for Steward, and custom label mappings remain deferred by the user.

## Released alpha.2 baseline

**Updated: 2026-10-10.** The direct-command HTTP slice is implemented and exercised
with disposable local Caddy 2.11.7 and Traefik 3.7.14 on Linux x86_64. This is an
alpha extension, not completion of every proxy-management mode.
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
| Plain Caddy JSON route writes | Live add/update/disable/enable/remove, full candidate validation, root ETag concurrency, external preservation, compensation and interruption tests | Initial same-host HTTP JSON mode implemented; Caddyfile/import/TLS/controller modes retain their gates |
| Caddy Docker controller managed-base writes | v2.13.1, commit `2434d713b4906d12f3379ecb8a584e0ef97e6039`: base read + timer/generated-byte comparison verified upstream source | Live file-only activation, events/restart and external-route preservation unverified; release gate |
| Unfamiliar Caddy Docker plugin inspection | Effective output may use understood native structures | Conditional planned read coverage only; no management certification |
| Traefik current API inspection | Steward three-endpoint mock tests and native snapshots | Implemented; native status/qualified identities, aliases not merged; no `/api/rawdata` or engine certification |
| Traefik watched-directory writes | Live 3.7.14 native activation, atomic aggregate replacement/final deletion, external-file preservation, failed activation and interrupted recovery | Initial same-host HTTP mode implemented; container mount/restart/TLS variants retain their gates |
| Docker explicit endpoint/known labels | AppTrail v1.44 container list and Traefik labels verified from code | Planned read-only; no context negotiation or write authority implied |
| Baseline Caddy Docker labels | Not implemented in AppTrail | Planned built-in read-only reader; not removed from product scope |
| Custom label mappings | Accepted discovery addendum | Deferred; no mapping language/editor/plugin framework in first release |
| Exact-host one-upstream routes | Manifest validation, direct command lifecycle and conservative conflict tests | Direct local HTTP writes plus read-only manifest preview implemented |
| Ownership, journal, crash recovery | Kernel state/input locks, private durable journal, template receipts, source/native checks and live interrupted recovery | Initial bounded single-operation path implemented; historical journal, broader scopes and manifest apply remain later work |
| Explicit install/setup/upgrade/uninstall | Required product scope | Planned; no automatic discovery-triggered setup; preserve certificates/user data |
| Linux general native services | AppTrail fixed self-service code exists, not general service API | Planned later systemd user/system support; live scope/lifecycle tests required |
| macOS general native services | Fixed AppTrail LaunchAgent code and skip-capable smoke script | Planned later launchd support; native logs unavailable in audit, no execution claim |
| Independent service-only operation | CLI test invokes unavailable service path with deliberately invalid config | Placeholder initializes no proxy/Docker; general service capability remains planned |
| CLI/TUI and composition | Direct route/proxy/operation commands, stored API connections and shell completion | CLI slice implemented; TUI, services and composition planned |
| Agent guidance and branding | Public source-imported llms.txt/full guide; user selected Waymark as the identity | Selected mark promoted to site/header/favicon/brand guide; unused concepts and picker removed |
| AppTrail Stage 0 discovery | Existing Caddy/Traefik read boundaries verified from code | Requires no new provider in principle; live resulting-route identity test pending |
| AppTrail versioned export/import/control | No file-inventory provider or service-control endpoint at baseline | Later new code/contracts; no shared DB/credentials |
| Nginx/additional native/platform adapters | Later product scope | Deferred; Apache excluded; Windows later |

## Version/platform claims

| Component | Known evidence | Qualification |
|---|---|---|
| AppTrail | `main` `2a3ac36bed33aca03645d45f5612ad0e02c384ed`; Go 1.26.8 | Transfer baseline only, not a Steward runtime dependency |
| AppTrail CI | Checks run [35741675602](https://github.com/samishal1998/apptrail/actions/runs/35741675602) successful | Prior CI; macOS native logs unavailable; green skip-capable job does not prove lifecycle execution |
| Docker | API v1.44 request hardcoded in baseline | Engine 25+ source intent; no new live daemon test/older negotiation proof |
| Caddy | Live native 2.11.7 local JSON/HTTP tests; installed 2.6.2 lacked the required ETag | Conditional config API required; not a universal module/controller/TLS certification |
| caddy-docker-proxy | v2.13.1 pinned upstream source | Source inspection, not live activation compatibility |
| Traefik | Live native 3.7.14 watched directory and literal Host rule tests | Local HTTP native topology exercised; container mapping/TLS acceptance pending |
| Steward Linux x86_64 | Rust 1.98.0 local tests/build and real local proxy lifecycle | Initial local HTTP write/recovery slice exercised; general services remain pending |
| Steward Linux arm64 | Alpha.2 native build/tests/executable/archive checks passed in run 38014774774 | Distribution checks are separate from exact live routing topology proof |
| Steward macOS ARM64 CI host | Run 38014422913 passed real Caddy 2.11.7/Traefik 3.7.14 local HTTP lifecycle/recovery and Rust/installer checks | Initial local HTTP write slice exercised; broader modes and native-service lifecycle remain pending |
| Steward macOS arm64/x86_64 distribution | Alpha.2 native build/tests/executable/archive checks passed in run 38014774774 | Distribution/installer checks do not certify every proxy or native-service topology |

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
