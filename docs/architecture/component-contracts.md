# Component Contract Template

Each product component must document the following fields.

## Template

### Purpose

State the single responsibility of the component.

### Public Inputs And Outputs

List typed values crossing the component boundary.

### Owned Invariants

List invariants that this component validates and preserves.

### Dependencies

List permitted product dependencies and explain each one.

### Forbidden Dependencies

List components that must not be imported.

### Resource Behavior

Describe admission policy, materialization, scratch reuse, cancellation, and worst-case growth.

### Extension Points

List supported composition boundaries.

### Conformance Tests

Name semantic, compatibility, negative, and resource test families.

### Benchmarks

Name phase-specific workload families and their comparator class.

### Files Changed Together

List source descriptors, generated files, docs, tests, and benchmark metadata that must remain synchronized.

## Initial Ownership

### Model

- Purpose: own exact Stim circuit and DEM values plus syntax.
- Inputs and outputs: bytes or text, typed models, gate and target values, IDs, parse or validation errors, and cheap resource estimates.
- Invariants: closed Stim v1.16.0 dialect, canonical text and byte printing, opaque comment payloads, exact unescaped tag bytes, structural validity, typed indices and probabilities, exact byte spans, bounded diagnostic text, and honest estimate classifications.
- Dependencies: algebra values needed by the closed Stim model, plus foundational standard-library and parsing support.
- Forbidden: engine, records, CLI, ops.
- Resource behavior: `ParseLimits` owns caller-selectable source-line admission and a caller-tightenable 256-level parsed-model repeat envelope. Programmatic models may exceed that parser envelope only for APIs with an established deeper contract; those consumers must be iterative or reject before recursive work. `DemFlattenLimits` belongs to the analysis adapter rather than the model.
- Extension points: immutable typed circuit passes consume and return models.

### Records

- Purpose: own typed result layouts, packed batches, strict codecs, sources, and sinks.
- Inputs and outputs: measurement, detection, observable, sampled-error, and prediction batches.
- Invariants: explicit width, namespace, layout, tail-bit, record-boundary, and PTB64 grouping contracts.
- Dependencies: packed bit storage.
- Forbidden: circuit execution, filesystem paths, CLI, ops.
- Resource behavior: dense and packed codec scratch is bounded by declared record width and the active batch, never total stream length or duplicate-token count. Raw sparse and typed-token visitors may retain one encoded record because duplicate order is their returned value. An explicitly in-memory codec sink may retain its caller-requested encoded output bytes; that materialized result is reported separately from bounded working scratch.
- Extension points: typed record sinks and bounded record visitors.

### Algebra

- Purpose: own Pauli, Clifford, Tableau, and Flow mathematics.
- Inputs and outputs: algebraic values and typed algebra errors.
- Invariants: exact phase, sign, commutation, composition, and resource admission.
- Dependencies: packed bit storage.
- Forbidden: Stim parsing, CLI, ops.
- Resource behavior: scalar stable defaults with bounded construction and optional Nightly acceleration above storage kernels.
- Extension points: pure functions and owned values.

### Engine

- Purpose: compile exact models into plans and execute reusable sessions.
- Inputs and outputs: models, compiler options, plans, sessions, typed batches, execution summaries.
- Invariants: private executable IR, one backend decision per plan, session-owned mutable state, bounded execution batches.
- Dependencies: model, records, algebra values, pure analysis lowering, and raw SIMD kernels.
- Forbidden: textual codecs, filesystem paths, CLI, ops.
- Resource behavior: `DetectionConversionLimits` and `DemSamplerLimits` own the caller-selectable conversion and DEM execution budgets. Circuit sampling, measurement-to-detection conversion, circuit detection sampling, and DEM sampling each have operation-specific compiler, immutable-plan, mutable-session, and typed-sink boundaries. Session construction uses fallible reservation and conservatively rejects more than 256 MiB of reusable frame, reference, record, error, and bit-plane storage before allocation; fused detection admits the aggregate sampling-plus-conversion estimate, not each component independently. DEM logical returned-record limits remain distinct from its active retained-byte limit, which shrinks the reusable batch when necessary and includes compatibility-sink scratch. Detection and DEM execution otherwise reuse at most one 64-shot batch, conversion may accept smaller caller batches or one record at a time, and cancellation is checked only at documented batch boundaries. One expensive shot has no wall-clock cancellation deadline.
- Extension points: measurement, detection, and DEM-sample sinks.

### Analysis

