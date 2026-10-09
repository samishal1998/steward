# Steward / AppTrail — Repository placement decision

**Template status: NOT COMPLETED.** The AppTrail agent must replace the prompts with source-backed findings. This template does not represent an audit or a recommendation that has already been made.

## 1. Audit baseline

| Field | Finding |
|---|---|
| AppTrail repository | TO VERIFY |
| Branch and immutable commit | TO VERIFY |
| Audit date / agent | TO FILL |
| Working-tree changes relevant to the audit | TO VERIFY; distinguish uncommitted code from the baseline |
| Runtime/language/toolchain | TO VERIFY |
| Provider implementations inspected | Paths, symbols, and tests |
| Build, test, and release entrypoints inspected | Paths and commands |
| Missing access or evidence | Explicit gaps and their effect on the decision |

The historical PRD is context only. Use status labels: `verified from code`, `verified by test`, `documented upstream`, `historical proposal`, `inferred`, or `unknown`.

## 2. Current architecture and actual data flow

Describe the current entrypoints, provider/client construction, connection settings, native response handling, observation normalization, reconciliation, persistence, UI boundaries, and error/retry/watch behavior.

Attach a diagram showing real import/call dependencies. Identify which boundaries are pure logic and which depend on a database, server process, authenticated user session, background loop, web framework, or application-specific models.

**Evidence:** immutable source permalinks, local `path:line` ranges, relevant symbol names, and the tests that exercise them.

## 3. Reuse matrix

Fill one row for every meaningful candidate; include rejected candidates. Similar upstream API names are not evidence of equivalent implementation responsibilities.

| Candidate path / symbol | Existing inputs → outputs | Existing behavior, side effects, dependencies | Exact Steward need | Adaptation required | Same semantics? | Decision / evidence |
|---|---|---|---|---|---|---|
| TO VERIFY | TO VERIFY | TO VERIFY | TO COMPARE | Be concrete, including new abstractions | Yes / no / partial, with reason | Share / keep separate / duplicate small helper / investigate |

For each proposed extraction, show actual call sites in both products and explain why it does not require additional generic interfaces, lossy type conversions, architecture changes, or unrelated dependencies. A hypothetical future caller does not satisfy this requirement.

## 4. Alternatives evaluated

| Option | Concrete benefits in this codebase | Concrete costs and coupling | Release/build impact | Result |
|---|---|---|---|---|
| Same repo, narrow proven shared code | TO FILL | TO FILL | TO FILL | Select / reject |
| Same repo, independent code | TO FILL | TO FILL | TO FILL | Select / reject |
| Separate repo, explicit contracts | TO FILL | TO FILL | TO FILL | Select / reject |

Do not equate a monorepo with mandatory sharing or lockstep versions. Do not equate separation with an inability to integrate later. Evaluate maintainability using the current implementation, not an imagined common framework.

## 5. Decision

**Selected option:** TO DECIDE.

**Why this is simpler for the actual code:** state the decisive evidence, not just preferences.

**Code to share, if any:** exact paths/symbols and the smallest retained interface.

**Code deliberately not shared:** specific mismatches and why adaptation would cost more than separation.

**Conditions or uncertainties:** distinguish a genuine blocker from a follow-up that does not prevent the decision.

**User approval/access needed:** identify any broad rewrite, remote repository creation, new permission, or material product change. Do not perform those actions merely because this template contains a space for them.

## 6. Independent build and dependency proof

Record the command that builds/tests only Steward and the command that builds/tests AppTrail. Show that Steward's service-only execution requires no AppTrail server/database/UI or proxy/Docker connection.

For a shared repo, include the intended dependency/import graph, CI path filters, and package boundaries. For a separate repo, identify the initial repo layout and any package/fixture vendoring strategy, including source/license attribution.

## 7. Release and installation decision

Specify component tags/versioning, build target naming, artifact names, and how the installer selects Steward rather than a newer unrelated AppTrail release. Include a regression test for that exact failure mode.

Separate release repositories are an option, not a prescribed architecture. Record only the strategy selected for this codebase and its credential/CI requirements.

## 8. Plan changes and first implementation steps

Update the Steward implementation plan with actual repository paths and verified integration references. List the first runnable slice, required compatibility spikes, test commands, expected evidence, and any unresolved behavior that must not be guessed.

| Plan section / milestone | Change | Reason / evidence |
|---|---|---|
| TO FILL | TO FILL | TO FILL |

## 9. Required accompanying artifacts

For **same repo**, include the proven shared-boundary/dependency map, existing AppTrail regression-test results or pending test list, and independently buildable Steward entrypoint plan.

For **separate repo**, include the **completed** [APPTRAIL_TECHNICAL_TRANSFER.md](APPTRAIL_TECHNICAL_TRANSFER.md), sanitized fixtures or precise fixture-generation instructions, and updated Steward plan. A blank template, a link to the old PRD, or undocumented personal memory does not satisfy the transfer requirement.

## 10. Review outcome

| Check | Evidence / result |
|---|---|
| Actual commit and current code inspected | TO FILL |
| Calling the same APIs was not treated as sufficient for sharing | TO FILL |
| No unnecessary generic interfaces or rewrites introduced | TO FILL |
| Service and route independence preserved | TO FILL |
| Lightweight independent distribution preserved | TO FILL |
| Decision is clear and actionable | TO FILL |
| Required transfer or shared-boundary artifacts completed | TO FILL |
