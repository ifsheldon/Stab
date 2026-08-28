# Stab Product Architecture

This directory defines Stab's active product architecture.

The active migration sequence, tests, benchmarks, and release criteria live in [the Stim core parity and lean evidence plan](../plans/stim-core-parity-and-lean-evidence-plan.md).

The source-current public Rust migration guide is [Migrating to Stab 0.2](../MIGRATING-0.2.md). Superseded extraction maps, migration ledgers, and package baselines remain available in Git history.

The stable byte contract for source-model identities is defined by [model fingerprint schema version 1](model-fingerprint-schema-v1.md).

The stable byte contract for backend-neutral compiler inputs is defined by [compilation request fingerprint schema version 1](compilation-request-fingerprint-schema-v1.md).

The stable byte contract for backend-bearing executable identities is defined by [plan fingerprint schema version 1](plan-fingerprint-schema-v1.md).

The current successful machine-output contracts are [capabilities schema version 5](agent-cli-capabilities-schema-v5.md), [plan schema version 4](agent-cli-plan-schema-v4.md), and inspect in [agent CLI schema version 2](agent-cli-schema-v2.md). Superseded schemas remain available in Git history and are not emitted as compatibility views.

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
| Facade | Finite canonical root contracts and direct component namespaces | Algorithms, duplicate models, universal errors, qualification plans |
| CLI | Argument parsing, file-role preflight, sink construction, rendering | Quantum algorithms |
| Ops | Tests, oracle, qualification, benchmarks, release operations | Product runtime behavior |

Detailed component contracts use [the component contract template](component-contracts.md).

## Permitted Dependencies

The current product graph after the A8 circuit-pass and backend-selection boundary is:

```text
stab-cli -> stab-analysis + stab-bits + stab-engine + stab-model + stab-records

stab-reference-decoder -> stab-decoder + stab-model + stab-records
stab-reference-noise-pass -> stab-analysis + stab-model
stab-decoder -> stab-model + stab-records
stab-core -> stab-engine + stab-analysis + stab-model + stab-algebra + stab-records + stab-decoder
stab-engine -> stab-model + stab-records + stab-algebra + stab-analysis + stab-bits
stab-analysis -> stab-model + stab-algebra
stab-model -> no Stab crate
stab-algebra -> stab-bits
stab-records -> stab-bits
```

`stab-bits`, `stab-records`, `stab-algebra`, `stab-model`, `stab-analysis`, `stab-engine`, and `stab-decoder` are physical Cargo packages with one owner per capability. `stab-core` reexports common owned values and aliases `analysis`, `decoder`, and `execution` directly to their component crates. Low-level storage, codecs, traversal, algorithms, and extension contracts remain in their owners. Dependency arrows point from a consumer to its dependency:

```text
stab-kernels-simd -> no Stab crate

stab-bits --portable-simd--> stab-kernels-simd
stab-records -> stab-bits
stab-algebra -> stab-bits
stab-algebra --portable-simd--> stab-kernels-simd
stab-model -> no Stab crate
stab-analysis -> stab-model + stab-algebra
stab-engine -> stab-model + stab-records + stab-algebra + stab-analysis + stab-bits
stab-decoder -> stab-model + stab-records
stab-core -> stab-engine + stab-analysis + stab-model + stab-algebra + stab-records + stab-decoder
stab-cli -> stab-analysis + stab-bits + stab-engine + stab-model + stab-records

ops -> product crates
product crates -X-> ops
```

The unpublished `stab-reference-decoder` proves decoder composition using only `stab-decoder`, `stab-model`, and `stab-records` at runtime; its analysis and engine dependencies are test-only experiment fixtures. The unpublished `stab-reference-noise-pass` proves circuit-pass composition using only `stab-analysis` and `stab-model`. Neither proof crate can depend on `stab-core`, CLI, ops, private modules, or Nightly features.

`just architecture::check` derives package identity, Stable status, allowed product dependencies, binary targets, and protected package names from one product-contract table. It enforces every workspace edge, rejects product dependencies on operational crates or external packages impersonating reserved Stab product names, permits only the source-owned public-component edges required by each test-support proof, rejects Stable defaults that reach portable SIMD, and requires each Stable component manifest to declare the exact Rust 1.97.1 minimum. It also parses every GitHub Actions workflow and rejects mutable remote action and reusable-workflow refs; remote actions require full commit SHAs, Docker actions require `sha256` digests, and repository-local actions remain source-bound to the checked-out commit.

