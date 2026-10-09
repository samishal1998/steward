---
title: Route planning
description: Understand exact-host previews, conflicts, fingerprints, and why this alpha does not apply changes.
---

## The current boundary

The alpha validates route-only intent and compares it with inventory. It does not own definitions, write files, activate a proxy, or create an apply-ready operation.

```sh
steward --config edge.yaml plan --file routes.yaml --json
```

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

Each route is a complete specification for its ID. IDs are unique. The manifest and binding scopes must match. Only explicitly referenced bindings are contacted.

The first route subset uses one concrete ASCII/punycode DNS hostname and one explicit HTTP(S) upstream origin. Wildcards, host ports, URL credentials, fragments, queries, and implicit upstream path rewrites are rejected. TLS modes express intent: `http`, `internal`, or `public`; the alpha does not configure or validate certificate issuance from that declaration.

Empty/absent route collections request no changes and do not prune resources. Nonempty service specifications return an unsupported-capability error before connecting to proxies; native services are a later milestone.

## Reading the result

```json
{
  "schema_version": "1",
  "status": "planned_read_only",
  "changed": false,
  "data": {
    "applicable": false
  }
}
```

The complete result also includes proposed actions, upstream/TLS intent, conflict diagnostics, and binding/source/native fingerprints. Equivalent inputs and snapshots produce deterministic plan data.

Existing exact, wildcard, catch-all, and uncertain overlaps are conservatively reported. Disabled definitions are still configured resources. Unrecognized native behavior prevents a complete conflict assessment. A name resembling a Steward resource remains external without ownership evidence.

## Why `applicable` stays false

Before writes can be enabled, Steward needs:

- An explicitly approved authoritative file/controller integration.
- Ownership receipts and exact native identity/path fingerprints.
- A scoped writer lock and durable operation journal.
- Revalidation immediately before mutation.
- Verified activation and drift-aware compensation/recovery.
- Proxy-context network and TLS verification appropriate to the operation.

The alpha reports missing apply/setup/network prerequisites rather than turning read access into permission to mutate.

`apply --yes` still returns `capability.apply_unavailable`. It does not load an arbitrary plan, initialize a proxy client, or bypass the missing foundation.

See the [architecture and roadmap](../../project/architecture/) for the full management scope and release gates.
