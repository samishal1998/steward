# Implemented CLI contract — schema v1

Development branch `0.2.0-dev.1` extends this released alpha.2 contract with the
[management workflows](../site/src/content/docs/guides/management.md). Managed
plans now support apply; native services, TLS, Caddyfile/controller authorities,
targets, composition, history/rollback and TUI have dedicated command paths. The
profile-only read/preview contract below remains compatible. Export schema:
[`schemas/export.schema.json`](../schemas/export.schema.json).

This document describes the current alpha.2 implementation, not the archived
future command grammar. Direct local HTTP route writes are available through
stored explicit authority; manifest apply and native services remain unavailable.

## Direct commands, authority and recovery

Canonical groups are `proxies`, `routes`, `operations`, and `completions`; singular
`proxy`/`route` remain aliases. Without `--config`, inventory uses stored connections:
`proxies add ID --engine caddy|traefik --endpoint URL [--auth-file ABSOLUTE_FILE]`.
API-only connections remain read-only and can use remote or Unix endpoints.

Writable connections additionally declare `--config-file ABSOLUTE_FILE` and
`--listener NAME`. Caddy uses the complete JSON startup source and a same-environment
validator (`--caddy-bin`, default PATH `caddy`); source/native equality and ETags
are required. Traefik adds `--directory ABSOLUTE_DIRECTORY`, whose exact path and
watch setting must appear in the static startup source. Writable APIs are local
loopback only; HTTP frontends are explicit and TLS/setup/controller modes remain
gated. Native compatibility tests use Caddy 2.11.7 and Traefik 3.7.14.

```sh
steward routes add ID --proxy PROXY --host HOST --upstream URL --tls http
steward routes update ID --proxy PROXY --host HOST
steward routes update ID --proxy PROXY --upstream URL
steward routes disable ID --proxy PROXY
steward routes enable ID --proxy PROXY
steward routes remove ID --proxy PROXY
steward operations list --json
steward operations recover OPERATION_ID --json
steward completions bash
```

These route commands apply immediately; they do not require intent YAML or generic
`apply`. They reject `--config` overrides and never adopt external routes by name.
Mutations verify receipt-owned native definitions, conservative listener/host
conflicts, candidate validation, source/native fingerprints, atomic publication,
native activation and active routes' HTTP 2xx/3xx root responses. State/input
scope locks coordinate Steward writers, including distinct state workspaces using
one authoritative input. Human external writes must not race publication; observed
drift blocks writes/recovery. Symlink and hard-linked inputs are rejected.

State defaults to `$XDG_STATE_HOME/steward` or `~/.local/state/steward`; explicit
`--state-dir` wins over `STEWARD_STATE_DIR`. Existing state directories must be
private. Limits: 16 connections, 128 receipts, 1 MiB state/source inputs, one
retained operation up to 8 MiB. Receipt state survives terminal journal replacement.
Caddy validation has a 15-second deadline; active-route probing has a 15-second
aggregate deadline with at most four seconds per request. Native polling has a
10-second observation window; individual bounded API reads can extend a final poll.

Prepared/publishing/published/activation-pending operations are durable before
their next effect. An interrupted operation blocks new writes. Recovery compensates
only unfinished inputs/runtime while fingerprints match; verified/compensated
operations are terminal, not generic undo. Full backups stay in private state.
`operation.recovery_required` can report `changed: true`; inspect the diagnostic
and ID rather than interpreting every nonzero exit as effect-free.

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

YAML anchors/aliases and merge keys are rejected in this first input contract;
parser work is also limited to 100,000 events and 50,000 nodes. Retained expanded
native evidence is capped at 32 MiB per binding and 64 MiB per command so shared
large native service objects cannot multiply memory without a limit. Larger
inventories require a shared-reference reader improvement, not simply raising
input limits. These are serialized-size budgets, not OS-enforced RSS caps.

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
This legacy manifest path does not use the managed receipt/operation path or infer
source authority. Its result always has `applicable: false` and explains the missing
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

Statuses: `observed`, `partial`, `planned_read_only`, `connected`, `applied`,
`compensated`, `rejected`. Diagnostics use
stable scoped codes plus a message and optional resource reference. Strings are
escaped in JSON; human error output escapes terminal control characters. Exit 0
means the command completed, exit 2 means rejection/failed or unresolved activation,
exit 1 means output failure. Reads/previews report `changed: false`; successful
managed writes include an operation receipt/verification. Routing verification is
null when no enabled owned route needed an HTTP probe. Help/version/completion bypass
configuration/state/network initialization; completion emits shell code, not JSON.
CLI usage errors in JSON mode also use the envelope. `--yes` cannot override
authority, ownership, drift or capability gates.

Broader manifest/setup/native-service management contracts remain future milestones.
