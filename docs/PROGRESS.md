# Steward progress

**Updated: 2026-10-09 — read-only foundation implemented.** Workspace:
`/home/samimishal/projects/rust/steward`; public repository:
https://github.com/samishal1998/steward.

## Completed handoff

| Task | Status | Evidence |
|---|---|---|
| AppTrail audit/repository placement | Accepted; user approved standalone Rust repository | [Decision](decisions/repository-placement.md); AppTrail baseline `main` `2a3ac36bed33aca03645d45f5612ad0e02c384ed` |
| Source-backed standalone transfer | Documentation completed | [Transfer](APPTRAIL_TECHNICAL_TRANSFER.md): actual transport/providers/contracts/auth/lifecycle/release boundaries, immutable links, synthetic fixture map and future commands |
| Updated actionable execution plan | Documentation completed | [Plan](IMPLEMENTATION_PLAN.md): selected local path, one-package refinement, read-only first slice, journal and compatibility gates, full roadmap |
| Capability/evidence matrix | Documentation completed | [Capabilities](CAPABILITIES.md): no inferred write/platform support |
| Historical source pack | Archival copy tracked in [index](PLANNING_ARCHIVE.md) | Original wording, filenames and relative-link layout retained; current completed docs linked outside archive |

The documentation subtask performed source inspection and authored the handoff;
it did not run tests. The implementation work below adds independently authored
Rust code and fixtures. No live proxy/service configuration or Apptrail data was changed.

## Implemented foundation

- One Cargo package with internal config, model, transport, inventory, rule,
  plan, CLI and diagnostic modules. No Apptrail/Go/web runtime dependency.
- Versioned YAML/JSON inputs and output, duplicate/unknown/type validation,
  explicit bindings/scope, bounded HTTP(S)/Unix reads and authorization-file refs.
- Caddy nested-handler/native-reference inventory and Traefik native router,
  entrypoint/service inventory, including disabled/error/internal records.
  Unknown behavior is partial; aliases retain native route identity.
- Offline response fixtures with explicit non-runtime qualification; deterministic
  route-only previews with fingerprints, conservative conflicts and unavailable apply.
- Synthetic tests cover base-path/auth transport, limits/truncation/denial/redirects,
  raw-secret nonserialization, scope/version/duplicate checks, and CLI independence.
  Linux Unix-socket and auth-rotation tests use disposable local servers/files.
- CLI and input schemas: [CLI_CONTRACT.md](CLI_CONTRACT.md), `schemas/`,
  actual profiles and manifest under `examples/`.
- No ownership store/journal, writes, fallback Docker/file inventory, host process
  detection, proxy installs, TUI, general service control or composition yet.

### Local execution record

Host: Linux x86_64; compiler/Cargo 1.98.0. Project dependencies were resolved to
`Cargo.lock`; build concurrency one. Inputs were freshly written synthetic fixtures
and mock HTTP/Unix servers, not production exports or copied Apptrail source.

| Command | Result / evidence boundary |
| --- | --- |
| `cargo check -j 1` | Passed; first dependency resolution/build |
| `cargo test --locked -j 1 -- --test-threads=1` | Passed final local suite: 13 unit/mock/Unix tests and 6 CLI integration tests; 19 total, no skips |
| `cargo clippy --locked -j 1 --all-targets -- -D warnings` | Passed after fixing compiler/lint findings |
| `cargo fmt --check` | Passed |
| `cargo run --locked -j 1 -- --config examples/read-only.yaml plan --file examples/routes-only.yaml --json` | Passed: one `planned_read_only` envelope, `changed: false`, stable source digests, offline/setup/apply diagnostics and `applicable: false`; no live connections |

CI runs the same fixture/mock checks on Linux/macOS and checks the declared
Rust 1.89 MSRV separately. Native supervisor, reboot/logout, proxy reload/watch,
file-only controller refresh, TLS/proxy-context network and release install tests
remain unexecuted. A green CI build is not that live acceptance evidence.

