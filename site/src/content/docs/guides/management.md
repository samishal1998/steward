---
title: Management workflows
description: Checked plans, TLS, native services, proxy lifecycle, remote targets, recovery, and AppTrail exports.
---

These workflows are implemented on **`feature/complete-management`**, version
**0.2.0-dev.1**. Build that branch to use them. The public installer remains pinned
to the released alpha.2 until a new release is finalized.

## Find and maintain connections

```sh
steward proxies discover --probe
steward proxies discover --docker unix:///var/run/docker.sock
steward proxies inspect-file --engine traefik --file /absolute/dynamic/routes.yaml
steward proxies update edge --engine traefik --endpoint http://127.0.0.1:8080
steward proxies remove edge
```

Discovery reports process, known-path, API and known Docker-label evidence; it does
not enable APIs or authorize writes. Native-file inspection is explicitly offline.
Connection updates are full replacements. An owned route prevents repointing its
connection. `proxies detach ID` explicitly relinquishes the connection and its
route receipts while retaining native inputs; it is different from uninstall.

## Preview a manifest, then apply that exact plan

With registered writable connections:

```sh
steward plan --file routes.yaml --out "$PWD/reviewed-plan.json" --json
steward apply --plan "$PWD/reviewed-plan.json" --yes --json
```

The saved plan contains intent and fingerprints, not private native backups.
Apply checks owner, state, authoritative input and native fingerprints again. It
upserts only supplied IDs and never prunes omitted routes. Multi-proxy application
is sequential: a failure reports already applied operation IDs for inspection or
rollback. `--config PROFILE plan --file …` remains a read-only profile preview.

## Caddyfiles and existing imports

```sh
steward proxies add edge --engine caddy --endpoint http://127.0.0.1:2019 \
  --config-file /absolute/Caddyfile --caddyfile --listener 127.0.0.1:8088 \
  --caddy-bin /absolute/caddy
```

Steward appends its own marked site section, preserves existing text/imports,
adapts and validates the complete candidate, and conditionally activates native
JSON. Imported-input changes invalidate the recorded native fingerprint.

For a **caddy-docker-proxy base file**, add `--docker-controller`. The controller
must already use that exact base path. Steward publishes base content and waits
for the controller; it never overwrites generated runtime JSON. The first live
controller acceptance is pinned to 2.13.1 and includes timer-only activation,
later Docker events, restart and label preservation.

## HTTPS routes

Choose `--tls internal` or `--tls public` on `routes add`. The selected listener
and native certificate setup must match the policy.

```sh
steward proxies add secure --engine caddy --endpoint http://127.0.0.1:2019 \
  --config-file /absolute/Caddyfile --caddyfile --listener 127.0.0.1:8443 \
  --caddy-bin /absolute/caddy --ca-file /absolute/local-root.crt

steward routes add app --proxy secure --host app.localhost \
  --upstream http://127.0.0.1:3000 --tls internal
```

Caddy internal issuance uses the native CA. A Caddy JSON connection needs an
explicit TLS listener and enabled automatic HTTPS. Traefik public TLS uses a
registered `--cert-resolver` present in its startup configuration. Traefik internal
TLS uses `--certificate`, `--private-key`, and `--ca-file` references.

Verification uses the requested hostname for SNI and certificate validation, with
a direct connection to the selected listener. Steward does not install trust
roots or disable certificate validation. Public ACME requires correct external
DNS/challenge reachability; local acceptance does not establish those conditions.

## Install, set up, upgrade, and uninstall a proxy

```sh
steward proxies install local --engine caddy --version 2.11.7 \
  --listen 127.0.0.1:8088 --admin 127.0.0.1:22019
steward proxies upgrade local --version 2.11.7
steward proxies uninstall local
```

Install verifies the architecture-specific upstream archive against its checksum,
creates isolated config/data, and uses the OS service manager. For an existing
binary, use `proxies setup … --binary /absolute/caddy`. Setup defaults to HTTP;
`--tls internal` or `--tls public` selects TLS setup. Traefik public setup requires
`--acme-email`; its internal setup requires certificate/key/CA paths. User scope is
the default; `--scope system` requires an explicitly elevated invocation.

An upgrade preserves config and verifies startup/readiness. An interrupted upgrade
uses `proxies recover ID`. Uninstall removes the owned service and downloaded
binary, while retaining config/certificate/data files. Remove or detach owned
routes first. External executables stay with their original installer.

