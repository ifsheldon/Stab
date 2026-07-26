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
- Dependencies: foundational standard-library and parsing support only.
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
- Dependencies: model-level Pauli values and packed bit storage.
- Forbidden: Stim parsing, CLI, ops.
- Resource behavior: scalar stable defaults with bounded construction and optional Nightly acceleration above storage kernels.
- Extension points: pure functions and owned values.

### Engine

- Purpose: compile exact models into plans and execute reusable sessions.
- Inputs and outputs: models, compiler options, plans, sessions, typed batches, execution summaries.
- Invariants: private executable IR, one backend decision per plan, session-owned mutable state, bounded execution batches.
- Dependencies: model, records, pure analysis lowering, and raw SIMD kernels; algebra is reached through model and analysis ownership where appropriate.
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
