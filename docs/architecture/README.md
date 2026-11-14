# Stab Product Architecture

This directory defines Stab's active product architecture.

The active migration sequence, tests, benchmarks, and release criteria live in [the Stim core parity and lean evidence plan](../plans/stim-core-parity-and-lean-evidence-plan.md).

The source-current public Rust migration guide is [Migrating to Stab 0.2](../MIGRATING-0.2.md). Superseded extraction maps, migration ledgers, and package baselines remain available in Git history.

The stable byte contract for source-model identities is defined by [model fingerprint schema version 1](model-fingerprint-schema-v1.md).

The stable byte contract for backend-neutral compiler inputs is defined by [compilation request fingerprint schema version 1](compilation-request-fingerprint-schema-v1.md).

The stable byte contract for backend-bearing executable identities is defined by [plan fingerprint schema version 1](plan-fingerprint-schema-v1.md).

The current successful machine-output contracts are [capabilities schema version 6](agent-cli-capabilities-schema-v6.md), [plan schema version 4](agent-cli-plan-schema-v4.md), and inspect in [agent CLI schema version 2](agent-cli-schema-v2.md). Superseded schemas remain available in Git history and are not emitted as compatibility views.

The E4 classification of every resource limit, including each removed one, is recorded in [the resource policy inventory](a2-resource-policy-inventory.md).

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
| Model | Circuit and DEM values, the closed `GateKind` identity and its one gate semantics table, targets, IDs, parsing, printing, structural validation, folded circuit summaries | Simulation, analysis algorithms, CLI, files |
| Bits | Packed storage, checked views, scalar kernels | Quantum semantics, formats, execution |
| Records | Measurement and detection layouts, packed batches, codecs, sources, sinks | Circuit execution, filesystem paths |
| Algebra | Pauli, Clifford, Tableau, Flow semantics | Stim parsing, CLI, operational plans |
| SIMD kernels | Slice-level `fearless_simd` word kernels and the only dependency on the SIMD library | Models, codecs, policy, filesystem access, Stab types |
| Engine | Compilation, private IR, plans, sessions, sampling, detection conversion, DEM sampling | Text codecs, paths, CLI, ops |
| Analysis | Pure circuit and DEM transforms, search, generation | CLI, operational plans, mutable execution sessions |
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
stab-algebra -> stab-bits + stab-kernels-simd
stab-records -> stab-bits
stab-bits -> stab-kernels-simd
stab-kernels-simd -> no Stab crate
```

`stab-kernels-simd`, `stab-bits`, `stab-records`, `stab-algebra`, `stab-model`, `stab-analysis`, `stab-engine`, and `stab-decoder` are physical Cargo packages with one owner per capability. `stab-core` reexports common owned values and aliases `analysis`, `decoder`, and `execution` directly to their component crates. Low-level storage, codecs, traversal, algorithms, and extension contracts remain in their owners. Dependency arrows point from a consumer to its dependency:

```text
stab-kernels-simd -> no Stab crate

stab-bits -> stab-kernels-simd
stab-records -> stab-bits
stab-algebra -> stab-bits + stab-kernels-simd
stab-model -> no Stab crate
stab-analysis -> stab-model + stab-algebra
stab-engine -> stab-model + stab-records + stab-algebra + stab-analysis + stab-bits
stab-decoder -> stab-model + stab-records
stab-core -> stab-engine + stab-analysis + stab-model + stab-algebra + stab-records + stab-decoder
stab-cli -> stab-analysis + stab-bits + stab-engine + stab-model + stab-records

