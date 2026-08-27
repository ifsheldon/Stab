# A6 Physical Component Extraction Map

This document freezes the source, API, feature, dependency, test, and benchmark moves for milestone A6 of the [agent-native modular QEC architecture plan](../plans/agent-native-modular-qec-architecture-plan.md).

Status: Historical. The component extraction remains implemented, but the superseded complete-matrix and focused-attestation executable lifecycle was retired during the pre-0.2 entropy pass. Its [measurement contract](../../benchmarks/archive/a6/measurement-contract.json) and [predecessor registry](../../benchmarks/archive/a6/predecessors.json) are preserved byte-for-byte, while the current scalar-versus-portable SIMD compare and report commands remain active.

The map is deliberately written before source relocation. Its purpose is to keep compiler-driven repair from silently changing the intended component boundaries.

## Decision Summary

A6 extracts five implementation owners from `stab-core`:

1. `stab-algebra`
2. `stab-model`
3. `stab-analysis`
4. `stab-engine`
5. `stab-kernels-simd`

`stab-core` remains a Nightly-oriented facade over those owners plus `stab-bits` and `stab-records`.

The extraction is a move, not a copy. Every implementation has one canonical crate after its extraction step. Facade compatibility modules may reexport or adapt canonical APIs, but they may not retain a second parser, transform, compiler, session, codec, or kernel implementation.

## Why This Order

The dependency graph is acyclic only when extraction follows semantic ownership:

```text
stab-kernels-simd -> no Stab crate

stab-bits --portable-simd--> stab-kernels-simd
stab-records -> stab-bits
stab-algebra -> stab-bits
stab-algebra --portable-simd--> stab-kernels-simd
stab-model -> no Stab crate
stab-analysis -> stab-model + stab-algebra
stab-engine -> stab-model + stab-records + stab-algebra + stab-analysis
stab-core -> stab-engine + stab-analysis + stab-model + stab-algebra + stab-bits + stab-records
stab-cli -> stab-analysis + stab-bits + stab-engine + stab-model + stab-records
```

The policy reserves the future `stab-decoder` package name, but A6 does not claim that crate or an edge to it. A7 must add the implementation and update the graph from source-current Cargo metadata.

Algebra moves before the model because its production implementation no longer imports `Gate`. Model syntax remains independent of algebra values, while analysis and engine code may project model syntax into algebra-owned semantics without creating a reverse edge.

Model moves before analysis because transforms, search, generation, and semantic gate projections consume model values. Algorithmic inherent methods must leave model types before this step because Rust cannot add inherent methods to a type owned by another crate.

Analysis moves before engine because execution reuses canonical lowering and gate semantics. Analysis must not call a simulator to answer a nominally pure query.

The raw SIMD crate is physically created before the optional feature is enabled, but backend registration happens only after scalar and SIMD implementations are independently selectable and semantically equivalent.

## Shared Boundary Decisions

### Diagnostics And Resources

`stab-model` owns stable byte spans, parse diagnostics, structural validation diagnostics, and the shared resource-estimate vocabulary required by model and downstream operations.

`stab-records` continues to own result-format diagnostics. `stab-algebra`, `stab-analysis`, and `stab-engine` own their domain errors.

`stab-core::CircuitError` becomes a facade compatibility error assembled through lossless `From` conversions. Component crates must not depend on that facade error.

This placement avoids a new miscellaneous utility crate while preserving the permitted dependency graph. If a diagnostic is not needed by model or its consumers, it remains with its actual owner instead of moving into the shared vocabulary.

### Filesystem APIs

Model crates accept and emit bytes or text. Compatibility helpers that directly open or create filesystem paths remain in the facade or CLI.

This keeps explicit file-role validation and retained-handle safety in the CLI boundary and prevents the model from acquiring filesystem policy.

### Advanced Model Traversal

Folded DEM traversal remains model-owned because it describes compact model structure rather than one analysis algorithm.

Cross-crate consumers use a documented `stab_model::advanced` boundary containing borrowed folded blocks, traversal state, checked summaries, and visitor contracts. The boundary remains typed and read-only; it does not expose model storage fields.

Repeat selections carry only model facts and traversal ceilings. When cumulative expansion exceeds a ceiling, the visitor constructs its owner-domain failure; model traversal does not name logical-search, SAT, execution, or facade resource operations.

### Compatibility Methods

The following pre-0.2 inherent methods cannot survive on reexported foreign types:

