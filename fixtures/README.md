# Native-response fixtures

These inputs were written synthetically for Steward; they are not captured from
the developer's host or copied Apptrail source. They model documented API fields,
not certified engine/plugin versions. `captured_at` is illustrative. Offline reads
are always labeled `runtime_confirmed: false` regardless of their age.

- `traefik/snapshot.json`: one native router with two aliases, a disabled Docker
  router, qualified/unqualified service relationships, TLS and TCP entrypoints.
  Both native routers remain distinct inventory records. Disabled is not healthy.
- `caddy/snapshot.json`: inherited host/path constraints through a subroute and a
  reverse-proxy upstream; an unknown matcher/module remains visible and makes
  coverage partial. The reader does not reconstruct a Caddyfile or controller.

The snapshot wrapper is Steward schema v1, not an upstream API. Its `responses`
values are native JSON responses keyed by requested path. It cannot establish
current activation, ownership, reachability, or an approved write integration.

See `docs/APPTRAIL_TECHNICAL_TRANSFER.md` for reference test cases to carry forward,
and `docs/planning/05_TESTING_AND_ACCEPTANCE.md` for the live compatibility gates.
