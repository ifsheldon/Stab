# Agent-Native Modular QEC Architecture Plan

Active architecture migration contract as of 2026-07-27.

## Summary

Stab is already agent-friendly as a repository, but its product library is still organized around one broad Stim-compatibility crate.

This plan evolves Stab into a compiler-style QEC toolkit:

```text
typed Stim models
    -> explicit validation and transforms
    -> immutable compiled plans
    -> reusable execution sessions
    -> typed packed batches
    -> codecs, decoders, and analysis consumers
```

The migration targets one coordinated breaking Rust API release, Stab `0.2.0`.

The implemented Stim v1.16.0 file formats, CLI behavior, strict grammars, path-alias safety, statistical semantics, and `1.25x` performance gate remain compatibility contracts.

`stab-core` remains the primary high-performance Nightly facade.

Focused model, record, bit-storage, and scalar-algebra crates become usable on Rust 1.97.1 without compiling portable SIMD.

The current architecture-migration checkpoint is `b03b3c75`.

Clean revision `68d107a42f655254f31628f0cbedc55479f6c0f3` remains the accepted pre-refactor formal compatibility and AArch64 performance checkpoint.

## Response To The External Review

### Recommendations Accepted

- Keep the Stim circuit and detector-error-model dialects closed and exact.
- Formalize parsing, validation, lowering, backend compilation, and execution as distinct phases.
- Separate immutable plans from mutable execution sessions.
- Make typed packed batches and bounded streams the primary interoperability path.
- Keep decoders outside the simulation core and prove integration through a public batch boundary.
- Replace scattered configurable limits with operation-owned resource policies.
- Add structured diagnostics and machine-readable capability introspection.
- Select scalar or portable-SIMD execution once during compilation instead of dispatching in hot loops.
- Isolate every direct `std::simd` use behind a Nightly-only kernel component.
- Remove qualification-only exports from the product feature graph.
- Keep an ergonomic facade and avoid one crate per source module.

### Recommendations Refined

- The claim that models own algorithms is directionally correct at the API boundary, but many `Circuit` methods already delegate to free-function implementations.
- The migration therefore separates enforceable ownership and dependencies instead of rewriting algorithms that are already internally separated.
- Logical ownership is established before physical crate extraction so dependency cycles and public replacements are resolved while behavior still has one compilation boundary; A6 performs the mechanical extraction only after those seams are tested.
- One universal `RecordBatch` would erase meaningful differences between measurements, typed `M` or `D` or `L` records, detector-observable pairs, sparse records, and 64-shot bit planes.
- Stab will define focused batch families over shared packed storage instead.
- A global `ResourcePolicy` would become another broad configuration object.
- Stab will use operation-specific policies with unchanged safe defaults and separate exact admission from advisory estimates.
- Public policy inputs that can be confused use named constrained quantities, but read-only estimate accessors retain ordinary integer values whose unit is fixed by the accessor. Creating a public wrapper for every byte, item, and work-unit field would multiply API surface without preventing an actual construction error because `ResourceEstimate` has no public positional constructor.
- A configurable repeat budget may tighten the shared recursive safety envelope but may not raise it until all recursive model consumers are redesigned. This refines the general rule that configurable safety budgets may be relaxed without allowing a policy to bypass a current hard implementation invariant.
- A universal pass or decoder framework will not be designed from hypothetical plugins.
- Public traits are introduced only after a real built-in implementation and a separate external implementation prove the common contract.
- Plan fingerprints are versioned reproducibility identities, not promises that compiled-plan hashes remain stable across Stab versions or backends.
- Seeded execution guarantees successful chunking equivalence on one session, but not random-access shot ranges, cross-backend identity, or exact Stim random streams.
- The current `ops-contracts` feature is contained debt, not a product dependency inversion.
- It is removed only after its useful product capability data and its qualification-only plans have distinct owners.

### Recommendations Deferred

- Python, JS or WASM, GPU execution, and previously deferred Stim products remain outside this migration.
- The external-process decoder protocol is documented after the Rust seam is proven but is not implemented in `0.2.0`.
- Dynamic Rust plugins, runtime gate registration, a public executable IR, and serialized backend plans are explicitly rejected.
- Controlled x86-64 evidence remains unseeded unless an authorized controlled host becomes available.

