# Apptrail — Product Requirements Document

**Status:** Draft v0.1  
**Product:** Apptrail  
**Tagline:** Discover, organize, and launch your self-hosted apps.  
**Primary concept:** Infrastructure-aware app registry with a customizable dashboard.

---

## 1. Summary

Apptrail is a self-hosted application hub that automatically discovers services from infrastructure and reverse-proxy configuration, enriches them with metadata, and presents them in a dashboard that can still be manually organized like a traditional home page.

The product is intentionally not "another manually maintained dashboard." Its main value is that the dashboard reflects what is actually deployed.

A user should be able to point Apptrail at one or more providers—Docker, Traefik, Caddy, Nginx, Apache, files, or a database—and receive a continuously reconciled registry of applications. Apptrail then enriches those applications using HTTP metadata, optional labels, and an optional Apptrail discovery manifest served by compatible applications.

Users can pin, hide, rename, regroup, reorder, and lay out the resulting applications without losing the relationship to their discovered source.

---

## 2. Problem

Self-hosted environments accumulate services quickly. The practical problems are usually not "I need a prettier dashboard" but:

- forgetting which services are currently deployed;
- forgetting the route, hostname, or port used by a service;
- manually duplicating routing configuration into a dashboard;
- dashboard entries becoming stale after infrastructure changes;
- losing useful metadata that already exists in Docker labels or proxy configuration;
- lacking a simple convention for an application to expose richer dashboard information;
- wanting manual dashboard layout without giving up automatic discovery.

Existing dashboards solve the presentation layer well, but Apptrail's focus is maintaining a live registry from infrastructure and then making that registry pleasant to browse.

---

## 3. Goals

### 3.1 Primary goals

1. Automatically discover exposed applications from one or more providers.
2. Normalize different provider formats into one common application model.
3. Reconcile duplicate observations of the same application.
4. Enrich entries with useful defaults when explicit metadata is absent.
5. Support explicit metadata and behavior using labels/configuration.
6. Support an Apptrail application discovery manifest for richer integrations.
7. Preserve user layout, grouping, visibility, naming, and favorite overrides.
8. Keep the system useful even when an application has zero Apptrail-specific integration.
9. Ship as a small, self-hostable service with minimal operational requirements.
10. Make provenance visible: users should be able to see where each field came from.

### 3.2 Secondary goals

- health status and lightweight availability checks;
- per-application quick actions;
- optional widgets/views;
- import/export of dashboard configuration;
- multiple dashboards/views over the same registry;
- API access to the normalized registry.

---

## 4. Non-goals for the first release

- full observability/metrics platform;
- service orchestration or deployment;
- replacing Traefik, Caddy, Nginx, Apache, or Kubernetes ingress;
- secrets management;
- arbitrary remote-code execution;
- a plugin marketplace in v1;
- a full browser-based admin panel for each application;
- complex multi-user RBAC in MVP.

---

## 5. Product principles

### 5.1 Discovery first

The system should produce a useful dashboard without requiring application-specific setup.

### 5.2 Explicit metadata beats guesses

When explicit Apptrail metadata is present, it should override inferred metadata.

### 5.3 Manual preferences beat discovery

User changes such as display name, icon override, hide/pin state, categories, and layout are durable and should not be overwritten by future scans.

### 5.4 Providers emit facts, not final objects

Each provider reports observations. A reconciliation layer decides how those observations become stable applications.

### 5.5 Provenance is part of the data model

For important fields, Apptrail should retain the source and confidence so users can understand why an application looks the way it does.

### 5.6 Safe by default

Discovery can touch local infrastructure and HTTP endpoints, so probing must be bounded and explicit. Apptrail must avoid turning metadata enrichment into an unrestricted SSRF primitive.

---

## 6. Core user experience

### 6.1 First run

1. User starts Apptrail.
2. Apptrail opens a setup screen.
3. User enables one or more providers.
4. Apptrail scans those providers.
5. Discovered applications appear automatically.
6. Apptrail enriches them with title, description, favicon, and optional manifest data.
7. User can immediately launch an application.
8. User may later drag/drop, group, hide, favorite, or rename applications.

