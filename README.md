# Steward

A standalone Rust CLI/TUI project for reverse-proxy routes and native services.
Routes and services are independent capabilities; the OS supervises applications,
and Steward has no required daemon or Apptrail dependency.

**Current status: read-only foundation, not the proxy-management release.** The CLI
inspects explicitly configured Caddy/Traefik APIs or synthetic saved responses and
previews exact-host route intent. Writes, proxy setup/install, general native
services, composition, and the TUI remain planned behind the documented gates.

## Try the implemented slice

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

## Exact commands in this build

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
envelope on stdout, including command/config/connection errors. `changed` is always
false. Exit 0 means the read/preview completed, **not that mutations are approved**;
partial coverage is reported in the envelope. Exit 2 means validation, capability,
or connection rejection; exit 1 is an output failure. Broken pipes exit normally.

## Configuration and scope

See [CLI contracts](docs/CLI_CONTRACT.md) and `schemas/`. YAML and JSON reject
duplicate keys, unknown configuration fields, unknown major versions, wrong
scalar types, unsupported tags, and multiple documents. Inputs and API responses
have explicit byte/record/depth limits. Snapshot paths are relative to the profile;
authorization-file paths are absolute.

Every observed native route remains **external**. Names, labels, matching hosts,
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
