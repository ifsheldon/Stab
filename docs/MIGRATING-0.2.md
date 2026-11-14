# Migrating To Stab 0.2

Stab 0.2 is a coordinated breaking Rust API release. Stim v1.16.0 semantics remain the compatibility target, but pre-0.2 Stab paths and adapters are not preserved.

## Product Crates

| Crate | Primary use |
| --- | --- |
| `stab-model` | Circuits, detector error models, gates, targets, parsing, printing, validation, and model diagnostics |
| `stab-bits` | Packed bit storage plus bit operations, with whole-slice XOR through `stab-kernels-simd` |
| `stab-records` | Typed batches, layouts, strict Stim result codecs, readers, writers, and sinks |
| `stab-algebra` | Pauli, Clifford, tableau, flow, and unitary mathematics |
| `stab-analysis` | Pure transforms, generation, circuit-to-DEM analysis, flow analysis, search, SAT, and error matching |
| `stab-engine` | Compilers, immutable plans, mutable sessions, sampling, conversion, and typed batch delivery |
| `stab-decoder` | Decoder model views, truth-hidden inputs, caller-owned predictions, and reusable decoder sessions |
| `stab-kernels-simd` | Slice-level SIMD leaf kernels built on `fearless_simd` with per-call instruction-set selection, always used by `stab-bits` and `stab-algebra`, not a simulation backend |
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
| Caller-selectable sampling backend preferences | Removed; scalar is the sole execution plan and SIMD is internal leaf acceleration |

The CLI composes owner errors into private human or JSON diagnostics. Library callers should preserve typed owner errors or define an application-specific aggregate only where a workflow genuinely crosses domains.

The unused `stab_bits::SparseXorVec`, `inplace_xor_sort`, and `is_subset_of_sorted` APIs are removed without compatibility aliases. Sparse result codecs and sparse reverse-frame analysis remain supported through their independent owners. The tests-only numerical wrappers `is_power_of_2`, `floor_lg2`, and `first_set_bit` are also removed; callers can use Rust's integer bit operations directly. `Gate::stim_name_hash`, which exposed Stim's internal gate-table hash, is removed without replacement.

The partial `stab_analysis::mbqc_decomposition` helper, also exposed through `stab_core::analysis`, is removed without a replacement. MBQC help decomposition remains outside the selected computational API; supported circuit base-gate lowering continues through `stab_analysis::decomposed_circuit`.

`DemSamplerLimits` per-shot and total work limits were removed in E4; see Memory Budget below.
Sessions stream any number of shots whose reusable storage fits the budget.

`Tableau::random` now samples uniformly over the Clifford group, signs included, using the Bravyi–Maslov construction ported from Stim. Earlier builds applied a bounded random gate walk whose distribution was not uniform, so seeded callers receive different tableaux for the same seed; the signature is unchanged. Since E4 the sampler runs on packed rows at any width the memory budget admits, and it consumes the random stream exactly as the earlier 64-qubit sampler did, so seeded tableaux of at most 64 qubits are unchanged.

Circuit sampling compiles one resolved program shared with detection, and no public item was removed or renamed. Four observable behaviors changed. `SamplingCompiler::compile` then counted its fixed one-million boundary, removed in E4, in expanded circuit instructions, as `DetectionSamplingCompiler` does, so `SamplingCompileError::ExpandedOperationLimit` reports instruction counts and wide circuits that detection already accepted now compile; the sampling compiler schema is `6` and the executable-contract schema is `6`, so every sampling request and plan fingerprint changed. A circuit whose only work is one Z measurement, optionally after one single-qubit Pauli channel on the measured qubit, no longer uses a dedicated direct-Z plan, so its seeded output stream differs while its distribution does not. Without that plan, a single measurement of a very large qubit index is refused by the same frame and reference storage limits as any other circuit, including the `MeasurementToDetectionCompiler` reference sample. `count_determined_measurements` with `unknown_input` now matches pinned Stim on circuits that reset every qubit before measuring them, where it previously undercounted. The measurement-to-detection compiler schema reported by `capabilities` is `4`, because its reference sample and sweep correction now share one program admitted within the conversion's configured compiled-byte limit instead of a separate sampling plan.