- `Gate::{tableau, has_tableau, flows, has_flows, unitary_matrix, has_unitary_matrix, h_s_cx_m_r_decomposition, has_h_s_cx_m_r_decomposition}`
- `GateDecomposition::to_circuit`
- `Circuit::{without_tags, to_tableau, inverse_unitary, inverse_qec, inverse_qec_with_options, time_reversed_for_flows, time_reversed_for_flows_with_options, simplified, flattened, flattened_operations, without_noise, decomposed, with_inlined_feedback}`
- `DetectorErrorModel::{without_tags, flattened, rounded}`
- `Circuit::{reference_sample, reference_sample_tree, count_determined_measurements}`

Their free functions are the canonical `0.2.0` replacements in `stab-analysis` or `stab-engine`. Internal, CLI, ops, test, and documentation call sites migrate before the owning type moves.

Filesystem conveniences are different: they may remain facade free functions because they compose model byte APIs with paths without pretending to be model-owned behavior.

## Source Move Table

### `stab-algebra`

| Current source | Destination | Public ownership and rationale |
| --- | --- | --- |
| `stab-core/src/stabilizers/mod.rs` | `stab-algebra/src/lib.rs` plus focused modules | Own the algebra root and curated exports. |
| `stab-core/src/stabilizers/pauli.rs` | `stab-algebra/src/pauli.rs` | Own Pauli bases, phases, signs, strings, and multiplication semantics. |
| `stab-core/src/stabilizers/clifford.rs` | `stab-algebra/src/clifford.rs` | Own single-qubit and packed Clifford values. |
| `stab-core/src/stabilizers/tableau.rs` | `stab-algebra/src/tableau.rs` | Own tableau construction, composition, inversion, and validation. |
| `stab-core/src/stabilizers/flow.rs` | `stab-algebra/src/flow.rs` | Own algebraic stabilizer-flow values without gate syntax. |
| `stab-core/src/stabilizers/iter.rs` | `stab-algebra/src/iter.rs` | Own algebra iterators. |
| `stab-core/src/stabilizers/conversions.rs` | `stab-algebra/src/conversions.rs` | Own conversions among algebra values. |
| `stab-core/src/stabilizers/unitary.rs` | `stab-algebra/src/unitary.rs` | Own unitary-to-tableau mathematics. |
| `stab-core/src/stabilizers/limits.rs` | `stab-algebra/src/limits.rs` | Own algebra-specific resource admission. |
| `stab-core/src/stabilizers/error.rs` | `stab-algebra/src/error.rs` | Own `StabilizerError` and `StabilizerResult`. |
| `stab-core/src/bits/scalar.rs` and the Pauli-word wrapper in `bits/mod.rs` | `stab-algebra/src/kernels/scalar.rs` | Keep quantum word semantics out of generic packed storage. |
| Scalar portions of `stab-core/src/bits/clifford.rs` | `stab-algebra/src/kernels/clifford.rs` | Provide the Stable default implementation and tail behavior. |

The crate depends on `stab-bits = { path = "../stab-bits", version = "=0.2.0" }`.

Its default feature set is empty. The additive `portable-simd` feature enables `stab-kernels-simd` and selects its raw kernel only when the caller explicitly asks for that implementation.

Canonical Pauli, Clifford, Flow, Tableau, iterator, solver, resource, and unitary-conversion qualification executes integration targets under `crates/stab-algebra/tests/`. The facade keeps only tests that exercise a real model, circuit, analysis, CLI, or reexport boundary. The generated inventory rejects an implemented `stab_algebra` API parent whose primary Cargo selector does not name `stab-algebra`; the same rule applies to `stab_bits`.

### `stab-model`

