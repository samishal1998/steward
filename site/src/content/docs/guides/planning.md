---
title: Route planning
description: Review route intent, save an applicable plan, and revalidate it before making changes.
---

## Save a managed plan

Register an existing proxy with explicit writable authority first; the
[management guide](../management/) shows Caddy JSON/Caddyfile and Traefik examples.
Then review route intent without publishing it:

```sh
steward plan --file routes.yaml --out "$PWD/reviewed-plan.json" --json
```

Use a new absolute output path. The saved artifact contains the requested routes,
state identity, and source/native/tool fingerprints. Private configuration backups
belong in the operation journal, not the plan output.

## Route-only input

```yaml
schema_version: "1"
scope: user
routes:
  - id: notes
    proxy: edge
    hostname: notes.example.test
    enabled: true
    upstream:
      url: http://127.0.0.1:3000
    tls:
      mode: http
services: []
```

Each route is a complete specification for its ID. A plan upserts only supplied
IDs; empty or omitted route collections never prune resources. Routes use one
concrete ASCII/punycode hostname and one HTTP(S) upstream origin, without URL
credentials, query strings, fragments, or implicit path rewriting. TLS policies
are `http`, `internal`, or `public` and must match the registered native setup.

Service-and-route composition uses its own stack format and `up`/`down` commands.
Keep the `services` array empty in a route-only plan.

## Apply the reviewed artifact

```sh
steward apply --plan "$PWD/reviewed-plan.json" --yes --json
```

Apply rechecks the owner, state, source, native configuration and selected tool
fingerprints before writing. A changed input rejects the saved plan; generate a
fresh one instead of overriding that check. Every write uses the same ownership,
conflict, activation and recovery path as the direct route commands.

Multi-proxy application is sequential. If a later proxy fails, the error reports
the operation IDs already applied. Inspect those operations before retrying or
rolling back. `--yes` accepts this plan; it does not bypass changed inputs or
unsupported authority.

## Read-only profile previews

An explicitly supplied `--config` profile selects the compatible read-only path:

```sh
steward --config examples/read-only.yaml plan --file examples/routes-only.yaml --json
```

```json
{
  "schema_version": "1",
  "status": "planned_read_only",
  "changed": false,
  "data": { "applicable": false }
}
```

This path can use saved snapshots and never treats API access as write authority.
It reports exact/wildcard/catch-all overlaps, partial coverage and missing evidence.
Its result cannot be passed to `apply`. Offline examples are available in the full
release archive and as [profile](../../downloads/examples/read-only.yaml),
[intent](../../downloads/examples/routes-only.yaml),
[Caddy snapshot](../../downloads/fixtures/caddy/snapshot.json), and
[Traefik snapshot](../../downloads/fixtures/traefik/snapshot.json) downloads.

## Inspect or reverse an operation

```sh
steward operations list --json
steward operations inspect OPERATION_ID --json
steward operations rollback OPERATION_ID --json
```

Rollback requires the completed operation's state/source/runtime to still match
and rechecks conflicts. `operations recover ID` is for unfinished route writes.
Both refuse restoration over newer intent; neither adopts an external route by name.
