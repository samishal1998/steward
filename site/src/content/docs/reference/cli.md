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
| `routes list` | Native route records, ownership, source/freshness/status/coverage |
| `routes inspect ID --proxy PROXY` | Native ID or a managed route's short ID |
| `routes add ID --proxy PROXY --host HOST --upstream URL --tls http` | Create, activate and verify an owned HTTP route |
| `routes update ID --proxy PROXY --host HOST` / `--upstream URL` | Update an owned route; at least one change is required |
| `routes disable` / `routes enable` / `routes remove ID --proxy PROXY` | Pause, restore or remove a receipt-owned route |
| `operations list` / `operations recover ID` | Inspect the retained operation; compensate unfinished writes |
| `completions bash\|zsh\|fish\|elvish\|powershell` | Generate shell code without configuration/state initialization |
| `doctor` | Bounded inventory/access diagnostics; not complete host diagnosis yet |
| `plan --file FILE` | Deterministic, non-applicable route preview |
| `apply`, `service list` | Explicit unavailable-capability error before config/network initialization |

`--config FILE`, `--state-dir DIRECTORY` and `--json` are global flags. Without `--config`, reads use connections saved by `proxies add`; there is no automatic discovery/setup. `proxy`/`route` remain visible aliases for `proxies`/`routes`. Direct writes use stored authority and reject `--config` overrides. No-argument invocation prints help.

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

Statuses: `observed`, `partial`, `planned_read_only`, `connected`, `applied`, `compensated`, `rejected`. Diagnostics have scoped codes, messages, and optional resource references. Private full configuration backups remain in the operation journal, not stdout.

| Exit | Meaning |
| --- | --- |
| `0` | Requested command completed; reads can still report partial coverage |
| `2` | Usage, schema, capability, connection, authority, drift or activation rejection |
| `1` | Output failure |

A successful read is not an operation receipt or write authorization. Reads/previews report `changed: false`; registrations and successful direct mutations report true. Rejected writes can report true when publication/activation remains unresolved—read the diagnostic and operation ID, then inspect/recover before retrying. [Agent guidance](../../llms-full.txt) records the exact recovery rules.

## Writable scope and state

The first write mode is same-host local HTTP routing with an exact host and one HTTP(S) upstream origin. Caddy requires a complete JSON startup file matching the loaded API, one HTTP listener with automatic HTTPS disabled, a validator executable, and conditional-update ETags. Traefik requires a static startup file enabling the exact watched directory and HTTP entrypoint. Native/routing activation is checked; DNS and TLS readiness are not inferred.

State defaults to `$XDG_STATE_HOME/steward` or `~/.local/state/steward`. `--state-dir` wins over `STEWARD_STATE_DIR`. The state directory must be private. Limits: 16 connections, 128 managed route receipts, 1 MiB state/source inputs, one retained operation with an 8 MiB journal limit. State and input scopes have writer locks; symlink/hard-linked inputs are rejected. Human edits are fingerprint-checked before publication/recovery; external tools must not concurrently write Steward's owned aggregate.

## Input validation

YAML and JSON accept schema version `"1"`. Duplicate keys, unknown profile/manifest fields, wrong scalar types, unsupported tags, multiple documents, YAML anchors/aliases/merge keys, and oversized/deep inputs are rejected.

Profile/snapshot/route schemas live in the same repository:

- [Binding profile](https://github.com/samishal1998/steward/blob/main/schemas/config.schema.json)
- [Route-only manifest](https://github.com/samishal1998/steward/blob/main/schemas/routes.schema.json)
- [Native snapshot wrapper](https://github.com/samishal1998/steward/blob/main/schemas/snapshot.schema.json)

Rust also performs semantic validation beyond JSON Schema, including unique IDs, endpoint rules, hostname/upstream semantics, scope, and binding relationships. The [source-level contract](https://github.com/samishal1998/steward/blob/main/docs/CLI_CONTRACT.md) records the exact limits and serialization policy.