| Current source | Destination | Public ownership and rationale |
| --- | --- | --- |
| `ids.rs`, `target.rs` | `ids.rs`, `target.rs` | Own typed Stim identifiers, probabilities, circuit targets, and target-token `Pauli`. |
| `gate.rs`, `gate/metadata.rs` | `gate.rs`, `gate/metadata.rs` | Own the closed gate table, aliases, categories, argument rules, and target rules. |
| `gate/flows.rs`, `gate/unitary.rs`, `gate/decomposition.rs` | `gate/descriptors/*` | Own raw closed-table descriptors. Analysis converts these descriptors into algebra or model values. |
| `circuit.rs`, `circuit/counts.rs`, `circuit/iter.rs` | `circuit/*` | Own circuit values, structural counts, and structural iterators. |
| `circuit/parser.rs`, `circuit/parser/fast.rs`, `circuit/printing.rs` | `circuit/*` | Own exact byte parsing and canonical printing. |
| In-memory portions of `circuit/api.rs` | `circuit/api.rs` | Own mutation, repetition, structural counts, and coordinate queries. |
| `dem.rs`, `dem/api.rs`, `dem/coordinate_scan.rs`, `dem/drop_impl.rs`, `dem/parser.rs`, `dem/tag.rs`, `dem/traversal.rs` | `dem/*` | Own DEM values, syntax, canonical printing, compact traversal, coordinates, and iterative destruction. |
| `model_bytes.rs`, `model_parse.rs`, `model_tag.rs`, `source_text.rs`, `parse_limits.rs` | Focused model support modules | Own byte admission, opaque metadata, source spans, and parser resource limits. |
| `fingerprint.rs` | `fingerprint.rs` | Own schema-versioned structural circuit and DEM identities. |
| Model portions of `diagnostics.rs`, `resources.rs`, and `error.rs` | `diagnostics.rs`, `resources.rs`, `error.rs` | Replace facade-coupled construction errors with stable model-owned errors and shared typed context. |

The model crate has no Stab dependency. Raw gate descriptors remain model values, while every algebra-valued projection belongs to `stab-analysis`.

The following do not move into model:

- path-opening and path-creating helpers;
- gate-to-tableau, gate-to-flow, gate-to-unitary, and decomposition parsing adapters;
- materializing circuit or DEM transforms;
- graphlike search, SAT generation, error matching, or circuit-to-DEM analysis;
- reference sampling or determined-measurement counting.

The gate slice is physically complete. The canonical registry and raw descriptors live under `crates/stab-model/src/gate/`; `stab-core/src/gate.rs` contains only facade reexports, parser-facing delegation, and the test-only semantic surface contract. Semantic families remain a core qualification classification instead of a field on `GateInfo`, because they describe selected execution evidence rather than closed Stim syntax. `Gate::from_name`, validation, and generalized-inverse construction now return `ModelError`; facade call sites preserve their established `CircuitError` behavior through the existing lossless conversion.

The complete model slice is physically extracted. `stab-model` owns `Circuit`, `DetectorErrorModel`, exact byte parsing, canonical text and byte writing, compact iteration and traversal, structural counts and coordinates, opaque metadata, fingerprints, `ByteSpan`, parse codes and contexts, dialect identity, parser limits and their typed resource failures, structural `ValidationError`, `Estimate`, and `ResourceEstimate`; `stab-core` preserves public aliases, filesystem helpers, and aggregate-error conversion. The model resource and validation contexts are intentionally closed so adding a cause forces exhaustive facade conversion, while public operation and dimension enums remain non-exhaustive for callers. `stab_model::advanced` now contains only checked construction and folded-traversal seams required by the future analysis and engine crates, rather than temporary access from core-owned parsers. `stab_records::EncodedSizeEstimate` remains independent: the former facade-local generic `From` implementation cannot survive Rust's orphan rules after both types become foreign, so internal callers convert explicitly rather than introducing a forbidden model-to-records edge.

### `stab-analysis`

| Current source | Destination | Public ownership and rationale |
| --- | --- | --- |
| `analysis/*` | `analysis/*` or focused crate-root modules | Own semantic gate adapters and pure circuit or DEM adapters after deleting foreign inherent implementations. |
| `circuit_detecting_regions*` | `circuit/detecting_regions/*` | Own pure detecting-region analysis. |
| `circuit_feedback.rs` | `circuit/feedback.rs` | Own feedback inlining. |
| `circuit_flow*` | `circuit/flow/*` | Own flow generation, checking, solving, and transitions. |
| `circuit_generation*` | `circuit/generation/*` | Own repetition, surface, and color-code generation. |
| `circuit_inverse*` | `circuit/inverse/*` | Own unitary, QEC, and flow-aware inversion. |
| `circuit_missing_detectors*` | `circuit/missing_detectors/*` | Own missing-detector analysis. |
| `circuit_simplify.rs`, `circuit_tableau.rs`, `circuit_transforms.rs` | `circuit/*` | Own simplification, tableau lowering, decomposition, noise removal, and bounded flattening. |
| `dem/analyze*` | `circuit_to_dem.rs`, `circuit_to_dem/*` | Own circuit-to-DEM analysis, options, diagnostics, error-probability decomposition, and folded lowering. |
| `dem/arena_index.rs`, `dem/error_traversal.rs` | `dem/*` | Own analysis indexing and bounded error-mechanism traversal. |
| `dem/flatten.rs` | `dem/flatten.rs` | Own materialized DEM flattening and `DemFlattenLimits`. |
| `dem/sat*` | `dem/sat*` | Own SAT/WCNF materialization and `SatMaterializationLimits`. |
| `dem/graphlike*`, `dem/hyper*`, `dem/search_budget.rs` | `dem/*` | Own graph and hypergraph search and their operation-specific policies. |
| `error_matcher*`, `matched_error.rs` | `error_matcher/*` | Own error explanation and provenance values. |
| `mbqc_decomposition.rs` | `mbqc.rs` | Own MBQC decomposition. |
| `sparse_rev_frame_tracker*` | `sparse_reverse_tracker/*` | Own pure reverse tracking shared by flow and DEM analysis. |