## Memory Budget

E4 replaces count and work limits with one typed byte budget per operation.
`stab_model::MemoryBudget`, also exported from the `stab-core` root with `MemoryBudgetParseError`, parses an integer byte count with an optional `KiB`, `MiB`, or `GiB` suffix through `FromStr` and defaults to one gibibyte.
Every admitted workload produces byte-identical output; only workloads that the removed caps refused are newly admitted.

| Removed item | Replacement |
| --- | --- |
| `DetectionConversionLimits::{max_record_bits, max_expanded_instructions, max_repeat_iterations, max_compiled_terms, max_compiled_bytes}` and their `with_*` builders | `DetectionConversionLimits::{new, memory_budget, with_memory_budget}`; the budget bounds the compiled plan, one byte per record bit, the reference scratch, and session storage |
| `DetectionResourceKind::{RecordBits, SamplingExpandedOperations, ExpandedInstructions, RepeatIterations, CompiledTerms}` and `DetectionRecordLimitSubject` | `DetectionResourceKind::CompiledBytes` and `RepeatNesting` |
| `SamplingCompileError::ExpandedOperationLimit` | `SamplingCompileError::CompiledBytesLimit { required, budget }`; configure the budget with `SamplingCompiler::memory_budget` |
| The general stabilizer-frame sampling fallback for programs above 256 MiB | A typed `CompiledBytesLimit` refusal against the budget |
| `DemSamplerLimits::{max_sampled_error_applications_per_shot, max_replay_work_units_per_shot, max_active_batch_bytes}` and their builders; `DemResourceKind::{SampledErrorApplications, ReplayWorkUnits}`; `DemSamplingExecutionError::SessionStorageLimit` | `DemSamplerLimits::{new, memory_budget, with_memory_budget}`; session storage refusals report `DemResourceKind::ActiveBatchBytes` |
| `DemSamplingPlan::{validate_replay_with_limits, validate_replay_work_units_with_limits, validate_detector_sample_work_units_with_limits, validate_sampled_error_work_units_with_limits}` | `DemSamplingPlan::validate_replay`, which only checks representability |
| `CircuitFlattenLimits` count fields, `DEFAULT_MAX_*` constants, and builders | `CircuitFlattenLimits::{new, memory_budget, with_memory_budget}` |
| `DemFlattenLimits` repeat-unroll, expanded-instruction, repeat-iteration, target, argument, and byte fields, `DEFAULT_MAX_*` constants, and builders | `DemFlattenLimits::{new, memory_budget, with_memory_budget}` |
| `CircuitPassLimits` item, target, argument, and payload fields, `DEFAULT_MAX_*` constants, and builders | `CircuitPassLimits::new(memory_budget, repeat_nesting)` with `memory_budget` and `with_memory_budget` |
| Fixed analyzer, feedback-inlining, Pauli-conjugation, time-reversal, flow-generator, missing-detector, detecting-region, ErrorMatcher, and coordinate-query work caps | Removed; retained storage is admitted against the default budget, and `ErrorAnalyzerOptions::memory_budget`, `circuit_with_inlined_feedback_with_limits`, `DetectorErrorModel::detector_coordinates_with_memory_budget`, and the generator parameters' `with_memory_budget` accept a caller budget |
| The generator's 131,072-physical-qubit cap | A projected 2048 bytes per physical qubit admitted against the generator parameters' budget |

`ErrorAnalyzerOptions` gained the public `memory_budget` field, so struct literals must add it or use `..ErrorAnalyzerOptions::default()`.
The CLI's fixed 64 MiB input caps and 1 MiB or 64 MiB per-record caps are replaced by `--memory-budget`, which bounds whole inputs at the budget and streamed records at half of it.

## Cooperative Cancellation

