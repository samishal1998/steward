---
title: Proxy connections
description: Configure explicit Caddy or Traefik endpoints, authorization files, Unix sockets, and snapshots.
---

## Explicit bindings

Save a connection once with `proxies add`, or select a version-1 binding profile with `--config` for reads and previews. Stored connections are used when `--config` is absent:

```sh
steward proxies add edge --engine traefik --endpoint http://127.0.0.1:8080
steward proxies list
```

An API-only connection grants inventory access. Writable commands require separately declared native authority. Discovery never chooses or enables a proxy automatically.

```yaml
schema_version: "1"
scope: user
proxies:
  - id: edge
    engine: traefik
    endpoint: https://proxy-admin.example.test/prefix
    auth_file: /absolute/path/proxy-authorization
```

The profile accepts 1–16 unique binding IDs and `scope: user` or `system`. Scope identifies planning context; it does not request elevation or grant write permission.

## Traefik

Enter the API origin and any configured base-path prefix, not `/dashboard/` or a complete `/api/...` path. The API must already be enabled and accessible under your authorized policy.

Steward reads:

```http
GET /api/http/routers?per_page=10001
GET /api/entrypoints?per_page=10001
GET /api/http/services?per_page=10001
```

Provider qualification such as `@file` and `@docker`, native status, hostname aliases, and original native evidence remain distinct. Internal and disabled/error routes are retained. This is not a dashboard app-merging model.

Literal `Host`, `Path`, and `PathPrefix` expressions can be interpreted conservatively. Regex, negation, unknown matchers, and missing dependencies mark coverage partial. Original evidence and source references remain available internally; private raw rule values are not printed by default.

Traefik is paginated. Additional `X-Next-Page` values reject the bounded snapshot instead of pretending it is complete. The three GETs are sequential, not an atomic runtime snapshot.

## Caddy

Steward reads `GET /config/` from the configured admin endpoint. Caddy’s documented default is `localhost:2019`, but the actual listener can be changed or disabled.

```yaml
schema_version: "1"
scope: user
proxies:
  - id: caddy-edge
    engine: caddy
    endpoint: http://127.0.0.1:2019
```

Nested configuration is inspected with native JSON references. Unfamiliar matchers/modules and dynamic handlers remain partial; a readable configuration does not prove controller identity, ownership, reachability, or safe management compatibility.

Caddy JSON changes are validated and conditionally activated. Caddyfile connections use `--caddyfile`, preserving existing imports and adapting the complete candidate. For a Docker controller's real base file, add `--docker-controller`: the controller performs activation. See [management workflows](../management/).

## Authorization

`auth_file` is an absolute path readable by the calling process. Its contents are one complete header value:

```text
Bearer YOUR_READ_ONLY_TOKEN
```

For Basic authentication, use `Basic ` plus the Base64 encoding of `username:password`. Protect the file with appropriate user permissions. The value is read each command, limited to 8 KiB, and omitted from outputs/errors. Credentials are not inherited from Apptrail.

No redirects are followed. API HTTPS uses system CA roots; API endpoints cannot contain URL credentials, query parameters, or fragments. Route verification has a separate `--ca-file` setting for internal frontend TLS. That setting does not change API trust or provide mTLS credentials.

## Unix sockets

On Unix platforms:

```yaml
endpoint: unix:///run/caddy/admin.sock
```

The caller needs socket permissions. Requests use that socket directly, not the host’s network listener. Rootless/container/Desktop paths can differ; Steward does not guess them or change permissions.

## Saved snapshots

Set `snapshot_file` to an explicitly prepared native-response fixture. No network client or authorization file is initialized. Its engine/binding must match the profile. Capture time is source metadata, not evidence of current activation. See the repository’s [snapshot schema](https://github.com/samishal1998/steward/blob/main/schemas/snapshot.schema.json).

## Limits and visibility

Requests are bounded to 12 seconds, with a 30-second inventory deadline per binding. Bodies are capped at 8 MiB and native arrays at 10,000 records. Retained evidence expansion is limited to 32 MiB per binding and 64 MiB per command.

Denied or failed access is a diagnostic, not an empty inventory. Other successful bindings may still be returned as partial results. `proxies discover --probe` reports native/API candidates; `--docker unix:///var/run/docker.sock` adds known-label observations. `proxies inspect-file --engine ENGINE --file FILE` supplies qualified offline native-file evidence.
