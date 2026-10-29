# Steward agent guide — v0.2.0-alpha.2

Check `steward --version` and `steward --help` before acting. This guide describes
v0.2.0-alpha.2; earlier 0.1 alphas do not implement the full management workflows.
Use `--json` for machine-readable command results. Help, version, completion scripts
and the interactive TUI have their own output forms.

## Install and inspect

```sh
curl -fsSL https://samishal1998.github.io/steward/install.sh |
  sh -s -- --version v0.2.0-alpha.2
steward proxies list --json
steward routes list --json
steward services list --json
```

An API-only connection can be saved without a YAML profile:

```sh
steward proxies add edge --engine traefik --endpoint http://127.0.0.1:8080 --json
```

Use the operator's actual endpoint and an absolute `--auth-file` reference if needed.
Do not put credentials in URLs or infer write authority from readable APIs or names.
`proxies discover --probe [--docker unix:///var/run/docker.sock]` reports candidates;
`proxies inspect-file --engine ENGINE --file FILE` is qualified offline evidence.

## Register writable native authority

```sh
steward proxies add local --engine caddy --endpoint http://127.0.0.1:22019 \
  --config-file /absolute/caddy-http.json --listener web --caddy-bin /absolute/caddy

steward proxies add local-traefik --engine traefik --endpoint http://127.0.0.1:8080 \
  --config-file /absolute/traefik.yaml --directory /absolute/dynamic --listener web
```

Registration declares an existing input/listener; it does not install a proxy or
enable a provider. Caddy JSON must match native state and support ETags. Caddyfiles
add `--caddyfile`, use `--listener IP:PORT`, preserve imports, and adapt the complete
candidate. For a controller's actual base file add `--docker-controller`; the
controller remains the effective writer.

`proxies update` takes a full replacement connection. `proxies remove` refuses
owned routes; `proxies detach` explicitly relinquishes ownership while retaining
native inputs. Do not use detach as a shortcut around unexplained drift.

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

These commands make live changes. Start the replacement backend first; enabled
routes require a verified HTTP 2xx/3xx root response through the selected proxy.
Definitions use one exact hostname and one HTTP(S) upstream origin, without
credentials, queries, fragments, or implicit rewrites.

TLS modes are `http`, `internal`, and `public`, with matching native listener/issuer
setup. Internal verification requires explicit `--ca-file`; Traefik also needs
certificate/key references or a public `--cert-resolver`. Verification preserves
hostname/SNI and certificate trust. Public DNS/challenge reachability is not
established by a local listener check.

`routes adopt ID --proxy PROXY --native-id ID --expected-fingerprint HASH` supports
exactly representable native HTTP definitions. Traefik also requires the dedicated
file via `--source`. Unsupported custom middleware/rules remain external.

## Plan, apply and recover

```sh
steward plan --file routes.yaml --out /absolute/reviewed.json --json
steward apply --plan /absolute/reviewed.json --yes --json
steward operations list --json
steward operations inspect OPERATION_ID --json
steward operations rollback OPERATION_ID --json
```

Managed plans use registered writable connections and revalidate owner, state,
source, native and tool fingerprints. Omitted routes are never pruned. Multi-proxy
application is sequential: inspect reported earlier operation IDs after a later
failure. `--config PROFILE plan --file MANIFEST` is a read-only preview with
`applicable: false` and cannot authorize apply.

- JSON output is one schema-v1 envelope: `status`, `changed`, `data`, `diagnostics`.
- Reads/previews report `changed: false`; partial coverage is not conflict absence.
- Exit 0 means the command completed, exit 2 indicates rejection/failure, exit 1
  indicates output failure. A rejected write can still report `changed: true`.
- `operation.recovery_required` means effects remain uncertain; inspect its ID.
- `operation.compensated` means original owned input/native state was restored.
- Pending operations block further writes in that operation path.

Use `operations recover ID` for unfinished routes, `services recover` for an
unfinished native-service operation, and `proxies recover ID` for a proxy upgrade.
Rollback is for verified route/service operations while lineage, inputs and native
state still match; it never forces restoration over newer changes.

## Proxy and native-service lifecycle

`proxies install ID --engine ENGINE --version VERSION` verifies an upstream artifact
and creates an owned native service. `setup --binary PATH` uses an existing binary.
`upgrade` preserves config and prior running state; `uninstall` retains configuration
and certificate data. Inspect installer receipts/drift before retrying a failure.

`services add/update` takes the complete executable, argv, environment and working
directory. `start`, `stop`, `restart`, `enable`, `disable`, `inspect`, `logs`, and
`remove` act on receipt-owned systemd/launchd definitions. `--ready-url` requires
HTTP 2xx within `--ready-timeout` (1–300 seconds). Put global `--json` before an
executable's final `--` argument separator. Native discovery does not grant ownership.

`up --file STACK` starts services and waits for explicit readiness dependencies
before publishing routes. `down STACK_ID` is route-first and retains services by
default; `--remove-services` removes only unchanged stack-created services. Persistent
operation checkpoints retain partial outcomes across interruption.

## Targets, state, completion and exports

`--ssh HOST` or `--container NAME` delegates to the target's installed `--remote-bin`.
Paths, native managers and loopback probes belong to that target. SSH uses normal
host-key policy and batch authentication. Inspect target operations after a timeout.

State is `$XDG_STATE_HOME/steward` or `~/.local/state/steward`; `--state-dir` overrides
`STEWARD_STATE_DIR`. Keep that private, caller-owned directory across a resource's
lifecycle. Limits include 16 connections, 128 route receipts, 1 MiB state/source
inputs and 8 MiB operation journals. Route/service history is bounded at 1024 entries
and 256 MiB each; hitting a limit does not silently erase history.

`completions bash|zsh|fish|elvish|powershell --dynamic` supplies local saved IDs without
network access or state creation. `tui` calls the same checked core interactively.

`export --out /absolute/shared/steward.json` emits the versioned redacted inventory.
AppTrail's matching file provider is currently on its `feature/complete-management`
branch. Share only the export, refresh within 15 minutes, and preserve source
identity. Partial/stale/invalid exports retain prior observations/preferences;
the bridge grants no infrastructure-control authority.

Management guide: https://samishal1998.github.io/steward/guides/management/
CLI reference: https://samishal1998.github.io/steward/reference/cli/
Export schema: https://samishal1998.github.io/steward/schemas/export.schema.json
Evidence: https://github.com/samishal1998/steward/blob/main/docs/FEATURE_COMPLETION.md