## Architectural Invariants

1. Product crates never depend on `ops`.
2. Models never depend on execution or CLI code.
3. Execution never depends on textual codecs or filesystem paths.
4. Analysis never depends on CLI code.
5. The Stim gate table remains closed.
6. Backend selection occurs during compilation.
7. No dynamic dispatch occurs inside gate, word, or shot hot loops.
8. Plans are immutable and shareable; sessions own mutable state.
9. Batch sizes are bounded implementation details and are not semantic output.
10. Existing file-format bytes and default CLI behavior change only when pinned Stim evidence requires it.
11. Resource-policy overrides may relax configurable safety budgets but may not violate semantic invariants or platform representability.
12. Historical evidence remains bound to its original source and schema identity.

## Target Product Crates

| Crate | Toolchain | Responsibility |
| --- | --- | --- |
| `stab-bits` | Stable 1.97.1 | Packed bit storage, checked views, scalar kernels, layout primitives |
| `stab-model` | Stable 1.97.1 | Circuit, DEM, gates, targets, IDs, parse, print, structural validation |
| `stab-records` | Stable 1.97.1 | Measurement and detection batches, layouts, codecs, sources, sinks |
| `stab-algebra` | Stable 1.97.1 by default | Pauli strings, Cliffords, tableaus, flows, scalar algebra |
| `stab-kernels-simd` | Pinned Nightly | Raw portable-SIMD kernels over word slices and fixed word blocks, with no Stab dependency |
| `stab-engine` | Pinned Nightly | Sampling, detection conversion, DEM sampling, backend compilation over shared analysis lowering |
| `stab-analysis` | Stable where independent of execution | Circuit transforms, circuit-to-DEM analysis, search, generation, error matching |
| `stab-decoder` | Stable 1.97.1 | Decoder batch interoperability and conformance support |
| `stab-core` | Pinned Nightly | Curated ergonomic facade and compatibility conveniences |
| `stab-cli` | Pinned Nightly | Thin command adapter over public facade APIs |

Dependency arrows point from a consumer to its dependency:

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