Gate semantic projections, recursive circuit and DEM tag removal, full-circuit tableau conversion, simplification, decomposition, bounded circuit and DEM flattening, circuit and DEM flatten resource admission, DEM probability rounding, SAT/WCNF materialization and resource admission, graphlike and hypergraph logical-error search and resource admission, error matching and provenance values, noise removal, repetition/surface/color generation, MBQC decomposition, unsigned flow checking/generation/solving, sparse reverse-frame tracking, unitary and selected QEC inversion, tracker-driven flow reversal, bounded feedback lowering, detecting regions, missing-detector analysis, circuit-to-DEM analysis, folded analyzer lowering, and XYZ error-probability decomposition are physically extracted. Canonical detecting-region options and maps use the model-owned `CircuitTick` domain, while `stab-core` converts its established raw-tick DTOs at the facade boundary. `stab-core` wrappers retain the old aggregate error, generated-value, flow, inversion-option, resource, detecting-region, missing-detector, analyzer, DEM-transform, SAT, logical-search, matcher-entry, and matched-error DTO signatures, while the canonical Stable implementation and owning exact-output tests live in `stab-analysis`; generated-QEC semantic equivalence, mixed facade, CLI, and focused error-conversion tests remain in core. The pure analysis extraction is complete.

The crate depends only on exact-version `stab-model` and `stab-algebra` edges.

Any helper that samples, constructs a mutable execution frame, or depends on `stab-records` belongs in engine instead.

### `stab-engine`

| Current source | Destination | Public ownership and rationale |
| --- | --- | --- |
| `sampling.rs`, `sampling/*` | `sampling/*` | Own sampling compiler, immutable plan, session, private operations, execution frames, and compatibility sampler. |
| `execution/reference_sample_tree.rs` | `reference_sample_tree.rs` | Own bounded reference-tree construction, lookup, simplification, decompression, storage admission, and typed failures. |
| `execution/sampled_flow.rs` | `sampled_flow.rs` | Own simulator-backed sampled-flow execution, typed shot and randomness inputs, batching, compilation, and typed failures. |
| `detection.rs`, `detection/*` | `detection/*` | Own measurement-to-detection compilation, direct detector-frame and fused detection sampling, sessions, delivery, and limits. |
| `dem_sampler.rs`, `dem_sampler/*` | `dem_sampling/*` | Own DEM compilation, sampling, replay, sessions, and limits; the old facade files are deleted. |
| `probability_util.rs` | `probability.rs` | Own execution-side randomization helpers. |
| `compilation_fingerprint.rs` | `fingerprint.rs` | Own backend-neutral request fingerprints. |
| Engine-owned portions of `capabilities.rs` | `capabilities.rs` | Own compiler descriptors and actual plan implementation identity. |

The crate depends on exact-version `stab-model`, `stab-records`, `stab-algebra`, and `stab-analysis`.

It does not depend on `stab-kernels-simd`: the current sampling plans do not execute through the raw XOR or Clifford kernels, so a kernel edge would blur build-time leaf acceleration with engine backend ownership.

Execution code does not import `SampleFormat`, text codecs, filesystem paths, CLI types, or ops descriptors. Byte-oriented materializers that remain for compatibility live in the facade and delegate through typed sinks.

`detection/output.rs` does not move as written because it imports `SampleFormat` and byte writers. Its semantic detector and observable routing moves behind typed sinks, while byte-format compatibility wrappers remain in `stab-core`.

