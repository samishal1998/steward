# Steward

**Give every route a clear way home.**

A standalone Rust CLI and TUI for proxy routes, native services, and checked,
recoverable operations. The OS supervises applications; Steward has no required
daemon or AppTrail dependency.

**Current release: v0.2.0-alpha.2.** Includes Caddy/Traefik HTTP/TLS routes,
Caddyfile/import and Docker-controller workflows, saved plan/apply, proxy lifecycle,
systemd/launchd services, SSH/container targets, composition, operation history,
rollback, explicit adoption, dynamic completion and redacted AppTrail exports.

**[Website & guides](https://samishal1998.github.io/steward/)** ·
**[Downloads](https://github.com/samishal1998/steward/releases)** ·
**[Brand assets](https://samishal1998.github.io/steward/project/brand/)**

## Install

Linux/macOS, x86_64 or ARM64:

```sh
curl -fsSL https://samishal1998.github.io/steward/install.sh | sh
~/.local/bin/steward --version
~/.local/bin/steward --help
~/.local/bin/steward tui
```

The selector follows the latest stable release, or explicitly selects the documented
alpha `v0.2.0-alpha.2` when no stable release exists. It verifies the release script,
archive digest and executable version, then places the CLI atomically in
`~/.local/bin`. No sudo, shell edits, proxy setup or service startup is performed
by the CLI installer. Pin a version or choose a directory with:

```sh
curl -fsSL https://samishal1998.github.io/steward/install.sh |
  sh -s -- --version v0.2.0-alpha.2 --dir "$HOME/bin"
```

Release installers use Bedouin-resolved platform URLs and embedded checksums.
Bedouin, Python, Node and Rust are not runtime dependencies of the shell installer.

## Start with direct commands

Save an inventory-only API connection without a Steward YAML profile:

```sh
steward proxies add edge --engine traefik --endpoint http://127.0.0.1:8080
steward routes list --json
```

For live HTTP writes, declare an existing native source once:

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

The [quick start](https://samishal1998.github.io/steward/guides/quick-start/)
provides a complete disposable example. The
[management guide](https://samishal1998.github.io/steward/guides/management/)
covers HTTPS, Caddyfiles/controllers, proxy installation, native services, targets,
and service-and-route composition.

## Plans, history and completion

```sh
steward plan --file routes.yaml --out "$PWD/reviewed-plan.json" --json
steward apply --plan "$PWD/reviewed-plan.json" --yes --json
steward operations list --json
steward operations inspect OPERATION_ID --json
steward operations rollback OPERATION_ID --json
source <(steward completions bash --dynamic)
```

Managed plans recheck state/source/native/tool fingerprints and never prune omitted
IDs. Rollback requires the completed operation's preconditions to still match.
`operations recover ID`, `services recover`, and `proxies recover ID` handle their
respective unfinished operations. Inspect a rejected write's `changed` flag and
diagnostic before retrying; rejection does not always mean no effects occurred.

Completion supports Bash, Zsh, Fish, Elvish and PowerShell. Agents can read
[llms.txt](https://samishal1998.github.io/steward/llms.txt) and the source-backed
[full guide](https://samishal1998.github.io/steward/llms-full.txt).

## Offline examples

Full release archives include these profiles and fixtures:

```sh
./steward --config examples/read-only.yaml proxies list --json
./steward --config examples/read-only.yaml routes list --json
./steward --config examples/read-only.yaml plan --file examples/routes-only.yaml --json
```

The saved responses are synthetic and make no network requests. Profile-only plans
remain non-applicable previews. Writable plans use explicitly registered managed
connections without a `--config` override.

## Ownership and scope

Native identities, status and partial coverage remain visible. An API, hostname,
label or matching resource name does not confer ownership. Direct changes require
receipts and declared native authority; adoption is explicit and limited to exactly
representable definitions. Unknown/complex external behavior stays external.

Services use systemd/launchd and work independently of proxy integrations. SSH and
container commands execute in the selected target's context. Optional AppTrail
exports share redacted observations rather than private state or control authority;
the matching AppTrail file provider is on its `feature/complete-management` branch.

See [capabilities](docs/CAPABILITIES.md), [CLI contracts](docs/CLI_CONTRACT.md),
[feature evidence](docs/FEATURE_COMPLETION.md) and [progress](docs/PROGRESS.md)
for exact implementation and executed deployment boundaries. Public ACME/DNS,
system-scope runtime and logout/reboot are not inferred from user-scope CI tests.
Nginx, Windows for Steward and custom label mappings are deferred; Apache is excluded.

## Development

Requires Rust 1.89 or newer; CI/release compiler is pinned to 1.98.0.

```sh
cargo build --locked -j 1
cargo fmt --check
cargo test --locked -j 1 -- --test-threads=1
cargo clippy --locked -j 1 --all-targets -- -D warnings
python3 -B -m unittest discover -s scripts -p 'test_*.py'
```

Astro Starlight lives in `site/`, built with Node.js 24:

```sh
npm ci --prefix site
npm run dev --prefix site
npm run build --prefix site
```

The local base is `/steward/`. `.github/workflows/pages.yml` deploys relevant `main`
changes through GitHub Pages. `.github/workflows/release.yml` builds four native
targets, packages the CLI/guides/examples, checks the real binaries and archives,
generates the pinned Bedouin-backed installer, and exercises published installation.
Stable tags become latest only after verification; alpha tags stay prereleases.

The [original product pack](docs/planning/README.md) and
[technical transfer](docs/APPTRAIL_TECHNICAL_TRANSFER.md) remain available as history.
Licensing is pending the owner's choice; this package is not published to crates.io.