E4 adds one cancellation signal, `stab_model::Cancellation`, with its error `stab_model::Cancelled`, both also exported from the `stab-core` root.
`Cancellation::new()` creates a settable signal and `Cancellation::never()`, also the `Default`, one that cannot be set, so code that never cancels keeps its behavior.
See the resource policy inventory's Cooperative Cancellation section for where each operation polls the signal and what it returns.

| Changed item | Migration |
| --- | --- |
| `SamplingCancellation`, `DemSamplingCancellation` | Removed; every session's `cancellation()` now returns `stab_model::Cancellation`, which keeps `cancel`, `reset`, and `is_cancelled` |
| `SamplingCompiler`, `MeasurementToDetectionCompiler`, `DetectionSamplingCompiler`, `DetectionConversionLimits`, `DemSamplerLimits`, `CircuitFlattenLimits`, `DemFlattenLimits` | No longer `Copy`; their `compile` methods take `&self`, limit accessors take `&self`, and `cancellation`/`with_cancellation` builders were added; clone a value that is used twice |
| `ErrorAnalyzerOptions`, `MissingDetectorOptions`, `DetectingRegionOptions`, `DetectingRegionTargetOptions`, `TimeReversedForFlowsOptions`, `InverseQecOptions` | Gained a public `cancellation` field and are no longer `Copy`; struct literals add `cancellation: Cancellation::never()` or use `..Default::default()`, and `MissingDetectorOptions` now implements `Default` |
| `circuit_with_inlined_feedback_with_memory_budget(circuit, budget)` | `circuit_with_inlined_feedback_with_limits(circuit, FeedbackInliningLimits::new(budget))` |
| None | `circuit_flow_generators_with_options` with `FlowGeneratorOptions`, `explain_errors_from_circuit_with_options` with `ExplainErrorsOptions`, `DemFlattenedInstructionIter::with_cancellation`, the defaulted `FoldedDemVisitor::poll_cancellation` hook, and `stab_model::advanced::CancellationCountdown` |
| Exhaustive matches on `ModelError`, `AnalysisError`, `SamplingCompileError`, `SamplingExecutionError`, `DetectionError`, `DetectionExecutionError`, and `DemError` | Add the new `Cancelled(Cancelled)` variant; `SamplingCompileErrorCode::Cancelled` reports `cancelled`, and `AnalysisError` converts a model `Cancelled` into its own variant |

A frame-sampling session that observes cancellation inside a batch returns the `Cancelled` error and is poisoned, because its random stream already advanced; between batches it still returns the resumable `Cancelled` status.
Measurement-to-detection conversion and DEM sampling and replay report an in-batch cancellation as the resumable `Cancelled` status.

## Algebra Byte Admission

E4 removes every count and work cap in `stab-algebra` by replacing the dense algorithms with word-parallel ones over packed rows, and admits the storage those caps bounded in bytes.
`stab_algebra::AlgebraMemoryBudget`, also exported from the `stab-core` root, is the algebra's own typed byte bound with the same one-gibibyte default as `MemoryBudget`, because `stab-algebra` does not depend on `stab-model`.
Constructors that turn a requested size into storage admit its bytes before allocating or drawing randomness; operations on values the caller already holds reserve a bounded multiple of them fallibly.

