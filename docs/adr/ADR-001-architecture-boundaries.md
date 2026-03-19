# ADR-001: Architecture Boundaries

## Status

Accepted

## Context

Omnihive v0.4.0 was a monolithic Tauri app where all logic lived in `app/src-tauri/src/`. As the system evolves from an LLM API polling loop into an execution control plane, clear architecture boundaries are needed to:

1. Enable non-desktop usage (CLI, daemon, SDK)
2. Prevent circular dependencies
3. Allow independent testing of core logic
4. Keep the codebase maintainable as it grows

## Decision

### Three Planes

The architecture is divided into three planes:

```
┌─────────────────────────────────────────────────┐
│                  Control Plane                   │
│  (state machine, policy engine, task runner)     │
│  crates/omnihive-core                            │
├─────────────────────────────────────────────────┤
│                 Execution Plane                  │
│  (tool adapters, API clients, providers)         │
│  crates/omnihive-core/tools + app/src-tauri      │
├─────────────────────────────────────────────────┤
│                 Knowledge Plane                  │
│  (consensus.md, agent memory, trace JSONL)       │
│  project filesystem + logs/                      │
└─────────────────────────────────────────────────┘
```

**Control Plane** owns task lifecycle, state transitions, policy enforcement, checkpoint/resume, and retry logic. It is pure Rust with no framework dependencies.

**Execution Plane** implements the actual work: LLM API calls, shell commands, file operations, GitHub interactions. It depends on the Control Plane for policy checks and trace correlation.

**Knowledge Plane** is the shared data layer: consensus documents, agent memory files, configuration, and trace logs. Both Control and Execution planes read/write to it, but through well-defined interfaces.

### Dependency Rules

```
Control ← Execution    (Execution depends on Control)
Control ← Consumers    (CLI, Desktop App depend on Control)
Control → Knowledge    (Control reads/writes via defined I/O)
Execution → Knowledge  (Execution reads/writes via defined I/O)
```

**Forbidden dependencies:**

| From | To | Why |
|------|----|-----|
| `omnihive-core` | `tauri` | Core must be framework-agnostic |
| `omnihive-core` | `omnihive` (app) | Core cannot depend on consumers |
| `omnihive-cli` | `omnihive` (app) | CLI cannot depend on desktop app |
| `tools/shell.rs` | `api_client.rs` | Tool adapters are independent |
| `state_machine.rs` | `tool_protocol.rs` | State machine is pure logic, no I/O |

### Crate Structure

```
omnihive/
├── crates/
│   └── omnihive-core/          # Control Plane (framework-agnostic)
│       ├── state_machine.rs    # Pure state transitions
│       ├── task_model.rs       # Task/Step data models + I/O
│       ├── policy_engine.rs    # Default-deny rule evaluation
│       ├── checkpoint.rs       # Crash recovery
│       ├── retry.rs            # Exponential backoff + idempotency
│       ├── tool_protocol.rs    # Tool trait + registry
│       ├── runner.rs           # Task execution loop
│       ├── eval.rs             # Metrics from trace data
│       ├── trace_export.rs     # JSONL trace I/O
│       ├── guardrails.rs       # Command safety checks
│       ├── extract.rs          # Response parsing
│       └── tools/              # Tool adapters
│           ├── shell.rs        # Subprocess execution
│           ├── filesystem.rs   # File operations
│           └── github.rs       # GitHub via gh CLI
├── apps/
│   └── cli/                    # CLI binary (depends on omnihive-core)
├── app/
│   └── src-tauri/              # Desktop app (depends on omnihive-core)
└── schemas/                    # JSON Schema definitions
```

### Key Principles

1. **Core is pure**: `omnihive-core` has zero framework dependencies. It compiles and tests without Tauri, without a GUI, without network access.

2. **Policy at the boundary**: Every tool call passes through the policy engine before execution. No tool adapter can bypass this.

3. **Trace everything**: Every state transition, tool call, and error emits a trace event with `trace_id`/`task_id`/`step_id` correlation.

4. **Immutable updates**: Task and Step models use immutable update patterns. `with_status()` returns a new copy, never mutates in place.

5. **Default-deny**: The policy engine denies all actions unless an explicit allow rule matches. Ship with permissive defaults, users opt into restrictions.

## Consequences

### Positive

- CLI and desktop app share the same core logic without duplication
- Core crate can be tested in CI without Tauri or GUI dependencies
- New consumers (daemon, SDK, WASM) can be added by depending on `omnihive-core`
- Clear ownership: each module has a single responsibility

### Negative

- Some code duplication between `app/src-tauri/src/engine/` and `crates/omnihive-core/src/` during migration (both contain state_machine, policy_engine, etc.)
- Desktop app must re-export or delegate to core crate, adding a thin adapter layer
- Tool adapters in core crate may need platform-specific code (e.g., shell commands differ on Windows vs Unix)

### Migration Path

The desktop app (`app/src-tauri/`) currently has its own copies of modules that also exist in `omnihive-core`. The plan is to gradually replace the app's copies with imports from the core crate, one module at a time, verifying the desktop app works after each change.

## References

- [Cargo Workspaces](https://doc.rust-lang.org/book/ch14-03-cargo-workspaces.html)
- [Hexagonal Architecture](https://alistair.cockburn.us/hexagonal-architecture/)
- Omnihive refactoring plan (Phase 0-4)
