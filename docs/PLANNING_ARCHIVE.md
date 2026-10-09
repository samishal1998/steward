# Planning archive provenance and coverage

**Archived: 2026-10-09.** Source:
`/home/samimishal/projects/rust/app-trail/steward-plan`.
Destination: `/home/samimishal/projects/rust/steward/docs/planning`.

**Coverage: all 24 files, 223,502 bytes; no sections omitted.** Exact-byte copies
were checked using SHA-256 against each original, including a source recheck
after copying. Original source files were not overwritten. All original
filenames/subdirectories and relative links are preserved. No source code or
runtime fixture code was included; the YAML examples are historical design
inputs from the pack, not newly implemented CLI fixtures.

The archival README still says placement is undecided and the two templates
still say NOT COMPLETED. That is their original historical status, not the
current handoff status. Use the completed external
[repository decision](decisions/repository-placement.md),
[technical transfer](APPTRAIL_TECHNICAL_TRANSFER.md),
[implementation plan](IMPLEMENTATION_PLAN.md), [capabilities](CAPABILITIES.md)
and [progress](PROGRESS.md). The original proposed four-crate layout is refined
to one Cargo package/internal modules in the current plan. No product capability
is removed by that refinement or by the partial first slice.

## Copied-file manifest

| Group | Files (links point to the archival copies) |
|---|---|
| Pack entry point (1) | [README.md](planning/README.md) |
| Product/architecture/execution (3) | [01_PRD.md](planning/01_PRD.md), [02_ARCHITECTURE.md](planning/02_ARCHITECTURE.md), [03_IMPLEMENTATION_PLAN.md](planning/03_IMPLEMENTATION_PLAN.md) |
| Audit/testing/integration/research (4) | [04_APPTRAIL_AGENT_HANDOFF.md](planning/04_APPTRAIL_AGENT_HANDOFF.md), [05_TESTING_AND_ACCEPTANCE.md](planning/05_TESTING_AND_ACCEPTANCE.md), [06_APPTRAIL_INTEGRATION.md](planning/06_APPTRAIL_INTEGRATION.md), [07_VERIFIED_REFERENCES.md](planning/07_VERIFIED_REFERENCES.md) |
| Accepted discovery clarification (1) | [08_DISCOVERY_ADDENDUM.md](planning/08_DISCOVERY_ADDENDUM.md) |
| Charts (8) | [README.md](planning/charts/README.md), [01-components.mmd](planning/charts/01-components.mmd), [02-resources.mmd](planning/charts/02-resources.mmd), [03-route-apply.mmd](planning/charts/03-route-apply.mmd), [04-service-and-route.mmd](planning/charts/04-service-and-route.mmd), [05-bootstrap.mmd](planning/charts/05-bootstrap.mmd), [06-roadmap.mmd](planning/charts/06-roadmap.mmd), [07-apptrail-integration.mmd](planning/charts/07-apptrail-integration.mmd) |
| Proposed examples (4) | [README.md](planning/examples/README.md), [steward.routes-only.yaml](planning/examples/steward.routes-only.yaml), [steward.services-only.yaml](planning/examples/steward.services-only.yaml), [steward.combined.yaml](planning/examples/steward.combined.yaml) |
| Original AppTrail draft (1) | [Apptrail_PRD.original.md](planning/reference/Apptrail_PRD.original.md) |
| Original uncompleted templates (2) | [REPOSITORY_DECISION.md](planning/templates/REPOSITORY_DECISION.md), [APPTRAIL_TECHNICAL_TRANSFER.md](planning/templates/APPTRAIL_TECHNICAL_TRANSFER.md) |

Archive/source inspection is not build or test execution. Prior AppTrail Checks
success and absent native macOS logs are qualified in the current transfer;
the pack's original research qualifications remain intact. Source/fixture rights
must be reviewed before copying AppTrail implementation or tests; this archival
copy does not choose a new Steward license.