The scalar engine is physically extracted. `stab-engine` owns `CompilationOperation`, `CompilationRequestFingerprint`, `biased_randomize_bits`, source-owned descriptors for all four public compiler families, the sampling, detection, and DEM compilers, immutable plans, mutable sessions, direct-Z, small-frame, general-frame, deterministic reference samples, conversion reference and sweep state, direct detector-frame execution, fused sample-and-convert execution, lowered folded DEM execution, detector-only and sampled-error DEM sampling, incremental replay, reference-sample trees, sampled-flow execution, cancellation, progress, poisoning, and typed measurement, detection, and DEM-sample delivery. Descriptors explicitly report when a compiler has no public request-fingerprint identity instead of omitting the compiler or inventing an identity. Reference trees have private checked structure, exact logical-size and nesting admission, iterative random access and decompression, and fallible materialization. The crate root is the sole canonical public execution namespace. P2 deleted `CompiledSampler`, `CompiledDetectionConverter`, `CompiledDemSampler`, their callback or whole-output routes, byte encoding, facade-only helpers, and hidden engine bridges; all three execution families now have one compiler-plan-session-sink route.

### `stab-kernels-simd`

| Current source | Destination | Public ownership and rationale |
| --- | --- | --- |
| Restored portable-SIMD fixed-block kernels over `stab-bits` and `stab-algebra` scalar references | `stab-kernels-simd/src/lib.rs` | Own new direct `std::simd` code over raw four-word bit and Clifford blocks without absorbing tails, model values, or backend policy. |
| Measured four-word XOR and non-identity Clifford right-multiplication kernels introduced during A6 | Focused raw modules | Give the optional bit and algebra edges genuine implementations while keeping unmeasured masks, scans, transpose, and execution policy out of the raw crate. |

This crate has no Stab dependency. It is the only crate containing `#![feature(portable_simd)]` or direct `std::simd` paths.

Public functions accept fixed `[u64; 4]` word blocks and fixed arrays of Clifford planes. The crate does not expose circuits, gates, algebra values, layouts, policies, or backend enums.

### `stab-core` Facade

The facade retains:

- curated root reexports for common models, algebra values, compilers, plans, sessions, batches, diagnostics, and policies;
- `advanced` reexports for explicit storage, layout, backend, and bounded traversal APIs;
- an intentionally empty `experimental` namespace pending P2 deletion;
- remaining filesystem and byte-materializing detection and DEM compatibility conveniences;
- the byte-oriented portion of `detection/output.rs`, retained only until the detection adapter is removed;
- non-lossy `StabError` or `CircuitError` compatibility conversion;
- capability aggregation across model, records, engine, decoder, and pass descriptors.

The facade does not retain implementation modules after their owner moves.

The pre-0.2 compiled sampler, detection converter, and DEM sampler adapters are gone. Component-owned compilers, immutable plans, mutable sessions, and typed sinks are the only execution routes.

## Feature Map

| Package | Default features | `portable-simd` behavior |
| --- | --- | --- |
| `stab-bits` | Stable scalar | Enables measured raw packed-bit kernels from `stab-kernels-simd`. |
| `stab-records` | Stable | No direct feature; it follows the selected `stab-bits` implementation only through an explicit dependency feature if needed. |
| `stab-algebra` | Stable scalar | Enables raw Clifford kernels from `stab-kernels-simd`. |
| `stab-model` | Stable | No SIMD feature. |
| `stab-analysis` | Stable | No SIMD feature. |
| `stab-engine` | Scalar | Does not depend on the raw kernel crate and keeps explicit `PortableSimd` requests unavailable until a later packed-frame implementation supplies a distinct execution plan. |
| `stab-core` | Scalar by default | Forwards the additive feature to bits and algebra only; it does not reinterpret build-time leaf acceleration as sampling-backend selection. |
| `stab-cli`, `stab-oracle`, `stab-bench` | Scalar by default | `stab-cli` forwards its optional feature only to the packed-bit owner it consumes directly; oracle and benchmark packages may also enable facade feature paths needed by their direct consumers. Current qualification builds select scalar explicitly and the A6 diagnostic selects each variant independently. |

There is no `scalar` feature. Scalar behavior is defined by the absence of `portable-simd`.

## `ops-contracts` Removal

The product feature is removed rather than renamed.

The authoritative gate statistical plans, rejection boundaries, and qualification family lists live in `ops/oracle`. Core semantic tests may retain test-only statistical fixtures, but no product build or rustdoc surface exposes them.