product crates -X-> ops
ops -> product crates
```

`stab-engine` depends on `stab-analysis` for shared gate tableau, decomposition, and canonical lowering semantics.

The inverse edge is forbidden.

Simulation-backed helpers that are currently described as analysis move to `stab-engine` so `stab-analysis` can remain independent of the Nightly execution crate.

## Public API Direction

### Diagnostics

Domain crates expose `ParseError`, `ValidationError`, `FormatError`, `ResourceLimitError`, `CompileError`, `ExecutionError`, and `AnalysisError`.

The facade may expose a non-lossy `StabError` wrapper.

Every diagnostic has a stable kebab-case code, severity, human message, optional byte span, labels, optional help, and typed context.

Parser spans use byte offset and byte length.

Human line and column values are renderer-derived so the source span remains valid for UTF-8 and byte-oriented grammars.

CLI JSON errors use schema version 1:

```json
{
  "schema_version": 1,
  "code": "invalid-record-width",
  "severity": "error",
  "message": "record has 7 bits but 8 were expected",
  "span": {
    "byte_start": 14,
    "byte_length": 7
  },
  "labels": [],
  "help": null,
  "context": {
    "actual_bits": 7,
    "expected_bits": 8
  }
}
```

The human formatter remains the default and preserves current stderr classes and exit statuses.

### Resource Policies And Estimates

Public policies are operation-specific: `ParseLimits`, `CompileLimits`, `SamplingLimits`, `MaterializationLimits`, and `SearchLimits`.

Their defaults reproduce every current source-owned constant and first-rejection boundary.

Hard semantic limits remain private and cannot be overridden.

`ResourceEstimate` labels each field as exact, upper-bound, or unknown and may report input items, expanded operations, folded traversal, scratch bytes, resident bytes, output bytes, and work units.

Estimation must not execute the expensive operation it describes.

Sampling estimation counts folded structure and representable expanded operations without compiling, reports exact output bytes only for fixed-width encodings, and leaves sparse output size, runtime work, scratch, and resident memory unknown until their owners can calculate defensible values.

### Fingerprints And Capabilities

`ModelFingerprint` hashes the dialect identity, fingerprint schema, and canonical model structure with SHA-256.

Schema one starts with the fixed `stab:model-fingerprint\0` domain, a big-endian `u16` schema, and a one-byte dialect discriminator. The remaining stream length-frames every sequence and UTF-8 string with a big-endian `u128`, uses explicit item, instruction, and target discriminators, encodes integers at fixed widths in big-endian order, and encodes exact `f64` bits after normalizing signed zero.

The fingerprint does not hash `.stim` or `.dem` printer output. Compatibility printers intentionally round some floating-point values, so using their text would merge semantically distinct models and would make a schema identity change whenever presentation formatting changed. The structural encoder retains semantic precision, allocates no storage proportional to model volume, uses traversal storage proportional only to repeat depth, and is frozen by independently reconstructed rich circuit and DEM vectors.

The exact field order, discriminator assignments, primitive encodings, frozen vectors, traversal resource behavior, and schema-evolution rule are normative in [model fingerprint schema version 1](../architecture/model-fingerprint-schema-v1.md).

`CompilationRequestFingerprint` hashes the model fingerprint, compiler schema, operation kind, normalized options, and effective limits before backend selection.

Schema one uses the fixed `stab:compilation-request-fingerprint\0` domain, a big-endian request schema, operation and model-dialect discriminators, compiler and model schemas, the raw model digest, and big-endian `u128` counts before normalized option and configurable-limit entries.

Sampling compiler schema one has zero caller-selectable compilation options and zero configurable compile limits. Sweep rejection and representability checks are fixed compiler semantics; shots, seed, reference mode, output format, and paths are execution or routing inputs and are excluded.

The exact byte contract and independently reconstructed vector are normative in [compilation request fingerprint schema version 1](../architecture/compilation-request-fingerprint-schema-v1.md).

`PlanFingerprint` is completed in A4 after compilation and hashes the request fingerprint, selected backend, and executable-contract identity.

Request fingerprints are comparable only when their fingerprint schema and operation kind match.

Plan fingerprints are comparable only when their fingerprint schema and backend identity match.

Compiled plans are not serializable.

`CapabilitySet` is generated from gate descriptors, compiler registrations, codec registrations, and backend registrations.

No feature checklist or manually synchronized capability manifest is used at runtime.

The first registry reads gates from `Gate::all()`, codecs from one records-owned six-format table, and sampling from a descriptor colocated with `CompiledSampler::compile`. Its selectable-backend iterator is empty until A4 introduces a genuine backend-selection boundary.

### Plans, Sessions, And Execution

Sampling uses:

```rust
let plan = SamplingCompiler::new()
    .backend(BackendPreference::Auto)
    .limits(SamplingLimits::default())
    .compile(&circuit)?;

let mut session = plan.session(RandomPolicy::Seeded(Seed::new(42)));
let summary = session.run(ShotCount::new(10_000), &mut sink)?;
```

Execution uses four distinct plan families:

```text
SamplingCompiler
    -> SamplingPlan
    -> SamplingSession

MeasurementToDetectionCompiler
    -> MeasurementToDetectionPlan
    -> MeasurementToDetectionSession

DetectionSamplingCompiler
    -> DetectionSamplingPlan
    -> DetectionSamplingSession

DemSamplingCompiler
    -> DemSamplingPlan
    -> DemSamplingSession
