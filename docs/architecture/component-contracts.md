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
- Inputs and outputs: bytes or text, typed models, gate and target values, IDs, parse or validation errors.
- Invariants: closed Stim v1.16.0 dialect, canonical text and byte printing, opaque comment payloads, exact unescaped tag bytes, structural validity, typed indices and probabilities.
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
| `circuit.rs`, `circuit/**`, `model_bytes.rs`, `model_parse.rs`, `model_tag.rs`, `source_text.rs`, `parse_limits.rs` | Model | The model keeps syntax, values, byte-aware parsing, canonical text and byte printing, iteration, structural counts, and operation-owned parse admission. The shared byte preparation path preserves source-order failures and opaque Stim metadata without applying lossy whole-input UTF-8 conversion. Named line and repeat limits preserve the default parsed-model boundary and cap preallocation by admitted work. Programmatic depth beyond the parser envelope remains consumer-specific rather than being silently accepted by recursive algorithms. Algorithmic inherent methods are temporary adapters implemented under `analysis` or `execution`. |
| `dem.rs`, `dem/api.rs`, `dem/coordinate_scan.rs`, `dem/parser.rs`, `dem/tag.rs`, `dem/traversal.rs` | Model | The DEM model shares the byte-aware model preparation contract and retains exact opaque tag bytes. Folded traversal is the model-owned advanced boundary shared by DEM queries, analysis, and execution. Consumer-specific search, filtering, and probability policies remain with their analysis owners, while compact transforms use explicit stacks in `analysis/dem_adapters.rs`. |
| `gate.rs`, `gate/**` | Model | Gate syntax and closed Stim scalar or textual descriptors remain model-owned; algebra-valued projections and decomposition parsing are implemented by `analysis/gate_adapters.rs`. |
| `ids.rs`, `target.rs` | Model | Typed identifiers, targets, and validated probability primitives are foundational model values. |
| `fingerprint.rs` | Model | Versioned circuit and DEM identities stream dialect-separated structural model encodings into SHA-256 without depending on compatibility-printer precision or allocating model-sized text. An explicit traversal stack is inline through the parser's repeat envelope and spills by depth only for deeper programmatic models. Compilation-request and backend-bearing plan identities remain with engine compilation rather than extending the model fingerprint. |
| `compilation_fingerprint.rs`, `capabilities.rs` | Engine and facade, temporarily | Backend-neutral request identity binds one source-owned compiler registration without inventing backend selection or compile budgets. The facade assembles runtime discovery from model, records, and engine descriptors; descriptor ownership remains with the operation that implements each capability. |
| `crates/stab-bits/src/**` | Bits | Stable Rust 1.97.1 packed storage, checked views, scalar kernels, sparse XOR storage, and transpose implementation are physically extracted. This leaf package has no dependency on another Stab product crate. |
| `crates/stab-core/src/bits/clifford.rs`, `crates/stab-core/src/bits/scalar.rs` | Algebra and SIMD kernel bridge | Quantum-specific Clifford and Pauli-word operations remain in core until A6. Direct portable SIMD is confined to the Clifford implementation; the scalar Pauli-word operation consumes `stab-bits` storage without moving quantum semantics into the storage crate. |
| `diagnostics.rs` | Facade, temporarily | A2 owns shared byte-span, severity, stable code, parse, format, and resource-context primitives here. Validation, compile, execution, and analysis errors wait for their owning A3 through A6 boundaries instead of adding placeholder variants. Serialization remains CLI-owned. A6 must place the shared stable primitives without making model, records, analysis, or execution depend on the facade. |
| `resources.rs` | Facade, temporarily | A2 owns shared estimate classifications and lossless resource-limit context here while operation-owned policies are introduced beside their model, engine, or analysis operations. A6 must place the shared vocabulary without creating a global resource-policy dependency. |
| `crates/stab-records/src/**` | Records | Stable Rust 1.97.1 strict codecs, typed semantic widths, shot-major and 64-shot bit-plane batches, typed DETS layouts, bounded visitors, and measurement, detection, and DEM-sample sinks are physically extracted. Shared text and packed decoders own grammar and length diagnostics so materialized and streaming consumers cannot drift. The codec capability registry lives beside these implementations and is consumed by the facade and CLI instead of being copied into a status manifest. |
| `crates/stab-core/src/result_formats.rs`, `crates/stab-core/src/result_streaming.rs` | Facade compatibility adapters | These wrappers re-export canonical `stab-records` types, convert structured format failures losslessly into `CircuitError`, and preserve established callback signatures while callers migrate. `MeasureRecordWriter` and `MeasureRecordBatchWriter` remain byte-oriented compatibility adapters for existing core and CLI paths; new component code uses typed sink traits and typed DETS namespace selection. The wrappers must not reimplement codec grammar, layout, or buffering policy. |
| `stabilizers/**` | Algebra | Pauli, Clifford, Tableau, and Flow mathematics do not own gate syntax. |
| `sampling.rs`, `sampling/**`, `execution/**`, `detection.rs`, `detection/**`, `dem_sampler.rs`, `dem_sampler/**`, `probability_util.rs` | Engine | Circuit sampling, measurement-to-detection conversion, circuit detection sampling, and DEM sampling expose distinct compiler, immutable-plan, mutable-session, cancellation, progress, and typed-sink families through `stab_core::execution`. Mutable sessions own RNG where applicable, reference and conversion scratch, simulator or detector frames, bounded 64-shot batches, counters, and poison state. Direct-Z, small-frame, general-frame, direct-detector-frame, fused sample-convert, detector-only DEM, sampled-error DEM, and replay implementations remain private operation-specific variants. Finite-shot sampling materializers and visitors delegate through these paths. `CompiledDetectionConverter` remains the public per-record compatibility kernel used by measurement-to-detection sessions, and the unknown-length DEM replay iterator retains direct folded traversal because it cannot declare a shot count before iteration; neither exception is used by the CLI. Execution emits separate measurement, detector, observable, and optional sampled-error planes and imports no text codec, filesystem, CLI, or ops API. Physical extraction into `stab-engine` remains A6 work. |
| `sampling_output_compat.rs`, `sampling_estimate.rs` | Facade compatibility and planning | Materialized and byte-returning `CompiledSampler` methods are explicit adapters over sampling sessions and typed record sinks. Sampling estimates may inspect codec size formulas without making result encoding part of the executable engine contract. |
| `analysis/**` | Analysis | Cross-model semantic adapters live here even when their implementation delegates to a source module awaiting extraction. |
| `circuit_detecting_regions*`, `circuit_feedback.rs`, `circuit_flow*`, `circuit_generation*`, `circuit_inverse*`, `circuit_missing_detectors*`, `circuit_simplify.rs`, `circuit_tableau.rs`, `circuit_transforms.rs` | Analysis | These are pure circuit transforms, generation, lowering, or analysis algorithms. |
| `dem/analyze*`, `dem/arena_index.rs`, `dem/error_traversal.rs`, `dem/graphlike*`, `dem/hyper*`, `dem/sat*`, `dem/search_budget.rs` | Analysis | These consume the model-owned folded traversal boundary and own analysis-specific policies and outputs. |
| `error_matcher*`, `matched_error.rs`, `mbqc_decomposition.rs`, `sparse_rev_frame_tracker*` | Analysis | These are pure matching, decomposition, and reverse-tracking algorithms; simulator-backed sampled-flow checks have moved to execution. |
| `error.rs` | Facade, temporarily | A2 splits typed domain diagnostics while retaining lossless facade conversion; result-format failures wrap `FormatError`, and configurable admission failures wrap `ResourceLimitError`, without changing their established human display. |
| `lib.rs` | Facade | Root reexports remain curated compatibility adapters and do not determine implementation ownership. |

`stab-cli/src/agent.rs` is a CLI adapter, not a new product component. It discovers commands from Clap, renders core descriptors and identities, reuses retained-handle input admission, and may compose parsing, compilation validation, and estimates. It must not become an alternate source of gate, codec, compiler, backend, or qualification truth.

The `stab-core::bits` module and root bit-type paths are compatibility re-exports of canonical `stab_bits` items. The `stab-core::result_formats` and `stab-core::result_streaming` modules similarly adapt canonical `stab_records` behavior without owning a second codec implementation.

New source modules must fit exactly one row or update this table and the architecture decision record in the same change.