DEM analyzer phase observations used only by benchmarks are not promoted into a product diagnostic API. The ops-owned benchmark adapter validates compact repeat blocks and recurrence detector shifts from the public `DetectorErrorModel` output, so benchmark evidence follows observable work without retaining counters in the timed algorithm.

The following names disappear from product rustdoc and qualification inventory:

- `__gate_contract_family_names`
- `__gate_contract_surface_names`
- `__gate_contract_statistical_plans`
- `__gate_contract_statistical_rejection_boundaries`
- `__circuit_to_detector_error_model_with_diagnostics`
- `ErrorAnalyzerDiagnostics` when it remains benchmark-only

### Sequence Amendment

This removal was completed before moving the full gate registry instead of after engine extraction. The implementation exposed a concrete dependency: `GateInfo` carried qualification-only semantic-family data, so preserving the original order would have copied an operations concern into `stab-model` or required a temporary duplicate registry. Removing the feature first keeps one gate table and does not alter any supported product behavior.

## Extraction Commits

The physical work is split into focused commits:

1. Add this frozen map and architecture checks for the future packages.
2. Replace all internal foreign-inherent call sites with owner free functions.
3. Extract Stable scalar `stab-algebra`.
4. Extract the Stable `stab-model` value and error boundary.
5. Remove `ops-contracts` and migrate operational descriptors before moving the gate registry.
6. Complete Stable `stab-model` and facade error/path adapters.
7. Extract Stable `stab-analysis`.
8. Extract Stable scalar `stab-engine`.
9. Extract dependency-free `stab-kernels-simd` and add feature forwarding for measured bit and algebra kernels while retaining scalar-only sampling backend registration.
10. Add external-consumer fixtures, dependency rejection fixtures, API-tier checks, and generated inventory ownership.
11. Run moved-path benchmarks, audit, review, and A6 evidence closure.

The A6 scalar-versus-portable diagnostic uses `just bench::simd-compare`. It is intentionally narrower than a new qualification group: it reuses the existing M5 dense-XOR and M6 non-identity Clifford runtime contracts at medium and large scales, binds both explicit Cargo feature selections into private build receipts, requires exact semantic output, alternates pair order, and reports portable-over-scalar ratios without making Stim parity, self-regression, release, or backend claims.

Each commit must compile the workspace state it creates. Temporary duplicate implementations are not an accepted way to keep an intermediate commit green.

## Test Ownership

### Stable Component Checks

The following run on Rust 1.97.1:

```text
cargo +1.97.1 check -p stab-bits -p stab-records -p stab-algebra -p stab-model -p stab-analysis -p stab-engine
cargo +1.97.1 test -p stab-bits -p stab-records -p stab-algebra -p stab-model -p stab-analysis -p stab-engine
```

External fixture crates prove that Stable consumers can compile the scalar engine without compiling the facade, CLI, ops, or Nightly kernel code.

### Algebra

Move or retarget the exact Pauli, Clifford, tableau, flow, conversion, limit, and amplitude-comparison tests. Add scalar-versus-SIMD tests over empty, tail-only, one-block, and multi-block inputs, dirty logical tails, every 24-by-24 valid Clifford product, unequal widths, metadata counts, cancellation-free mutation, and allocation-free warmed execution. Safe Rust prevents mutable and immutable aliases before a fixed-block kernel is called, so alias rejection is a compile-time language property rather than a runtime test.

### Model

Move or retarget exact circuit and DEM format tests, parser fast paths, parser diagnostics, opaque metadata, model fingerprints, gate lookup and metadata, typed IDs, structural counts, coordinates, repeat-depth admission, and iterative drop behavior.

Path alias and CLI lifecycle tests remain CLI-owned.

### Analysis

Move or retarget every transform, flow, generation, circuit-to-DEM, graphlike, hypergraph, SAT, error-matcher, MBQC, and sparse reverse tracker test. Add architecture checks proving the crate has no records, engine, facade, CLI, test-support runtime, or ops edge.

### Engine

Retarget every A4 and A5 compiler, plan, session, cancellation, poisoning, sink-lifecycle, replay, direct-frame, fused-conversion, seeded partitioning, deterministic, and statistical test. P2 later removed unavailable-backend behavior and the one-element registry; implementation selection must not return until two real engine plans exist.

### Facade And Features

Add external-consumer fixtures for:

