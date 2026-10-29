---
title: Get started
description: Inspect an existing proxy, put a local app behind a hostname, and change routes with CLI commands.
---

Install [Steward](../installation/) and check for **v0.2.0-alpha.2** with `steward --version`. Add your local bin directory to `PATH`, or replace `steward` below with `~/.local/bin/steward`.

## 1. See what your proxy is serving

Connect an already authorized API once. You do not need a Steward YAML profile:

```sh
steward proxies add edge --engine traefik --endpoint http://127.0.0.1:8080
steward proxies list
steward routes list
steward routes inspect photos@file --proxy edge
```

For Caddy, use `--engine caddy` and its actual admin endpoint. API-only connections are inventory-only: they do not grant permission to change files or configure the proxy. `photos@file` is an example; use the native ID shown by your own route list.

Steward remembers connections in your private state directory. `--state-dir /absolute/path` or `STEWARD_STATE_DIR` selects a separate workspace.

## 2. Put a local app behind a hostname

This example uses a **local HTTP-only Caddy server** and an application already listening on `127.0.0.1:3000`. It needs an existing Caddy executable with conditional configuration updates; the live checks use Caddy **2.11.7**.

For a disposable demo, download the small native Caddy configuration:

```sh
curl -fsSL https://samishal1998.github.io/steward/downloads/examples/caddy-http.json -o caddy-http.json
caddy run --config "$PWD/caddy-http.json"
```

Keep Caddy running. In another terminal, start your app. A simple static example is:

```sh
curl -fsSL https://samishal1998.github.io/steward/downloads/examples/static-server.py -o static-server.py
python3 static-server.py --port 3000 --bind 127.0.0.1
```

In a third terminal, connect the existing proxy with explicit file authority, then add the route:

```sh
steward proxies add local --engine caddy \
  --endpoint http://127.0.0.1:22019 \
  --config-file "$PWD/caddy-http.json" --listener web

steward routes add app --proxy local \
  --host app.localhost --upstream http://127.0.0.1:3000 --tls http

curl --resolve app.localhost:8088:127.0.0.1 http://app.localhost:8088/
```

**`routes add` makes a live change.** Steward validates the full candidate, records a recoverable operation, publishes the source atomically, activates it, and verifies the native route plus an HTTP request through the proxy. No route-intent YAML or separate `apply` command is needed. The upstream root path must return HTTP 2xx/3xx for verification.

For your existing JSON-based Caddy server, use its actual startup file, server name, and endpoint. This HTTP example requires one local listener and `automatic_https.disable: true`. The [management guide](../management/) covers Caddyfiles/imports, TLS, Docker controllers, proxy installation, and SSH/container targets.

## 3. Move, pause, or remove your app

Start the replacement app on port 4000 before changing the upstream:

```sh
steward routes update app --proxy local --upstream http://127.0.0.1:4000
steward routes inspect app --proxy local
steward routes disable app --proxy local
steward routes enable app --proxy local
steward routes remove app --proxy local
```

These commands change only receipt-owned routes in this Steward state directory. Existing external routes keep their native identity; explicit checked adoption is a separate command. A matching name or hostname is not an ownership receipt.

## 4. Use a Traefik watched directory

For an already running local Traefik whose static startup file enables this exact watched directory and HTTP entrypoint:

```sh
steward proxies add local-traefik --engine traefik \
  --endpoint http://127.0.0.1:8080 \
  --config-file "$HOME/.config/traefik/traefik.yaml" \
  --directory "$HOME/.config/traefik/dynamic" --listener web

steward routes add app --proxy local-traefik \
  --host app.localhost --upstream http://127.0.0.1:3000 --tls http
```

Steward uses its own aggregate file and leaves other provider files alone. It waits for native router/service activation and checks routing through the selected entrypoint. Removing the final enabled route removes the owned aggregate file. The live checks use Traefik **3.7.14**.

## If a change is interrupted

```sh
steward operations list --json
steward operations recover OPERATION_ID --json
```

A failed activation is compensated when the published input and runtime still match the operation. An unfinished operation blocks further writes. Use `recover` for an interrupted route write, or `operations rollback ID` to reverse a verified operation while state/source/native checks still match. Inspect the diagnostic and operation ID before retrying.

## Try offline output instead

The [saved-response examples](../planning/) still work with `--config` and `plan --file`. Those are read-only previews and always return `applicable: false`.

Next: [management workflows](../management/) · [shell completion](../completion/) · [CLI reference](../../reference/cli/) · [agent guide](../agents/).
