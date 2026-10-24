# Feature completion work

Requested scope: the missing-feature list plus proxy update/remove, dynamic ID
completion, history, rollback and explicit adoption. Nginx, Windows and custom
Docker-label mappings remain deferred by the user's follow-up. Apache is excluded.

Acceptance is working behavior with meaningful failure coverage, not command
placeholders. Existing read-only profiles and alpha.2 state remain readable.

| Area | Acceptance requirement | Status |
| --- | --- | --- |
| Connections | Update/remove/detach preserve routes and reject unsafe repointing | Implemented; connection CRUD and in-use refusal exercised |
| Completion | Saved IDs complete without network activity or state creation | Implemented; Bash runtime completion and five shell generators exercised |
| History/recovery | Persistent inspectable history, bounded reads, guarded completed-operation rollback, interrupted recovery | Implemented; route/service rollback, source drift, native interruption and history exercised |
| Adoption | Explicit source/resource selection and expected fingerprint; unsupported external definitions remain untouched | Implemented for representable Caddy JSON and dedicated Traefik HTTP files; live adoption/rollback exercised |
| Plan/apply | Saved versioned plans, state/source/native revalidation, no implicit prune, partial outcomes recorded | Implemented; real apply, replay and modified-source rejection exercised |
| HTTPS | Explicit public/internal TLS, verified certificate/hostname, isolated local CA test; no insecure verification fallback | Implemented; internal Caddy JSON/Caddyfile and Traefik trust/route lifecycle exercised; external public ACME/DNS not certified by local tests |
| Caddy files | Full native adaptation/validation, imported-source preservation and guarded activation/recovery | Implemented; real import preservation, HTTP/HTTPS, and rollback exercised |
| Docker controller | Owned base contribution activated by the controller, no competing runtime writer | Implemented; pinned 2.13.1 timer/event/restart/label-preservation/removal suite passed on Linux |
| Discovery | Native/API/file/known-label evidence with provenance; inaccessible sources remain partial | Implemented; known-label parsing is bounded and unsupported matcher values are redacted |
| Remote/container | Explicit execution target; native operations and verification run in that target's context | Implemented; real key-authenticated SSH and container-local writes/removal passed |
| Proxy lifecycle | Explicit install/setup/upgrade/detach/uninstall with artifact integrity and data preservation | Implemented; real Caddy download/install/upgrade/uninstall and config retention passed on Linux |
| Native services | systemd/launchd definitions, lifecycle, enablement, status, bounded logs and ownership | Implemented; systemd live suite passed; launchd CI readiness issue under investigation |
| Composition | Service readiness before route publication; route-first shutdown; explicit service retention/removal | Implemented with pre-effect operation checkpoints; Linux up/down/retention/removal suite passed |
| TUI | Interactive Routes/Proxies/Services/Operations views using core command paths | Implemented; real PTY navigation, command execution and terminal restoration passed |
| AppTrail | Versioned redacted export and compatible reader contract; preserve provenance and partial coverage | Implemented in both feature branches; AppTrail Linux/macOS/Windows CI passed initial reader, frontend and migration suite |

Live acceptance uses disposable processes, files, containers and uniquely named
test services. Public DNS/ACME, logout/reboot and remote platforms require actual
execution evidence before support claims are promoted. Record results below.

## Current evidence (2026-10-24)

Development version: `0.2.0-dev.1`. The shipped installer remains on alpha.2.
Both repositories use `feature/complete-management`; neither main branch has been
changed by this work. The user explicitly authorized branch publication and CI.

- Local Rust formatting, Clippy with warnings denied, tests and Rust 1.89 checks passed.
- `scripts/live_route_smoke.py`: Caddy 2.11.7/Traefik 3.7.14 lifecycle, explicit
  adoption, manifest apply/stale checks, history/rollback, source drift, locks,
  failed activation and interrupted recovery.
- `scripts/live_tls_files.py` and `live_tls_native.py`: native Caddyfile/import,
  JSON and Traefik internal TLS; untrusted certificates fail, explicit CA trust
  and hostname verification succeed.
- `scripts/live_services.py`: real systemd user service/readiness/lifecycle/logs,
  drift and completed-operation rollback. No user applications were test targets.
- `scripts/live_stack_setup.py`: real checksum-verified proxy install, upgrade,
  service-first composition, route-first down, retained services, cleanup and export.
- `scripts/live_controller.py`: Docker controller 2.13.1, pinned image digest
  `sha256:3e6cf4a6382da0cd14108a69929d7f8cc59e9a59cae4c9795071fa392ffa8036`.
- `scripts/live_container.py`: Docker target-local state/authority and routing.
- `scripts/live_ssh.py`: isolated loopback sshd with strict host-key checking,
  private fixture keys, paths with spaces, live apply/removal. No real remote host
  or existing SSH configuration was changed.
- `scripts/live_tui.py`: PTY navigation/core commands and canonical terminal restoration.
- AppTrail `go test -p 1 ./...`, `go vet ./...`, and frontend build passed locally.
  Initial [branch checks 40119055849](https://github.com/samishal1998/apptrail/actions/runs/40119055849)
  passed Linux/macOS/Windows, including existing native-service checks.
- Initial [Steward checks 40119051666](https://github.com/samishal1998/steward/actions/runs/40119051666)
  passed Linux, docs and MSRV. macOS passed route/TLS suites but found a launchd
  service-readiness failure; its failure is retained and follow-up checks will
  establish the fix rather than relabeling it as a pass.

macOS diagnosis: a process sample in run
[40124697697](https://github.com/samishal1998/steward/actions/runs/40124697697)
showed Python 3.15's stock HTTP server blocked in `socket_gethostbyaddr` → mDNS
before listening. Independent launchd registration/start checks passed. The
portable `examples/static-server.py` now binds the numeric address without that
unrelated reverse lookup, while the acceptance suite still requires real native
startup, HTTP readiness, enablement, lifecycle, rollback and failed-readiness
compensation. A subsequent run passed startup and exposed macOS 26's textual
`enabled`/`disabled` output; the reader now supports both that representation and
older boolean overrides. Temporary process-sampling workflows/examples were removed.

This implementation does not certify arbitrary modules/topologies, public DNS/
ACME reachability, or logout/reboot behavior without on-target evidence. Nginx,
Windows support for Steward, and custom label mappings remain explicitly deferred.
