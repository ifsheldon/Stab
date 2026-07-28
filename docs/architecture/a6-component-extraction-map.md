# A6 Physical Component Extraction Map

This document freezes the source, API, feature, dependency, test, and benchmark moves for milestone A6 of the [agent-native modular QEC architecture plan](../plans/agent-native-modular-qec-architecture-plan.md).

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
stab-model -> stab-algebra
stab-analysis -> stab-model + stab-algebra
stab-engine -> stab-model + stab-records + stab-algebra + stab-analysis + stab-kernels-simd
stab-core -> all product components
stab-cli -> stab-core
```

Algebra moves before the model because its production implementation no longer imports `Gate`. Model syntax can then depend on algebra values without creating a reverse edge.

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

The model crate depends only on `stab-algebra = { path = "../stab-algebra", version = "=0.2.0" }` among Stab crates.

The following do not move into model:

- path-opening and path-creating helpers;
- gate-to-tableau, gate-to-flow, gate-to-unitary, and decomposition parsing adapters;
- materializing circuit or DEM transforms;
- graphlike search, SAT generation, error matching, or circuit-to-DEM analysis;
- reference sampling or determined-measurement counting.

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
| `dem/analyze*` | `dem/analyze/*` | Own circuit-to-DEM analysis, options, diagnostics, decomposition, and folded lowering. |
| `dem/arena_index.rs`, `dem/error_traversal.rs` | `dem/*` | Own analysis indexing and bounded error-mechanism traversal. |
| `dem/flatten.rs` | `dem/flatten.rs` | Own materialized DEM flattening and `DemFlattenLimits`. |
| `dem/graphlike*`, `dem/hyper*`, `dem/sat*`, `dem/search_budget.rs` | `dem/*` | Own graph and hypergraph search, SAT materialization, and their operation-specific policies. |
| `error_matcher*`, `matched_error.rs` | `error_matcher/*` | Own error explanation and provenance values. |
| `mbqc_decomposition.rs` | `mbqc.rs` | Own MBQC decomposition. |
| `sparse_rev_frame_tracker*` | `sparse_reverse_tracker/*` | Own pure reverse tracking shared by flow and DEM analysis. |

The crate depends only on exact-version `stab-model` and `stab-algebra` edges.

Any helper that samples, constructs a mutable execution frame, or depends on `stab-records` belongs in engine instead.

### `stab-engine`

| Current source | Destination | Public ownership and rationale |
| --- | --- | --- |
| `sampling.rs`, `sampling/*` | `sampling/*` | Own sampling compiler, immutable plan, session, private operations, execution frames, and compatibility sampler. |
| `execution/*` | `execution/*` | Own reference sampling and simulator-backed semantic queries. Delete foreign inherent adapters. |
| `detection.rs`, `detection/*` | `detection/*` | Own measurement-to-detection compilation, direct detector-frame and fused detection sampling, sessions, delivery, and limits. |
| `dem_sampler.rs`, `dem_sampler/*` | `dem_sampling/*` | Own DEM compilation, sampling, replay, sessions, compatibility adapter, and limits. |
| `probability_util.rs` | `probability.rs` | Own execution-side randomization helpers. |
| `compilation_fingerprint.rs` | `fingerprint.rs` | Own backend-neutral request fingerprints. |
| Engine-owned portions of `capabilities.rs` | `capabilities.rs` | Own compiler and backend descriptors. |

The crate depends on exact-version `stab-model`, `stab-records`, `stab-algebra`, `stab-analysis`, and optional or direct raw `stab-kernels-simd` as required by the selected build.

Execution code does not import `SampleFormat`, text codecs, filesystem paths, CLI types, or ops descriptors. Byte-oriented materializers that remain for compatibility live in the facade and delegate through typed sinks.

`detection/output.rs` does not move as written because it imports `SampleFormat` and byte writers. Its semantic detector and observable routing moves behind typed sinks, while byte-format compatibility wrappers remain in `stab-core`.

### `stab-kernels-simd`

| Current source | Destination | Public ownership and rationale |
| --- | --- | --- |
| Portable-SIMD block logic in `stab-core/src/bits/clifford.rs` | `stab-kernels-simd/src/clifford.rs` | Own direct `std::simd` code over raw word blocks. |
| Measured portable-SIMD packed-bit kernels introduced during A6 | Focused raw modules | Give the optional bit-storage edge a genuine implementation rather than an empty feature. |

This crate has no Stab dependency. It is the only crate containing `#![feature(portable_simd)]` or direct `std::simd` paths.

Public functions accept raw `&[u64]`, `&mut [u64]`, arrays of those slices, or fixed `[u64; 4]` blocks. The crate does not expose circuits, gates, algebra values, layouts, policies, or backend enums.

### `stab-core` Facade

The facade retains:

- curated root reexports for common models, algebra values, compilers, plans, sessions, batches, diagnostics, and policies;
- `advanced` reexports for explicit storage, layout, backend, and bounded traversal APIs;
- `experimental` reexports for the later pass and decoder seams whose contracts are intentionally less stable before 1.0;
- filesystem and byte-materializing compatibility conveniences;
- `sampling_output_compat.rs` and the byte-oriented portion of `detection/output.rs`, rewritten as adapters over engine sessions and records-owned codecs;
- non-lossy `StabError` or `CircuitError` compatibility conversion;
- capability aggregation across model, records, engine, decoder, and pass descriptors.

The facade does not retain implementation modules after their owner moves.

`CompiledSampler`, `CompiledDemSampler`, record-at-a-time visitors, and byte-returning helpers remain only where the `0.2` migration inventory explicitly labels them adapters. They delegate through component APIs and are not architectural extension points.

## Feature Map

| Package | Default features | `portable-simd` behavior |
| --- | --- | --- |
| `stab-bits` | Stable scalar | Enables measured raw packed-bit kernels from `stab-kernels-simd`. |
| `stab-records` | Stable | No direct feature; it follows the selected `stab-bits` implementation only through an explicit dependency feature if needed. |
| `stab-algebra` | Stable scalar | Enables raw Clifford kernels from `stab-kernels-simd`. |
| `stab-model` | Stable | No SIMD feature. |
| `stab-analysis` | Stable | No SIMD feature. |
| `stab-engine` | Scalar when absent | Registers and executes a distinct portable-SIMD backend when present. |
| `stab-core` | High-performance facade | Forwards the additive feature to bits, algebra, and engine. |
| `stab-cli`, `stab-oracle`, `stab-bench` | Nightly consumer | Explicitly enable `stab-core/portable-simd`; they do not rely on another package to unify it. |

There is no `scalar` feature. Scalar behavior is defined by the absence of `portable-simd`.

## `ops-contracts` Removal

The product feature is removed rather than renamed.

Gate statistical plans, rejection boundaries, and qualification family lists move into `ops/oracle` or a shared ops-owned descriptor module. Their values remain checked against public model descriptors and executable product behavior, but product crates do not export hidden qualification APIs.

DEM analyzer phase observations used only by benchmarks move behind an ops-owned benchmark adapter. If a phase value is a useful product diagnostic, it receives a documented analysis API; otherwise the benchmark measures through an internal ops harness without adding a public product item.

The following names disappear from product rustdoc and qualification inventory:

- `__gate_contract_family_names`
- `__gate_contract_surface_names`
- `__gate_contract_statistical_plans`
- `__gate_contract_statistical_rejection_boundaries`
- `__circuit_to_detector_error_model_with_diagnostics`
- `ErrorAnalyzerDiagnostics` when it remains benchmark-only

## Extraction Commits

The physical work is split into focused commits:

1. Add this frozen map and architecture checks for the future packages.
2. Replace all internal foreign-inherent call sites with owner free functions.
3. Extract Stable scalar `stab-algebra`.
4. Extract Stable `stab-model` and facade error/path adapters.
5. Extract Stable `stab-analysis`.
6. Extract Nightly `stab-engine`.
7. Extract dependency-free `stab-kernels-simd`, add feature forwarding, and register the distinct backend.
8. Remove `ops-contracts`, curate facade tiers, and migrate operational descriptors.
9. Add external-consumer fixtures, dependency rejection fixtures, API-tier checks, and generated inventory ownership.
10. Run moved-path benchmarks, audit, review, and A6 evidence closure.

Each commit must compile the workspace state it creates. Temporary duplicate implementations are not an accepted way to keep an intermediate commit green.

## Test Ownership

### Stable Component Checks

The following run on Rust 1.97.1:

```text
cargo +1.97.1 check -p stab-bits -p stab-records -p stab-algebra -p stab-model -p stab-analysis
cargo +1.97.1 test -p stab-bits -p stab-records -p stab-algebra -p stab-model -p stab-analysis
```

External fixture crates prove that Stable consumers do not compile facade, engine, CLI, ops, or Nightly kernel code.

### Algebra

Move or retarget the exact Pauli, Clifford, tableau, flow, conversion, limit, and amplitude-comparison tests. Add scalar-versus-SIMD property tests over empty, tail-only, one-block, multi-block, alias-rejected, and maximum practical inputs.

### Model

Move or retarget exact circuit and DEM format tests, parser fast paths, parser diagnostics, opaque metadata, model fingerprints, gate lookup and metadata, typed IDs, structural counts, coordinates, repeat-depth admission, and iterative drop behavior.

Path alias and CLI lifecycle tests remain CLI-owned.

### Analysis

Move or retarget every transform, flow, generation, circuit-to-DEM, graphlike, hypergraph, SAT, error-matcher, MBQC, and sparse reverse tracker test. Add architecture checks proving the crate has no records, engine, facade, CLI, test-support runtime, or ops edge.

### Engine

Retarget every A4 and A5 compiler, plan, session, cancellation, poisoning, sink-lifecycle, replay, direct-frame, fused-conversion, seeded partitioning, deterministic, and statistical test. Add scalar-versus-portable-backend equivalence for every private plan family.

### Facade And Features

Add external-consumer fixtures for:

- each Stable component with default features;
- scalar facade with `default-features = false`;
- Nightly facade with `portable-simd`;
- CLI, oracle, and benchmark explicit feature intent;
- feature unification from multiple consumers;
- forbidden product-to-ops, kernel-to-Stab, Stable-to-engine, Stable-to-facade, and direct-`std::simd` edges;
- root, `advanced`, and `experimental` rustdoc tiers;
- absence of qualification-only product exports.

## Benchmark Ownership

The extraction does not invent new aggregate benchmark rows.

Rerun every existing row whose implementation call path moves:

- M4 circuit parsing, canonical printing, and gate lookup;
- M5 packed-bit and sparse XOR kernels;
- M6 Pauli, Clifford, tableau, and flow algebra;
- M7 record conversion paths affected by feature forwarding;
- M8 sampling compilation, session construction, execution, typed batch consumption, encoding, and CLI sampling;
- M9 measurement-to-detection compilation, conversion, fused detection sampling, replay, routing, and CLI paths;
- M10 DEM parsing, printing, analysis, decomposition, search, compilation, sampling, and replay;
- PF analysis and query rows whose owner changes;
- A2, A4, and A5 phase diagnostics whose source paths or crate identities change.

Measurements remain phase-specific. A facade call-path move is not evidence for a parser, compiler, session, or codec phase unless that exact phase is the timed boundary.

All output-producing rows keep their independent semantic witness. Compile-and-release rows keep source-owned plan dimensions or fingerprints outside the timed region.

## Acceptance Matrix

| Requirement | Authoritative evidence |
| --- | --- |
| Physical target graph | `cargo metadata --all-features` plus `just architecture::check` |
| Stable components exclude Nightly | Rust 1.97.1 component and external-consumer checks plus source scanning |
| Only kernel crate owns direct SIMD | Architecture source scan and rustdoc/build fixtures |
| Scalar and SIMD are semantically identical | Cross-backend property, frozen-vector, deterministic, and statistical tests |
| Portable backend is genuine | Distinct backend plan identity, exercised kernel counter or test seam, and phase benchmark evidence |
| Facade preserves intended compatibility | API migration inventory, facade tests, CLI oracle, and implemented-only oracle |
| Product graph has no qualification exports | All-features rustdoc inventory and source scan |
| Moved paths have no unexplained regression | Source-current phase reports tied to exact moved rows |
| Documentation matches physical state | Generated API/status checks and architecture link checks |

## Known Risks

1. A facade-wide `CircuitError` currently crosses every logical owner. Extraction must split component errors before model movement rather than making Stable crates depend on `stab-core`.
2. Foreign inherent compatibility adapters cannot be reimplemented after their types move. Call sites must migrate to free functions first.
3. The gate semantic-contract tree mixes product descriptors, tests, and statistical qualification plans. Its product and ops portions must be separated deliberately.
4. DEM folded traversal uses crate-private internals from several consumers. The advanced boundary must expose behavior, not storage representation.
5. Backend selection currently advertises portable SIMD as unavailable. Registration is valid only after a genuinely distinct raw kernel path exists.
6. Feature unification can accidentally pull Nightly code into Stable consumers. External fixtures must inspect the resolved graph, not only compile one package in the workspace.
7. Qualification ownership paths and rustdoc identities will change substantially. Inventories must regenerate only after reviewers confirm the new canonical owners and facade aliases.
