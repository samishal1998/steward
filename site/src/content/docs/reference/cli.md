---
title: CLI & configuration
description: Implemented Steward commands, JSON results, input schemas, and exit codes.
---

## Implemented commands

| Command | Behavior |
| --- | --- |
| `steward`, `--help`, `--version` | Help/version only; no proxy initialization |
| `proxy list` | Inspect explicitly configured bindings |
| `proxy inspect ID` | Inspect one binding |
| `route list` | Native route records with source/freshness/status/coverage |
| `route inspect --proxy ID NATIVE_ID` | Inspect a native route such as `photos@file` |
| `doctor` | Bounded inventory/access diagnostics; not complete host diagnosis yet |
| `plan --file FILE` | Deterministic, non-applicable route preview |
| `apply`, `service list` | Explicit unavailable-capability error before config/network initialization |

`--config FILE` and `--json` are global flags. There is no implicit binding profile or automatic setup. No-argument invocation prints help; it does not open the planned TUI yet.

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

Statuses: `observed`, `partial`, `planned_read_only`, `rejected`. Diagnostics have scoped codes, messages, and optional resource references. Native credentials and full raw configurations are not serialized by default.

| Exit | Meaning |
| --- | --- |
| `0` | Read/preview completed; partial coverage can still be present |
| `2` | Usage, schema, capability, or connection rejection |
| `1` | Output failure |

A successful read is not an operation receipt or write authorization. `changed` is always false in this alpha.

## Input validation

YAML and JSON accept schema version `"1"`. Duplicate keys, unknown profile/manifest fields, wrong scalar types, unsupported tags, multiple documents, YAML anchors/aliases/merge keys, and oversized/deep inputs are rejected.

Profile/snapshot/route schemas live in the same repository:

- [Binding profile](https://github.com/samishal1998/steward/blob/main/schemas/config.schema.json)
- [Route-only manifest](https://github.com/samishal1998/steward/blob/main/schemas/routes.schema.json)
- [Native snapshot wrapper](https://github.com/samishal1998/steward/blob/main/schemas/snapshot.schema.json)

Rust also performs semantic validation beyond JSON Schema, including unique IDs, endpoint rules, hostname/upstream semantics, scope, and binding relationships. The [source-level contract](https://github.com/samishal1998/steward/blob/main/docs/CLI_CONTRACT.md) records the exact limits and serialization policy.