- each Stable component with default features;
- scalar facade with `default-features = false`;
- Nightly facade with `portable-simd`;
- CLI, oracle, and benchmark explicit feature intent;
- feature unification from multiple consumers;
- forbidden product-to-ops, kernel-to-Stab, mandatory Stable-to-kernel, Stable-to-facade, and direct, aliased, or macro-contained `std::simd` and `core::simd` sites outside the kernel crate;
- every unstable Rust feature gate in a Stable component, facade module path overrides, item-generating facade macros, and exported macros anywhere in `stab-core`;
- root, `advanced`, and `experimental` rustdoc tiers;
- absence of qualification-only product exports.

`just architecture::consumer-check` owns the four external-consumer builds and their resolved-feature assertions. The fixtures are standalone workspaces under `test-support/consumers/`, so successful workspace-internal compilation cannot substitute for the downstream Stable, scalar-facade, portable-facade, and mixed-consumer contracts.

## Benchmark Ownership

The extraction does not invent new aggregate benchmark rows.

### Source-Owning SIMD Evidence

Only the workloads that execute an extracted optional kernel are SIMD evidence:

- M5 dense packed-bit XOR;
- M6 non-identity Clifford right multiplication;
- selected scalar `PERFQ-M5-SIMD-BITS` / `xor-complete-vector` at the `small` scale;
- selected scalar `PERFQ-M6-CLIFFORD-STRING` / `right-multiply-identity` at the `small` scale.

Parser, records, sampling, conversion, DEM, and analysis rows are not SIMD evidence merely because their owning crates were extracted.

The M6 short-right-operand measurements have no equivalent Stim workload and remain report-only extraction diagnostics. They do not substitute for either the paired identity qualification measurement or the separate scalar-versus-SIMD non-identity kernel report.

### Extraction Continuity

The final A6 closure set is finite. Every selected row retains its existing timing boundary, semantic witness, and comparator classification.

| Owner | Group or row | Exact measurements and scales | Inclusion reason |
| --- | --- | --- | --- |
| A2 | `PERFQ-A2-CIRCUIT-MODEL-FINGERPRINT` | `fingerprint`; all three scales | Circuit model and canonical gate ownership moved to `stab-model`. |
| A2 | `PERFQ-A2-SAMPLING-REQUEST-FINGERPRINT` | `fingerprint-inclusive`; all three scales | Sampling request identity moved to the public engine boundary. |
| A2 | `PERFQ-A2-SAMPLING-REQUEST-ESTIMATE` | `estimate`; all three scales | Resource estimation now crosses the extracted model and engine packages. |
| A2 | `PERFQ-A2-SAMPLER-COMPILE` | `compile-and-release`; all three scales | Sampling compilation moved to `stab-engine`. |
| A2 | `m4-circuit-parse` | `stab_circuit_parse`, `stab_circuit_parse_sparse`, and their row-owned Stim filters | The parser and model allocation owner moved to `stab-model`. |
| A4 | `m8-sample-analysis-1shot` | `stab_sample_compile_plan_auto_noisy_1q`, `stab_sample_compile_plan_scalar_noisy_1q`, `stab_sample_construct_session_noisy_1q`, `stab_sample_execute_witness_sink_64_continuous_session`, `stab_sample_consume_typed_batch_64`, `stab_sample_encode_b8_64`, `stab_sample_repeated_session_16x4_continuous_session` | Compilation, session execution, typed delivery, and codec ownership now cross explicit engine and records packages. |
| A4 | `m8-sample-throughput-1024` | `stab_sample_1024_zero_one` and its row-owned Stim command measurement | The public sampling CLI now traverses the extracted facade-to-engine path. |
| A4 | `m8-sample-throughput-1000000` | `stab_sample_1000000_zero_one` and its row-owned Stim command measurement | The production-scale public sampling path proves the extraction did not add shot-count scaling overhead. |
| A5 | `m9-detection-batch-phases` | `stab_detection_plan_compile_and_release_basic`, `stab_detection_session_sample_to_detection`, `stab_detect_ptb64_routing` | Detection compilation, fused execution, and records routing moved across engine and records boundaries. |
| A5 | `m9-m2d-batch-phases` | `stab_m2d_plan_compile_and_release_basic`, `stab_m2d_session_convert_batch` | Measurement-to-detection compilation and bounded conversion moved to `stab-engine`. |
| A5 | `m11-dem-batch-phases` | `stab_dem_plan_compile_and_release_surface_like`, `stab_dem_session_detector_only`, `stab_dem_session_with_sampled_errors`, `stab_dem_session_replay`, `stab_sample_dem_cli_ptb64_routing` | DEM compilation, execution, replay, and records routing moved across engine and records boundaries. |

