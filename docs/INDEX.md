# Steward documentation

## Implemented slice

- [CLI and schema contract](CLI_CONTRACT.md) — actual command/input/result semantics.
- [Synthetic native fixtures](../fixtures/README.md) — offline reproduction, not live compatibility proof.
- [Project README](../README.md) — build and first-command instructions.

## Current handoff

These completed documents are the starting point for implementation:

1. [Repository placement decision](decisions/repository-placement.md) — approved
   separate Rust repository, no shared production code, independent releases.
2. [AppTrail technical transfer](APPTRAIL_TECHNICAL_TRANSFER.md) — audited actual
   interfaces, immutable sources, inline-test/fixture map and remaining gates.
3. [Implementation plan](IMPLEMENTATION_PLAN.md) — selected workspace, first
   read-only slice, journaled mutation foundation and full product roadmap.
4. [Capabilities](CAPABILITIES.md) — evidence and supported-versus-planned status.
5. [Progress](PROGRESS.md) — completed documentation, implementation owners and
   exact verification records to add as code lands.

## Historical planning pack

See [archive coverage](PLANNING_ARCHIVE.md) and the
[original pack entry point](planning/README.md). All 24 source files were copied
byte-for-byte with their relative layout intact. Original undecided decisions,
uncompleted templates and proposed examples are historical. Current completed
documents above resolve repository placement and the technical transfer without
rewriting the archive. Read the archived PRD/architecture/testing plan and
effective-discovery addendum for the full approved product scope.
