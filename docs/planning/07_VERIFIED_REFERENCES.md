# Steward — Verified references and research qualifications

**Research date:** October 8, 2026.  
**Purpose:** distinguish upstream facts, historical project material, proposed Steward design, and behaviors still requiring experiments.  
**Important:** documentation/source inspection is not execution testing. No AppTrail repository audit or live Steward/proxy/native-manager test is claimed in this pack.

## 1. How to use these references

`[S01]`–`[S28]` identify official vendor/project documentation or upstream project source inspected for the design. Versionless documentation, `latest` package references, and moving source branches can change. The implementing agent must pin supported versions, recheck the relevant behavior, and add immutable source links or captured evidence before declaring compatibility.

`[A01]` identifies the supplied historical AppTrail draft. It supports statements about the earlier design, not current source code, implemented schemas, language choice, or available endpoints.

The CLI commands, Rust port names, resource schemas, paths, status vocabulary, and milestones proposed elsewhere are Steward design contracts—not APIs asserted to exist in an upstream product.

## 2. Proxy and Docker references

| ID | Primary source | What it supports / qualification |
|---|---|---|
| S01 | [Traefik configuration overview](https://doc.traefik.io/traefik/getting-started/configuration-overview/) | Distinction between install/startup configuration and routing configuration, historically static/dynamic. Startup sources and provider enablement must be handled explicitly; ordinary routing changes use the configured dynamic mechanism. |
| S02 | [Caddyfile `import`](https://caddyserver.com/docs/caddyfile/directives/import) | Files/globs/snippets are included during configuration parsing. An import is not itself a background watcher. |
| S03 | [Caddy command-line interface](https://caddyserver.com/docs/command-line) | Adaptation, validation, reload, and relevant provisioning behavior. Separate validation may encounter resources held by a running process; do not promise completely side-effect-free validation. |
| S04 | [caddy-docker-proxy upstream README](https://github.com/lucaslorentz/caddy-docker-proxy) | Compiled Docker extension, `caddy docker-proxy`, label-derived configuration, configured base Caddyfile, controller modes, and polling options. Verify exact plugin release and deployment mode. |
| S05 | [caddy-docker-proxy generator source](https://raw.githubusercontent.com/lucaslorentz/caddy-docker-proxy/master/generator/generator.go) | Inspected generator reads the configured base file during generation. A moving branch is not proof of behavior in every installed build. |
| S06 | [caddy-docker-proxy loader source](https://raw.githubusercontent.com/lucaslorentz/caddy-docker-proxy/master/loader.go) | Inspected loader includes periodic regeneration and generated-content comparison before application. File-only refresh, imported-file behavior, timing, and error handling need pinned-release tests. |
| S07 | [Traefik file provider](https://doc.traefik.io/traefik/reference/install-configuration/providers/others/file/) | Watched directory/file configuration and filesystem-event limitations, including container bind mounts and parent-directory guidance. |
| S08 | [Docker object labels](https://docs.docker.com/engine/manage-resources/labels/) | Container labels are static over a container's lifetime; recreating is required to change them. Some other Docker object types, including Swarm services, differ. Steward's read-only label policy is broader than this technical limitation. |
| S09 | [Protect access to the Docker daemon](https://docs.docker.com/engine/security/protect-access/) | Docker daemon access is privileged and requires an appropriate transport/access policy. Treating a read-only socket mount as a method filter would not provide that policy. |
| S10 | [Traefik Docker provider](https://doc.traefik.io/traefik/reference/install-configuration/providers/docker/) | Docker connection/discovery options and exposure policy. `exposedByDefault: false` is the proposed safe default for newly provisioned integrations, not permission to change existing policy. |
| S11 | [Traefik API and dashboard](https://doc.traefik.io/traefik/reference/install-configuration/api-dashboard/) | Read/diagnostic endpoints including HTTP routers, services, and raw configuration/error information. Endpoint access/base path and security depend on configuration. No generic route-write API is assumed. |
| S12 | [Docker Desktop networking](https://docs.docker.com/desktop/features/networking/) | Desktop host-access behavior, including `host.docker.internal`; not a universal guarantee that any host-bound application is reachable. |
| S20 | [Caddy automatic HTTPS](https://caddyserver.com/docs/automatic-https) | Public/local certificate behavior and prerequisites. Configuring a route is not the same as provisioning DNS or installing local CA trust. |
| S21 | [Traefik ACME certificate resolvers](https://doc.traefik.io/traefik/reference/install-configuration/tls/certificate-resolvers/acme/) | Resolver configuration and challenge requirements. Reuse or explicitly set up a resolver rather than inventing one in an ordinary route write. |
| S22 | [Docker container run reference](https://docs.docker.com/reference/cli/docker/container/run/) | Explicit host mapping including `host-gateway`. This is runtime setup, not a magic replacement for interface/firewall reachability. |
| S23 | [Extending Caddy](https://caddyserver.com/docs/extending-caddy) | Caddy extensions are compiled into a build. Pair with the plugin's build instructions; verify custom artifact provenance and included modules. |
| S24 | [Caddy admin API](https://caddyserver.com/docs/api) | Config inspection/mutation, adaptation/load operations, and documented ETag/If-Match concurrency control. Does not authorize competing with an external configuration controller. |

### Specific integration conclusions

A **file-backed management policy** does not imply identical activation mechanisms. Plain Caddy loads the approved complete configuration; the Docker controller composes its input; Traefik consumes its configured watched files. The product architecture assigns one effective configuration writer/controller to each mode.

The inspected Caddy Docker source supports investigating a managed base-file contribution, but it does not justify claiming that every imported-file edit is detected in every build. That behavior is an explicit release gate. Do not trigger refresh by fabricating Docker events or recreating application containers.

The design deliberately avoids claiming a universal Traefik validation command. The implementation must validate what it can before publication and then observe actual activation, documenting the limits.

## 3. Native service-manager references

| ID | Primary source | What it supports / qualification |
|---|---|---|
| S13 | [systemctl manual source](https://raw.githubusercontent.com/systemd/systemd/main/man/systemctl.xml) | Native lifecycle/state/enablement and manager-reload concepts. Use the installed version's manual and machine-readable state interface during implementation. |
| S14 | [systemd.service manual source](https://raw.githubusercontent.com/systemd/systemd/main/man/systemd.service.xml) | Service execution/startup/restart behavior and command-line semantics. Pin rendered directives to the supported manager versions. |
| S15 | [loginctl manual source](https://raw.githubusercontent.com/systemd/systemd/main/man/loginctl.xml) | User manager/login lifetime and linger operations. Linger changes require their own explicit permission-bearing setup. |
| S16 | [Apple: Creating Launch Daemons and Agents](https://developer.apple.com/library/archive/documentation/MacOSX/Conceptual/BPSystemStartup/Chapters/CreatingLaunchdJobs.html) | **Archived** Apple material on launchd jobs, plist structure, foreground execution, and scope/lifetime concepts. Not a current command-reference guarantee. |
| S17 | [systemd.exec manual source](https://raw.githubusercontent.com/systemd/systemd/main/man/systemd.exec.xml) | Environment, execution context, working directory, and related process settings. Consult S14 too for exact command/argument handling. |
| S18 | [OpenRC service script guide](https://github.com/OpenRC/openrc/blob/master/service-script-guide.md) | OpenRC has its own service scripting/manager contract. It must not be treated as a synonym for systemd or a universal `initd` adapter. Later scope. |
| S27 | [Apple Terminal guide: Script management with launchd](https://support.apple.com/guide/terminal/script-management-with-launchd-apdc6c1077b-5d5d-4d35-9c19-60f2397b2369/mac) | Apple guidance on launchd and user/system locations. Exact modern domain-targeted control operations still require on-target verification. |

Where hosted systemd manual pages could not be fetched, the official upstream manual XML was inspected instead. These are moving `main` references. Record the version actually supported rather than copying a new directive into an older host's unit.

For launchd, the agent must consult the target machine's `man launchctl` and `man launchd.plist` and run lifecycle tests. This research did not certify a complete current bootstrap/bootout/enable/disable/kickstart command cookbook. No unsupported parity with systemd restart/backoff, dependency ordering, or environment-file behavior is implied.

## 4. Rust and distribution references

| ID | Primary source | What it supports / qualification |
|---|---|---|
| S19 | [Cargo workspaces](https://doc.rust-lang.org/cargo/reference/workspaces.html) | Multiple packages within a workspace. Repository sharing is a product/source-audit decision, not a consequence of using Cargo. |
| S25 | [Clap documentation](https://docs.rs/clap/latest/clap/) | Proposed CLI implementation option. Select and verify a version before writing actual integration code. |
| S26 | [Ratatui project documentation](https://ratatui.rs/) | Proposed TUI implementation option. No exact API signatures are assumed by the conceptual architecture. |
| S28 | [GitHub REST releases API](https://docs.github.com/en/rest/releases/releases) | Release operations and latest-release resolution are repository-scoped. A multi-product installer needs explicit component-aware resolution rather than blind repository-latest selection. |

Library choices are recommendations, not a frozen dependency manifest. Audit maintenance, licenses, supported targets, binary size, and existing compatible AppTrail dependencies before pinning them.

## 5. Historical AppTrail reference

**A01 — [Apptrail_PRD.original.md](reference/Apptrail_PRD.original.md)**, supplied draft v0.1, copied without modifications.

Relevant sections: 4 (non-goals), 7–10 (architecture/providers/observations/identity), 12 (application metadata manifest), 14 (file provider), 19 (suggested API), 21 (conceptual connections), and 27 (open decisions).

This file is not a repository scan. It cannot establish implemented functions, actual Rust crates, current schema validators, available endpoints, authentication behavior, or release workflows. The receiving agent must attach current commit/path/symbol/test evidence to the repository decision and any technical transfer.

## 6. Verification gates to complete during implementation

| Gate | Required evidence before declaring support |
|---|---|
| Current AppTrail behavior | Repository/commit, actual symbols/schemas, and relevant tests. |
| Proxy compatibility | Pinned engine/plugin versions and complete supported deployment recipes. |
| Caddy Docker file-only refresh | Managed route changes activate, external Docker routes survive, later events/restarts preserve both. |
| Caddy validation and reload | Correct complete authority, required module availability, live resource/provisioning behavior, safe input recovery. |
| Traefik file watching | Actual host/container mounts, atomic replacement, empty/final-route removal, runtime diagnostics. |
| Native services | Real systemd/launchd lifecycle, scope, startup, stop/restart, persistence, and failure evidence. |
| Networking | Proxy-context reachability for each supported native/container recipe. |
| Installer | Real artifact availability, build/module inventory, integrity/authenticity policy, target execution, recovery. |
| AppTrail integration | Current schema contract, duplicate handling, freshness semantics, authorization if control is added. |

A conflicting result must update the relevant capability and plan with evidence. Material changes to agreed scope, behavior, or security require a user decision—not an undocumented workaround.