| Removed or changed item | Replacement |
| --- | --- |
| `StabilizerResource::{PauliQubits, CliffordQubits, TableauQubits, RandomTableauQubits, StabilizerSolveQubits, UnitaryMatrixDimension, FlowClassicalTerms, CircuitTableauRepeatWork}` and the `limit` and `ensure` methods | `StabilizerResource::{PauliString, CliffordString, Tableau, RandomTableau, StabilizerSolve, FlowTerms}`, which name the refused or failed storage and carry no limit |
| `StabilizerError::ResourceLimitExceeded { resource, requested, limit }` | `StabilizerError::MemoryBudgetExceeded { resource, requested_bytes, budget_bytes }` for a refused request and `StabilizerError::AllocationFailed { resource, bytes }` for a failed fallible reservation |
| `StabilizerError::OverconstrainedStabilizers` | Removed; as in Stim, a generator beyond the independent ones is reported as redundant or contradictory |
| `Tableau::x_output` and `Tableau::z_output` returning `&PauliString` | Both return an owned `PauliString`, because generator images are packed rows rather than stored strings |
| `stab_algebra::advanced::{tableau_identity_unchecked, tableau_from_output_columns_unchecked}` | `Tableau::identity_with_budget` and `Tableau::from_conjugated_generators`; a Tableau with mismatched generator shapes can no longer be constructed |
| None | `Tableau::identity_with_budget`, `Tableau::random_with_budget`, `stabilizers_to_tableau_with_budget`, `Tableau::is_identity`, `stab_analysis::circuit_to_tableau_with_options` with `CircuitToTableauOptions`, and `stab_analysis::ResourceOperation::TableauConversion`, which exhaustive matches on `ResourceOperation` must add |

Behavior changes for workloads the removed caps admitted:

- `stabilizers_to_tableau` now runs Stim's elimination, so its X outputs and the stabilizers it adds to an underconstrained input match Stim exactly; earlier builds returned valid but different destabilizers and completions for most inputs. Its Z outputs are unchanged. An anticommuting input reports the first anticommuting pair in input order, the later generator as `stabilizer` and the earlier as `conflict`, and every error prints the generator as given rather than padded to the widest one.
- `circuit_to_tableau` refuses a Tableau above the memory budget with `AnalysisError::ResourceLimit` for the `tableau-conversion` operation instead of an invalid-conversion message, and no longer caps repeat work.
- Unitary `circuit_time_reversed_for_flows` validates a wide circuit that the sparse tracker cannot fold against the dense Tableau instead of refusing it.
- Ignored-only and unitary `circuit_flow_generators` admit their flow rows in bytes instead of a 4096-qubit bound, and detecting regions no longer refuse an output region wider than 1,048,576 qubits by count.
- `unitary_to_tableau` accepts any power-of-two dimension, as Stim does; a matrix that is not Clifford still reports `UnitaryMatrixNotClifford`.

## Parser Limits

Circuit and DEM command boundaries accept Stim's ASCII whitespace, including vertical tabs and form feeds before commands and repeat terminators. Whitespace inside instruction arguments and targets keeps the stricter Stim grammar, and diagnostics retain exact source-byte spans.

`ParseLimits::new` now takes a `MemoryBudget` and a `RepeatNestingLimit`.
The budget bounds the source bytes plus the conservative retained size of the parsed model, including every declaration, target, argument, and tag, and defaults to one gibibyte; source lines, compact declarations, and retained targets are no longer capped.
`SourceByteLimit`, `SourceLineLimit`, `RepresentedInstructionLimit`, and `RepresentedTargetLimit` and the `ParseLimits` accessors and builders for them are removed; use `with_memory_budget` and `memory_budget`.
`ResourceKind::SourceLines`, `RepresentedInstructions`, and `RepresentedTargets` are replaced by `ResourceKind::RetainedBytes`.

`ResourceLimitContext` and the parser-only `stab_model::advanced` error and target-parser constructors were removed. A `ResourceLimitError` now reports its `dialect`, `operation`, `resource`, optional `source_line`, `actual`, `limit`, and source `span` directly.

## Execution APIs

Whole-output and callback compatibility adapters are removed. Reusable execution follows one ownership shape:

```text
compiler -> immutable plan -> mutable session -> optional sink-bound transaction -> typed batch sink
```