```

`DetectionSamplingPlan` preserves the current distinction between direct detector-frame execution and fused measurement sampling plus conversion.

Plans are immutable, cloneable, `Send + Sync`, nonserializable, and own lowered operations plus metadata.

Sessions own RNG state, frames, scratch buffers, temporary records, counters, and cancellation state.

Sessions are reusable but are not promised to be `Sync`.

Internal execution batches contain at most 64 shots.

This ceiling aligns with PTB64, bounds cancellation latency, and permits backend-native bit planes.

Successful `run(a)` followed by `run(b)` on one seeded session must produce the same record sequence as `run(a + b)` on an equivalent session.

This guarantee applies only to the same compiler schema, backend, plan, seed, and Stab version.

Zero shots do not call the sink or advance the RNG.

Request and resource validation occurs before RNG advancement or sink calls.

A pre-execution rejection leaves the session reusable.

Cancellation is checked between bounded internal batches and leaves the session resumable.

A sink error stops immediately and poisons the session because the sink may have accepted an unknown prefix of the offered batch.

Further calls on a poisoned session return `session-poisoned`.

An internal execution failure after work begins and a sink finalization failure also poison the session.

Failure reports identify shots committed before the failing sink call and the attempted batch size without claiming that a partially accepted batch committed.

### Typed Batch Families

`stab-records` owns:

- `PackedShotBatch` and `PackedShotBatchView`
- `BitPlane64Batch` and `BitPlane64BatchView`
- `MeasurementBatchView`
- `DetectionBatchView`, with detector and observable storage kept separate
- `DemSampleBatchView`, with optional sampled-error storage
- `ObservablePredictionBatch`

Record layouts are explicit and never inferred from byte length.

Measurement, detector, observable, sampled-error, and correction widths use distinct typed values.

DETS continues to use `DetsLayout` with independent `M`, `D`, and `L` namespaces.

`MeasurementSink`, `DetectionSink`, and `DemSampleSink` use associated sink errors and have `write_batch` plus `finish` operations.

Execution returns `RunError<SinkError>` without erasing either the engine or sink failure.

Codecs implement sinks.

Engines contain no `SampleFormat`, textual writer, path, or file type.

### Decoder Seam

`stab-decoder` defines stable detection-input, prediction-output, and `DecoderSession` contracts.

Decoder compilation remains implementation-specific in `0.2.0`.

The common contract is extracted only after a real bounded repetition-code decoder and a conformance implementation use it.

The reference decoder is a separate unpublished workspace crate and depends only on public stable model, record, and decoder crates.

It implements exact maximum-likelihood decoding for selected small repetition-code DEMs with at most 20 detectors and one logical observable.

The algorithm performs bounded dynamic programming over independent DEM error mechanisms and records the most likely observable class for each detector syndrome.

It is a real conformance and research example, not a production matching decoder.

### Extension Passes And Backends

The Stim gate table remains static.

A `CircuitPass` trait is introduced only after one built-in transform and one external noise-insertion pass establish the shared input, option, report, diagnostic, and resource contracts.

Passes return structurally validated Stim-compatible circuits.

An operation that cannot lower into the closed dialect is rejected instead of entering a runtime gate registry.

`BackendPreference` contains `Auto`, `Scalar`, and `PortableSimd`.

There is no placeholder `Gpu` variant.

Public plans wrap private backend-specific plan variants, and hot loops remain statically dispatched.

## Milestone A0: Architecture Contract And Baseline

### Tasks

- Add this plan and make `GOAL.md` its active execution contract.
- Add `docs/architecture/README.md`.
- Add ADRs for the closed Stim dialect, dependency graph, plans and sessions, batch layouts, diagnostics and resources, and Stable versus Nightly boundaries.
- Add one component-contract template and initial contracts for model, records, algebra, engine, and analysis.
- Freeze the pre-`0.2` public API and package graph for migration documentation.
- Mark the previous qualification goal complete and historical rather than overwriting its evidence.

### Tests And Checks

- Validate documentation links and instruction-document policy.
- Run the existing correctness and performance inventory checks without regeneration drift.
- Confirm the source worktree is clean before the first behavior-changing milestone.

### Done Criteria

- One document owns the target graph, one document owns current execution, and no active document still names the completed qualification-economy plan as current work.
- No product behavior or checked evidence identity changes.

## Milestone A1: Logical Ownership And Dependency Enforcement

### Tasks

- Assign every current module to one logical model, bits, records, algebra, execution, analysis, or facade owner.
- Introduce owning code namespaces in A1 where behavior moves across the current monolith, specifically `analysis` for gate projections and pure circuit or DEM transforms, and `execution` for compiled sampling, reference sampling, determined-measurement counting, and sampled-flow checks; keep physical source and crate extraction exclusively in A6, and defer detection-converter and DEM-sampler namespace completion to A5.
- Preserve implementation behavior while changing internal ownership.
- Remove algebra's dependency on `Gate`.
- Move named-gate-to-tableau, flow, unitary, and decomposition conversion into semantic adapters that depend on both the model and algebra.
- Move algorithmic `Circuit` conveniences out of the model implementation.
- Move simulation-backed analysis helpers into execution.
- Keep folded DEM traversal model-owned through a documented crate-internal advanced visitor boundary used by analysis and execution; A6 decides the minimum cross-crate visibility required during physical extraction.
- Make execution depend on analysis lowering instead of duplicating gate semantic tables.
- Add an `ops` Rust architecture checker backed by `cargo metadata`.
- Expose it through `just architecture::check` and CI.

### Tests

- Reject every forbidden product dependency edge through fixture package graphs.
- Prove permitted optional feature edges.
- Preserve gate semantic metadata, circuit transform, analyzer, and sampler behavior.
- Verify the public migration inventory identifies every removed or moved root API.

### Benchmarks

- Run benchmark smoke and one warmed, single-measurement-run primary diagnostic comparison against the latest accepted clean primary baseline, write it to a unique `target/benchmarks/` path, and record the baseline identity, source revision, and local-modification state; a dirty-worktree result is permitted only as non-promotable diagnostic evidence.
- Do not create new timing rows for namespace-only moves.

### Done Criteria

- The proposed physical crate graph has no unresolved dependency cycle.
- Every module has one documented owner.
- Architecture checking is fast, deterministic, and enforced in CI.

## Milestone A2: Diagnostics, Resources, Fingerprints, And Capabilities

### Tasks

- Introduce domain error types and non-lossy facade conversion.
- Add exact byte spans to circuit, DEM, and result-format parse errors.
- Move configurable constants into operation-owned policy defaults without changing values.
- Add typed resource estimates with exact, upper-bound, and unknown classifications.
- Add versioned model and backend-neutral compilation-request fingerprints.
- Generate runtime capabilities from execution descriptors.
- Add `--error-format=human|json`.
- Add `stab capabilities`, `stab inspect`, and `stab plan sample` as documented Stab extensions.

### Rationale

- Keep successful machine output separate from diagnostic JSON Lines so one schema never has to represent both a report and a stream of warnings or failures.
- Generate discovery output from owning product descriptors so agents see the implementation that will actually parse, compile, and encode their request, not a qualification inventory or manually synchronized checklist.
- Describe gate entries as accepted circuit syntax rather than universal execution support; individual compilers still validate operation-specific capability.
- Keep the selectable-backend list empty until A4 creates a real backend-selection boundary. A placeholder backend would make capability and fingerprint contracts lie about caller choice.
- Keep shots, seed, reference mode, codec, paths, and compatibility no-ops in run configuration rather than backend-neutral compilation identity.
- Let `inspect` stop after parsing and structural inspection. Let `plan sample` compile only for validation, then use folded checked counting for estimates; neither command executes a shot or expands a compact repeat merely to estimate output width.
- Benchmark the owning phases, not `stab_cli::run_from` end to end. A combined CLI number could not distinguish parsing, hashing, compilation, estimation, serialization, and I/O.

### Tests

- Exact parser spans for LF, CRLF, UTF-8 tags, malformed bytes, and EOF.
- Stable diagnostic codes and schema-version-1 JSON.
- Existing CLI error class, precedence, exit status, and path-safety behavior.
- Exact old accepted maxima and first rejections under default policies.
- Rejection of policy overflow and attempted semantic-limit overrides.
- Model and request fingerprint determinism, schema separation, canonical-input identity, operation distinction, and normalized-option identity.
- Capability generation consistency with gate and compiler descriptors.

### Benchmarks

- Reuse the existing successful circuit-parse benchmark instead of creating an overlapping parse product.
- Add exactly four Stab-only diagnostic runtime groups: circuit model fingerprint, inclusive sampling-request fingerprint, sampling-request estimate, and sampler compilation.
- Give each diagnostic one measurement and structural scales of 64, 4,096, and 65,536 top-level circuit items.
- Parse and fixture construction occur before timing. Output witness construction, digesting, RSS collection, and serialization occur after the `raw-work-v2` finish clock.
- The inclusive request-fingerprint measurement includes the model fingerprint calculated by `CompilationRequestFingerprint::for_sampling`; do not subtract independently measured medians to imply incremental cost.
- Do not add these rows to Stim parity policy, self-regression baselines, release rollups, the legacy manifest, or formal completion receipts. They are Stab-only product diagnostics until a scientifically equivalent comparator and a demonstrated release risk exist.
- Do not create separate capability-enumeration or JSON-rendering benchmarks without profiling evidence that either is a meaningful product cost.

### Done Criteria

- Agents can discover supported operations, parse structured failures, and inspect a sampling request without executing it.
- Existing human CLI behavior remains the default.

## Milestone A3: Stable Packed Records And Codecs

### Tasks

- Extract `stab-bits` and `stab-records`.
- Add owned and borrowed shot-major and bit-plane batch types.
- Keep detector and observable planes separate.
- Implement bounded layout conversion.
- Make every result writer a typed sink.
- Keep record-at-a-time visitors as adapters.
- Preserve strict text lexers and typed DETS parsing.

### Tests

- Run all 62 checked result-format corpus cases through the extracted crates.
- Cover every format, width boundary, tail bit, empty record, namespace, duplicate, and PTB64 group rule.
- Property-test shot-major to bit-plane conversions and round trips.
- Prove cancellation and first-error behavior.
- Prove allocation and retained capacity are bounded by width and batch size, not record count.
- Verify Stable Rust 1.97.1 builds.

### Benchmarks

- Shot-major writing.
- Bit-plane writing.
- Packed transpose.
- DETS parsing.
- Representative `01`, `b8`, DETS, and PTB64 conversions.
- Allocation counts for reusable codecs.

### Done Criteria

- Codecs operate on typed batches.
- Exact Stim bytes remain unchanged.
- Stable users can parse and convert result records without `stab-core` or Nightly.

## Milestone A4: Sampling Compiler, Plan, Session, And Sink

### Tasks

- Replace `CompiledSampler` as the architectural center.
- Introduce compiler, immutable plan, mutable session, random policy, run summary, sink finalization, and sink error composition.
- Keep the executable IR private.
- Select scalar or portable SIMD at compilation.
- Complete the backend-bearing `PlanFingerprint` only after backend selection and bind it to the request fingerprint and executable-contract identity.
- Reuse frames, RNG, reference samples, records, and output batches across calls.
- Preserve direct-Z, small-frame, and general stabilizer-frame execution as private plan variants.
- Make existing materialized and byte-returning conveniences thin adapters on the new path.
- Retain current compiled types as migration adapters until every CLI and oracle call site uses the new path, then remove them from the `0.2.0` root API.
- Migrate `stab sample`.

### Tests

- Compilation and unsupported-capability diagnostics.
- Plan-fingerprint determinism, schema separation, backend distinction, and executable-contract distinction.
- Plan sharing across threads and session isolation.
- Same-session chunking equivalence.
- Zero-shot behavior.
- Cancellation at batch boundaries.
- Sink write and finalization error poisoning, exact progress, and immediate stop.
- Pre-execution validation rejection without poisoning.
- Reference-sample and skip-reference behavior.
- Direct-Z, small-frame, and general-frame seeded old-versus-new equivalence.
- Deterministic and statistical Stim parity.
- No allocation growth after session warmup.

### Benchmarks

- Compilation.
- Session construction.
- Raw execution.
- Batch delivery.
- Encoding.
- Repeated execution on one session.
- Scalar versus portable SIMD.
- CLI end-to-end sampling.

### Done Criteria

- Execution imports no codec or filesystem API.
- Existing sampling compatibility remains green.
- Comparable rows retain the `1.25x` Stim gate and `15%` self-regression gate.

## Milestone A5: Detection And DEM Batch Pipelines

### Tasks

- Introduce separate measurement-to-detection, circuit-detection-sampling, and DEM-sampling compiler, plan, and session families.
- Preserve detector-frame execution as a first-class private detection-sampling plan variant.
- Reuse conversion and reference-sample scratch across batches.
- Add a measurement-to-detection sink adapter.
- Represent detectors, observables, and optional sampled errors as distinct batch planes.
- Preserve distinct DEM detector-only and sampled-error algorithms because they consume randomness differently.
- Validate and rewind replay input before activating output sinks, preserving existing malformed-replay file safety.
- Keep initial `m2d` input delivery record-at-a-time so a later malformed record cannot suppress already valid output.
- Preserve replay semantics and bounded folded DEM traversal.
- Migrate `detect`, `m2d`, and `sample_dem`.

### Tests

- Streamed versus materialized equivalence.
- Direct detector-frame and fused sampling-conversion equivalence.
- Sweep-conditioned conversion.
- Observable append, prepend, and side output.
- Replay input and sampled-error output.
- Correlated DEM events.
- Cancellation, sink error, and poisoned sessions.
- Same-session partitioning.
- Large bounded streams across all supported formats.
- Existing path-alias preflight and writer-error propagation.
- `m2d` valid-prefix output before a later malformed record.
- Replay validation before any output creation or truncation.

### Benchmarks

- Detection compilation.
- Batch conversion.
- Sample-to-detection composition.
- DEM session execution.
- Replay.
- PTB64 routing.
- Affected CLI rows.

### Done Criteria

- Sample-to-detection and DEM sampling memory scale with width and batch size, not total shots.
- No CLI command bypasses the public plan, session, and sink path.

## Milestone A6: Physical Component Extraction And Nightly Isolation

### Tasks

- Extract `stab-model`, `stab-algebra`, `stab-engine`, and `stab-analysis`.
- Extract `stab-kernels-simd` as the only direct portable-SIMD owner.
- Extract in dependency order: algebra after removing `Gate`, model after removing foreign inherent algorithm methods, analysis after model and algebra, engine after analysis, then SIMD kernels after scalar paths are explicit.
- Make model, bits, records, scalar algebra, and pure analysis compile on Stable 1.97.1.
- Keep the full facade, engine, and CLI on pinned Nightly.
- Give `stab-kernels-simd` no Stab dependencies and restrict its cross-crate API to raw `[u64]`, `&mut [u64]`, and fixed `[u64; 4]` kernels.
- Make scalar behavior the absence of the additive `portable-simd` feature; do not create mutually exclusive scalar and SIMD features.
- Add exact `=0.2.0` versions beside every publishable path dependency.
- Make CLI, oracle, and benchmark Nightly intent explicit by enabling the facade's `portable-simd` feature instead of relying on feature unification.
- Remove `ops-contracts`.
- Move useful capability descriptions into product descriptors.
- Move statistical plans and benchmark-only descriptors into ops.
- Curate `stab-core` root, `advanced`, and `experimental` namespaces.

### Tests

- Stable and Nightly CI matrices.
- Default-feature and portable-SIMD feature-unification checks.
- Stable external-consumer and Nightly facade-consumer fixtures.
- Architecture rejection of `std::simd` outside `stab-kernels-simd`, any Stab dependency from that kernel crate, Stable default features reaching Nightly, and Stable dev dependencies reaching engine, facade, CLI, or ops.
- Scalar and SIMD semantic equivalence.
- Architecture dependency checks.
- Rustdoc public API inventory and tier checks.
- No product-to-ops dependency or qualification-only public item.

### Benchmarks

- Re-run every bit, algebra, parse, records, sampler, converter, DEM, and analysis row whose call path moved.
- Attribute regressions to phase-specific measurements instead of aggregate facades.

### Done Criteria

- Stable component consumers do not compile `std::simd`.
- The Nightly facade preserves high-performance execution.
- Only `stab-kernels-simd` contains `#![feature(portable_simd)]`.
- Every product crate has a documented contract and permitted dependencies only.

