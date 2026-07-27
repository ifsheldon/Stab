# Stab Product Architecture

This directory defines Stab's active product architecture.

The detailed migration sequence, tests, benchmarks, and release criteria live in [the agent-native modular QEC plan](../plans/agent-native-modular-qec-architecture-plan.md).

The starting package graph and public inventory are frozen in [the pre-0.2 baseline](pre-0.2-api-baseline.md).

Intentional public Rust API changes are tracked in [the Stab 0.2 API migration inventory](0.2-api-migration-inventory.md).

The stable byte contract for source-model identities is defined by [model fingerprint schema version 1](model-fingerprint-schema-v1.md).

The stable byte contract for backend-neutral compiler inputs is defined by [compilation request fingerprint schema version 1](compilation-request-fingerprint-schema-v1.md).

The successful machine-output contract for Stab-native discovery, inspection, and planning commands is defined by [agent CLI schema version 1](agent-cli-schema-v1.md).

The A2 decision for each caller-selectable, fixed, semantic, representational, and implementation resource boundary is recorded in [the A2 resource policy inventory](a2-resource-policy-inventory.md).

## Architectural Center

```text
Stim source bytes
    -> parsed model
    -> structural validation
    -> explicit transforms and analysis
    -> private canonical executable IR
    -> backend-specific immutable plan
    -> mutable reusable session
    -> typed packed batches
    -> codecs, decoders, statistics, or other sinks
```

The primary product boundary is:

> typed models -> explicit passes -> opaque plans -> reusable sessions -> typed streams

## Compatibility Boundary

Stim v1.16.0 remains the frozen compatibility target.

`Circuit`, `DetectorErrorModel`, Stim gate syntax, and result formats are closed compatibility models.

Custom research behavior lowers into those models through typed passes.

Stab does not extend the Stim gate table through a runtime registry.

## Product Components

| Component | Owns | Must not own |
| --- | --- | --- |
| Model | Circuit and DEM values, gate syntax, targets, IDs, parsing, printing, structural validation | Simulation, analysis algorithms, CLI, files |
| Bits | Packed storage, checked views, scalar kernels | Quantum semantics, formats, execution |
| Records | Measurement and detection layouts, packed batches, codecs, sources, sinks | Circuit execution, filesystem paths |
| Algebra | Pauli, Clifford, Tableau, Flow semantics | Stim parsing, CLI, operational plans |
| SIMD kernels | Portable-SIMD kernels over packed storage | Models, codecs, policy, filesystem access |
| Engine | Compilation, private IR, plans, sessions, sampling, detection conversion, DEM sampling over shared analysis lowering | Text codecs, paths, CLI, ops |
| Analysis | Pure circuit and DEM transforms, search, generation, error matching | CLI, operational plans, mutable execution sessions |
| Decoder API | Detection input and prediction output contracts | Decoder implementations, simulation internals |
| Facade | Curated ergonomic composition and conveniences | Qualification plans |
| CLI | Argument parsing, file-role preflight, sink construction, rendering | Quantum algorithms |
| Ops | Tests, oracle, qualification, benchmarks, release operations | Product runtime behavior |

Detailed component contracts use [the component contract template](component-contracts.md).

## Permitted Dependencies

The current A3 product graph is:

```text
stab-cli -> stab-core
stab-core -> stab-bits
```

`stab-bits` was physically extracted at revision `3de29da0c177c150f74b1fa93ed5217db186ead1`. `stab-records` and the remaining target component crates have not yet been extracted. The completed target graph below remains normative for later A3 through A6 work. Dependency arrows point from a consumer to its dependency:

```text
stab-kernels-simd -> no Stab crate

stab-bits --portable-simd--> stab-kernels-simd
stab-records -> stab-bits
stab-algebra -> stab-bits
stab-algebra --portable-simd--> stab-kernels-simd
stab-model -> stab-algebra
stab-analysis -> stab-model + stab-algebra
stab-engine -> stab-model + stab-records + stab-algebra + stab-analysis + stab-kernels-simd
stab-decoder -> stab-model + stab-records
stab-core -> all product components
stab-cli -> stab-core

ops -> product crates
product crates -X-> ops
```

