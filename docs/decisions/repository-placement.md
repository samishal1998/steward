# Repository placement: standalone Steward

**Status: accepted; user approved. Date: 2026-10-09.**

Steward lives in **`/home/samimishal/projects/rust/steward`**, published at
**https://github.com/samishal1998/steward**. It is a separate Rust product with
**no shared production code** with AppTrail. Transfer knowledge and synthetic
upstream fixtures; do not extract a cross-language provider framework.

## Evidence baseline

AppTrail baseline: `main`, commit
[`2a3ac36bed33aca03645d45f5612ad0e02c384ed`](https://github.com/samishal1998/apptrail/tree/2a3ac36bed33aca03645d45f5612ad0e02c384ed).
Local source: `/home/samimishal/projects/rust/app-trail`. Audit findings refer to
this commit, not concurrent or uncommitted changes. The supplied full-audit
decision is supplemented by local source inspection during this documentation
handoff. No tests, builds, installations, or live experiments were run here.

**Verified from code:** Go 1.26.8, package `main`, standard-library HTTP server,
SQLite (`modernc.org/sqlite`), React 19/TypeScript/Vite frontend embedded from
`web/dist`. Evidence: [go.mod](https://github.com/samishal1998/apptrail/blob/2a3ac36bed33aca03645d45f5612ad0e02c384ed/go.mod),
[main.go](https://github.com/samishal1998/apptrail/blob/2a3ac36bed33aca03645d45f5612ad0e02c384ed/main.go),
[providers.go](https://github.com/samishal1998/apptrail/blob/2a3ac36bed33aca03645d45f5612ad0e02c384ed/providers.go),
[registry.go](https://github.com/samishal1998/apptrail/blob/2a3ac36bed33aca03645d45f5612ad0e02c384ed/registry.go).

**Prior CI evidence:** [Checks run 35741675602](https://github.com/samishal1998/apptrail/actions/runs/35741675602)
reported successful for the baseline. macOS native logs were unavailable in the
audit; a green skip-capable job does not establish execution of a specific
native lifecycle test. **Unknown:** live Steward/proxy compatibility and native
runtime results. These do not block repository placement; they block support
claims and the corresponding writes.

## Actual AppTrail flow

```text
owner HTTP JSON API -> SQLite provider row
  -> scheduler/manual scan -> 45s scan context
  -> newProviderClient -> bounded native GET responses
  -> dockerSnapshot / caddySnapshot / traefikSnapshot
  -> ([]observation, warning string, error)
  -> server.reconcile -> identities/observations/applications in SQLite
  -> priority-selected display facts + aliases + overrides -> dashboard
```

Native readers call helpers in package `main`; reconciliation, polling,
authentication and presentation are application-specific. Read normalization
does not preserve the configuration authority required for management.

## Concrete reuse matrix

| AppTrail candidate | Current contract / dependencies | Steward requirement and adaptation | Decision |
|---|---|---|---|
| `provider_http.go:newProviderClient/Get` | `provider` -> Go client; bounded GET JSON, absolute auth-file reference | Rust transport, instance-bound facts, explicit connection semantics; translation and new abstraction needed | Reimplement small transport; transfer limits/tests |
| `providers.go:dockerSnapshot` | Container list + labels -> dashboard observations; no ports/networks/events/context negotiation | Lossless scoped metadata and network evidence; labels never write authority | Transfer synthetic payload shapes; no production sharing |
| `provider_caddy.go:caddySnapshot` | Partial native JSON -> launch URLs, warning; ignores unknown handlers | Native routes, unresolved coverage, source identity; controller-aware owned-file writes | Reimplement reader; never turn existing read code into a write adapter |
| `provider_traefik.go:traefikSnapshot/groupTraefikAliases` | Three GET arrays -> grouped app aliases; omits internal/disabled routers | Retain native provider-qualified routers/status/constraints and collisions | Transfer parser cases; do not share dashboard grouping |
| `proxy_routes.go:traefikMatches/launchURL` | Go AST literal matcher parser and launch-link simplification | Rust parser preserving unsupported expressions; exact-host writable subset is separate | Transfer cases and bounds, not parser implementation |
| `registry.go:reconcile/normalizedURL` | Global URL identity, fact priority, SQLite transactions | Instance/resource ownership, fingerprints, dependency planning and journal | Deliberately separate; display identity is not route authority |
| `service*.go:manageService` | Install/control AppTrail itself, fixed names/arguments; marker ownership | Arbitrary native services, scopes, receipts, guarded changes and recovery | Transfer OS lessons only; new service adapters |
| Inline Go tests and release tooling | Synthetic httptest payloads, embedded web prerequisite; AppTrail-specific assets | Standalone Rust fixtures and product-specific release/install resolution | Transfer knowledge with attribution review; independent tests and distribution |

## Alternatives evaluated

| Option | Benefit | Cost in this codebase | Result |
|---|---|---|---|
| Same repo, narrowly shared production code | Joint edits to identical behavior if it existed | No proven equivalent Rust/Go library boundary; would require FFI, generic adapters or an unrelated rewrite | Rejected |
| Same repo, independent code | One checkout and adjacent fixtures | Go/web and Rust CI/release filtering; repository-wide latest-release routing remains product-sensitive | Viable, but not selected |
| Separate repo, explicit knowledge/contracts | Natural Rust boundary, independent history, CI, dependencies and releases | Maintain transfer references and fixture provenance | **Selected and approved** |

Separation does not preclude later optional integration or an evidence-backed
library extraction. Neither is needed now. There is no shared database, server,
web bundle, credential store, or mandatory daemon in Steward's dependency graph.

## Build, layout and release decision

**Accepted implementation refinement:** one Cargo package with internal modules,
not four speculative crates. Domain/config/transport/discovery/planning modules
arrive first; operation storage, adapters, native services and TUI follow as real
use cases justify them. Service-only commands initialize no proxy/Docker client.

Planned independent checks in the Steward workspace: `cargo check`, `cargo test`,
`cargo fmt --check`, `cargo clippy --all-targets -- -D warnings`. These are future
verification commands, not results. AppTrail reproduction commands, including
the `web/dist` prerequisite, are in the [technical transfer](../APPTRAIL_TECHNICAL_TRANSFER.md#10-fixture-and-test-transfer).

Steward uses its own `vX.Y.Z` tags and `steward_vX.Y.Z_<os>_<arch>.tar.gz` assets
in `samishal1998/steward`, with checksums and product/version verification.
Its release CI builds/tests Rust only. AppTrail's root `install.sh` hardcodes
`samishal1998/apptrail`, repository-wide latest and `apptrail_...` assets; it
cannot be reused as Steward's installer unchanged. Required future regression:
an unrelated newer AppTrail release cannot affect Steward version/asset selection;
wrong product, version, architecture or checksum must be rejected.

No AppTrail `LICENSE` is present in the audited root. Review rights and attribution
before copying existing source or fixtures. Independently authored synthetic
inputs may capture upstream behavior without vendoring AppTrail code. No new
license has been selected for the user.

## Plan changes and completion

| Original milestone | Selected change | Reason |
|---|---|---|
| M0 repository investigation | Complete standalone decision and source-backed transfer | Audit and user approval |
| M1–M3 bootstrap | First runnable read-only inventory/route-planning slice | Useful behavior while live activation gates remain open |
| M2 module layout | Single Cargo package/internal modules | Avoid premature crates and coupling |
| M4–M9 writes/services | Keep full scope; gate mutations on compatibility and journal/recovery proof | Discovery alone grants no control |
| Packaging | Dedicated repository/version/asset resolution | Prevent cross-product latest selection |

Accompanying artifacts: [technical transfer](../APPTRAIL_TECHNICAL_TRANSFER.md),
[actionable plan](../IMPLEMENTATION_PLAN.md), [capabilities](../CAPABILITIES.md),
[progress](../PROGRESS.md), and historical [planning archive](../planning/README.md).
The archive preserves its original undecided/template wording; these completed
documents supply the current decision. The first slice is a partial milestone,
not the whole product.

Review outcome: actual baseline and contracts identified; shared vendor APIs
not equated with reuse; no generic extraction required; route/service independence
and independent distribution preserved; next work and blocking unknowns recorded.