## Milestone A7: Decoder Interoperability And Reference Decoder

### Tasks

- Add stable `stab-decoder`.
- Define detection input and observable prediction contracts.
- Define `DecoderSession` after the reference and conformance implementations agree.
- Add an unpublished external repetition decoder crate.
- Compile selected small repetition DEMs into bounded exact maximum-likelihood tables.
- Compose sampling, detection conversion, decoding, and logical-error counting through public batches only.

### Tests

- Exhaustive small-model probability agreement.
- Distance-3 and distance-5 generated repetition circuits.
- Brute-force syndrome and observable agreement.
- Impossible syndrome handling.
- Detector and observable width mismatches.
- Resource rejection above 20 detectors or one observable.
- Batch partitioning and cancellation.
- End-to-end logical-error experiments with seeded reproducibility.

### Benchmarks

- Decoder compilation.
- Batch decode throughput.
- Full sample-to-detect-to-decode throughput.
- Bounded memory.
- Decoder benchmarks use Stab self-regression only because Stim has no faithful decoder comparator.

### Done Criteria

- The decoder crate depends only on public stable component APIs.
- One real end-to-end QEC experiment runs without `stab-core`, private APIs, or ops features.

## Milestone A8: Circuit Pass And Backend Extension Seams

### Tasks

- Adapt one built-in transform and one external noise-insertion transform to a common pass contract.
- Add typed pass context, options, report, diagnostics, and limits.
- Validate every pass output as a Stim-compatible circuit.
- Expose backend availability and selection through capabilities and plan summaries.
- Document the future external-process decoder protocol requirements without implementing transport.

