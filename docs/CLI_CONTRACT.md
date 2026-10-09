# Implemented CLI contract — schema v1

This document describes the current **read-only** implementation, not the archived
future command grammar. All infrastructure mutation and native service management
commands remain unavailable.

## Binding profile

```yaml
schema_version: "1"
scope: user
proxies:
  - id: edge
    engine: traefik
    endpoint: https://proxy-admin.example.test/prefix
    auth_file: /absolute/path/to/authorization
```

Required fields: `schema_version`, `scope`, `proxies`; binding `id`, `engine`,
`endpoint`. `scope` is `user` or `system` and identifies planning context only:
it does **not** request elevation or authorize the caller to change system resources.
`engine` is `traefik` or `caddy`. IDs are 1–64 ASCII letters/digits/`-`/`_` and unique
within a profile. 1–16 bindings are accepted; there is no implicit default profile.

Optional binding fields: `auth_file`, `snapshot_file`. Null is treated as absent
for these optional fields. All other unknown keys are rejected.

Endpoints: HTTP(S) base URLs with optional prefix or `unix:///absolute/socket` on
Unix. Userinfo, query, fragments, and control characters are rejected. Unix use
on non-Unix platforms returns a capability error. HTTPS uses system trust through
Rustls; no insecure/mTLS/custom-CA flags exist. Management API/version/controller
identity is not inferred from a URL or a server header.

`auth_file` contains one complete Authorization value, such as `Bearer ...` or
`Basic ...`. The path is absolute and read under the caller's permissions, only for
live API access. Contents are trimmed, validated, limited to 8 KiB, reread each
command, and omitted from outputs/errors. It does not copy Apptrail credentials.
Place the file where this user can read it and other users cannot; hardening the
privileged secret-loader boundary is part of the later write milestone.

With `snapshot_file`, no HTTP client or auth file is initialized. The snapshot
path is resolved relative to the profile. The schema-v1 wrapper must identify the
same binding/engine and contain all required endpoint responses and `captured_at`.
Offline evidence is never runtime-confirmed. The capture time is an opaque source
timestamp, not proof of freshness or authenticity.

## Native API reads

| Engine | GET requests |
| --- | --- |
| Caddy | `<base>/config/` |
| Traefik | `<base>/api/http/routers?per_page=10001`, `/api/entrypoints?per_page=10001`, `/api/http/services?per_page=10001` |

Redirects and non-200 responses fail. Error bodies are not displayed. Traefik
`X-Next-Page` must be absent, empty, `0`, or `1`; additional pages reject the scan.
Each endpoint accepts at most 10,000 records where a native array is expected.
Requests are sequential, not an atomic snapshot. Controller/runtime authority is
unknown; API data does not prove upstream reachability, DNS, certificate trust,
or safe write integration.

Limits: configuration/manifest 1 MiB; snapshot wrapper 24 MiB with at most 8 MiB
per required response; authorization 8 KiB; live body 8 MiB; received header data
32 KiB (checked after the HTTP stack parses headers); connect timeout 5 seconds;
request timeout 12 seconds; live inventory deadline 30 seconds per binding.
YAML/route parsing is depth-bounded; no includes, environment interpolation,
process execution, retry loop, background daemon, or unbounded filesystem scan.

The source fingerprint hashes the canonical parsed JSON, not original byte
formatting. Native evidence stays in memory for the command and is not cached to
disk. Source ETags are retained where supplied. Binding fingerprints include the
profile's endpoint/path references, never the authorization-file content.

## Inventory semantics

- Each native route has binding identity, native ID, exact source reference,
  origin, reported status, observation time, coverage, and fingerprint.
- Traefik hostname aliases are fields on one router record, not merged apps.
  Internal/disabled/error routers remain visible. Original rules and service
  objects are retained internally; known literal hosts/paths are potential
  coverage, not a rewritten semantically equivalent rule.
- Caddy nested handlers retain JSON pointer references and inherited constraints.
  Unknown matchers/modules/dynamic handlers make coverage partial. It does not
  recreate Caddyfile intent, decide a Docker plugin's authority, or flatten
  unrecognized behavior into a writable route.
- Raw native payloads are not serialized. Private header matcher values and
  backend URL credentials/query values are not printed by the default inventory.
- `complete` describes supported HTTP matcher/handler interpretation of the
  requested responses, not coverage of all protocols or global host topology.
- Noninteractive reads can report partial success, retaining other successful
  bindings rather than equating a denied source with an empty inventory.

## Route-only manifest and preview

Use the implemented example `examples/routes-only.yaml`. It accepts
`schema_version`, `scope`, optional `routes`, and optional `services`. The latter
must currently be empty; nonempty service intent rejects before network access.
An empty routes list requests no changes; no resources are implicitly removed.

Each route requires `id`, `proxy`, `hostname`, `enabled`, `upstream: {url}`, and
`tls: {mode}`. TLS modes are `http`, `internal`, `public`; these are declared intent,
not configuration generated/applied in this build. Upstream is a single explicit
HTTP(S) origin; credentials, path rewrites, queries, and fragments are rejected.
Service endpoint references are later composition scope.

Hostnames: concrete ASCII/punycode DNS names, lowercased, optional trailing dot
removed. No IP authorities, host ports, regex, wildcards, or URLs. Native observed
hostname spelling and raw configuration are retained separately.

`plan --file` only contacts bindings referenced by that manifest. It checks scope,
requested-host collisions, existing exact/wildcard/catch-all/uncertain overlaps,
offline evidence, and partial coverage. It records input/binding/native-source
fingerprints. Disabled external definitions are conservatively considered reserved.
There is no ownership store, source-authority detection, journal, or mutation
eligibility. The result always has `applicable: false` and explains the missing
apply/setup capabilities. Identical inputs/snapshots produce identical plan data.

## Results

Every JSON command emits one object:

```json
{
  "schema_version": "1",
  "status": "partial",
  "changed": false,
  "data": [],
  "diagnostics": [{"code":"discovery.offline","message":"File observations are not evidence that this configuration is currently loaded"}]
}
```

Statuses: `observed`, `partial`, `planned_read_only`, `rejected`. Diagnostics use
stable scoped codes plus a message and optional resource reference. Strings are
escaped in JSON; human error output escapes terminal control characters. Exit 0
means a read/preview completed, exit 2 means rejection, exit 1 means output failure.
Help/version bypass configuration/network access. CLI usage errors in JSON mode
also use the envelope. A result is not an operation receipt and changes are not
authorized by `--yes`.

Schema evolution and the full management result/receipt contracts will be frozen
at the compatibility/operation-engine milestones before enabling writes.