### 6.2 Dashboard behavior

Default dashboard views:

- **Overview** — curated layout.
- **Apps** — all stable applications.
- **Discover** — newly found / unresolved observations.
- **Providers** — provider state and scan diagnostics.
- **Layout** — edit dashboard arrangement.
- **Settings** — global configuration.

Each application card should expose at minimum:

- icon;
- display name;
- short description/category;
- launch URL;
- health/availability state when enabled;
- source/provider indicator on demand;
- favorite/pin control.

---

## 7. Architecture

```text
                        ┌──────────────────────┐
                        │      Providers       │
                        │ Docker / Traefik /   │
                        │ Caddy / Nginx / ...  │
                        └──────────┬───────────┘
                                   │ observations
                                   ▼
                        ┌──────────────────────┐
                        │ Observation Store    │
                        │ + provider metadata  │
                        └──────────┬───────────┘
                                   │
                                   ▼
                        ┌──────────────────────┐
                        │ Reconciler / Merger  │
                        │ identity + priority  │
                        └──────────┬───────────┘
                                   │ stable app
                                   ▼
               ┌────────────────────────────────────┐
               │        Enrichment Pipeline         │
               │ HTTP metadata / favicon / manifest │
               │ health / icon / category           │
               └──────────────────┬─────────────────┘
                                  │
                                  ▼
                        ┌──────────────────────┐
                        │  Application Registry│
                        └──────────┬───────────┘
                                   │
                ┌──────────────────┴──────────────────┐
                ▼                                     ▼
       ┌─────────────────┐                   ┌─────────────────┐
       │ User Overrides  │                   │ API / Dashboard │
       │ layout / hidden │                   │ search / launch │
       └─────────────────┘                   └─────────────────┘
```

---

## 8. Provider model

All providers implement the same conceptual interface:

```text
Provider
  id() -> ProviderID
  capabilities() -> CapabilitySet
  validate(config) -> ValidationResult
  snapshot(ctx) -> []Observation
  watch(ctx, emit) -> optional event stream
  diagnostics() -> ProviderDiagnostics
```

A provider may be snapshot-only or watch-capable.

### 8.1 Initial provider set

#### Docker provider

Reads containers/services and labels from Docker-compatible APIs.

Potential discovery data:

- container/service name;
- exposed/published ports;
- networks;
- labels;
- health state;
- image name;
- compose project/service labels where available.

Docker alone may not know the public URL, so it is most useful when paired with labels or a reverse-proxy provider.

#### Traefik provider

Traefik's Docker integration is label-driven and its routing configuration exposes the relationship between routers, services, host rules, paths, and backend ports. Apptrail can consume Traefik configuration or infer equivalent information from the same Docker labels.

Expected extraction:

- public hosts;
- route rules;
- entry points/schemes where resolvable;
- router → service mapping;
- backend port;
- TLS state;
- Apptrail labels.

#### Caddy provider

Preferred mechanisms, in order:

1. read Caddy's native JSON configuration through the admin API when explicitly configured;
2. parse a Caddyfile provided by file path;
3. optionally use Docker metadata when the user's Caddy deployment itself derives configuration from Docker.

Expected extraction:

- site addresses;
- reverse-proxy upstreams;
- path matchers;
- transport scheme;
- optional health URI when present.

#### Nginx provider

Reads one or more Nginx configuration files/directories and resolves includes.

Expected extraction:

- `server_name` values;
- listening ports/TLS;
- `location` paths;
- `proxy_pass` targets;
- redirects when they represent an application entry point.

Dynamic configuration that depends heavily on variables may be reported as unresolved rather than guessed.

#### Apache provider

Reads Apache HTTP Server configuration and virtual hosts.

Expected extraction:

- `VirtualHost` addresses;
- `ServerName` / `ServerAlias`;
- reverse-proxy mappings such as `ProxyPass`;
- optional path mappings.

#### File provider

Reads Apptrail-native YAML/JSON files from one file or directory.

Useful for:

- explicitly managed applications;
- services that cannot be discovered from infrastructure;
- generating configuration from another system;
- GitOps workflows.

#### Database/manual provider

The database stores manual applications in addition to dashboard state. Manual applications use the same normalized application model as discovered applications.

The database is not merely a provider: it is also the state store for user overrides, dashboards, placement, favorites, hidden state, and reconciliation identity.

---

## 9. Observation model

Providers emit observations rather than directly mutating applications.

Example conceptual shape:

```json
{
  "provider": "traefik-main",
  "source_id": "docker:container:abc123/router:photos",
  "observed_at": "2026-09-06T18:00:00Z",
  "identity_hints": {
    "apptrail_id": "photos",
    "external_urls": ["https://photos.example.net"],
    "backend": "http://10.0.0.24:2342"
  },
  "facts": {
    "name": "photos",
    "url": "https://photos.example.net",
    "backend_port": 2342,
    "tls": true
  },
  "metadata": {
    "labels": {}
  }
}
```

Each fact may internally carry:

- value;
- source/provider;
- confidence;
- timestamp;
- priority class.

---

## 10. Identity and reconciliation

A stable application must survive provider refreshes and container recreation.

### 10.1 Explicit identity

If available, `apptrail.id` is the strongest cross-provider identity hint.

Example:

```yaml
labels:
  apptrail.id: photos
```

The same ID can be placed in a file entry or discovery manifest to correlate observations.

### 10.2 URL identity

If no explicit ID exists, Apptrail should use normalized external entry points as a strong identity signal.

Normalization should account for:

- lower-case hostnames;
- default ports (`80`, `443`);
- trailing slash normalization;
- base path preservation;
- canonical scheme where known.

`https://example.com/photos` and `https://example.com/notes` must remain separate applications unless explicitly correlated.

### 10.3 Provider-native identity

Container IDs should not be the only durable identity because containers are routinely recreated. Prefer stable names, compose service identity, router names, host/path routes, and explicit Apptrail IDs.

### 10.4 Merge precedence

Recommended precedence for user-visible fields:

1. user override stored in Apptrail;
2. Apptrail-native manifest;
3. explicit Apptrail labels/provider metadata;
4. explicit file/manual provider data;
5. application HTTP metadata;
6. proxy/container-derived defaults;
7. generated fallback.

The exact ordering can be field-specific. For example, a manually selected icon should always remain stable even when a website later changes its favicon.

---

## 11. Automatic HTTP enrichment

Once Apptrail has a launchable URL, it may perform a bounded metadata request.

### 11.1 Default metadata extraction

From HTML:

- `<title>`;
- `meta[name="description"]`;
- Open Graph title/description where useful;
- favicon links;
- web-app manifest icon if available.

Fallback sequence for icon discovery:

1. explicit Apptrail icon;
2. Apptrail manifest icon;
3. page-declared favicon;
4. web app manifest icon;
5. `/favicon.ico`;
6. generated letter/icon fallback.

### 11.2 Probe constraints

Default restrictions:

- GET/HEAD only;
- short connect and response timeout;
- strict response-size cap;
- no JavaScript execution;
- bounded redirect count;
- DNS/IP revalidation across redirects;
- configurable private-network policy;
- block link-local/cloud metadata targets by default unless explicitly allowed;
- no credentials sent unless the user explicitly configures them.

---

## 12. Apptrail discovery manifest

Compatible applications may expose richer metadata from a conventional endpoint:

```text
/.well-known/apptrail.json
```

This keeps the integration independent from a particular framework and avoids requiring a custom API client.

### 12.1 Minimal manifest

```json
{
  "version": "1",
  "id": "immich",
  "name": "Photos",
  "description": "Personal photo library",
  "icon": "/app-icon.svg",
  "category": "media"
}
```

### 12.2 Rich manifest

```json
{
  "version": "1",
  "id": "home-assistant",
  "name": "Home Assistant",
  "description": "Home automation",
  "icon": "/static/apptrail.svg",
  "health": {
    "url": "/api/health",
    "method": "GET",
    "expected_status": [200]
  },
  "actions": [
    {
      "id": "logs",
      "label": "Logs",
      "url": "/logs"
    }
  ],
  "views": [
    {
      "id": "summary",
      "label": "Summary",
      "type": "iframe",
      "url": "/apptrail/summary"
    }
  ]
}
```