- Measurement sampling uses `SamplingCompiler`, `SamplingPlan`, `SamplingSession`, and `MeasurementSink`.
- Measurement-to-detection conversion uses `MeasurementToDetectionCompiler`, `MeasurementToDetectionPlan`, `MeasurementToDetectionSession`, and a short-lived `MeasurementToDetectionTransaction` that binds exactly one `DetectionSink` across incremental writes and finalization.
- DEM sampling uses `DemSamplingCompiler` and an immutable `DemSamplingPlan`. Call `DemSamplingPlan::session` for RNG-bearing sampling or `DemSamplingPlan::replay_session` for owned reusable replay state. Complete replay uses `run`; incremental replay uses a short-lived `DemReplayTransaction` that binds exactly one `DemSampleSink` across writes and finalization. Resetting a cancelled replay requires resetting its cancellation token first.
- Encoded in-memory output is available from codec sinks in `stab-records`; streaming callers can implement the sink traits directly or drain records into any `io::Write` with `stab_records::RecordEncoder`.

`stab_records::RecordEncoder` is the one result encoder behind the codec sinks and every CLI output. It accepts column groups (`RecordBatchPart`) of shot-major records or bit planes, or single records (`RecordPart`), writes all six formats, and reports a `RecordWriteError` that keeps layout errors (`FormatError`) apart from destination failures (`io::Error`). The packed and streaming helpers on `MeasureRecordWriter` are removed without aliases: `try_with_capacity`, `write_packed_record`, `write_packed_batch`, `write_bit_plane_batch`, `buffered_bytes`, and `clear_buffered_bytes`. Use `RecordEncoder` for packed data; `MeasureRecordWriter` keeps its per-bit Stim writer contract (`try_new`, `begin_dets_result_type`, `begin_result_type`, `write_bit`, `write_bits`, `write_bytes`, `write_end`, and `into_bytes`).

`stab_records::RecordStreamReader` now yields packed records. `read_batch(max_records)` decodes up to `max_records` records into a reusable `PackedShotBatch` and returns a borrowed `PackedShotBatchView`, delivering every record decoded before a failure ahead of that failure; `skip_records` and `has_more_records` consume records without returning them. `with_framing(RecordFraming::Records)`, `with_hits_duplicates(HitsDuplicates::Set)`, `with_empty_zero_width_input`, and `with_blank_dets_line_budget` select per-record framing and diagnostics, and `RecordStreamReadError` gains the matching structured variants. The per-record `next_record` and `next_b8_packed_record` methods and the `RecordStreamReader::ptb64` constructor are removed without aliases; use `RecordStreamReader::measurements` with `RecordFormat::Ptb64`. `DemReplayTransaction::write_packed_batch` accepts packed replay records alongside `write_batch`. `PackedShotBatch` adds `copy_shot_from_bits` and `copy_columns_from`, `PackedShotBatchView` adds `prefix`, and `stab_bits::BitMatrix` adds `row_words_mut`.

The record visitors and record types without a product caller are removed without aliases: `for_each_record`, `for_each_packed_record`, `for_each_sparse_record`, `for_each_ptb64_record`, `for_each_ptb64_record_all`, `for_each_dets_record`, `for_each_dets_packed_record`, `for_each_dets_token_record`, `for_each_dets_sparse_shot`, their `try_for_each_*` counterparts, `RecordStreamError`, `MeasureRecord`, `MeasureRecordBatch`, `MeasureRecordBatchWriter`, `SparseShot`, and the public `DetsToken`. Use `RecordStreamReader` for streaming decode or `read_records`, `read_measurement_records`, and `read_dets_records` for materialized decode.

The removed `CompiledSampler`, `CompiledDetectionConverter`, and `CompiledDemSampler` types have no facade compatibility replacements.

## Wider Frame Batches

E7 runs detection sampling on the same 256-lane Pauli-frame batches as circuit sampling and hands every sampled batch to its sink as bit planes without re-chunking. These changes are visible to library and CLI callers:

