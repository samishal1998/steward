# Steward agent guide

## Development branch: 0.2.0-dev.1

The `feature/complete-management` branch adds the workflows below. Check the
installed version: the public installer still supplies alpha.2, whose contract is
retained in the subsequent sections. Do not issue development-only commands to
an older installed binary.

- `proxies update` takes a complete replacement connection; `proxies remove`
  refuses owned routes; `proxies detach` explicitly relinquishes ownership.
- `proxies discover --probe [--docker unix:///var/run/docker.sock]` reports
  candidates, not write authority. `proxies inspect-file --engine … --file …`
  provides qualified offline native evidence.
- Managed `plan --file routes.yaml --out /absolute/review.json`, followed by
  `apply --plan /absolute/review.json --yes`, checks state/source/native digests.
  Omitted IDs are not pruned. Multi-proxy partial results include operation IDs.
- Caddyfile connections add `--caddyfile --listener IP:PORT --caddy-bin PATH`;
  a Docker controller's actual base file additionally uses `--docker-controller`.
  The controller remains the effective writer.
- Routes accept `--tls http|internal|public` with matching native listener/issuer
  setup. Internal TLS requires explicit `--ca-file`; Traefik additionally uses
  certificate/key paths or a configured public `--cert-resolver`. No insecure
  verification or trust-store edits are performed.
- `proxies install ID --engine … --version …` verifies upstream artifacts and
  installs a native service; `setup --binary PATH` uses an existing executable.
  `upgrade`, `recover`, and `uninstall` preserve configuration/certificate data.
- `services add/update` takes a complete executable/argv/environment specification.
  `start`, `stop`, `restart`, `enable`, `disable`, `inspect`, `logs`, and `remove`
  operate on receipt-owned systemd/launchd definitions. `services recover` handles
  unfinished operations. Keep global `--json` before an executable's `--` argv.
- `up --file STACK` is service-first/readiness-gated; `down STACK_ID` is route-first
  and retains services unless `--remove-services` explicitly removes stack-created
  services. Persistent pre-effect operation IDs support interrupted reconciliation.
- `--ssh HOST` and `--container NAME` execute the installed `--remote-bin` in that
  target. Paths, native managers, and loopback probes belong to the target.
- `operations list/inspect` expose bounded historical metadata. `rollback ID`
  reverses a verified route/service operation only while inputs and state match.
  `recover` is for unfinished route writes; never force restoration over drift.
- `routes adopt ID --proxy … --native-id … --expected-fingerprint …` handles
  exactly representable native HTTP definitions. Traefik requires the dedicated
  source file via `--source`; complex external definitions remain external.
- `completions SHELL --dynamic` completes saved local IDs without network calls.
  `tui` exposes the same application commands interactively.
- `export --out /absolute/shared/steward.json` emits schema-v1 `steward.inventory`
  for AppTrail's matching read-only file provider. Partial/stale snapshots preserve
  prior observations. Export files contain no private command environment/backups.

Use [management workflows](../site/src/content/docs/guides/management.md) and
[feature evidence](FEATURE_COMPLETION.md) for exact setup and verification limits.

## Released alpha.2 contract

Requires Steward v0.1.0-alpha.2 for direct commands and completions. Check `steward --version` and `steward --help` before acting; alpha.1 supports inventory/previews only.

## Discover connections and routes

```sh
steward proxies list --json
steward routes list --json
steward routes inspect NATIVE_ID --proxy PROXY_ID --json
```

An API-only connection can be stored without a YAML profile:

```sh
steward proxies add edge --engine traefik --endpoint http://127.0.0.1:8080 --json
```

Use the operator's endpoint and absolute `--auth-file` reference if needed. Authorization file contents are never command arguments. Do not infer authority or ownership from an API being readable, a native name, or a matching hostname.

## Register explicit writable authority

Caddy: an existing same-host HTTP-only server, its complete JSON startup file, and the matching Caddy validator executable:

```sh
steward proxies add local --engine caddy --endpoint http://127.0.0.1:22019 \
  --config-file /absolute/caddy-http.json --listener web \
  --caddy-bin /absolute/caddy --json
```

Traefik: a local HTTP entrypoint and a static startup file enabling the exact watched directory:

```sh
steward proxies add local-traefik --engine traefik --endpoint http://127.0.0.1:8080 \
  --config-file /absolute/traefik.yaml --directory /absolute/dynamic --listener web --json
```

These registrations declare file authority; they do not install/start a proxy or enable providers. First writable scope: same-host loopback API, user-owned state, exact hostname, explicit HTTP frontend, one HTTP(S) upstream origin, and no rewrite. Caddy requires one listener with `automatic_https.disable: true`; native compatibility checks use Caddy 2.11.7 and Traefik 3.7.14. Caddyfile/import, Docker-controller, container mappings, TLS setup, and native services remain separate gates.

## Direct route lifecycle

```sh
steward routes add app --proxy local --host app.localhost \
  --upstream http://127.0.0.1:3000 --tls http --json
steward routes inspect app --proxy local --json
steward routes update app --proxy local --upstream http://127.0.0.1:4000 --json
steward routes disable app --proxy local --json
steward routes enable app --proxy local --json
steward routes remove app --proxy local --json
```

These are live writes, not previews. Start the new backend before add/update/enable: the root path must return HTTP 2xx/3xx through the selected proxy. Successful mutations return `status: "applied"`, `changed: true`, an operation ID, and native/routing verification. Routing verification is null when no enabled owned route needed an HTTP probe. External or drifted routes cannot be adopted or edited by name.

## Interpret results and recover

- `--json` emits one schema-v1 envelope: `status`, `changed`, `data`, `diagnostics`.
- Reads report `changed: false`. `partial` means incomplete coverage, not an empty/safe inventory.
- Exit 0 means the requested command completed; exit 2 rejects arguments, capability, authority, drift, connection or activation; exit 1 is output failure.
- A rejected mutation is not necessarily effect-free. Read `changed` and the diagnostic. `operation.recovery_required` reports a published input/uncertain activation and supplies the operation ID.
- `operation.compensated` means the original owned input/native configuration was restored. Correct the failed prerequisite before retrying.
- `operation.pending` blocks new writes until the retained unfinished operation is recovered.

```sh
steward operations list --json
steward operations recover OPERATION_ID --json
```

Recovery compensates only unfinished operations while input/runtime fingerprints still match. It never forces restoration over drift and does not undo terminal verified operations. The journal retains one operation, not an unlimited audit history; receipts persist independently. Keep the same private state directory across a route's lifecycle. Its default is `$XDG_STATE_HOME/steward` or `~/.local/state/steward`; `--state-dir` overrides `STEWARD_STATE_DIR`.

## Read-only manifests and completion

Existing `--config PROFILE plan --file MANIFEST --json` remains a read-only preview with `applicable: false`. Generic `apply` and general service management are unavailable; direct routes use their own checked operation path.

`steward completions bash|zsh|fish|elvish|powershell` emits shell code, not JSON. Help/version/completion do not initialize proxy state.

Published reference: https://samishal1998.github.io/steward/llms.txt
Full agent guide: https://samishal1998.github.io/steward/llms-full.txt
Quick start: https://samishal1998.github.io/steward/guides/quick-start/
CLI: https://samishal1998.github.io/steward/reference/cli/
