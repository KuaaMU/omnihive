# ADR-001: Define architecture boundaries and dependency rules

- **Status**: Accepted
- **Date**: 2026-03-17
- **Related**: `docs/architecture/layers.md`

## Context

Omnihive is transitioning from a single-app repository into a monorepo with control, execution, and knowledge planes.
Without explicit dependency boundaries, components can couple across concerns (UI/orchestration/runtime/contracts), which makes recovery, policy enforcement, and observability hard to evolve safely.

Phase 1 already introduced scaffold directories (`apps/`, `crates/`, `packages/`, `plugins/`, `schemas/`, etc.) while keeping `app/` as the active build root during migration.
This ADR formalizes architectural boundaries and the allowed dependency direction so future issues can implement runtime features without cross-layer regressions.

## Decision

Adopt a three-plane architecture with strict one-way dependency rules:

1. **Control Plane**
   - Owns orchestration/API entrypoints and user coordination.
   - May depend on Execution and Knowledge planes.

2. **Execution Plane**
   - Owns deterministic execution semantics (runtime, queue, sandbox, policy, observe, provider/tool adapters).
   - Must not depend on Control plane.
   - May depend on Knowledge plane contracts/configuration.

3. **Knowledge Plane**
   - Owns schemas, SDK contracts, workflows, examples, benchmarks, and memory plugin assets.
   - Must not depend on Control or Execution planes.

Dependency matrix (authoritative detail in `docs/architecture/layers.md`):

- Control -> Control/Execution/Knowledge: Allowed
- Execution -> Control: Forbidden
- Execution -> Execution/Knowledge: Allowed
- Knowledge -> Control/Execution: Forbidden
- Knowledge -> Knowledge: Allowed

## Consequences

### Positive

- Reduces accidental coupling and keeps runtime core independent from UI/entrypoint concerns.
- Makes policy, replay, checkpoint/resume, and trace evolution safer by preserving layer contracts.
- Provides clear review criteria for PRs and CI checks.

### Trade-offs

- Some migrations require adapter layers instead of direct imports, adding short-term boilerplate.
- During transition, legacy `app/` code and new scaffolded modules will coexist, requiring discipline in review.

### Forbidden dependency examples

1. `crates/runtime-core` importing UI or route modules from `apps/desktop`.
2. `plugins/tools/shell` invoking command handlers from `apps/cli`.
3. `schemas` (or `packages/workflow-dsl`) importing internals from `crates/runtime-core`.
4. Any `py/*` SDK package importing runtime implementation code from `crates/runtime-*`.