`just architecture::check` currently enforces every edge that exists in the workspace, rejects product dependencies on operational crates, and will enforce the target edges as the component crates are extracted.

The checker classifies workspace packages from their repository paths, resolves Cargo metadata with all features enabled so optional edges cannot hide, validates every workspace dependency edge, and rejects product dependencies on operational crates.

During the pre-0.2 migration it reports three exact temporary allowances instead of hiding them: the dev-only `stab-core` and `stab-cli` dependencies on `stab-compat-corpus`, plus direct portable-SIMD use in `crates/stab-core/src/bits/clifford.rs`.

Any additional product-to-ops edge or direct `std::simd` source site fails the check.

The record-boundary and Nightly-isolation milestones remove these allowances; they are not permanent permitted dependencies.

## Toolchain Boundary

Rust 1.97.1 is the minimum supported Stable compiler for model, bits, records, scalar algebra, and pure analysis components. The extracted `stab-bits` package already builds and tests on that compiler.

`stab-kernels-simd`, `stab-engine`, the complete `stab-core` facade, and `stab-cli` use the pinned Nightly compiler.

Every direct `std::simd` use will belong to `stab-kernels-simd` after A6.

Generic packed storage and scalar kernels now live in Stable `stab-bits`. The remaining direct SIMD site is the quantum-specific Clifford kernel in `stab-core`; it moves behind the later kernel boundary without making Stable storage depend on Nightly.

`stab-kernels-simd` has no Stab dependency and accepts only raw word slices and fixed word blocks.

Stable components must compile without enabling or parsing Nightly-only code.

## Public API Tiers

- Facade root: common owned models, compilers, plans, sessions, batches, diagnostics, and policies.
- `advanced`: supported low-level storage, layout, explicit backend, and bounded traversal APIs.
- `experimental`: extension contracts that have real implementations but may change before 1.0.

Default root reexports are curated.

The qualification inventory records every exported item, but inventory ownership does not imply that every item belongs at the facade root.

## Resource And Diagnostic Policy

Every materializing, expanding, searching, or executing operation performs typed admission against either a caller-selectable operation policy or a fixed non-overridable safety contract.

Default policies preserve current source-owned safe boundaries.

Semantic hard limits are not configurable.

A public policy is introduced only when callers can meaningfully choose the budget. The presence of an internal safety constant alone is not sufficient justification for another public limits type.

Domain errors retain typed context and convert losslessly into structured diagnostics.

Human rendering is for people.

Stable diagnostic codes and JSON rendering are for tools and agents.

## Extension Policy

The first supported extension seams are:

- typed circuit passes that return validated Stim-compatible circuits;
- packed measurement and detection sinks;
- decoder sessions consuming detection batches;
- compile-time backend selection.

Dynamic Rust libraries, runtime gate registration, serialized executable plans, and unimplemented backend placeholders are forbidden.

An extension seam is accepted only after a separate crate uses it without private or operational APIs.

## Decision Records

- [ADR 0001: Stim Dialect Remains Closed](adr-0001-closed-stim-dialect.md)
- [ADR 0002: Plans, Sessions, And Sinks](adr-0002-plan-session-sink.md)
- [ADR 0003: Typed Batch Families](adr-0003-typed-batch-families.md)
- [ADR 0004: Diagnostics And Resource Policies](adr-0004-diagnostics-and-resources.md)
- [A2 Resource Policy Inventory](a2-resource-policy-inventory.md)
- [ADR 0005: Backend Selection And Nightly Isolation](adr-0005-backends-and-nightly.md)
- [ADR 0006: Decoder And External Extension Boundaries](adr-0006-decoder-extension-boundaries.md)

## Change Rules

Architecture changes require:

1. an updated decision record or a superseding decision record;
2. architecture dependency checks;
3. semantic tests for the moved contract;
4. focused benchmarks for performance-sensitive boundaries;
5. synchronized public and generated documentation;
6. milestone audit and full code review before formal evidence.