Initial remote run [37977871936](https://github.com/samishal1998/steward/actions/runs/37977871936)
passed Linux and the Rust 1.89 MSRV check, but found a macOS mock-server bug:
accepted sockets could inherit nonblocking mode and truncate the large response.
The fix explicitly uses bounded blocking accepted sockets in the harness and adds
an early Content-Length size rejection to the client. The oversized-body test also
exercises a response without a declared length, retaining the streaming body cap.
The failing native job was not relabeled as passing; a new run verifies the fix.

Remote verification of code commit `6ca80fb661b8a74b4a374cdfe51e777f49751b20`:
[Rust checks 37978578690](https://github.com/samishal1998/steward/actions/runs/37978578690)
**passed** Linux/macOS formatting, all 18 fixture/mock/CLI tests, and Clippy with
warnings denied. The separate Rust 1.89 `cargo check --locked` job passed.
This verifies the read-only foundation on those CI environments; no live proxy
activation, native service lifecycle, or boot/logout capability is implied.

The final reader refinement bounds retained evidence expansion per binding and
per command, rejects YAML anchors/aliases, and adds a large shared-service
regression. This addresses intermediate-state growth beyond the input/body caps;
its own source commit and checks follow the already passing foundation run.

## Evidence already available

* **Verified from code:** AppTrail Go/package-main/SQLite/web architecture;
  bounded GET transport; actual Docker/Caddy/Traefik read paths; observation and
  global URL/display identity; fixed AppTrail self-service lifecycle; product-specific
  installer/release. Source references in the transfer.
* **Prior CI evidence:** AppTrail [Checks run 35741675602](https://github.com/samishal1998/apptrail/actions/runs/35741675602)
  successful baseline. macOS native logs were unavailable in the audit; the
  skip-capable smoke script means specific native execution is unproven.
* **Verified upstream source:** caddy-docker-proxy v2.13.1 at
  `2434d713b4906d12f3379ecb8a584e0ef97e6039` reads base during generation and
  regenerates on timer/compares generated bytes. Live activation unverified.
* **Historical proposal:** archived original PRD/architecture/examples/templates.
  Original undecided/template status is preserved intentionally; completed
  external docs are current.

## Active and next work

| Work | Status | Owner / next action |
|---|---|---|
| Standalone Cargo/CLI modules | Implemented and locally checked | Maintainer: keep CLI/schema/README aligned and verify CI/MSRV |
| Read-only inventory and route planning | Implemented initial Caddy/Traefik slice | Next: Docker/file qualified evidence and native binding detection; writes stay gated |
| Synthetic fixture transfer | Independently authored in `fixtures/`, `src/tests.rs`, `tests/cli.rs` | No Apptrail production code/test files vendored |
| Mutation foundation | Planned before adapter writes | Operation-engine owner: locks/journal/receipts/fingerprints/bounded executor and failure/recovery proof |
| Plain Caddy compatibility | Unknown live behavior | Proxy owner: full authority/modules/import validation, activation/recovery and preservation spike |
| Caddy Docker refresh | Release-blocking unknown | Proxy owner: pinned base-only edit without Docker event; later events/restart/invalid content/recovery |
| Traefik watched files | Release-blocking unknown | Proxy owner: pinned startup authority/mounts; atomic replace/final removal/provider diagnostics/coexistence |
| Native/container networking and TLS | Unknown per topology | Networking owner: proxy-context reachability/verification; do not assume local loopback on remote daemon |
| Native Linux/macOS services | Full later milestone, not dropped | Native owner: on-target scope/lifecycle/log/KeepAlive/logout/reboot tests with non-skipped evidence |
| Setup/install/TUI/composition | Full roadmap planned | Implementers: advance after dependencies; no required daemon/process supervisor |
| License/attribution and release assets | Open | Maintainer: AppTrail has no root LICENSE; inspect copying rights and select Steward license only with user decision |
| Optional AppTrail integration | Stage 0 boundary known, live result unknown; later contracts proposed | Integration owner: use existing proxy discovery first, then explicit new versioned importer if needed |

## Required completion record for implementation work

Append a short entry with: date; task/commit; actual command and exit/result;
OS/engine/controller versions and topology; fixture or live coverage; failure/
recovery evidence; remaining gate; capability change. Mark skipped/unavailable
execution explicitly. Pending commands in the plan/transfer are instructions,
not passing results. The first read-only slice does not close route writes,
setup, TUI, native services or composition milestones.
