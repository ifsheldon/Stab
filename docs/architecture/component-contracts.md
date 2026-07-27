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
- Invariants: closed Stim v1.16.0 dialect, canonical printing, structural validity, typed indices and probabilities.
- Dependencies: algebra values needed by the closed Stim model, plus foundational standard-library and parsing support.
- Forbidden: engine, records, CLI, ops.
- Resource behavior: bounded parsing and explicit structural limits.
- Extension points: immutable typed circuit passes consume and return models.

### Records

- Purpose: own typed result layouts, packed batches, strict codecs, sources, and sinks.
- Inputs and outputs: measurement, detection, observable, sampled-error, and prediction batches.
- Invariants: explicit width, namespace, layout, tail-bit, record-boundary, and PTB64 grouping contracts.
- Dependencies: packed bit storage.
- Forbidden: circuit execution, filesystem paths, CLI, ops.
- Resource behavior: bounded by record width and batch size, never total stream length.
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
- Resource behavior: explicit compile and sampling limits, reusable scratch, cancellation at bounded batch boundaries.
- Extension points: measurement and detection sinks.

### Analysis

- Purpose: own pure transforms, circuit-to-DEM analysis, search, generation, and error matching.
- Inputs and outputs: immutable models, transformed models, reports, analysis errors.
- Invariants: no hidden execution session or filesystem state, explicit folded or materialized resource behavior.
- Dependencies: model and algebra.
- Forbidden: CLI, ops, and engine.
- Resource behavior: operation-specific search and materialization policies.
- Extension points: typed circuit passes.

## A1 Logical Source Ownership

This table assigns every current `stab-core` product source module to one target component before physical crate extraction.

Nested `tests.rs` and resource-test modules inherit the owner of their parent source family.

| Current source family | Logical owner | Migration note |
| --- | --- | --- |
| `circuit.rs`, `circuit/**` | Model | The model keeps syntax, values, parsing, printing, iteration, structural counts, and crate-private structure-preserving builders; algorithmic inherent methods, including recursive tag stripping, are temporary adapters implemented under `analysis` or `execution`. |
| `dem.rs`, `dem/api.rs`, `dem/coordinate_scan.rs`, `dem/parser.rs`, `dem/tag.rs`, `dem/traversal.rs` | Model | Folded traversal is the model-owned advanced boundary shared by DEM queries, analysis, and execution. Consumer-specific search, filtering, and probability policies remain with their analysis owners, while recursive transforms are implemented by `analysis/dem_adapters.rs`. |
| `gate.rs`, `gate/**` | Model | Gate syntax and closed Stim scalar or textual descriptors remain model-owned; algebra-valued projections and decomposition parsing are implemented by `analysis/gate_adapters.rs`. |
| `ids.rs`, `target.rs` | Model | Typed identifiers, targets, and validated probability primitives are foundational model values. |
| `bits/**` | Bits | Direct portable-SIMD sites are temporary A6 migration allowances. |
| `diagnostics.rs` | Facade, temporarily | A2 owns shared byte-span, severity, stable code, and typed-context primitives here while domain errors are introduced. Serialization remains CLI-owned. A6 must place the shared stable primitives without making model, records, analysis, or execution depend on the facade. |
| `result_formats.rs`, `result_formats/**`, `result_packed.rs`, `result_streaming.rs`, `result_text.rs` | Records | These modules become strict typed codecs and bounded record streams in A3. Shared text and packed decoders own grammar and length diagnostics so materialized and streaming consumers cannot drift. |
| `stabilizers/**` | Algebra | Pauli, Clifford, Tableau, and Flow mathematics do not own gate syntax. |
| `sampling.rs`, `sampling/**`, `execution/**`, `detection.rs`, `detection/**`, `dem_sampler.rs`, `probability_util.rs` | Engine | Simulator-backed helpers, randomized bit generation, compilation, reusable state, detection conversion, and DEM sampling are execution concerns. |
| `analysis/**` | Analysis | Cross-model semantic adapters live here even when their implementation delegates to a source module awaiting extraction. |
| `circuit_detecting_regions*`, `circuit_feedback.rs`, `circuit_flow*`, `circuit_generation*`, `circuit_inverse*`, `circuit_missing_detectors*`, `circuit_simplify.rs`, `circuit_tableau.rs`, `circuit_transforms.rs` | Analysis | These are pure circuit transforms, generation, lowering, or analysis algorithms. |
| `dem/analyze*`, `dem/arena_index.rs`, `dem/error_traversal.rs`, `dem/graphlike*`, `dem/hyper*`, `dem/sat*`, `dem/search_budget.rs` | Analysis | These consume the model-owned folded traversal boundary and own analysis-specific policies and outputs. |
| `error_matcher*`, `matched_error.rs`, `mbqc_decomposition.rs`, `sparse_rev_frame_tracker*` | Analysis | These are pure matching, decomposition, and reverse-tracking algorithms; simulator-backed sampled-flow checks have moved to execution. |
| `error.rs` | Facade, temporarily | A2 splits typed domain diagnostics while retaining lossless facade conversion; result-format failures now wrap `FormatError` without changing their human display. |
| `lib.rs` | Facade | Root reexports remain curated compatibility adapters and do not determine implementation ownership. |

New source modules must fit exactly one row or update this table and the architecture decision record in the same change.