The checker classifies workspace packages as product, operations, or test support from their repository paths, resolves Cargo metadata with all features enabled so optional edges cannot hide, validates every workspace dependency edge, and rejects test-support dependencies on product or operations code except the exact public-component edges assigned to the two proof crates above. It retains resolved package identities for workspace and transitive dependencies, so a path, Git, or registry package that reuses a protected Stab package name cannot bypass the local dependency graph.

The checker parses facade and product Rust sources with `syn`. It requires the `analysis`, `decoder`, and `execution` crate aliases, rejects public facade modules, module path overrides, item-generating macros, exported macros, glob exports, and direct public definitions, and compares every common root reexport against `ops/architecture/facade-root-reexports.txt`. Portable-SIMD inspection follows direct, grouped, lexically scoped aliased, and macro-token `std` or `core` paths plus nested `cfg_attr` feature gates without treating comments or string literals as code.

The shared result-format corpus lives under `test-support/compat-corpus` and is available to product crates only as a development dependency. It is not a runtime architecture allowance.

Portable SIMD belongs only to the optional `stab-kernels-simd` product crate. Any product-to-ops edge, product runtime edge to test support, unapproved test-support upward edge, direct `std::simd` or `core::simd` source site, portable-SIMD feature gate outside that crate, mandatory Stable-component dependency on that crate, or Stable default feature reaching that crate fails the check. Every unstable Rust `feature(...)` gate is rejected in Stable components, including target-gated and nested `cfg_attr` forms.

`just architecture::consumer-check` compiles standalone Stable component, scalar facade, portable Nightly facade, and mixed direct-component consumer workspaces under `test-support/consumers/`. It checks their resolved feature graphs, including the absence of the kernel from both scalar graphs and exactly one kernel package with `portable-simd` enabled through the feature paths selected by each portable consumer.

`just architecture::docs-check` uses `pulldown-cmark` with GitHub Flavored Markdown extensions to recursively validate repository-owned Markdown links. It derives same-file and cross-file heading anchors using GitHub's Unicode stripping and duplicate-suffix rules, treats heading text such as `{#custom}` literally instead of enabling non-GFM custom heading attributes, resolves local targets without permitting traversal outside the repository, reports all failures in deterministic source order, skips explicit external schemes, and excludes generated, build, vendor, and Git trees.

## Toolchain Boundary

Rust 1.97.1 is the minimum supported Stable compiler for model, bits, records, scalar algebra, pure analysis, the current scalar engine, and decoder interoperability. The extracted `stab-bits`, `stab-records`, scalar-default `stab-algebra`, `stab-model`, `stab-analysis`, complete scalar `stab-engine`, and `stab-decoder` packages build on that compiler. The optional raw SIMD crate and consumers that enable it require the pinned Nightly target.

`stab-kernels-simd`, the complete `stab-core` facade, and `stab-cli` use the pinned Nightly compiler. `stab-engine` currently remains Stable-compatible because its only registered sampling backend is scalar.

Every direct portable-SIMD import and feature gate belongs to `stab-kernels-simd`.

Generic packed storage and scalar kernels live in Stable `stab-bits`, while quantum-specific scalar Clifford and Pauli-word kernels live in Stable `stab-algebra`. The former direct SIMD implementation has been removed from `stab-core`; the optional raw kernel crate accelerates only source-current measured leaf operations. It does not register a sampling backend because the engine has no distinct packed-frame plan yet.

Strict `01`, `b8`, `r8`, HITS, DETS, and PTB64 codecs live in Stable `stab-records` behind one public `RecordFormat` type. Generic materialized and streaming readers accept all six formats. Record-at-a-time writers are fallible because PTB64 requires complete 64-record groups, while batch codecs own grouped finalization.

The specialized `for_each_*` visitors remain bounded convenience adapters for callers already using records diagnostics. The generic `try_for_each_*` variants are the modular sink boundary because they preserve an arbitrary visitor error and stop immediately after the first callback failure; returning that error is the explicit cancellation mechanism.

