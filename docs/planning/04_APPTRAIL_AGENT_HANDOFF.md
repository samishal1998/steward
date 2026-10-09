# Handoff to the existing AppTrail agent

**Purpose:** investigate repository placement and genuine code reuse before implementing Steward.  
**This document is separate from the product plan intentionally.** The product requirements do not depend on which repository wins.  
**Read next:** [PRD](01_PRD.md), [architecture](02_ARCHITECTURE.md), [plan](03_IMPLEMENTATION_PLAN.md), [integration](06_APPTRAIL_INTEGRATION.md).

## 1. Your assignment

You have access to the existing AppTrail project and its accumulated implementation context. Inspect the actual code and decide whether Steward should be developed in the same repository or a new repository.

Do not treat the tentative naming discussion as a placement decision. The current product name is **Steward**. AppTrail remains the existing discovery/dashboard product. Earlier references to “Apptail” were naming errors, not a separate product.

The decisive constraint from the user is this: **calling the same Docker, Caddy, and Traefik APIs does not justify sharing code when the functions, interfaces, structures, responsibilities, or architecture differ.** Unnecessary generic interfaces, rewrites, or dependency coupling are a reason not to share. Genuine equivalent functionality that can be reused with little or no adaptation is a reason to consider sharing.

This is not an instruction to merge the products, extract a common framework, or build Steward on AppTrail's database or web backend.

## 2. Preserve the product decisions

Steward is one lightweight CLI/TUI with independently usable service management and proxy-route management, plus optional composition. Proxy support starts with Caddy and Traefik; Docker labels are read-only discovery; managed changes use owned file-backed inputs. Setup/install/provider-enabling and disruptive changes are explicit. No required Steward daemon, Docker, AppTrail, or proxy dependency for service-only use. Native OS managers supervise applications. Nginx and other service-manager/platform adapters follow; Apache is excluded.

Deliver proxy management first, then general native services and combined flows. Minimal native lifecycle code needed to install the proxy can arrive earlier. Do not make the later service milestone an excuse to ship a temporary homegrown process supervisor.

The Rust workspace and internal modules are a proposed default for Steward. AppTrail's actual language/runtime must be determined from code. A different AppTrail language is not grounds to rewrite it or introduce an FFI/service framework solely to share a small HTTP call.

## 3. What is already known—and what is not

The included historical [AppTrail PRD](reference/Apptrail_PRD.original.md) describes a discovery/dashboard system with provider observations, reconciliation, enrichment, manual overrides, labels, and a file provider. Its conceptual provider interface includes snapshot/watch/diagnostic behavior. Its implementation language and several concrete API/provider choices are explicitly open. [A01: §§7–10, 19, 21, 27]

Use that file to recover product intent, not to assert which interfaces are now implemented. The plan author has not audited your repository. No current file path, symbol, production credential format, or HTTP endpoint has been verified from AppTrail source in this pack.

Your first output must identify the actual repository, branch, and commit inspected and distinguish implemented behavior from old PRD proposals, tests, work-in-progress, and obsolete code.

## 4. Audit procedure

### 4.1 Map the current project

Record runtime/language, entrypoints, dependency boundaries, storage, provider implementations, watch/poll behavior, error handling, authentication, test harnesses, CI, and release tooling. Identify whether discovery is reusable logic or tightly coupled to a long-running dashboard, database, user session, or reconciliation loop.

Locate the actual code that connects to Docker, reads Caddy configuration, and inspects Traefik routes/configuration. Trace both call sites and data flow. Read relevant tests, not only interface declarations. Identify normalization that is useful for dashboard display but lossy or unsafe for configuration management.

### 4.2 Evaluate concrete reuse candidates

For each candidate, record the source path/symbol and actual input/output types, behavior, side effects, dependency footprint, and tests. Compare that exact function with Steward's need.

| Candidate | May be reusable when… | Do not share when… |
|---|---|---|
| Docker connection setup | Endpoint resolution, TLS, API negotiation, and error behavior match without AppTrail session/storage dependencies. | A generic wrapper must be introduced merely to translate incompatible clients or lifetimes. |
| Docker snapshot/label parsing | Lossless facts and provenance already satisfy both consumers. | Dashboard normalization discards raw routing or ownership information needed for safe changes. |
| Caddy configuration reading | It returns a usable native representation with no dashboard coupling. | It only emits simplified URLs and would need restructuring into a management API. |
| Traefik route reading | Actual native IDs, provider qualification, and errors are preserved. | Different rule/route abstractions must be forced into one shared model. |
| Low-level config parsing | Existing parser is tested, lossless enough, and naturally independent. | Supporting round-trip editing requires a rewrite that discovery does not need. |
| Identity/reconciliation | A small normalization helper truly has the same semantics. | Dashboard merge precedence would become infrastructure authority. |
| Provider interface | Same responsibilities and lifetime genuinely fit. | Read-only snapshot/watch must be stretched to include installation, writes, rollback, and service control. |
| Test fixtures/tooling | Fixtures are sanitized and represent identical upstream inputs. | Sharing production abstractions is being justified only by common tests. |

A small duplicate HTTP request or simple data structure may be cheaper and clearer than a shared abstraction. Record that choice rather than treating deduplication as a goal in itself.

### 4.3 Compare repository options fairly

Evaluate a shared repository with narrowly shared modules, a shared repository without shared production code, and a separate repository with documented contracts. Consider local development, dependency/build footprint, release/version scripts, review ownership, CI filtering, portability, and update distribution.

Do not make a shared repository imply lockstep releases or a dashboard dependency in the Steward executable. Conversely, do not assume a separate repository prevents future integration or later extraction of an actually stable library.