| Changed item | Migration |
| --- | --- |
| `BitPlane64Batch`, `BitPlane64BatchView` | Renamed without aliases to `BitPlaneBatch` and `BitPlaneBatchView`, which hold any number of shots, 64 per plane word. `BitPlaneBatchView::try_from_words(words, words_per_plane, shot_count, bits_per_shot)` takes the plane stride; `try_from_blocks` borrows one 256-lane `BitBlock` per plane, `shot_window` selects a word-aligned window, and `copy_word_column_into` replaces `copy_words_into`. `BitPlane64Batch::copy_plane_from_word` is removed; use `set` or build planes from `from_shot_major`. |
| `write_bit_plane_64_batch` | Removed without an alias; encode bit planes through `RecordEncoder::write_batch` or `MeasurementCodecSink`. |
| `RecordEncoder::write_bit_planes` | Removed; pass a `RecordBatchPart` built from a `BitPlaneBatchView` to `write_batch`. `RecordBatchPart::new` accepts either layout, and parts of one record may mix layouts. |
| `DetectionBatchView::detectors`, `observables`, `DemSampleBatchView::sampled_errors`, `DecoderInputBatchView::detectors` | Return the new `stab_records::RecordBatchView`, which holds shot-major records or bit planes and reads either through `get`, `shot_count`, and `bits_per_shot`; `shot_major_records` and `bit_planes` expose the layout. `DetectionBatchView::try_new` and `DecoderInputBatchView::from_detectors` accept either layout, and `DemSampleBatchView::try_new` takes `Option<RecordBatchView>`, so pass `Some(records.into())`. `MeasurementBatchView` adds `from_records` and `records`. |
| Batch sizes | Sampling, detection, and generated DEM sessions deliver batches of up to `stab_engine::MAX_BATCH_SHOTS` (256) shots instead of 64, and `MeasurementToDetectionSession` accepts batches of up to 256 records. Size per-batch sink storage, such as decoder predictions, from that constant. DEM replay batches still hold at most 64 records. |
| Session memory | A detection session now charges 32 bytes per frame plane instead of 8 and no longer charges two packed delivery batches, so a circuit with millions of qubits is refused at a lower qubit index than before under the same `--memory-budget`. |

New supporting items are `stab_engine::MAX_BATCH_SHOTS`, `PackedShotBatchView::copy_rows_into_bit_plane_words`, `BitBlock::as_words`, and `BitMatrix::copy_transpose_from_word_rows_at` and `copy_transpose_rows_into_word_rows` for 64-row blocks at any row offset.

Seeded `detect` output changes for every noisy circuit, because a 256-lane frame draws the random stream in a different order; noiseless `detect` output and its distribution are unchanged, and the `detect` compiler schema reported by `capabilities` moves from `3` to `4` to mark the new stream. Seeded `sample` and `sample_dem` output and all `m2d`, `convert`, and `analyze_errors` output are byte-identical to the previous build.

## Extension APIs

The typed circuit-pass seam is canonical in `stab-analysis`. It admits folded input, requires a conservative output-resource projection before proportional allocation, validates the returned closed-dialect circuit, and reports typed input, projection, or output failures. The separate Stable `stab-reference-noise-pass` crate proves this public boundary without depending on `stab-core`.

Decoder interoperability is canonical in `stab-decoder`. The separate `stab-reference-decoder` crate proves the reusable session boundary without depending on the facade.

Neither seam implies dynamic plugins, runtime gate registration, GPU placeholders, or a universal compiler registry.

## SIMD Kernels

SIMD leaf kernels are always on, and there is no Cargo feature to select them. `stab-bits` and `stab-algebra` depend on `stab-kernels-simd` unconditionally, so every consumer, direct or through `stab-core`, resolves one `stab-kernels-simd` package plus `fearless_simd` and `fearless_simd_macros`.

Unreleased 0.2 development revisions had an opt-in `portable-simd` feature on `stab-bits`, `stab-algebra`, `stab-engine`, `stab-core`, and `stab-cli`; it is removed without an alias, so delete `features = ["portable-simd"]` from any dependency declaration that copied it. Results are unchanged, because the kernels compute the same words as the scalar formulas they replace. The kernels select the instruction set once per call: statically on AArch64 and with a runtime check on x86-64. See [ADR 0008](architecture/adr-0008-unconditional-fearless-simd-kernels.md).

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
