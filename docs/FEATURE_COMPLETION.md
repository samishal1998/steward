# Feature completion work

Requested scope: the missing-feature list plus proxy update/remove, dynamic ID
completion, history, rollback and explicit adoption. Nginx, Windows and custom
Docker-label mappings remain deferred by the user's follow-up. Apache is excluded.

Acceptance is working behavior with meaningful failure coverage, not command
placeholders. Existing read-only profiles and alpha.2 state remain readable.

| Area | Acceptance requirement | Status |
| --- | --- | --- |
| Connections | Update/remove/detach preserve routes and reject unsafe repointing | Pending |
| Completion | Saved IDs complete without network activity or state creation | Pending |
| History/recovery | Persistent inspectable history, bounded reads, guarded completed-operation rollback, interrupted recovery | Pending |
| Adoption | Explicit source/resource selection and expected fingerprint; unsupported external definitions remain untouched | Pending |
| Plan/apply | Saved versioned plans, state/source/native revalidation, no implicit prune, partial outcomes recorded | Pending |
| HTTPS | Explicit public/internal TLS, verified certificate/hostname, isolated local CA test; no insecure verification fallback | Pending |
| Caddy files | Full native adaptation/validation, imported-source preservation and guarded activation/recovery | Pending |
| Docker controller | Owned base contribution activated by the controller, no competing runtime writer | Pending |
| Discovery | Native/API/file/known-label evidence with provenance; inaccessible sources remain partial | Pending |
| Remote/container | Explicit execution target; native operations and verification run in that target's context | Pending |
| Proxy lifecycle | Explicit install/setup/upgrade/detach/uninstall with artifact integrity and data preservation | Pending |
| Native services | systemd/launchd definitions, lifecycle, enablement, status, bounded logs and ownership | Pending |
| Composition | Service readiness before route publication; route-first shutdown; explicit service retention/removal | Pending |
| TUI | Interactive Routes/Proxies/Services/Operations views using core command paths | Pending |
| AppTrail | Versioned redacted export and compatible reader contract; preserve provenance and partial coverage | Pending |

Live acceptance uses disposable processes, files, containers and uniquely named
test services. Public DNS/ACME, logout/reboot and remote platforms require actual
execution evidence before support claims are promoted. Record results below.