## General native services

```sh
steward services add web --executable /absolute/python3 \
  --working-directory /absolute/site --ready-url http://127.0.0.1:3000/ \
  --start -- -m http.server 3000 --bind 127.0.0.1
steward services inspect web
steward services enable web
steward services restart web
steward services logs web --lines 100
steward services disable web
steward services stop web
steward services remove web
```

`services update` takes the complete replacement specification. `--env NAME=VALUE`,
`--restart`, `--ready-timeout 30` (1–300 seconds), and `--scope user|system` configure the native definition. Environment
values and command arguments are omitted from inspection/export output. Put global
`--json` before the final `--` executable-argument separator.

systemd and launchd supervise the app; Steward has no supervisor daemon. Definitions
must still match their ownership fingerprints. `services recover` compensates an
unfinished service operation. Completed service operations can be reversed with
`operations rollback ID` while registry, definition, running state and native enablement still match.

Use `services discover --scope user` and `services inspect-native NAME` for
read-only observations of external native services; discovery does not adopt them.

## Service-first up, route-first down

Use a version-1 stack with `id`, `scope: user`, a `services` array and a `routes`
array. Each route entry has a `spec` (the existing route specification) and a
`depends_on` list of service IDs. Every dependency must provide a readiness URL.

[Download the stack example](../../downloads/examples/service-stack.json), then
replace its two absolute paths and select your registered proxy ID. The executable
and working directory must exist on the target where you run Steward.

```sh
steward up --file service-stack.json
steward down demo
steward down demo --remove-services
```

Up starts/waits for services before publishing routes. Down removes the stack's
routes first and retains services by default. `--remove-services` removes only
services created by that stack, after checking that their specifications have not
changed. Partial results persist a stack receipt and retain individual operation
evidence. Recover unfinished native operations before continuing down.

## SSH and container targets

```sh
steward --ssh user@server --remote-bin /usr/local/bin/steward routes list --json
steward --container proxy --remote-bin /usr/local/bin/steward routes list --json
```

The target must already have a compatible Steward executable. All forwarded paths,
state, native managers and loopback routing checks belong to that target. SSH uses
batch authentication and the normal host-key policy; arguments are quoted as data.
Container execution uses argv through `docker exec`. A timed-out mutation is
reported as potentially changed, so inspect operations on the target before retrying.

## Adoption, history, and rollback

```sh
steward routes inspect NATIVE_ID --proxy edge --json
steward routes adopt app --proxy edge --native-id NATIVE_ID \
  --expected-fingerprint INSPECTED_FINGERPRINT
steward operations list --json
steward operations inspect OPERATION_ID --json
steward operations rollback OPERATION_ID --json
```

Adoption requires an exactly representable native HTTP route. Caddy JSON adoption
checks its direct native definition. Traefik additionally requires `--source` for
a dedicated single-router/service file inside the registered watched directory,
using a new connection with no owned routes. Arbitrary middleware, rewrites and
complex external definitions remain external.

History retains up to 1024 operations / 256 MiB per route/service history directory;
limits stop new operations rather than silently deleting history. Rollback refuses
newer state/source/runtime changes. `operations recover` remains for unfinished
route writes; it is not a forced overwrite of drift.

## Dynamic completion and terminal UI

```sh
source <(steward completions bash --dynamic)
steward tui
```

Dynamic completion reads only saved local IDs and never contacts a proxy or creates
state. Use `STEWARD_STATE_DIR` or include `--state-dir` in the command being completed.
The TUI provides Routes, Proxies, Services and Operations views: Tab changes views,
arrows select, Enter inspects, `r` refreshes, `:` runs a checked Steward command, and
`q` exits. It calls the same core logic rather than a shell.

## AppTrail integration

```sh
steward export --out /absolute/shared/steward.json
```

In AppTrail's matching feature branch, choose the **Steward** provider and enter
`file:///absolute/shared/steward.json`. Share only the export, not private state.
Configure suitable file permissions for the AppTrail process/container. Re-run or
explicitly schedule exports within 15 minutes; no exporter daemon is installed.

The versioned snapshot preserves host/state identity, scope, kind, native source
coverage, desired enablement and observed status. Background services get no invented
URL. Partial/stale/invalid snapshots retain prior AppTrail observations and user
preferences. This is a read-only integration; dashboard edits do not become
infrastructure commands.
