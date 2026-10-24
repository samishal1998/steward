# Steward

**Give every route a clear way home.**

[Waymark logo & brand assets](https://samishal1998.github.io/steward/project/brand/)

A standalone Rust CLI/TUI project for reverse-proxy routes and native services.
Routes and services are independent capabilities; the OS supervises applications,
and Steward has no required daemon or Apptrail dependency.

**Feature branch: 0.2.0-dev.1 management expansion.** Includes HTTP/TLS routes,
Caddyfile/import and Docker-controller authority, saved plan/apply, proxy lifecycle,
systemd/launchd services, SSH/container targets, composition, operation history/
rollback, explicit adoption, dynamic completion, TUI and a versioned AppTrail export.
The published installer still selects alpha.2 until a new release is finalized.
See [feature coverage and evidence](docs/FEATURE_COMPLETION.md) and the
[management guide](site/src/content/docs/guides/management.md).

Build this branch to use the new workflows:

```sh
git clone --branch feature/complete-management https://github.com/samishal1998/steward.git
cd steward
cargo build --locked -j 1
./target/debug/steward --version
./target/debug/steward tui
```

**[Website & guides](https://samishal1998.github.io/steward/)** · **[Downloads](https://github.com/samishal1998/steward/releases)**

## Install the alpha

Linux/macOS x86_64 or ARM64:

```sh
curl -fsSL https://samishal1998.github.io/steward/install.sh | sh
~/.local/bin/steward --help
```

The installer follows the latest stable release, or explicitly falls back to the
documented alpha `v0.1.0-alpha.2` when none exists. It verifies the release
installer, archive digest, and executable version, then places the CLI atomically
in `~/.local/bin`. No sudo, shell edits, proxy setup, or service startup. Use
`sh -s -- --version v0.1.0-alpha.2 --dir "$HOME/bin"` for a pin/custom destination.

Release-specific installers use Bedouin-resolved platform URLs and embedded
digests; no Bedouin/Python/Node/Rust runtime is required by the shell installer.
See the [installation guide](https://samishal1998.github.io/steward/guides/installation/).

## Use commands directly

No Steward YAML is needed to register an inventory-only connection:

```sh
steward proxies add edge --engine traefik --endpoint http://127.0.0.1:8080
steward routes list --json
```

For live local HTTP writes, declare the existing native startup authority once:

```sh
steward proxies add local --engine caddy --endpoint http://127.0.0.1:22019 \
  --config-file "$PWD/caddy-http.json" --listener web
steward routes add app --proxy local --host app.localhost \
  --upstream http://127.0.0.1:3000 --tls http
steward routes update app --proxy local --upstream http://127.0.0.1:4000
steward routes disable app --proxy local
steward routes enable app --proxy local
steward routes remove app --proxy local
```

The first writable slice needs an HTTP-only local Caddy server or Traefik watched
directory, a reachable backend returning 2xx/3xx at `/`, and a private persistent
state directory. Caddy source must match loaded native JSON and provide an ETag;
full candidates are validated and conditionally activated. Traefik uses a separate
owned aggregate and verifies native router/service activation. Both preserve
external routes and use source/state locks, receipts and guarded compensation.
See [Get started](https://samishal1998.github.io/steward/guides/quick-start/) for a
complete disposable example using [`examples/caddy-http.json`](examples/caddy-http.json).

```sh
steward operations list --json
steward operations recover OPERATION_ID --json
source <(steward completions bash)
```

Completion supports Bash, Zsh, Fish, Elvish and PowerShell. Agents can read
[llms.txt](https://samishal1998.github.io/steward/llms.txt) and the source-backed
[full guide](https://samishal1998.github.io/steward/llms-full.txt).

## Read-only profiles and previews

Requires Rust 1.89 or newer. This repository has one Cargo package and a committed
dependency lockfile; no Go, Node.js, Apptrail server, or database is needed.

```sh
cargo build --locked -j 1
./target/debug/steward --help
./target/debug/steward --config examples/read-only.yaml proxy list --json
./target/debug/steward --config examples/read-only.yaml route list --json
./target/debug/steward --config examples/read-only.yaml doctor --json
./target/debug/steward --config examples/read-only.yaml plan --file examples/routes-only.yaml --json
```

The example profile uses synthetic snapshots and makes **no network requests**.
Offline observations always say `runtime_confirmed: false`. Every plan has
`applicable: false`; its proposed actions and collision diagnostics are a preview,
not proof of a safe managed integration.

For your own already enabled, authorized API, edit the endpoint in one of these
profiles and run:

```sh
./target/debug/steward --config examples/live-traefik.yaml proxy inspect edge --json
./target/debug/steward --config examples/live-caddy.yaml proxy inspect caddy-edge --json
```

`localhost` is the calling process's network context. Steward does not enable an
API, mount a socket, restart a proxy, or change network access. Docker labels and
file-authority discovery are not implemented in this first slice.

## Read-only profile commands

| Command | Behavior |
| --- | --- |
| No arguments / `--help` / `--version` | Help/version only, including noninteractive terminals |
| `proxy list` / `proxy inspect ID` | Inspect explicit bindings; no network port scan |
| `route list` | Native route records with source, freshness, status, coverage, and external ownership |
| `route inspect --proxy ID NATIVE_ID` | Inspect a native ID such as `photos@file` |
| `doctor` | Same bounded inventory with access/coverage diagnostics; not native host diagnosis yet |
| `plan --file FILE` | Validate route-only intent and produce a deterministic non-applicable preview |
| `apply` / `service list` | Structured unavailable-capability error before config loading or connection setup |

`--config FILE` and `--json` are global options. JSON mode emits one schema-v1
envelope on stdout, including command/config/connection errors. `changed` is
false for these reads/previews. Exit 0 means the read/preview completed, **not that mutations are approved**;
partial coverage is reported in the envelope. Exit 2 means validation, capability,
or connection rejection; exit 1 is an output failure. Broken pipes exit normally.

## Configuration and scope

See [CLI contracts](docs/CLI_CONTRACT.md) and `schemas/`. YAML and JSON reject
duplicate keys, unknown configuration fields, unknown major versions, wrong
scalar types, unsupported tags, and multiple documents. Inputs and API responses
have explicit byte/record/depth limits. Snapshot paths are relative to the profile;
authorization-file paths are absolute.

Profile-only observed native routes remain **external**. Direct managed reads can
qualify receipt-matched routes as `owned` or `owned_drifted`. Names, labels, matching hosts,
or an API's availability do not confer ownership. Unsupported matchers/handlers,
disabled/error statuses, and generated internal routes remain visible. Native
payloads remain available internally for analysis/fingerprints; full raw configs
and credential-bearing matcher values are not printed by default.

Service-only paths must not initialize proxy/Docker integrations. The unavailable
service placeholder enforces this now; actual systemd/launchd management is a
later milestone, not an Apptrail self-installer wrapped as a general service API.

## Development checks

```sh
cargo fmt --check
cargo test --locked -j 1 -- --test-threads=1
cargo clippy --locked -j 1 --all-targets -- -D warnings
python3 -B -m unittest discover -s scripts -p 'test_*.py'
```

Tests use synthetic native responses and disposable local mock HTTP servers.
They do not mutate native services, live proxy configurations, or production data.
See [progress](docs/PROGRESS.md) for actual execution evidence and remaining gates.

## Product plan and handoff

- [Documentation index](docs/INDEX.md)
- [Repository decision](docs/decisions/repository-placement.md)
- [Source-backed Apptrail technical transfer](docs/APPTRAIL_TECHNICAL_TRANSFER.md)
- [Implementation plan](docs/IMPLEMENTATION_PLAN.md)
- [Capabilities and limits](docs/CAPABILITIES.md)
- [Accepted product pack](docs/planning/README.md) and [discovery addendum](docs/planning/08_DISCOVERY_ADDENDUM.md)

The full plan retains Caddy ordinary-file and Caddy Docker-controller management,
Traefik watched-file management, explicit bootstrap, CLI/TUI, journaled recovery,
Linux/macOS native services, and optional composition. Successful inventory alone
does not satisfy those release gates. Apache and configurable label mappings
are excluded/deferred as specified in the plan.

Licensing is pending the owner's choice. The repository is public, but no
open-source license has been selected and this package is not published to crates.io.

## Website and releases

Astro Starlight lives in `site/`, built with Node.js 24:

```sh
npm ci --prefix site
npm run dev --prefix site
npm run build --prefix site
```

Local base path: `/steward/`. `.github/workflows/pages.yml` deploys to GitHub Pages
on relevant `main` changes; use **Pages → Source → GitHub Actions**. The root
channel-selector script and real fixture/example files are imported into static
download endpoints rather than maintained as duplicate copies.

`.github/workflows/release.yml` builds and executes native Linux x86_64/ARM64
musl and macOS Intel/Apple Silicon targets for tags matching the Cargo version.
It publishes immutable archives/digests, uses verified Bedouin 0.23.1 generation,
validates the resolved installer table, and exercises the published installer.
Stable releases become latest only after the installer succeeds; alpha tags stay
prereleases. A retry checks public binary digests rather than replacing them.

These releases package the current alpha. Broader proxy modes, TLS/setup, TUI,
bootstrap and native-service support still require their acceptance gates.
See [release maintenance](https://samishal1998.github.io/steward/project/releases/).
