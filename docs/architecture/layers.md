# Omnihive Architecture Layers (Phase 1)

Omnihive is being refactored into a **Control Plane / Execution Plane / Knowledge Plane** monorepo.

This document defines the boundary rules for Issue 1 and is the source of truth for allowed and forbidden dependencies during migration.

## Layer Definitions

### Control Plane
- Owns task lifecycle orchestration, APIs, CLI/daemon entrypoints, and user-facing coordination.
- Target directories:
  - `apps/desktop`
  - `apps/cli`
  - `apps/daemon`
  - `crates/runtime-api`

### Execution Plane
- Owns deterministic execution semantics and runtime guarantees.
- Includes state machine, queue, sandbox, policy evaluation, and observability emitters.
- Target directories:
  - `crates/runtime-core`
  - `crates/runtime-queue`
  - `crates/runtime-sandbox`
  - `crates/runtime-policy`
  - `crates/runtime-observe`
  - `plugins/providers/*`
  - `plugins/tools/*`

### Knowledge Plane
- Owns schemas, workflows, SDK contracts, benchmarks, and reusable memory components.
- Target directories:
  - `schemas`
  - `packages/*`
  - `py/*`
  - `plugins/memory/*`
  - `examples/*`
  - `benchmarks/*`
  - `library/*` (legacy assets, migration in progress)

## Dependency Matrix

| From \ To | Control Plane | Execution Plane | Knowledge Plane |
|---|---|---|---|
| **Control Plane** | ✅ Allowed | ✅ Allowed | ✅ Allowed |
| **Execution Plane** | ❌ Forbidden | ✅ Allowed | ✅ Allowed (read-only contracts/config) |
| **Knowledge Plane** | ❌ Forbidden | ❌ Forbidden | ✅ Allowed |

## Rules

1. Cross-plane calls must follow the matrix above.
2. Execution components must not call Control entrypoints directly.
3. Knowledge assets are dependency targets, not orchestrators.
4. Legacy `app/` remains the active build root during migration and must not be broken by new scaffolding.

## Forbidden Dependency Examples

1. `crates/runtime-core` importing UI modules from `apps/desktop`.
2. `plugins/tools/shell` invoking CLI command handlers in `apps/cli`.
3. `schemas` or `packages/workflow-dsl` importing runtime internals from `crates/runtime-core`.

## Migration Notes

- This phase introduces the target directory scaffolding while preserving current build paths (`app/`).
- New modules should be created in the new structure first; legacy code is migrated incrementally.
