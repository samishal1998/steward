---
title: Your first inventory
description: Try Steward offline, inspect your proxy, and preview a route without changing infrastructure.
---

## 1. Install the CLI

Follow [installation](../installation/), then check:

```sh
~/.local/bin/steward --help
```

The current alpha performs reads and route previews. `apply` explicitly returns an unavailable-capability error.

## 2. Try synthetic inputs offline

Create a small example directory and download the profile and its two snapshots:

```sh
mkdir -p examples fixtures/traefik fixtures/caddy
curl -fsSL https://samishal1998.github.io/steward/downloads/examples/read-only.yaml -o examples/read-only.yaml
curl -fsSL https://samishal1998.github.io/steward/downloads/fixtures/traefik/snapshot.json -o fixtures/traefik/snapshot.json
curl -fsSL https://samishal1998.github.io/steward/downloads/fixtures/caddy/snapshot.json -o fixtures/caddy/snapshot.json
```

```sh
~/.local/bin/steward --config examples/read-only.yaml proxy list --json
~/.local/bin/steward --config examples/read-only.yaml route list --json
```

These files are synthetic, not your machine’s inventory. The profile reads `snapshot_file` inputs, makes no network requests, and reports `runtime_confirmed: false`. Snapshot paths resolve relative to the profile file.

The example includes Traefik hostname aliases on one native router, a disabled Docker-origin router, a Caddy nested reverse proxy, and a Caddy route with unfamiliar behavior. Unknown coverage stays visible.

## 3. Connect an already authorized proxy API

Create `edge.yaml` for your own Traefik API:

```yaml
schema_version: "1"
scope: user
proxies:
  - id: edge
    engine: traefik
    endpoint: http://127.0.0.1:8080
```

```sh
~/.local/bin/steward --config edge.yaml proxy inspect edge --json
~/.local/bin/steward --config edge.yaml route list --json
```

Use the actual protected API base URL for your deployment. `127.0.0.1` belongs to the calling process’s network namespace. A proxy in another container needs reachable, explicitly configured access. Steward does not enable APIs or grant access automatically.

For Caddy, use `engine: caddy` and its configured admin endpoint. See [proxy connections](../connections/) for paths and authentication.

## 4. Preview route intent

Download the route-only example:

```sh
curl -fsSL https://samishal1998.github.io/steward/downloads/examples/routes-only.yaml -o examples/routes-only.yaml
~/.local/bin/steward --config examples/read-only.yaml plan --file examples/routes-only.yaml --json
```

The result contains proposed actions, source fingerprints, overlap diagnostics, and missing prerequisites. It always says `applicable: false` in this alpha. Nothing is written, reloaded, started, or stopped.

Planning from a saved snapshot cannot establish current conflict absence. [Learn the planning boundary](../planning/) before interpreting the preview as operational authority.
