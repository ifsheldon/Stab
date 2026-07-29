# Stab Product Architecture

This directory defines Stab's active product architecture.

The detailed migration sequence, tests, benchmarks, and release criteria live in [the agent-native modular QEC plan](../plans/agent-native-modular-qec-architecture-plan.md).

The exact A6 source, API, feature, test, and benchmark relocation contract is frozen in [the A6 physical component extraction map](a6-component-extraction-map.md).

The starting package graph and public inventory are frozen in [the pre-0.2 baseline](pre-0.2-api-baseline.md).

Intentional public Rust API changes are tracked in [the Stab 0.2 API migration inventory](0.2-api-migration-inventory.md).

The stable byte contract for source-model identities is defined by [model fingerprint schema version 1](model-fingerprint-schema-v1.md).

The stable byte contract for backend-neutral compiler inputs is defined by [compilation request fingerprint schema version 1](compilation-request-fingerprint-schema-v1.md).

The stable byte contract for backend-bearing executable identities is defined by [plan fingerprint schema version 1](plan-fingerprint-schema-v1.md).

The current successful machine-output contract for Stab-native discovery, inspection, and planning commands is defined by [agent CLI schema version 2](agent-cli-schema-v2.md). [Schema version 1](agent-cli-schema-v1.md) remains historical.

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

The current A6 product graph after complete model and analysis extraction plus the first engine foundation slice is:

```text
stab-cli -> stab-core
stab-core -> stab-engine + stab-analysis + stab-model + stab-algebra + stab-bits + stab-records
stab-engine -> stab-model
stab-analysis -> stab-model + stab-algebra
stab-model -> stab-algebra
stab-algebra -> stab-bits
stab-records -> stab-bits
```

`stab-bits`, `stab-records`, `stab-algebra`, `stab-model`, `stab-analysis`, and `stab-engine` are physical Cargo packages. `stab-model` owns the complete circuit and DEM compatibility models. `stab-analysis` owns every implemented pure analysis slice: gate-to-algebra semantic projections, recursive circuit and DEM tag stripping, full-circuit tableau conversion, simplification, decomposition, bounded circuit and DEM flattening, DEM probability rounding, SAT/WCNF materialization, graphlike and hypergraph logical-error search, error matching and provenance values, noise removal, repetition/surface/color circuit generation, MBQC decomposition, unsigned flow checking/generation/solving, sparse reverse-frame tracking, unitary and selected QEC inversion, tracker-driven flow reversal, bounded feedback lowering, detecting regions, missing-detector analysis, circuit-to-DEM lowering, loop folding, and XYZ error-probability decomposition. The first `stab-engine` slice owns backend-neutral compilation-request fingerprints and execution-side biased randomization; `stab-core` keeps only thin compatibility reexports for those paths. Circuit sampling, measurement-to-detection conversion, circuit detection sampling, and DEM sampling still physically reside in `stab-core` while exposing operation-specific compiler, immutable-plan, mutable-session, cancellation, progress, and typed-sink APIs through `stab_core::execution`; their extraction is the active next work. The completed target graph below remains normative for A6 work. Dependency arrows point from a consumer to its dependency:

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

`just architecture::check` currently enforces every edge that exists in the workspace, rejects product dependencies on operational crates, permits test-support dependencies only as development edges, and will enforce the remaining target edges as later component crates are extracted.

The checker classifies workspace packages as product, operations, or test support from their repository paths, resolves Cargo metadata with all features enabled so optional edges cannot hide, validates every workspace dependency edge, and rejects upward dependencies from test support into product or operations code.

The shared result-format corpus lives under `test-support/compat-corpus` and is available to product crates only as a development dependency. It is not a runtime architecture allowance.

Portable SIMD is currently absent from product code after the scalar algebra extraction removed the former direct implementation. Any product-to-ops edge, product runtime edge to test support, test-support upward edge, or direct `std::simd` source site outside the future `stab-kernels-simd` crate fails the check.

The record-boundary and Nightly-isolation milestones remove these allowances; they are not permanent permitted dependencies.

## Toolchain Boundary

Rust 1.97.1 is the minimum supported Stable compiler for model, bits, records, scalar algebra, and pure analysis components. The extracted `stab-bits`, `stab-records`, scalar-default `stab-algebra`, `stab-model`, and `stab-analysis` packages build on that compiler. The current fingerprint and probability-only `stab-engine` foundation also compiles on Stable 1.97.1, but completed engine and backend integration retain the pinned Nightly target.

`stab-kernels-simd`, `stab-engine`, the complete `stab-core` facade, and `stab-cli` use the pinned Nightly compiler.

Every direct `std::simd` use will belong to `stab-kernels-simd` after A6.

Generic packed storage and scalar kernels live in Stable `stab-bits`, while quantum-specific scalar Clifford and Pauli-word kernels live in Stable `stab-algebra`. The former direct SIMD implementation has been removed from `stab-core`; portable SIMD is registered again only after the dependency-free raw kernel crate provides a distinct tested implementation.

Strict `01`, `b8`, `r8`, HITS, DETS, and PTB64 codecs now live in Stable `stab-records`. `SampleFormat` remains the five-format compatibility enum used by legacy record-at-a-time APIs, while `RecordFormat` is the six-format component registry that also represents PTB64. The overlap is explicit migration debt rather than an assertion that the two enums are interchangeable.

The specialized `for_each_*` visitors remain bounded convenience adapters for callers already using records diagnostics. The generic `try_for_each_*` variants are the modular sink boundary because they preserve an arbitrary visitor error and stop immediately after the first callback failure; returning that error is the explicit cancellation mechanism.

Dense and packed readers apply HITS and DETS token events directly, so duplicate-heavy input cannot make their scratch grow beyond the declared width. Raw sparse and typed-token visitors intentionally preserve one record's token order and duplicates. `MeasurementCodecSink`, `DetectionCodecSink`, and `DemSampleCodecSink` are explicitly in-memory sinks, so their encoded output grows with requested output bytes while their additional scratch stays bounded by one active batch or PTB64 group.

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
