# Migrating To Stab 0.2

Stab 0.2 is a coordinated breaking Rust API release. Stim v1.16.0 semantics remain the compatibility target, but pre-0.2 Stab paths and adapters are not preserved.

## Product Crates

| Crate | Primary use |
| --- | --- |
| `stab-model` | Circuits, detector error models, gates, targets, parsing, printing, validation, and model diagnostics |
| `stab-bits` | Packed and sparse bit storage plus scalar bit operations |
| `stab-records` | Typed batches, layouts, strict Stim result codecs, readers, writers, and sinks |
| `stab-algebra` | Pauli, Clifford, tableau, flow, and unitary mathematics |
| `stab-analysis` | Pure transforms, generation, circuit-to-DEM analysis, flow analysis, search, SAT, and error matching |
| `stab-engine` | Compilers, immutable plans, mutable sessions, sampling, conversion, and typed batch delivery |
| `stab-decoder` | Decoder model views, truth-hidden inputs, caller-owned predictions, and reusable decoder sessions |
| `stab-kernels-simd` | Dependency-free Nightly portable-SIMD leaf kernels, not a simulation backend |
| `stab-core` | A small facade over the product crates |
| `stab-cli` | The `stab` command-line binary |

Use an owner crate when building a focused library. Use `stab-core` when one dependency for integrated workflows is more convenient.

## Facade Contract

`stab-core` now has one deliberately small shape:

- The root reexports common owned model, algebra, decoder, and record values.
- `stab_core::analysis` is a direct alias of `stab-analysis`.
- `stab_core::decoder` is a direct alias of `stab-decoder`.
- `stab_core::execution` is a direct alias of `stab-engine`.

There are no `advanced` or `experimental` tiers. Low-level APIs and extension contracts live in their owning crates. The facade does not wrap component algorithms, translate all failures into one error enum, or provide duplicate model types.

## Path Changes

| Removed path or API | Replacement |
| --- | --- |
| `stab_core::advanced::storage::*` | `stab_bits::*` |
| `stab_core::advanced::records::*` | `stab_records::*` |
| `stab_core::advanced::traversal::*` | `stab_model` folded or flattened traversal APIs |
| `stab_core::advanced::algebra::*` | `stab_algebra::*` |
| `stab_core::experimental::{CircuitPass, run_circuit_pass, ...}` | `stab_analysis::{CircuitPass, run_circuit_pass, ...}` or `stab_core::analysis::*` |
| Root analysis algorithms such as `circuit_to_detector_error_model` | `stab_analysis::*` or `stab_core::analysis::*` |
| Root execution compilers and sessions | `stab_engine::*` or `stab_core::execution::*` |
| Root `estimate_sampling_request` | `stab_engine::estimate_sampling_request` or `stab_core::execution::estimate_sampling_request` |
| `stab_core::CircuitError` and `CircuitResult` | The owning `ModelError`, `AnalysisError`, `StabilizerError`, `FormatError`, engine error, or decoder error |
| `SampleFormat` | `stab_records::RecordFormat` or the root `stab_core::RecordFormat` value reexport |
| `CapabilitySet` and facade compiler descriptors | Model, record, and engine descriptors queried from their owners |
| Caller-selectable sampling backend preferences | Removed; scalar is the sole execution plan and portable SIMD is internal leaf acceleration |

The CLI composes owner errors into private human or JSON diagnostics. Library callers should preserve typed owner errors or define an application-specific aggregate only where a workflow genuinely crosses domains.

## Parser Limits

`ParseLimits::new` now takes typed source-byte, source-line, represented-instruction, represented-target, and repeat-nesting limits in that order. The new `SourceByteLimit`, `RepresentedInstructionLimit`, and `RepresentedTargetLimit` values are also available from the `stab-core` facade. Existing parser entry points use inclusive defaults of 64 MiB, 1,000,000 physical lines, 1,000,000 compact declarations, 32,000,000 retained targets, and 256 repeat levels.

`ResourceLimitContext` and the parser-only `stab_model::advanced` error and target-parser constructors were removed. A `ResourceLimitError` now reports its `dialect`, `operation`, `resource`, optional `source_line`, `actual`, `limit`, and source `span` directly.

## Execution APIs

Whole-output and callback compatibility adapters are removed. Reusable execution follows one ownership shape:

```text
compiler -> immutable plan -> mutable session -> optional sink-bound transaction -> typed batch sink
```

- Measurement sampling uses `SamplingCompiler`, `SamplingPlan`, `SamplingSession`, and `MeasurementSink`.
- Measurement-to-detection conversion uses `MeasurementToDetectionCompiler`, `MeasurementToDetectionPlan`, `MeasurementToDetectionSession`, and a short-lived `MeasurementToDetectionTransaction` that binds exactly one `DetectionSink` across incremental writes and finalization.
- DEM sampling uses `DemSamplingCompiler` and an immutable `DemSamplingPlan`. Call `DemSamplingPlan::session` for RNG-bearing sampling or `DemSamplingPlan::replay_session` for owned reusable replay state. Complete replay uses `run`; incremental replay uses a short-lived `DemReplayTransaction` that binds exactly one `DemSampleSink` across writes and finalization. Resetting a cancelled replay requires resetting its cancellation token first.
- Encoded in-memory output is available from codec sinks in `stab-records`; streaming callers can implement the sink traits directly.

The removed `CompiledSampler`, `CompiledDetectionConverter`, and `CompiledDemSampler` types have no facade compatibility replacements.

## Extension APIs

The typed circuit-pass seam is canonical in `stab-analysis`. It admits folded input, requires a conservative output-resource projection before proportional allocation, validates the returned closed-dialect circuit, and reports typed input, projection, or output failures. The separate Stable `stab-reference-noise-pass` crate proves this public boundary without depending on `stab-core`.

Decoder interoperability is canonical in `stab-decoder`. The separate `stab-reference-decoder` crate proves the reusable session boundary without depending on the facade.

Neither seam implies dynamic plugins, runtime gate registration, GPU placeholders, or a universal compiler registry.

## Portable SIMD

Scalar behavior is the default. Portable SIMD is additive and opt-in:

```toml
[dependencies]
stab-core = { version = "=0.2.0", features = ["portable-simd"] }
```

The facade feature forwards only the algebra kernel feature it exposes. Direct `stab-bits` users enable `stab-bits/portable-simd` themselves. Feature unification still resolves one `stab-kernels-simd` package when direct and facade dependencies are combined.

## Error Migration Example

Before 0.2, a workflow often returned `stab_core::CircuitResult<T>`. In 0.2, return the owner result for a single operation:

```rust
fn analyze(circuit: &stab_model::Circuit) -> stab_analysis::AnalysisResult<stab_model::DetectorErrorModel> {
    stab_analysis::circuit_to_detector_error_model(
        circuit,
        stab_analysis::ErrorAnalyzerOptions::default(),
    )
}
```

For an application workflow that parses, analyzes, and writes records, define one local error enum with transparent variants for those three owners. Do not introduce a workspace-wide catch-all error solely to hide component boundaries.

The frozen pre-0.2 baseline, extraction map, and earlier API migration ledger remain historical design evidence. This document and the source-current component contracts describe the final 0.2 API.