The GitHub release-list concern is a packaging consideration, not the sole architecture criterion. Document the chosen release resolution strategy and how installers select the intended product.

## 5. Required decision record

Complete [templates/REPOSITORY_DECISION.md](templates/REPOSITORY_DECISION.md). The record must include the inspected commit, evidence-backed reuse matrix, alternatives, selected placement, any shared modules with exact boundaries, modules deliberately not shared, release implications, and first implementation steps.

Make a clear recommendation and decision based on the code; do not finish with an open-ended list of possible architectures. Present unresolved evidence honestly. Where a missing repository capability or material product tradeoff prevents a defensible decision, state the blocker and ask the user the narrow question needed.

Do not carry out a broad extraction, rewrite, or remote repository mutation before recording the investigation result. Work in the selected authorized workspace after the decision; request any additional access that is genuinely required.

## 6. When choosing the same repository

Keep Steward independently buildable, testable, and installable. Add it under a clear product boundary, with separate CLI/TUI entrypoint and documentation. Share only candidates that passed the audit. The proof should show real consumers on both sides with minimal adaptation, not a theoretical future caller.

Include an import/dependency diagram proving that Steward service-only paths do not require AppTrail's web framework, registry database, enrichment pipeline, or server runtime. Preserve AppTrail behavior and run its existing tests. Add component-aware CI/release handling without turning every release into a joint product release.

Update the Steward implementation plan with actual workspace paths and verified shared symbols. The result should be executable by another agent without reconstructing your intentions from conversation history.

## 7. When choosing a separate repository

**Producing a source-backed technical transfer/help document is mandatory before handing work to a new agent.** Do not just say “use the same providers,” link the old PRD, or refer to information you personally remember.

Complete [templates/APPTRAIL_TECHNICAL_TRANSFER.md](templates/APPTRAIL_TECHNICAL_TRANSFER.md) and place it with the updated Steward plan. Include the relevant accumulated AppTrail integration knowledge, while clearly separating it from Steward's own design and excluding secrets or unrelated dashboard details.

The transfer must cover:

| Area | Required evidence/content |
|---|---|
| Baseline | Repository/commit, runtime, paths, symbols, implemented versus planned status. |
| Connection setup | Docker endpoint/context/TLS/version behavior; Caddy admin/file modes; Traefik label/file/API modes; timeouts; authentication; permissions; native/container network assumptions. |
| Actual schemas | Real configuration types, observation models, native payload examples, required/optional/null semantics, versioning, and error types. |
| API interactions | Exact methods/paths/parameters or CLI invocations actually used; response shapes; pagination/watch behavior; relevant pinned upstream versions. |
| Call flows | Source-backed trace from config to client to normalized result, with sanitization, caches, retries, and cancellation. |
| Known problems | Tested workarounds, unsupported modes, failed approaches, rootless/remote Docker issues, API-disabled behavior, and provenance gaps. |
| Fixtures | Sanitized sample configs/labels/API responses and the tests that consume them. |
| Documentation | Primary upstream links and repository permalinks, with date/version and what each supports. |
| Future integration | Existing AppTrail extension points and schema that can actually consume Steward output; no invented endpoints. |
| New-agent bootstrap | A copyable first-session checklist, fixture/test commands, expected results, and relevant architectural prohibitions. |

Source references should use immutable commit permalinks where possible, supplemented by local `path:line` and symbol names. Record terminal commands and expected output shapes. When a fact is unknown, label it unknown and describe a precise verification step. Never invent a schema or fill a placeholder with something “probably equivalent.”

Then update the implementation plan with the selected new-repository layout, concrete verified integration notes, and the first runnable vertical slice. Include the completed transfer in the handoff bundle. Do not require the new agent to have access to your private conversation history to do its job.

## 8. Specific technical hazards to verify during the audit

The plan separates engine, controller, runtime, and network context. Preserve that distinction. In particular, investigate whether any AppTrail parser loses complex route matches, how it distinguishes `@docker` and `@file`, whether Caddy admin data is runtime-only, and whether Docker endpoint/context identity survives normalization.

Check the Caddy Docker base-input refresh path against the pinned controller build. Do not assume the existing AppTrail discovery code contains any safe write/refresh mechanism. Check Traefik startup source authority and file mounts. Check actual systemd/launchd support rather than assuming AppTrail already has it.

Existing AppTrail credential references are knowledge to document, not authority to copy credentials into Steward. Preserve least-privilege separation. Public metadata, icons, discovery manifests, and Docker labels are untrusted data and cannot authorize service creation or execution.

## 9. Reporting and escalation

Return the completed repository decision first, together with a brief statement of what was inspected and why the chosen topology is simpler. For separation, attach the completed transfer document and fixture/test guide. For a shared repository, attach the proven shared-boundary/dependency map and regression-test plan.

Use explicit status labels: **verified from code**, **verified by test**, **documented upstream**, **inferred**, **historical proposal**, or **unknown**. Do not equate a code path existing with that path having passed a live compatibility test.

Any material contradiction between this plan and the real APIs must be surfaced with evidence and options before changing scope. Small internal naming or layout improvements can be documented and implemented without reopening the agreed product decisions.

## 10. Completion checklist

The handoff is complete when there is one chosen repository topology, a source-backed explanation of genuine reuse or non-reuse, an updated actionable Steward plan, and either a completed standalone transfer pack or a verified same-repository dependency map. The next agent must know what to implement, what it may reuse, what it must not assume, how to connect to the relevant tools, and how to prove that the first slice works.
