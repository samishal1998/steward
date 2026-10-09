# Steward — Proposed configuration examples

These YAML files are **design fixtures** for Steward's future input schema. They are not working configuration for an existing CLI, AppTrail, Caddy, or Traefik. The implementation agent must freeze the schema after compatibility spikes, implement validators, and test these fixtures against them. YAML and equivalent JSON are the user-facing formats.

| Example | Purpose |
|---|---|
| [steward.routes-only.yaml](steward.routes-only.yaml) | Expose an already running process through an already approved native proxy. No service management. |
| [steward.services-only.yaml](steward.services-only.yaml) | Run a background worker through the native user manager. No route, port, Docker, proxy, or dashboard required. |
| [steward.combined.yaml](steward.combined.yaml) | Start a managed application, pass readiness, then expose its named endpoint. |

## 1. Common contract

`schema_version` is an explicit string major version. `scope` identifies one user or system management scope. All resources and proxy bindings in an initial combined request must be usable within that scope. Cross-scope composition is not silently escalated.

`services` and `routes` are independent optional collections. An absent collection or an empty collection requests no changes to that capability; it must not delete already managed resources. Each supplied resource is a complete proposed specification for that resource ID. Applying a subset is an upsert of those explicit IDs, **not a prune of the host's inventory**. Deletion uses explicit remove actions/plans. A future prune feature needs a separate bounded selector, preview, and approval.

The imported document is a planning input. It is not a second live authority or a continuously watched GitOps source. Successful changes are committed to Steward's canonical desired state and receipts. Repeated identical input should produce a no-op plan.

Reject duplicate keys/IDs, unknown mutation fields, invalid scalar types, unknown major versions, invalid hostnames/ports, unsupported features, and unsafe paths. Do not silently coerce arbitrary strings into booleans or interpret YAML tags as code. The agent will publish the exact JSON Schema and semantic validations after M1.

## 2. Route interpretation

`proxy` refers to an existing approved binding stored in Steward's connection configuration. It is not an instruction to install an engine or infer a provider from a name. Missing integration yields `setup.required` before changes.

`upstream` is exactly one of an explicit URL or an explicit managed service endpoint reference. A URL refers to the reachable upstream from the chosen proxy's network context; a host-loopback URL is appropriate only for the shown native-proxy recipe. Container proxies require separately verified mappings. Scheme, authority, port, and TLS verification matter.

The first writable route subset uses one exact hostname and one HTTP/HTTPS origin upstream, preserving request paths. No arbitrary provider-specific matcher expression or upstream path rewrite is accepted in these examples. Hostnames and addresses are placeholders; Steward does not create DNS records.

`tls.mode: public` expresses public HTTPS intent. It requires the proxy's actual certificate/entrypoint configuration and DNS/challenge prerequisites. It cannot silently add a Traefik resolver or install trust. The finalized schema may also provide explicit HTTP-only and local/internal policies, with capability checks and clear security effects.

## 3. Service interpretation

`manager: auto` selects a supported native manager after verified detection; it does not pretend all init systems share systemd semantics. A mismatch or unavailable manager is an explicit capability failure.

`command` is an executable-plus-argument array, not shell text. The example paths are Linux-oriented placeholders. On macOS or another installation, choose real absolute executable/workdir paths rather than assuming an interactive version-manager shell is available.

`state: running` requests immediate runtime convergence during apply. It does not imply an ongoing Steward reconciliation daemon. `startup` expresses a distinct next-login/boot policy; `restart` is mapped only to native behavior the adapter supports. Unsupported scope/policy combinations are rejected, not approximated invisibly.

User-scoped jobs run under the selected user manager. A system-scoped application must have an explicit non-root `run_as` account unless privileged execution is deliberately approved. The example account must already exist; account creation belongs to explicit setup, not a hidden service side effect.

Environment values are explicit strings. These fixtures contain no secrets. The implementation must protect/redact sensitive environment data and must not import the whole invoking shell environment by default.

A declared endpoint describes expected behavior; it does not make the app listen on that port. The combined example assumes the application reads `HOST`/`PORT` and provides `/healthz`. Adapt the command/environment to the application's real interface. A worker can have no endpoints.

## 4. Combined behavior

The combined example requires both service and route preflight to pass before mutation. It creates/updates the service, starts it, performs the bounded readiness check, and only then publishes the route. `operation.on_failure: preserve_service` records the proposed default: if routing fails after startup, keep the service for diagnosis, compensate only safe owned routing input, and return partial non-success.

This is one-time readiness gating. Future application crashes, restart ordering after reboot, and continuous health-based route suppression are not controlled by a hidden Steward daemon.

The `operation` block configures this request's execution policy; it is not an application environment setting, an AppTrail metadata object, or a persistent background job.

## 5. Proposed command flow

The command interface below is a design to implement, not an existing installation instruction:

```sh
steward plan --file ./steward.combined.yaml --out ./plan.json
steward apply --plan ./plan.json --yes --json
```

Planning must reveal effects, privilege requirements, verification limits, and native/source fingerprints. Apply revalidates them. `--yes` accepts the described ordinary plan; it never bypasses stale-source, scope, setup, ownership, or capability checks.

Connection-profile files are intentionally not supplied as if real AppTrail/Caddy/Docker schemas were known. The explicit setup flow must create validated Steward bindings from the actual deployment. The AppTrail agent's completed technical transfer will supply current AppTrail connection knowledge separately.