### 12.3 Manifest rules

- manifest must be versioned;
- relative URLs resolve against the application's public base URL;
- no secrets in the manifest;
- remote cross-origin resources require explicit user permission;
- unsupported fields are ignored for forward compatibility;
- unknown major versions are not interpreted.

---

## 13. Docker/Apptrail labels

Recommended namespace:

```text
apptrail.*
```

### 13.1 Core labels

```yaml
labels:
  apptrail.enable: "true"
  apptrail.id: "immich"
  apptrail.name: "Photos"
  apptrail.description: "Personal photo library"
  apptrail.category: "media"
  apptrail.icon: "https://example.net/icon.svg"
```

### 13.2 Routing hints

```yaml
labels:
  apptrail.url: "https://photos.example.net"
  apptrail.path: "/"
```

These are hints/overrides; when a reverse-proxy provider already supplies the public URL they are usually unnecessary.

### 13.3 Health

```yaml
labels:
  apptrail.health.url: "/api/health"
  apptrail.health.method: "GET"
```

### 13.4 Discovery manifest override

```yaml
labels:
  apptrail.manifest: "/.well-known/apptrail.json"
```

### 13.5 Inline metadata

For small static data, labels may supply individual fields. Large JSON blobs should not be encouraged in labels. A manifest URL or file is easier to validate and maintain.

Secrets must never be placed in labels.

---

## 14. File provider format

Example:

```yaml
version: 1
apps:
  - id: grafana
    name: Grafana
    url: https://grafana.example.net
    category: observability
    icon: https://grafana.example.net/public/img/grafana_icon.svg

  - id: internal-admin
    name: Internal Admin
    url: http://10.10.0.5:8080
    hidden: false
```

Directories may contain multiple files. Apptrail merges them as one provider snapshot while preserving file/line provenance for diagnostics.

---

## 15. Health model

Health is optional and should be distinct from discovery.

States:

- `unknown` — not checked;
- `checking`;
- `healthy`;
- `degraded`;
- `unhealthy`;
- `unreachable`.

Potential inputs:

- explicit Apptrail health endpoint;
- Docker health state;
- provider/backend health where exposed;
- simple launch-URL probe.

A service being hidden from the dashboard must not implicitly disable health checks if the user has configured them separately.

---

## 16. Widgets, embedded views, and micro-frontends

This is a post-MVP capability but should be represented in the model early.

Supported conceptual view types:

- `link` — launch another route;
- `iframe` — embedded application page;
- `json` — Apptrail-rendered data contract;
- `microfrontend` — optional future contract.

### Security requirements for iframe views

- sandbox by default;
- minimum required sandbox flags;
- explicit opt-in for same-origin/script-heavy embedding;
- clear origin shown in settings;
- Content Security Policy compatible design;
- never inject application HTML directly into the Apptrail DOM.

A JSON-rendered widget format is safer and should likely precede a general micro-frontend contract.

---

## 17. Persistence

### 17.1 Default store

Use an embedded relational database for the default single-instance deployment. SQLite is an appropriate reference choice because it supports durable local state without requiring an external database.

Potential tables:

- `providers`;
- `provider_observations`;
- `applications`;
- `application_endpoints`;
- `application_field_sources`;
- `application_overrides`;
- `dashboards`;
- `dashboard_items`;
- `groups`;
- `favorites`;
- `health_checks`;
- `health_samples` (bounded/optional);
- `settings`.

### 17.2 Future external database support

An external SQL backend may be added later for HA/multi-instance deployments, but should not be required for MVP.

---

## 18. Application model

Conceptual normalized object:

```json
{
  "id": "app_01...",
  "stable_key": "immich",
  "name": "Photos",
  "description": "Personal photo library",
  "category": "media",
  "icon": {
    "url": "https://photos.example.net/favicon.svg",
    "source": "http_metadata"
  },
  "endpoints": [
    {
      "id": "primary",
      "url": "https://photos.example.net",
      "kind": "public",
      "primary": true
    }
  ],
  "health": {
    "state": "healthy"
  },
  "sources": [
    {
      "provider": "traefik-main",
      "source_id": "router:photos"
    },
    {
      "provider": "docker-main",
      "source_id": "compose:immich/server"
    }
  ],
  "user": {
    "favorite": true,
    "hidden": false
  }
}
```

---

## 19. API

MVP API can be REST/JSON.

Suggested endpoints:

```text
GET    /api/apps
GET    /api/apps/:id
PATCH  /api/apps/:id/overrides
POST   /api/apps/:id/refresh
GET    /api/providers
POST   /api/providers
PATCH  /api/providers/:id
POST   /api/providers/:id/scan
GET    /api/providers/:id/diagnostics
GET    /api/dashboards
POST   /api/dashboards
PATCH  /api/dashboards/:id
PUT    /api/dashboards/:id/layout
GET    /api/events            # SSE or WebSocket later
```

Internal provider interfaces should remain independent of the HTTP transport.

---

## 20. Search and organization

Search fields:

- name;
- aliases;
- description;
- category/tags;
- hostname;
- provider/source;
- backend service name.

Dashboard organization:

- drag/drop placement;
- groups/sections;
- favorites;
- hide/unhide;
- multiple dashboards later;
- responsive grid positions stored independently per breakpoint if needed.

---

## 21. Provider configuration

Conceptual configuration:

```yaml
providers:
  - id: docker-main
    type: docker
    endpoint: unix:///var/run/docker.sock
    watch: true

  - id: traefik-main
    type: traefik
    source: docker-labels
    docker_provider: docker-main

  - id: caddy-main
    type: caddy
    admin_api: http://127.0.0.1:2019

  - id: nginx
    type: nginx
    paths:
      - /etc/nginx/nginx.conf

  - id: apps-files
    type: file
    paths:
      - /etc/apptrail/apps.d
```

Credentials should be referenced through secret files/environment/secret backends, not stored inline where avoidable.

---

## 22. Provider diagnostics

Each provider page should show:

- last successful scan;
- scan duration;
- number of observations;
- warnings;
- parse errors;
- unreachable dependencies;
- unresolved routes;
- permissions errors;
- latest provider version/capabilities known to Apptrail.

Diagnostics are important because reverse-proxy configuration can contain dynamic constructs that are impossible to resolve statically.

---

## 23. Security

### 23.1 Docker socket

Direct Docker socket access is highly privileged. Apptrail documentation should recommend the minimum practical access pattern and clearly explain the risk. Where deployments use an API proxy, Apptrail should support a restricted endpoint.

### 23.2 Reverse-proxy admin APIs

Admin APIs such as Caddy's must not be exposed publicly just for Apptrail. Prefer localhost/private networking, Unix sockets, or protected endpoints.

### 23.3 HTTP enrichment / SSRF

See probe constraints above. The application must treat discovered URLs as untrusted input.

### 23.4 Embedded content

Embedded application content should remain isolated from Apptrail's own origin and credentials.

### 23.5 Secrets

Never encourage certificates, API tokens, passwords, or credentials in Docker labels or discovery manifests.

---

## 24. UX details

### 24.1 Newly discovered apps

New applications should appear automatically but can receive a subtle "new" badge until acknowledged.

Optional user setting:

```text
Newly discovered apps:
  [x] Add to Apps automatically
  [ ] Add to Overview automatically
  [ ] Require approval before showing
```

Default recommendation:

- immediately appear in **Apps**;
- do not aggressively rearrange the user's curated **Overview** layout;
- show a Discover badge/counter.

### 24.2 Disappearing apps

Do not delete immediately.

Suggested lifecycle:

1. present;
2. missing;
3. stale after configurable grace period;
4. archived;
5. manually forgotten.

This prevents dashboard churn during restarts and deployments.

### 24.3 Conflicts

When two explicit sources disagree, Apptrail should:

- apply deterministic precedence;
- show the effective value;
- make the conflicting sources inspectable;
- allow a user override.

---

## 25. MVP scope

### MVP 0 — vertical slice

- single instance;
- embedded database;
- Docker provider;
- Traefik/Docker-label route discovery;
- common observation model;
- reconciliation;
- HTTP title/description/favicon enrichment;
- Apptrail labels;
- `/.well-known/apptrail.json` manifest;
- grid dashboard;
- favorites, hide, rename, icon override;
- persistent drag/drop layout;
- basic provider diagnostics;
- simple health state.

### MVP 1

- Caddy provider;
- Nginx provider;
- file provider;
- multiple dashboard sections;
- event-driven/watch refresh where providers support it;
- stronger merge/conflict UI.

### MVP 2

- Apache provider;
- JSON widgets;
- iframe views;
- import/export;
- optional external DB;
- multi-user support if demand justifies it.

---

## 26. Suggested milestone plan

### Milestone A — core registry

- provider interface;
- observation schema;
- SQLite persistence;
- application identity/reconciliation;
- CLI/provider diagnostics.

### Milestone B — Docker + Traefik

- Docker watch/snapshot;
- Traefik label parser;
- URL/port extraction;
- Apptrail label namespace;
- first stable application records.

### Milestone C — enrichment

- bounded HTTP probe;
- HTML metadata;
- favicon selection;
- Apptrail manifest;
- health state.

### Milestone D — dashboard

- app grid;
- launch;
- search/filter;
- favorites;
- hidden state;
- drag/drop layout;
- provider/source inspector.

### Milestone E — second provider family

- Caddy admin JSON provider;
- Nginx config provider;
- file provider.

---

## 27. Open decisions

1. Implementation language/runtime.
2. Whether the initial Traefik provider reads Docker labels only, Traefik API/config, or both.
3. Whether all discovered applications automatically enter Overview or only Apps.
4. Exact schema for widgets/views.
5. Auth model for the Apptrail UI/API.
6. Whether health samples are stored historically or only current state is retained.
7. Exact conflict-resolution rules for each field.
8. Whether Kubernetes ingress/gateway becomes a first-class provider after MVP.

---

## 28. Design direction

The selected Apptrail visual direction is:

- dark, atmospheric, high-contrast UI;
- warm orange sunset/mountain accent palette;
- glassy but restrained panels;
- strong application-grid readability;
- rounded geometry;
- minimal line iconography;
- orange used as the primary active/action color;
- status colors reserved for state rather than branding;
- a light companion theme using the same layout and brand mark.

The canonical Apptrail mark must be treated as a locked asset once exported. Theme variants may change surrounding color treatment, but not the mark geometry.

---

## 29. Verified implementation references

These references were checked while drafting the provider design:

- Traefik Docker routing/provider documentation: https://doc.traefik.io/traefik/reference/routing-configuration/other-providers/docker/
- Caddy admin API: https://caddyserver.com/docs/api
- Caddy `reverse_proxy`: https://caddyserver.com/docs/caddyfile/directives/reverse_proxy
- Nginx `ngx_http_proxy_module` / `proxy_pass`: https://nginx.org/en/docs/http/ngx_http_proxy_module.html
- Apache HTTP Server `mod_proxy`: https://httpd.apache.org/docs/2.4/mod/mod_proxy.html

Relevant verified behavior:

- Traefik's Docker provider uses Docker labels for routing configuration and explicitly warns against storing sensitive data in labels.
- Caddy exposes native JSON configuration through its admin API and supports reverse-proxy upstream and health configuration.
- Nginx reverse-proxy targets are represented through `proxy_pass` in HTTP configuration.
- Apache reverse-proxy mappings can be expressed with `ProxyPass`/`ProxyPassReverse`.

---

## 30. Success criteria

Apptrail is successful when a user can deploy a new service behind supported infrastructure and, with no Apptrail-specific setup, see a useful launchable entry appear automatically within the provider refresh window.

A fully integrated application should be able to supply richer metadata through labels or a discovery manifest without requiring Apptrail-specific code inside the dashboard itself.