### Tests

- Pass determinism.
- Tag, repeat, coordinate, and target preservation.
- Invalid lowering and unsupported extension rejection.
- Resource admission.
- External-crate compilation.
- Backend auto selection, explicit selection, unavailable backend errors, and capability consistency.

### Benchmarks

- Built-in transform before and after adaptation.
- External noise pass by input instruction count.
- Backend compile and execution selection overhead.

### Done Criteria

- A separate crate can add a meaningful transform without changing the Stim gate table, execution IR, or unrelated model code.
- No placeholder GPU or dynamic plugin interface exists.

## Milestone A9: Qualification And Stab 0.2.0

### Tasks

- Write `MIGRATING-0.2.md`.
- Update README, feature checklist, generated API docs, architecture docs, qualification inventories, benchmark policies, and generated status.
- Keep the performance program below 40 release groups and 60 diagnostic groups.
- Add only compiler, session, batch pipeline, codec, and decoder families that protect new architectural risks.
- Run milestone-audit and full-code-review before formal evidence.
- Fix every confirmed implementation, test, benchmark, documentation, and architecture finding.
- Regenerate from one clean commit.
- Publish all product crates and `stab-core` together as `0.2.0`.
- Keep ops crates and the reference decoder unpublished.

### Correctness Evidence

- Formatting and workspace Clippy.
- Stable component checks.
- All workspace tests.
- Architecture checks.
- Live result-format corpus.
- Implemented CLI oracle.
- Correctness inventory check and deterministic regeneration.
- PR, full, and soak correctness tiers for affected surfaces.

