# Steward progress

**Updated: 2026-10-29 — v0.2.0-alpha.2 packaging and public documentation prepared.** Workspace:
`/home/samimishal/projects/rust/steward`; public repository:
https://github.com/samishal1998/steward.

## Completed handoff

### Management release and Pages publication

The user authorized publication of `v0.2.0-alpha.1` and Steward's current docs.
The existing native release and Astro Starlight workflows are retained and updated:
archives now include offline examples/fixtures and agent guidance; extracted bundle
and installed-binary smoke checks cover the current command tree, completion,
isolated managed state, plan/apply and export. Four native release jobs exercise
real route/TLS/TUI behavior, and publication requires source CI for the tagged SHA.
The Bedouin standalone generator stays pinned and checksum-verified at 0.23.1.
Pages watches source-imported schemas as well as guides, examples and installer.

Current documentation and agent references describe the same alpha. The documented
fallback is `v0.2.0-alpha.2`; the release is published before `main`/Pages promotion
so the public selector always has downloadable assets. Actual publication evidence
is recorded after the workflow and public installation checks complete.

The first tag attempt `v0.2.0-alpha.1` at `f4cbe56c…` passed source CI and three
native release targets. Intel macOS built/executed its bundle and passed route/
Caddy TLS checks, but its LibreSSL `x509` command rejected the fixture's OpenSSL-only
`-copy_extensions` option. [Run 40935350575](https://github.com/samishal1998/steward/actions/runs/40935350575)
failed before publication; no public assets were uploaded. The user chose a fresh
`v0.2.0-alpha.2` tag rather than moving alpha.1. The fixture now supplies explicit
SAN/key-usage extensions through portable `-extfile`; trust/hostname assertions
remain required. Alpha.1 is retained as historical failed-tag evidence.

### Management expansion — 2026-10-24

The user requested all previously listed missing features and smaller CLI gaps,
then explicitly deferred Nginx, Windows for Steward and custom label mappings.
They authorized publication of feature branches in Steward and AppTrail to verify
macOS behavior. Main branches and released installer versions remain separate.

Implementation and acceptance records are maintained in
[FEATURE_COMPLETION.md](FEATURE_COMPLETION.md). Development version is
`0.2.0-dev.1`; source has a new management guide, agent instructions and versioned
read-only export schema. AppTrail's branch adds the matching file provider and
upgrade-safe provider-schema migration. Existing `steward-plan/` user work in the
AppTrail checkout is excluded from publication.

Steward [checks 40126510311](https://github.com/samishal1998/steward/actions/runs/40126510311)
and AppTrail [checks 40124701437](https://github.com/samishal1998/apptrail/actions/runs/40124701437)
passed. The feature coverage document records exact executed suites and qualified
boundaries, including the resolved macOS reverse-DNS fixture, launchd enablement
format and PTY-spawn issues. The work is published on review branches; release
alpha.2 remains the installer target until a merge/release is requested.

### Original handoff record

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

## Read-only foundation — alpha.1 history

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
file-only controller refresh, TLS/proxy-context network and native service install tests
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

### Release packaging and public documentation

The first installable distribution is `v0.1.0-alpha.1`, explicitly packaging the
read-only foundation. `.github/workflows/release.yml` builds/executes native Linux
x86_64/ARM64 musl and macOS Intel/Apple Silicon targets, verifies archive digests,
uses Bedouin 0.23.1 to resolve release-pinned platform URLs/digests, validates the
table, and stages the resulting CLI installation atomically.

The root `install.sh` selects stable/pinned releases, or the documented alpha only
when no stable release exists. It verifies the downloaded release installer
before executing it. Source-generated and release-generated scripts use the
same repository; no cross-product latest-release resolution is involved.

Local checks: 19 Rust tests, Clippy, 9 standard-library Python installer tests,
shell syntax, and workflow lint passed. Installer fixtures cover all platform
rows, latest/no-stable/pinned selection, checksum rejection, wrong binary version,
symlink destinations and interrupted input. These are synthetic tests, not native
cross-architecture executable proof; the release matrix supplies that evidence.

Astro Starlight `site/` supplies the public landing page and alpha guides, search,
GitHub Pages deployment and source-imported installer/example downloads. Local
browser checks passed 19 internal links, exact download contents, search, dark/
light themes and mobile layout. `npm audit` reports no vulnerabilities after
overriding the vulnerable transitive selector parser to patched 7.1.6+.
**ponytail:** that override spans the upstream 6.x parser constraint; remove it
when Expressive Code/postcss-nested resolves a patched parser normally. The actual
site build and code-block/theme rendering are verified with the override.

Publication/native package checks are recorded below. None of these packaging/UI
checks establishes proxy writes,
controller refresh, native-service lifecycle, or journal recovery capabilities.

First alpha release run [37996301701](https://github.com/samishal1998/steward/actions/runs/37996301701)
passed all four native builds, tests, executable checks and Linux static-link
checks for tag `v0.1.0-alpha.1` at `073a1692eaf7051e0f70f11ba3ebe1b4203e919a`.
Publication stopped before attaching the installer because the full Bedouin CLI
requires a provisioning config even for generation. The workflow now uses the
checksum-verified standalone `bedouin-installer` 0.23.1, sharing the same Forge
generator without config/state. Its real published platform table passed local
Steward validation and shell syntax with pipeline failure propagation enabled.
Manual recovery verifies the original tag/run/native results and reuses those
immutable build artifacts; no tag movement or native rebuild is required.

### Published alpha completion record — 2026-10-09

- Release: [v0.1.0-alpha.1](https://github.com/samishal1998/steward/releases/tag/v0.1.0-alpha.1),
  a public non-latest prerelease at source commit `073a1692eaf7051e0f70f11ba3ebe1b4203e919a`.
  [Recovery run 37998237758](https://github.com/samishal1998/steward/actions/runs/37998237758)
  **passed** using workflow fix `fba4dc53e7bb3dc25f1dd47b21eaf3795e9b250b`.
  It verified/reused the four successful original native jobs, matched public
  archive digests, generated/uploaded the pinned installer and exercised it.
- Public assets: four native archives, four SHA256 sidecars, `install.sh` and
  `SHA256SUMS`. Fresh `gh release download` followed by
  `sha256sum --check --strict SHA256SUMS` from the download directory **passed**
  all five catalog entries. `scripts/release_checksums.py --dir . --version
  v0.1.0-alpha.1 --verify SHA256SUMS` **passed** the archive/sidecar/retry checks.
- Live install on Linux x86_64: `curl -fsSL
  https://samishal1998.github.io/steward/install.sh | sh -s -- --dir
  /tmp/opencode/steward-alpha-live.us3M7H/bin` **passed**, with Bash `pipefail`
  enabled. No stable release existed; the selector explicitly chose the alpha.
  Installed `--version` returned `steward 0.1.0-alpha.1`; `file` confirmed a
  stripped static-PIE Linux executable.
- Downloaded all profile/intent/snapshot examples from the live Pages endpoints.
  Installed `route list --json` returned four native routes and `changed: false`;
  installed `plan --file examples/routes-only.yaml --json` returned
  `changed: false`, `applicable: false`. These were synthetic offline inputs,
  with no live controller or native-service topology involved.
- Site: [samishal1998.github.io/steward](https://samishal1998.github.io/steward/).
  [Pages run 37998238267](https://github.com/samishal1998/steward/actions/runs/37998238267)
  **passed**. Headless browser checks against the live public origin **passed**
  the landing/guide pages, 19 internal links, byte-exact installer/example
  downloads, search, mobile overflow and light/dark themes with no page errors.
- Capability change: the existing read-only CLI is now downloadable and
  checksum-verified. Proxy writes, setup/TUI, native-service lifecycle,
  controller refresh and journal recovery gates remain pending.

| Work | Status | Owner / next action |
|---|---|---|
| Standalone Cargo/CLI modules | Implemented and locally checked | Maintainer: keep CLI/schema/README aligned and verify CI/MSRV |
| Read-only inventory and route planning | Implemented initial Caddy/Traefik slice | Next: Docker/file qualified evidence and native binding detection; writes stay gated |
| Synthetic fixture transfer | Independently authored in `fixtures/`, `src/tests.rs`, `tests/cli.rs` | No Apptrail production code/test files vendored |
| Mutation foundation | Initial local HTTP path implemented and exercised | Next: broader authority/scope, historical journal and manifest apply; current bounded single-operation recovery is qualified below |
| Plain Caddy compatibility | Local JSON/HTTP 2.11.7 slice exercised | Next: Caddyfile/import, broader modules/TLS and controller modes |
| Caddy Docker refresh | Release-blocking unknown | Proxy owner: pinned base-only edit without Docker event; later events/restart/invalid content/recovery |
| Traefik watched files | Local HTTP directory 3.7.14 slice exercised | Next: container mounts, broader startup/TLS topologies and restart gates |
| Native/container networking and TLS | Unknown per topology | Networking owner: proxy-context reachability/verification; do not assume local loopback on remote daemon |
| Native Linux/macOS services | Full later milestone, not dropped | Native owner: on-target scope/lifecycle/log/KeepAlive/logout/reboot tests with non-skipped evidence |
| Setup/install/TUI/composition | Full roadmap planned | Implementers: advance after dependencies; no required daemon/process supervisor |
| Independent release assets and docs | Published alpha and Pages; native package/live installer checks passed | Maintainer: preserve tag/archive immutability and advance capabilities only with their acceptance evidence |
| License/attribution | Open | Select Steward license only with user decision; package publishing to crates.io remains disabled |
| Optional AppTrail integration | Stage 0 boundary known, live result unknown; later contracts proposed | Integration owner: use existing proxy discovery first, then explicit new versioned importer if needed |

## Required completion record for implementation work

Append a short entry with: date; task/commit; actual command and exit/result;
OS/engine/controller versions and topology; fixture or live coverage; failure/
recovery evidence; remaining gate; capability change. Mark skipped/unavailable
execution explicitly. Pending commands in the plan/transfer are instructions,
not passing results. The first read-only slice does not close route writes,
setup, TUI, native services or composition milestones.

## Alpha.2: direct commands, completion, agent docs and branding

User requested shell completion, direct command interaction (explicitly **add and
apply live**, not preview-only), simple use cases, published agent guidance, and
selectable logo/brand variants. Cargo version is `0.1.0-alpha.2`.

- Direct canonical `proxies`/`routes` groups keep singular aliases. `proxies add`
  stores API-only inventory connections without a YAML profile, or explicit local
  Caddy JSON/Traefik directory authority. Live HTTP route add/update/disable/enable/
  remove uses persistent receipts, conservative conflicts, input/state kernel locks,
  atomic and fsynced sources, complete Caddy candidate validation/conditional POST,
  native activation and active-route HTTP checks. Generic manifest apply stays
  unavailable. State/source is bounded at 1 MiB; one private operation journal is
  retained up to 8 MiB, with receipts independent of history retention.
- `operations list/recover` exposes phase/ID without private backups; interrupted
  writes block new mutations and compensation is fingerprint-guarded. A rejected
  write can report changed/uncertain effects instead of claiming no mutation.
- `completions` generates Bash/Zsh/Fish/Elvish/PowerShell scripts from the actual
  Clap command tree without loading configuration or initializing state.
  `clap_complete` is the one new dependency; existing `tempfile` is promoted from
  test-only to support same-filesystem durable staging. Rust MSRV remains 1.89.
- Get started now walks API-only inventory, a disposable native Caddy HTTP app,
  move/pause/remove commands, Traefik authority and recovery. A real native Caddy
  example download is source-imported. `llms.txt` and `llms-full.txt` import the
  repository index and full agent guide rather than duplicating instructions.
- Brand selection harness: Waymark (abstract S/route), Gatehouse (gateway) and
  Switchboard (literal connections), original monochrome SVGs, actual product
  chrome/app-icon/small-size contexts, query-param and keyboard selection. No
  brand direction has been promoted; selection remains the user's decision.

### Local execution — Linux x86_64, Rust/Cargo 1.98.0

| Command | Result / boundary |
| --- | --- |
| `cargo test --locked -j 1 -- --test-threads=1` | Passed 13 unit/mock tests + 9 CLI integration tests; 22 total |
| `cargo clippy --locked -j 1 --all-targets -- -D warnings` / `cargo fmt --check` | Passed |
| `cargo +1.89.0 check --locked -j 1` | Passed minimum-version build, including standard-library file locks |
| `scripts/live_route_smoke.py --binary target/debug/steward --caddy PINNED_CADDY --traefik PINNED_TRAEFIK` | Passed both real native lifecycle suites on disposable loopback listeners, private temporary HOME/XDG/state and synthetic HTTP backends |
| Native versions/provenance | Caddy 2.11.7 Linux amd64 archive verified against upstream SHA512 catalog; Traefik 3.7.14 Linux amd64 archive against upstream SHA256 catalog |
| Earlier Caddy 2.6.2 check | Explicit authority rejection: no required root ETag; no route mutation enabled on that installation |
| `python3 -B -m unittest discover -s scripts -p 'test_*.py'` / `sh -n install.sh` | Passed 9 installer contracts and shell syntax for the alpha.2 fallback |
| `go run github.com/rhysd/actionlint/cmd/actionlint@latest .github/workflows/checks.yml .github/workflows/pages.yml .github/workflows/release.yml` | Passed workflow lint |
| `scripts/fetch_test_proxies.py --dir /tmp/opencode/steward-proxy-fetch.OuJSjX` | Passed bounded downloads, executable-only extraction and both pinned upstream digest checks |
| `npm run build --prefix site` / local headless browser harness | Passed 13-page Starlight build; 22 internal links, byte-exact installer/agent/example downloads, search/themes/mobile and all three logo candidates at 1440/420/320px with query/button/keyboard selection and SVG downloads |

The native suites check API-only registration, all five completion scripts/Bash
syntax, live add/update/pause/enable/remove, preserved path/query forwarding,
receipt ownership/short-ID inspection, external host collision, manual input drift,
state and input-scope contention, final Traefik aggregate removal, failed-backend
compensation, killed-CLI recovery and Caddy stale-ETag rejection. Real proxies are
foreground child processes stopped in `finally`; no host service or live user proxy
is a test target. Linux/macOS CI runs the same non-skipped script with pinned native
test artifacts. Platform/publication/browser evidence follows its actual runs.

### Remote checks and public site

Source commit `728893e0aff8a9fc3a62412a0afc6f47d1b25f66` published the direct
commands, completion, examples, agent endpoints and brand harness. Its initial
[Rust checks 38014047056](https://github.com/samishal1998/steward/actions/runs/38014047056)
passed Linux live lifecycle, MSRV and docs, but macOS test artifact selection
failed: Caddy names its macOS archives `mac_arm64`, not Traefik's `darwin_arm64`.
Commit `ec2885d7c74b007d49e42ee0282467d194eb14dd` fixes that naming.
[Rust checks 38014422913](https://github.com/samishal1998/steward/actions/runs/38014422913)
**passed** Linux x86_64/macOS ARM64 Rust and installer checks, non-skipped real
Caddy 2.11.7/Traefik 3.7.14 lifecycle/recovery suites, Rust 1.89 MSRV and docs.

[Pages 38014047098](https://github.com/samishal1998/steward/actions/runs/38014047098)
**passed**. Browser verification against the public origin passed the 22 internal
links, byte-exact installer/llms/full-guide/native-example/snapshot downloads,
search/themes/mobile and all three SVG directions at 1440/420/320px with query,
button and keyboard selection. The picker is at the top so it does not obscure
the logo/context samples. Public entries:
`/steward/guides/quick-start/`, `/steward/llms.txt`, `/steward/llms-full.txt`,
`/steward/project/brand/`. No brand choice has been promoted.

Annotated tag `v0.1.0-alpha.2` points to verified code commit `ec2885d7…`;
[release run 38014774774](https://github.com/samishal1998/steward/actions/runs/38014774774)
supplies the four-target package and public installer gate. Its completion and
installed-artifact proof are recorded after execution, not inferred from main CI.

### Published alpha.2 completion

[Release 38014774774](https://github.com/samishal1998/steward/actions/runs/38014774774)
**passed** all four native targets and publish/install checks. Public non-latest
prerelease [v0.1.0-alpha.2](https://github.com/samishal1998/steward/releases/tag/v0.1.0-alpha.2)
contains four archives, four sidecars, `install.sh`, and `SHA256SUMS`.
Fresh `gh release download` followed by `sha256sum --check --strict SHA256SUMS`
**passed** all five catalog entries; `release_checksums.py --dir . --version
v0.1.0-alpha.2 --verify SHA256SUMS` **passed** archive/sidecar/immutability checks.

On Linux x86_64, with Bash `pipefail`, the live Pages default installer command
`curl -fsSL https://samishal1998.github.io/steward/install.sh | sh -s -- --dir
/tmp/opencode/steward-alpha2-install.ee7V5F/bin` **passed**, explicitly selecting
alpha.2 because no stable release exists. Installed `--version` reported
`steward 0.1.0-alpha.2`. The complete `live_route_smoke.py` suite then **passed
against this downloaded musl executable**, with the separately checksum-verified
Caddy 2.11.7 and Traefik 3.7.14 test executables. This verifies the packaged
commands, completion, real local writes and recovery—not just a source build.
Temporary native processes/inputs/install destinations remain test-only.

All five requested deliverables are implemented and public. Final brand promotion
awaits the user's choice among Waymark, Gatehouse and Switchboard.

## Waymark promotion — 2026-10-10

The user selected **Waymark**. Its original S/route geometry is now the canonical
mark for the site header, adaptive browser favicon and downloadable SVG. The
homepage/README use “Give every route a clear way home.” The existing brand URL
now contains selected-identity guidance. The picker, variant component and unused
Gatehouse/Switchboard assets were removed. One source SVG drives the CSS mask
(selected site theme) and source-imported public SVG endpoint (browser theme).

Local `npm run build --prefix site` and browser verification **passed**: actual
Waymark header/brand masks, favicon MIME/path and byte-exact SVG download, light/
dark layouts at 1440/390/320px, 22 internal links, search and existing public
installer/agent/example endpoints. Pages-origin verification completes publication.

Remaining gates: broader Caddyfile/import/controller, mapped-container/TLS/setup,
native services, full journal history, manifest apply and TUI. These changes do not
close the complete proxy-management roadmap milestone.