ops -> product crates
product crates -X-> ops
```

The `stab-engine -> stab-analysis` edge carries three analysis algorithms and their error type, not gate vocabulary, which `stab-model` owns: detector-frame lowering decomposes `SPP` and `SPP_DAG` through `visit_decomposed_spp_instructions`, reference-sample Clifford kernels take each gate's local tableau from `single_qubit_clifford_for_gate` and `gate_tableau` because tableaus are `stab-algebra` values that the model cannot name, and sampled-flow checking removes noise with `circuit_without_noise` and resolves record indices with `flow_record_index`. Engine errors wrap `AnalysisError` from those calls.

The unpublished `stab-reference-decoder` proves decoder composition using only `stab-decoder`, `stab-model`, and `stab-records` at runtime; its analysis and engine dependencies are test-only experiment fixtures. The unpublished `stab-reference-noise-pass` proves circuit-pass composition using only `stab-analysis` and `stab-model`. Neither proof crate can depend on `stab-core`, CLI, ops, private modules, or unstable Rust features.

`just architecture::check` derives package identity, allowed product dependencies, binary targets, and protected package names from one product-contract table. It enforces every workspace edge, rejects product dependencies on operational crates or external packages impersonating reserved Stab product names, permits only the source-owned public-component edges required by each test-support proof, rejects any resolved `fearless_simd` or `fearless_simd_macros` dependency outside `stab-kernels-simd`, and requires every product manifest and both proof-crate manifests to declare the exact Rust 1.100 minimum. It also parses every GitHub Actions workflow and rejects mutable remote action and reusable-workflow refs; remote actions require full commit SHAs, Docker actions require `sha256` digests, and repository-local actions remain source-bound to the checked-out commit.

The checker classifies workspace packages as product, operations, or test support from their repository paths, resolves Cargo metadata with all features enabled so optional edges cannot hide, validates every workspace dependency edge, and rejects test-support dependencies on product or operations code except the exact public-component edges assigned to the two proof crates above. It retains resolved package identities for workspace and transitive dependencies, so a path, Git, or registry package that reuses a protected Stab package name cannot bypass the local dependency graph.

The checker parses facade and product Rust sources with `syn`. It requires the `analysis`, `decoder`, and `execution` crate aliases, rejects public facade modules, module path overrides, item-generating macros, exported macros, glob exports, and direct public definitions, and compares every common root reexport against `ops/architecture/facade-root-reexports.txt`. Feature-gate inspection follows nested and target-gated `cfg_attr` forms without treating comments or string literals as code.

The shared result-format corpus lives under `test-support/compat-corpus` and is available to product crates only as a development dependency. It is not a runtime architecture allowance.

The SIMD library belongs only to the mandatory `stab-kernels-simd` product crate, as recorded in [ADR 0008](adr-0008-unconditional-fearless-simd-kernels.md). Any product-to-ops edge, product runtime edge to test support, unapproved test-support upward edge, or normal, development, or build dependency on `fearless_simd` or `fearless_simd_macros` from another package fails the check, including renamed dependencies, because the rule runs on resolved Cargo metadata. Every unstable Rust `feature(...)` gate is rejected in every product crate and both proof crates, including target-gated and nested `cfg_attr` forms.

`just architecture::consumer-check` compiles and tests the standalone component-only (`stable`), facade (`facade`), and mixed direct-component (`mixed`) consumer workspaces under `test-support/consumers/` with the pinned Rust 1.100.0 toolchain; each declares the 1.100 minimum. It requires exactly one resolved `stab-kernels-simd` package in each graph, requires `stab-core` in the facade and mixed graphs, and rejects it in the component-only graph.

`just architecture::docs-check` uses `pulldown-cmark` with GitHub Flavored Markdown extensions to recursively validate repository-owned Markdown links. It derives same-file and cross-file heading anchors using GitHub's Unicode stripping and duplicate-suffix rules, treats heading text such as `{#custom}` literally instead of enabling non-GFM custom heading attributes, resolves local targets without permitting traversal outside the repository, reports all failures in deterministic source order, skips explicit external schemes, and excludes generated, build, vendor, and Git trees.

## Toolchain Boundary

The workspace builds with one pinned Stable toolchain, Rust 1.100.0, recorded in `rust-toolchain.toml`. Every product crate, including `stab-core`, `stab-cli`, and `stab-kernels-simd`, declares Rust 1.100 as its minimum supported version through `[workspace.package]`, and there is no Nightly package class.

No product crate may enable an unstable Rust feature; `just architecture::check` rejects any `feature(...)` gate in product and proof-crate sources.

Generic packed storage lives in Stable `stab-bits`, and quantum-specific Clifford and Pauli-word operations live in Stable `stab-algebra`. A `stab-algebra` Tableau stores its generator images as packed X and Z row planes with one sign plane, conjugates 64 rows per block so each generator row is read once per block, and uses `stab-bits` matrices for transposition, so composition, inversion, random sampling, and the stabilizer solver are word-parallel with no width cap; values built from a requested size are admitted against the crate's own `AlgebraMemoryBudget`, which keeps the dependency graph unchanged. Whole-slice XOR in `stab-bits` and Clifford right multiplication in `stab-algebra` always run through the `fearless_simd` kernels in `stab-kernels-simd`, which select the instruction set once per call; the remaining word loops, including the single-block XOR and masked XOR, are scalar. The kernels accelerate only measured leaf operations. Circuit sampling, detection sampling, measurement-to-detection conversion, and sweep correction all run one generic Pauli-frame batch with scalar block operations, 256 lanes and one `BitBlock` per plane by default; circuit-sampling and detection-sampling sessions may instead be created with a 64-lane `LaneWidth`, one `u64` per plane, which the CLI selects for small requests ([ADR 0003](adr-0003-typed-batch-families.md)); E7 measured 1024-lane rows driven by the slice kernels and did not adopt them because the end-to-end cases did not improve, so the engine registers no SIMD sampling backend.

Strict `01`, `b8`, `r8`, HITS, DETS, and PTB64 codecs live in Stable `stab-records` behind one public `RecordFormat` type. Materialized readers and the one packed `RecordStreamReader` accept all six formats. The stream reader decodes straight into the words of a reusable `PackedShotBatch`: sparse formats set only hit bits, `01` packs eight bytes per step, `b8` copies bytes, and PTB64 transposes whole groups. It reads through one buffer that grows only while a record frame spans it, allocates record storage fallibly and only after input proves the first record exists, and delivers every record decoded before a failure ahead of that failure. Its framing options carry the `m2d` and `sample_dem` replay diagnostics (line offsets, newline and length checks, `b8` truncation, empty zero-width sweeps, set-semantics HITS), so the CLI contains no format-specific decode loop. Every product output route encodes through one `RecordEncoder`, which takes packed records as single records or as column groups that are each shot-major records or bit planes of any shot count, and drains the encoded bytes into any `io::Write`. Bit planes are the layout samplers produce; the encoder copies their words straight into PTB64 groups and transposes one 64-shot block per column group for every other format, so no engine path re-chunks or transposes planes before a sink sees them ([ADR 0003](adr-0003-typed-batch-families.md)). It owns record separators, DETS namespaces, B8 tail masking, and PTB64 grouping; sparse formats visit only set bits, and its retained state between calls is at most one incomplete PTB64 group, which callers must reject at finalization. The per-bit `MeasureRecordWriter` remains as the Stim writer-contract mirror and is fallible because PTB64 requires complete 64-record groups.

Materialized and streaming readers apply HITS and DETS token events directly, so duplicate-heavy input cannot make their scratch grow beyond the declared width. `MeasurementCodecSink`, `DetectionCodecSink`, and `DemSampleCodecSink` are explicitly in-memory sinks over `RecordEncoder`, so their encoded output grows with requested output bytes while their additional scratch stays bounded by one active batch or PTB64 group.

`stab-kernels-simd` has no Stab dependency and accepts only raw word slices.

Product crates must compile on the pinned Stable toolchain without enabling unstable features.

## Public Facade

- The facade root contains only the mechanically checked canonical contracts listed in `ops/architecture/facade-root-reexports.txt`. The inventory is limited to common values, their directly coupled errors, and sink/session contracts.
- `stab_core::analysis`, `stab_core::decoder`, and `stab_core::execution` are direct crate aliases with the same type identities as their owners.
- Sampling request estimation is engine-owned and available as `stab_engine::estimate_sampling_request` or `stab_core::execution::estimate_sampling_request`.
- Low-level storage and kernels use `stab-bits`; codecs and traversal use `stab-records` or `stab-model`; pure algorithms and extension contracts use `stab-analysis`; execution plans use `stab-engine`.
- `advanced`, `experimental`, duplicate owner-shaped modules, universal facade errors, and backend placeholders are absent.

The root inventory is intentionally smaller than the union of component APIs. Convenience does not imply facade ownership.

## Resource And Diagnostic Policy

Hostile-input protection means memory safety, no panics, checked arithmetic, bounded allocation, and bounded recursion.
It does not mean bounded CPU time, which belongs to the caller.

Every materializing or executing operation admits its conservative retained storage against one typed `MemoryBudget` before allocating, advancing randomness, or writing output, and reserves fallibly after admission.
The budget defaults to one gibibyte, is caller-selectable through each operation's limits type, and is exposed on the CLI as the one Stab-native `--memory-budget` option.

Format and representation invariants and the shared 256-level repeat-nesting envelope are not configurable resources.
Explicit search budgets with budget-exhausted results remain separate caller policy.
Count caps that only proxy memory and fixed work caps are not used; a compact input that fits the budget runs for as long as its work takes unless the caller cancels it.
Long-running library operations accept one `stab_model::Cancellation` signal on their limits or options value, poll it at bounded work intervals inside repeat interpretation and between batches, and stop with a typed `Cancelled` outcome.

The [resource policy inventory](a2-resource-policy-inventory.md) classifies every remaining limit constant and records each removed one.

Domain errors retain typed context. Applications and the CLI compose them into structured diagnostics only at their workflow boundary.

Human rendering is for people.

Stable diagnostic codes and JSON rendering are for tools and agents.

## Extension Policy

The first supported extension seams are:

- typed circuit passes that admit conservative output projections before lowering and return validated Stim-compatible circuits;
- packed measurement and detection sinks;
- decoder sessions consuming detection batches;
- unconditional leaf kernels with per-call instruction-set selection outside word loops.

Dynamic Rust libraries, runtime gate registration, serialized executable plans, and unimplemented backend placeholders are forbidden.

An extension seam is accepted only after a separate crate uses it without private or operational APIs.

The common circuit-pass executor admits the folded input before dispatch, admits each pass's conservative output projection, whose projected payload is charged to the caller's memory budget and whose repeat nesting is checked against the caller's nesting limit, before proportional lowering allocation, and validates the returned `Circuit` against both the caller policy and that projection. Projected payload excludes allocator metadata and spare collection capacity, so it is not a resident-memory claim. Pass-specific options, reports, and diagnostics remain associated types. Since the model dialect is closed, a research operation that cannot lower to ordinary Stim-compatible gates is rejected instead of registering a runtime instruction.

Requirements for a future process-isolated decoder are documented in [External Decoder Process Protocol Requirements](external-decoder-process-protocol-requirements.md); Stab 0.2 does not implement that transport.

## Decision Records

- [ADR 0001: Stim Dialect Remains Closed](adr-0001-closed-stim-dialect.md)
- [ADR 0002: Plans, Sessions, And Sinks](adr-0002-plan-session-sink.md)
- [ADR 0003: Typed Batch Families](adr-0003-typed-batch-families.md)
- [ADR 0004: Diagnostics And Resource Policies](adr-0004-diagnostics-and-resources.md)
- [Resource Policy Inventory](a2-resource-policy-inventory.md)
- [ADR 0005: Backend Selection And Nightly Isolation](adr-0005-backends-and-nightly.md), superseded by ADR 0008 for leaf-kernel selection and the toolchain boundary
- [ADR 0006: Decoder And External Extension Boundaries](adr-0006-decoder-extension-boundaries.md)
- [External Decoder Process Protocol Requirements](external-decoder-process-protocol-requirements.md)
- [ADR 0007: Product Dependency Graph](adr-0007-product-dependency-graph.md)
- [ADR 0008: Unconditional Leaf Kernels On Fearless SIMD](adr-0008-unconditional-fearless-simd-kernels.md)

## Change Rules

Architecture changes require:

1. an updated decision record or a superseding decision record;
2. architecture dependency checks;
3. semantic tests for the moved contract;
4. focused benchmarks for performance-sensitive boundaries;
5. synchronized public and generated documentation;
6. milestone audit and full code review before formal evidence.