M7 conversion rows, unrelated M8 simulators and readers, M9 analysis utilities, M10 parser and analysis families, PF rows, and other inherited rows are deliberately excluded because A6 did not change their selected feature, timed algorithm, or public process path. Their moved semantic owners are covered by direct Stable package tests and generated ownership rather than duplicate timing. A later executable-path change must add or replace an exact entry through the milestone specification-gap process instead of interpreting a broad category during execution.

These continuity rows prove owner migration and unchanged workload behavior. They do not become evidence for the optional SIMD kernels unless the selected feature changes the timed implementation.

Measurements remain phase-specific. A facade call-path move is not evidence for a parser, compiler, session, or codec phase unless that exact phase is the timed boundary.

The selected M5 and M6 scalar diagnostics execute their source-owned correctness prerequisites and semantic preflights before timing. They use the current paired qualification groups at the `small` scale, the scalar feature set, `raw-work-v2`, full-tier alternating samples, and the unchanged `1.25x` median and confidence-upper-bound policy. Host-unverified reports may establish A6 diagnostic continuity but are not promotable parity evidence; A9 owns controlled-host full and soak qualification across every scale. The migrated legacy M6 threshold remains retired.

The separate scalar-versus-SIMD report uses identical inputs and exact output witnesses at medium and large scales for dense XOR and non-identity Clifford right multiplication. It decides whether the optional kernel has a material benefit; it is not a Stim-relative comparison.

Only the exact A2, A4, and A5 entries in the table above are rerun for A6 continuity. Their existing semantic witnesses and comparator classifications remain authoritative. The complete historical 166-row matrices, predecessor-backport design, focused reports, typed profile receipts, and publication attempts are preserved as historical diagnostics but are not A6 closure requirements. Optional profiling may investigate a result, but an externally supplied profile cannot prove workload provenance or relabel a failed gate.

## Acceptance Matrix

| Requirement | Authoritative evidence |
| --- | --- |
| Physical target graph | `cargo metadata --all-features` plus `just architecture::check` |
| Stable components exclude Nightly | Rust 1.97.1 component and external-consumer checks plus source scanning |
| Only kernel crate owns direct SIMD | Architecture source scan and rustdoc/build fixtures |
| Scalar and SIMD are semantically identical | Cross-backend property, frozen-vector, deterministic, and statistical tests |
| Portable kernels are genuine | Distinct SIMD instructions over affected raw blocks, exact scalar equivalence, allocation evidence, and scalar-versus-SIMD XOR and non-identity Clifford reports |
| Sampling backend claims remain honest | Scalar remains the sole registered backend and explicit portable requests remain unavailable until a distinct packed-frame plan exists |
| Facade preserves intended compatibility | API migration inventory, facade tests, CLI oracle, and implemented-only oracle |
| Canonical semantics do not depend on the facade | Generated API ownership plus direct-package selector guards for every wholly owned implemented item in all six Stable component crates and a checked narrow integration-exception ledger |
| Product graph has no qualification exports | All-features rustdoc inventory and source scan |
| Moved paths have no unexplained regression | Source-current selected M5/M6 paired diagnostics plus affected-path diagnostics under their existing semantic, noise, and comparator policies |
| Documentation matches physical state | Generated API/status checks and architecture link checks |

## Known Risks

1. A facade-wide `CircuitError` currently crosses every logical owner. Extraction must split component errors before model movement rather than making Stable crates depend on `stab-core`.
2. Foreign inherent compatibility adapters cannot be reimplemented after their types move. Call sites must migrate to free functions first.
3. The gate semantic-contract tree mixes product descriptors, tests, and statistical qualification plans. Its product and ops portions must be separated deliberately.
4. DEM folded traversal uses crate-private internals from several consumers. The advanced boundary must expose behavior, not storage representation.
5. Backend selection currently advertises portable SIMD as unavailable. Raw bit and Clifford kernels do not change the engine's direct-Z, small-frame, or general-frame representation, so registration remains deferred until a genuinely distinct packed execution plan exists.
6. Feature unification can accidentally pull Nightly code into Stable consumers. External fixtures must inspect the resolved graph, not only compile one package in the workspace.
7. Qualification ownership paths and rustdoc identities will change substantially. Inventories must regenerate only after reviewers confirm the new canonical owners and facade aliases.