- Purpose: own pure transforms, circuit-to-DEM analysis, search, generation, and error matching.
- Inputs and outputs: immutable models, transformed models, reports, analysis errors.
- Invariants: no hidden execution session or filesystem state, explicit folded or materialized resource behavior.
- Dependencies: model and algebra.
- Forbidden: CLI, ops, and engine.
- Resource behavior: `CircuitFlattenLimits`, `DemFlattenLimits`, `LogicalErrorSearchLimits`, and `SatMaterializationLimits` own their independent expansion, retained-state, and output budgets. Other partial analysis algorithms retain documented fixed safety contracts instead of sharing a generic policy.
- Extension points: typed circuit passes.

## Current Source Ownership

This table records both physically extracted components and the remaining logical ownership inside `stab-core`.

Nested `tests.rs` and resource-test modules inherit the owner of their parent source family.

| Current source family | Logical owner | Migration note |
| --- | --- | --- |
| `crates/stab-model/src/circuit.rs`, `circuit/**`, `model_bytes.rs`, `model_parse.rs`, `model_tag.rs`, `source_text.rs` | Model | Circuit syntax, values, byte-aware parsing, canonical text and byte printing, iteration, structural counts, opaque tags, and operation-owned parse admission are physically model-owned. The shared byte preparation path preserves source-order failures and opaque Stim metadata without lossy whole-input UTF-8 conversion. Programmatic depth beyond the parser envelope remains consumer-specific rather than being silently accepted by recursive algorithms. A6 removed algorithmic inherent adapters; analysis and execution behavior is reached through named owner functions. |
| `dem.rs`, `dem/api.rs`, `dem/coordinate_scan.rs`, `dem/parser.rs`, `dem/tag.rs`, `dem/traversal.rs` | Model | The DEM model shares the byte-aware model preparation contract and retains exact opaque tag bytes. Folded traversal is the model-owned advanced boundary shared by DEM queries, analysis, and execution. Repeat selections contain model facts and ceilings only; visitors construct consumer-owned expansion failures so logical-search and SAT resource identities do not leak into model types. Consumer-specific search, filtering, and probability policies remain with their analysis owners, while compact transforms use explicit stacks in `analysis/dem_adapters.rs`. |
| `crates/stab-model/src/gate/**` | Model | The closed Stim gate registry, aliases, syntax validation, scalar unitary rows, raw flow strings, and raw decomposition text are physically model-owned. `stab-core/src/gate.rs` is a compatibility facade plus a test-only semantic-surface contract; algebra-valued projections and decomposition parsing are now physically owned by `stab-analysis`. |
| `crates/stab-analysis/src/circuit.rs`, `circuit_simplify.rs`, `circuit_tableau.rs`, `circuit_transforms.rs`, `circuit_generation.rs`, `circuit_generation/**`, `circuit_flow.rs`, `circuit_flow/**`, `circuit_inverse.rs`, `circuit_inverse/**`, `circuit_feedback.rs`, `circuit_detecting_regions.rs`, `circuit_detecting_regions/**`, `circuit_missing_detectors.rs`, `circuit_missing_detectors/**`, `circuit_to_dem.rs`, `circuit_to_dem/**`, `dem.rs`, `dem/**`, `sparse_rev_frame_tracker.rs`, `sparse_rev_frame_tracker/**`, `mbqc_decomposition.rs`, `gate.rs`, `error.rs`, `resources.rs` | Analysis | The current Stable analysis slices physically own recursive circuit and DEM tag removal, gate tableau and flow projections, fixed-shape unitary matrices, H/S/CX/M/R decomposition lowering, single-qubit Clifford lookup, full-circuit tableau conversion, simplification, decomposition, bounded circuit and DEM flattening, DEM probability rounding, SAT/WCNF materialization, graphlike and hypergraph logical-error search, noise removal, repetition/surface/color circuit generation, MBQC decomposition, unsigned flow checking/generation/solving, sparse reverse-frame tracking, unitary and selected QEC inversion, tracker-driven flow reversal, bounded feedback lowering, detecting regions, missing-detector analysis, circuit-to-DEM lowering, folded analyzer recurrence, XYZ error-probability decomposition, and the typed failures these operations can produce. Root `stab_analysis` exports are canonical; public source-module paths and existing `stab_core` paths are qualification-tracked aliases or facade wrappers. Analysis-owned flatten, SAT, logical-search, and generation admission expose typed failures beside their implementing operations instead of importing the facade aggregate. Repeat-contained flow generation reuses the canonical bounded circuit flatten operation, so facade and direct analysis callers cannot drift onto separate materialization policies; DEM flattening, SAT, and logical search likewise own their independent retained-payload, repeat-work, graph-shape, frontier, CNF-shape, and output limits beside their operations. Feedback and analyzer repeat ceilings preserve their existing invalid-DEM error class rather than being relabeled as configurable resource admission. |
| `crates/stab-model/src/ids.rs`, `crates/stab-model/src/target.rs` | Model | Typed identifiers, targets, and validated probability primitives are physically extracted into the Stable model package. Construction returns model-owned `ModelError`; `stab-core` retains explicit compatibility reexports and lossless aggregate-error conversion while remaining model sources move. |
| `crates/stab-model/src/diagnostics.rs`, `dialect.rs`, `parse_limits.rs`, `resource_limit.rs`, `resources.rs`, `validation.rs`, and `error.rs` | Model | Stable byte spans, parser diagnostics, model dialect identity, parser limits and their four real admission failures, structural validation, and the shared honest-estimate vocabulary are physically model-owned. Attacker-controlled diagnostic text remains UTF-8-safe and bounded, while `ModelError` aggregates typed parse, parse-resource, and validation failures. Closed resource and validation contexts make every future model cause a compile-time facade-conversion obligation. Advanced constructors remain only where future analysis or engine crates need checked construction without exposing model storage. |
| `fingerprint.rs` | Model | Versioned circuit and DEM identities stream dialect-separated structural model encodings into SHA-256 without depending on compatibility-printer precision or allocating model-sized text. An explicit traversal stack is inline through the parser's repeat envelope and spills by depth only for deeper programmatic models. Compilation-request and backend-bearing plan identities remain with engine compilation rather than extending the model fingerprint. |
| `crates/stab-engine/src/fingerprint.rs`, `probability.rs`, `sampling/mod.rs`, `sampling/**`, `detection/mod.rs`, `detection/**` | Engine | Backend-neutral request identity, execution-side biased randomization, sampling and detection capability descriptors, compilation, immutable plans, mutable sessions, direct-Z, small-frame, general-frame, deterministic reference-sample, measurement-to-detection conversion, direct detector-frame and fused detection sampling, cancellation, progress, poisoning, and typed measurement or detection delivery are physically engine-owned. The engine imports model, records, algebra, and analysis but no facade, codec, filesystem, CLI, or ops API. Its crate root is the sole canonical public execution namespace. |
| `crates/stab-core/src/compilation_fingerprint.rs`, `crates/stab-core/src/probability_util.rs`, `capabilities.rs`, `sampling.rs`, `sampling/stream.rs`, `sampling_output_compat.rs`, `detection.rs`, `detection/**` | Facade compatibility | Fingerprint, probability, compiler, plan, session, converter, and descriptor paths are thin reexports, wrappers, or aggregation over canonical engine owners. `CompiledSampler`, `CompiledDetectionConverter`, callback streaming, materialized records, byte-oriented encoding, and detector or observable output routing remain explicit compatibility adapters over engine plans, sessions, and records-owned sinks. |
| `crates/stab-bits/src/**` | Bits | Stable Rust 1.97.1 packed storage, checked views, scalar kernels, sparse XOR storage, and transpose implementation are physically extracted. This leaf package has no dependency on another Stab product crate. |
| `crates/stab-algebra/src/**` | Algebra | Stable Pauli, Clifford, tableau, flow, conversion, error, resource, and scalar quantum-word implementations are physically extracted. The crate depends only on `stab-bits` among Stab products. Low-level unchecked construction used by admitted analysis and execution algorithms is isolated under `stab_algebra::advanced`. Portable SIMD remains absent until the raw kernel extraction supplies a distinct optional implementation. |
| `crates/stab-core/src/diagnostics.rs` | Facade compatibility and result-format diagnostics | Reexports model-owned byte spans, severity, and parser diagnostics; retains only facade-level result-format conversion and compatibility error presentation. Result grammar and native format diagnostics remain records-owned, and serialization remains CLI-owned. |
| `crates/stab-core/src/resources.rs` | Facade compatibility and aggregate resource errors | Reexports model-owned estimate values and losslessly wraps closed model and analysis resource failures in the established aggregate `ResourceLimitError`. Operation-specific policies, causes, and human displays remain beside their model, analysis, or engine operations. |
| `crates/stab-records/src/**` | Records | Stable Rust 1.97.1 strict codecs, typed semantic widths, shot-major and 64-shot bit-plane batches, typed DETS layouts, bounded visitors, and measurement, detection, and DEM-sample sinks are physically extracted. Shared text and packed decoders own grammar and length diagnostics so materialized and streaming consumers cannot drift. The codec capability registry lives beside these implementations and is consumed by the facade and CLI instead of being copied into a status manifest. |
| `crates/stab-core/src/result_formats.rs`, `crates/stab-core/src/result_streaming.rs` | Facade compatibility adapters | These wrappers re-export canonical `stab-records` types, convert structured format failures losslessly into `CircuitError`, and preserve established callback signatures while callers migrate. `MeasureRecordWriter` and `MeasureRecordBatchWriter` remain byte-oriented compatibility adapters for existing core and CLI paths; new component code uses typed sink traits and typed DETS namespace selection. The wrappers must not reimplement codec grammar, layout, or buffering policy. |
| `crates/stab-core/src/stabilizers/mod.rs` | Facade compatibility | Reexports canonical `stab-algebra` APIs and owns no algebra implementation. |
| `execution/reference_sample_tree.rs`, `execution/sampled_flow.rs`, `dem_sampler.rs`, `dem_sampler/**` | Engine, not yet physically moved | DEM sampling, reference-sample trees, and sampled-flow execution expose execution behavior through `stab_core::execution`. The DEM family already has compiler, immutable-plan, mutable-session, replay-session, cancellation, progress, poisoning, and typed-sink contracts, but its implementation remains in the facade compilation boundary. Mutable sessions own RNG where applicable, bounded 64-shot batches, counters, and poison state. Detector-only DEM, sampled-error DEM, and replay implementations remain private operation-specific variants. DEM replay admits poison state and total work before caller-record traversal, while its unknown-length compatibility iterator retains direct folded traversal because it cannot declare a shot count before iteration; the CLI does not use that exception. These paths emit separate detector, observable, and optional sampled-error planes. Their physical extraction into `stab-engine` remains A6 work. |
| `sampling_output_compat.rs`, `sampling_estimate.rs` | Facade compatibility and planning | Materialized and byte-returning `CompiledSampler` methods are explicit adapters over sampling sessions and typed record sinks. Sampling estimates may inspect codec size formulas without making result encoding part of the executable engine contract. |
| `crates/stab-core/src/analysis/**`, `circuit_simplify.rs`, `circuit_tableau.rs`, `circuit_transforms.rs`, `circuit_generation.rs`, `circuit_flow.rs`, `circuit_inverse.rs`, `circuit_feedback.rs`, `circuit_detecting_regions*`, `circuit_missing_detectors*`, `dem/analyze.rs`, `dem/flatten.rs`, `dem/sat.rs`, `dem.rs`, `error_matcher.rs`, `matched_error.rs`, `mbqc_decomposition.rs` | Analysis facade | Pure transforms and analyses delegate to `stab-analysis` while preserving established `CircuitError`, generated-value, facade-owned option, aggregate resource, matcher-entry, and matched-error DTO signatures. The three matched-error wrapper types whose public methods historically return `CircuitResult` convert to canonical analysis values explicitly; simple provenance values are direct reexports. |
| `crates/stab-analysis/src/dem/search.rs`, `dem/search/**` | Analysis | These consume the model-owned folded traversal boundary and physically own compact detector indexing, bounded error-mechanism traversal, graphlike and hypergraph search, and operation-specific traversal, graph, hyperedge, and frontier resource policies. |
| `crates/stab-analysis/src/error_matcher.rs`, `error_matcher/**`, `matched_error.rs` | Analysis | These physically own pure error matching, compact filter traversal, resource admission, canonical provenance values, ordering, and diagnostic formatting; simulator-backed sampled-flow checks remain execution-owned. |
| `error.rs` | Facade compatibility | Losslessly converts model-owned `ModelError`, analysis-owned `AnalysisError`, including complete typed gate-projection failures, plus records and operation resource failures into `CircuitError` without changing established variants or human display. |
| `lib.rs` | Facade | Root reexports remain curated compatibility adapters and do not determine implementation ownership. |

`stab-cli/src/agent.rs` is a CLI adapter, not a new product component. It discovers commands from Clap, renders core descriptors and identities, reuses retained-handle input admission, and may compose parsing, compilation validation, and estimates. It must not become an alternate source of gate, codec, compiler, backend, or qualification truth.

The `stab-core::bits` module and root bit-type paths are compatibility re-exports of canonical `stab_bits` items. The `stab-core::result_formats` and `stab-core::result_streaming` modules similarly adapt canonical `stab_records` behavior without owning a second codec implementation.

New source modules must fit exactly one row or update this table and the architecture decision record in the same change.