### Performance Evidence

- Benchmark manifest and qualification checks.
- Legacy primary suite as diagnostic continuity.
- Controlled AArch64 full and soak evidence for every affected release group, expected to include all current 19 groups.
- Stim paired median and confidence upper bound no greater than `1.25x`.
- Stab self-regression no greater than `15%`, with missing identities reported as unseeded.
- Allocation, resident memory, and scaling checks for plans, sessions, batches, and decoder execution.
- No threshold relaxation or waiver may be introduced merely to close the migration.

### Done Criteria

- Each of the practical agent-native workflows in the external review is executable and documented.
- Existing default CLI and file-format behavior remains compatible for implemented surfaces.
- Stable component crates build on Rust 1.97.1.
- The external decoder and pass use public APIs only.
- Formal evidence and audits pass.
- Swap is restored after controlled timing, no qualification process remains, and the worktree is clean.

## Standard Verification

```text
cargo +1.97.1 check -p stab-bits -p stab-model -p stab-records -p stab-algebra
cargo fmt --all --check
cargo clippy --workspace --all-targets -- -D warnings
cargo test --workspace
just architecture::check
just oracle::result-formats --check
just oracle::run --implemented-only
just qualification::correctness-check
just qualification::correctness-regenerate --check
just bench::qualification-check
just bench::qualification-regenerate --check
just qualification::status --check
just bench::smoke
just maintenance::pre-commit
```

## Release Assumptions

- Rust API breaks are intentional and collected into the single `0.2.0` release.
- CLI behavior and public file formats are not granted the same breaking permission.
- Existing convenience workflows remain available through the new facade even when type names and signatures change.
- No exact C++ Stim RNG stream is required.
- Successful partitioning on one session is the only new RNG identity contract.
- The private executable IR remains private until a later real backend proves a stable abstraction.
- Historical qualification evidence remains readable but never becomes source-current automatically.