Dense and packed readers apply HITS and DETS token events directly, so duplicate-heavy input cannot make their scratch grow beyond the declared width. Raw sparse and typed-token visitors intentionally preserve one record's token order and duplicates. `MeasurementCodecSink`, `DetectionCodecSink`, and `DemSampleCodecSink` are explicitly in-memory sinks, so their encoded output grows with requested output bytes while their additional scratch stays bounded by one active batch or PTB64 group.

`stab-kernels-simd` has no Stab dependency and accepts only raw word slices and fixed word blocks.

Stable components must compile without enabling or parsing Nightly-only code.

## Public Facade

- The facade root contains only the mechanically checked canonical contracts listed in `ops/architecture/facade-root-reexports.txt`. The inventory is limited to common values, their directly coupled errors, and sink/session contracts.
- `stab_core::analysis`, `stab_core::decoder`, and `stab_core::execution` are direct crate aliases with the same type identities as their owners.
- Sampling request estimation is engine-owned and available as `stab_engine::estimate_sampling_request` or `stab_core::execution::estimate_sampling_request`.
- Low-level storage and kernels use `stab-bits`; codecs and traversal use `stab-records` or `stab-model`; pure algorithms and extension contracts use `stab-analysis`; execution plans use `stab-engine`.
- `advanced`, `experimental`, duplicate owner-shaped modules, universal facade errors, and backend placeholders are absent.

The root inventory is intentionally smaller than the union of component APIs. Convenience does not imply facade ownership.

## Resource And Diagnostic Policy

Every materializing, expanding, searching, or executing operation performs typed admission against either a caller-selectable operation policy or a fixed non-overridable safety contract.

Default policies preserve current source-owned safe boundaries.

Semantic hard limits are not configurable.

A public policy is introduced only when callers can meaningfully choose the budget. The presence of an internal safety constant alone is not sufficient justification for another public limits type.

Domain errors retain typed context. Applications and the CLI compose them into structured diagnostics only at their workflow boundary.

Human rendering is for people.

Stable diagnostic codes and JSON rendering are for tools and agents.

## Extension Policy

The first supported extension seams are:

- typed circuit passes that admit conservative output projections before lowering and return validated Stim-compatible circuits;
- packed measurement and detection sinks;
- decoder sessions consuming detection batches;
- compile-time leaf-kernel selection.

Dynamic Rust libraries, runtime gate registration, serialized executable plans, and unimplemented backend placeholders are forbidden.

An extension seam is accepted only after a separate crate uses it without private or operational APIs.

The common circuit-pass executor admits the folded input before dispatch, admits each pass's conservative represented-item, target, argument, projected-payload, and repeat-nesting output projection before proportional lowering allocation, and validates the returned `Circuit` against both the caller policy and that projection. Projected payload excludes allocator metadata and spare collection capacity, so it is not a resident-memory claim. Pass-specific options, reports, and diagnostics remain associated types. Since the model dialect is closed, a research operation that cannot lower to ordinary Stim-compatible gates is rejected instead of registering a runtime instruction.

Requirements for a future process-isolated decoder are documented in [External Decoder Process Protocol Requirements](external-decoder-process-protocol-requirements.md); Stab 0.2 does not implement that transport.

## Decision Records

- [ADR 0001: Stim Dialect Remains Closed](adr-0001-closed-stim-dialect.md)
- [ADR 0002: Plans, Sessions, And Sinks](adr-0002-plan-session-sink.md)
- [ADR 0003: Typed Batch Families](adr-0003-typed-batch-families.md)
- [ADR 0004: Diagnostics And Resource Policies](adr-0004-diagnostics-and-resources.md)
- [A2 Resource Policy Inventory](a2-resource-policy-inventory.md)
- [ADR 0005: Backend Selection And Nightly Isolation](adr-0005-backends-and-nightly.md)
- [ADR 0006: Decoder And External Extension Boundaries](adr-0006-decoder-extension-boundaries.md)
- [External Decoder Process Protocol Requirements](external-decoder-process-protocol-requirements.md)
- [ADR 0007: Product Dependency Graph](adr-0007-product-dependency-graph.md)

## Change Rules

Architecture changes require:

1. an updated decision record or a superseding decision record;
2. architecture dependency checks;
3. semantic tests for the moved contract;
4. focused benchmarks for performance-sensitive boundaries;
5. synchronized public and generated documentation;
6. milestone audit and full code review before formal evidence.
