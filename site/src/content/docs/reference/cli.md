---
title: CLI & configuration
description: Implemented Steward commands, JSON results, input schemas, and exit codes.
---

## Implemented commands

| Command | Behavior |
| --- | --- |
| `steward`, `--help`, `--version` | Help/version only; no proxy initialization |
| `proxies add ID --engine ENGINE --endpoint URL` | Save an API-only connection; no YAML required |
| `proxies add … --config-file FILE --listener NAME` | Explicit local file authority; Traefik also needs `--directory` |
| `proxies list` / `proxies inspect ID` | Inspect stored connections or explicit profile bindings |
| `proxies update ID …` / `proxies remove ID` / `proxies detach ID` | Replace a connection, remove it when unused, or explicitly relinquish ownership |
| `proxies discover` / `proxies inspect-file` | Native/API/known-label candidates and qualified offline file inventory |
| `proxies install` / `setup` / `upgrade` / `recover` / `uninstall` | Explicit checksum-verified proxy and native-service lifecycle |
| `routes list` | Native route records, ownership, source/freshness/status/coverage |
| `routes inspect ID --proxy PROXY` | Native ID or a managed route's short ID |
| `routes add ID --proxy PROXY --host HOST --upstream URL --tls MODE` | Create, activate and verify an owned HTTP/TLS route |
| `routes update ID --proxy PROXY --host HOST` / `--upstream URL` | Update an owned route; at least one change is required |
| `routes disable` / `routes enable` / `routes remove ID --proxy PROXY` | Pause, restore or remove a receipt-owned route |
| `routes adopt ID --proxy PROXY --native-id ID --expected-fingerprint HASH` | Checked adoption of an exactly representable native HTTP definition |
| `operations list` / `inspect ID` / `rollback ID` / `recover ID` | Historical metadata, guarded reversal and unfinished-route recovery |
| `completions bash\|zsh\|fish\|elvish\|powershell [--dynamic]` | Static command completion or saved local ID completion |
| `doctor` | Bounded inventory/access diagnostics; not complete host diagnosis yet |
| `plan --file FILE --out FILE` / `apply --plan FILE --yes` | Save and apply a revalidated managed plan |
| `--config PROFILE plan --file FILE` | Compatible non-applicable read-only preview |
| `services add` / `update` / `start` / `stop` / `restart` / `enable` / `disable` / `remove` | Receipt-owned systemd/launchd service lifecycle |
| `services list` / `inspect` / `logs` / `discover` / `inspect-native` / `recover` | Service observations, bounded logs and recovery |
| `up --file STACK` / `down ID [--remove-services]` | Readiness-gated service-first publication and route-first removal |
| `export --out FILE` | Versioned redacted inventory for AppTrail's optional file provider |
| `tui` | Interactive Routes, Proxies, Services and Operations views |

`--config FILE`, `--state-dir DIRECTORY` and `--json` are global flags. Without `--config`, reads use saved connections. `--ssh HOST` or `--container NAME` delegates to an installed `--remote-bin` in that target's context. `proxy`, `route`, and `service` remain aliases for the plural groups. Direct writes use stored authority and reject profile overrides. No-argument invocation prints help. Use each subcommand's `--help` for its exact arguments.

## JSON results

Every JSON command emits one schema-v1 object on stdout, including usage, configuration, and connection errors:

```json
{
  "schema_version": "1",
  "status": "partial",
  "changed": false,
  "data": [],
  "diagnostics": [
    {"code": "discovery.offline", "message": "File observations are not evidence that this configuration is currently loaded"}
  ]
}
```

Statuses include `observed`, `partial`, `planned`, `planned_read_only`, `connected`, `applied`, `rolled_back`, `compensated`, and `rejected`. Diagnostics have scoped codes, messages, and optional resource references. Private full configuration backups remain in the operation journal, not stdout. Help/version, completion scripts and the interactive TUI have their own output forms.

| Exit | Meaning |
| --- | --- |
| `0` | Requested command completed; reads can still report partial coverage |
| `2` | Usage, schema, capability, connection, authority, drift or activation rejection |
| `1` | Output failure |

A successful read is not an operation receipt or write authorization. Reads/previews report `changed: false`; registrations and successful direct mutations report true. Rejected writes can report true when publication/activation remains unresolved—read the diagnostic and operation ID, then inspect/recover before retrying. [Agent guidance](../../llms-full.txt) records the exact recovery rules.

## Writable scope and state

Writes require explicit native source authority. Caddy JSON/Caddyfiles use full candidate validation and conditional native activation; a Docker controller activates its own base-file contribution. Traefik uses an explicitly configured watched directory. HTTP/TLS verification runs in the selected host/container context with the requested hostname. Public DNS reachability is not inferred from a loopback verification request. See [management workflows](../../guides/management/) for setup, certificate and native-manager requirements.

State defaults to `$XDG_STATE_HOME/steward` or `~/.local/state/steward`. `--state-dir` wins over `STEWARD_STATE_DIR`. The directory must be private and owned by the caller. Limits include 16 connections, 128 route receipts, 1 MiB state/source inputs, and an 8 MiB per-operation journal. Route/service histories are bounded at 1024 entries / 256 MiB each. State/input locks and operation lineage protect writes and rollback; source drift is rechecked before publication/recovery. External tools must not concurrently write an owned aggregate.

## Input validation

YAML and JSON accept schema version `"1"`. Duplicate keys, unknown profile/manifest fields, wrong scalar types, unsupported tags, multiple documents, YAML anchors/aliases/merge keys, and oversized/deep inputs are rejected.

Profile/snapshot/route schemas live in the same repository:

- [Binding profile](https://github.com/samishal1998/steward/blob/main/schemas/config.schema.json)
- [Route-only manifest](https://github.com/samishal1998/steward/blob/main/schemas/routes.schema.json)
- [Native snapshot wrapper](https://github.com/samishal1998/steward/blob/main/schemas/snapshot.schema.json)
- [Read-only export](../../schemas/export.schema.json)

Rust also performs semantic validation beyond JSON Schema, including unique IDs, endpoint rules, hostname/upstream semantics, scope, and binding relationships. The [source-level contract](https://github.com/samishal1998/steward/blob/main/docs/CLI_CONTRACT.md) records the exact limits and serialization policy.
