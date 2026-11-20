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

`DemSamplerLimits` per-shot and total work limits were removed with the other count and work caps; see Memory Budget below.
Sessions stream any number of shots whose reusable storage fits the budget.

`Tableau::random` now samples uniformly over the Clifford group, signs included, using the Bravyi–Maslov construction ported from Stim. Earlier builds applied a bounded random gate walk whose distribution was not uniform, so seeded callers receive different tableaux for the same seed; the signature is unchanged. The sampler now runs on packed rows at any width the memory budget admits, and it consumes the random stream exactly as the earlier 64-qubit sampler did, so seeded tableaux of at most 64 qubits are unchanged.

Circuit sampling compiles one resolved program shared with detection, and no public item was removed or renamed. Four observable behaviors changed. `SamplingCompiler::compile` then counted its fixed one-million boundary, since removed with the other work caps, in expanded circuit instructions, as `DetectionSamplingCompiler` does, so `SamplingCompileError::ExpandedOperationLimit` reports instruction counts and wide circuits that detection already accepted now compile; the sampling compiler schema is `6` and the executable-contract schema is `6`, so every sampling request and plan fingerprint changed. A circuit whose only work is one Z measurement, optionally after one single-qubit Pauli channel on the measured qubit, no longer uses a dedicated direct-Z plan, so its seeded output stream differs while its distribution does not. Without that plan, a single measurement of a very large qubit index is refused by the same frame and reference storage limits as any other circuit, including the `MeasurementToDetectionCompiler` reference sample. `count_determined_measurements` with `unknown_input` now matches pinned Stim on circuits that reset every qubit before measuring them, where it previously undercounted. The measurement-to-detection compiler schema reported by `capabilities` is `4`, because its reference sample and sweep correction now share one program admitted within the conversion's configured compiled-byte limit instead of a separate sampling plan.

## Memory Budget

Count and work limits are replaced by one typed byte budget per operation.
`stab_model::MemoryBudget`, also exported from the `stab-core` root with `MemoryBudgetParseError`, parses an integer byte count with an optional `KiB`, `MiB`, or `GiB` suffix through `FromStr` and defaults to one gibibyte.
Every admitted workload produces byte-identical output; only workloads that the removed caps refused are newly admitted.

| Removed item | Replacement |
| --- | --- |
| `DetectionConversionLimits::{max_record_bits, max_expanded_instructions, max_repeat_iterations, max_compiled_terms, max_compiled_bytes}` and their `with_*` builders | `DetectionConversionLimits::{new, memory_budget, with_memory_budget}`; the budget bounds the compiled plan, one byte per record bit, the reference scratch, and session storage |
| `DetectionResourceKind::{RecordBits, SamplingExpandedOperations, ExpandedInstructions, RepeatIterations, CompiledTerms}` and `DetectionRecordLimitSubject` | `DetectionResourceKind::CompiledBytes` and `RepeatNesting` |
| `SamplingCompileError::ExpandedOperationLimit` | `SamplingCompileError::CompiledBytesLimit { required, budget }`; configure the budget with `SamplingCompiler::memory_budget` |
| The general stabilizer-frame sampling fallback for programs above 256 MiB | A typed `CompiledBytesLimit` refusal against the budget |
| `DemSamplerLimits::{max_sampled_error_applications_per_shot, max_replay_work_units_per_shot, max_active_batch_bytes}` and their builders; `DemResourceKind::{SampledErrorApplications, ReplayWorkUnits}`; `DemSamplingExecutionError::SessionStorageLimit` | `DemSamplingCompiler::{limits, memory_budget}` and `DemSamplerLimits::{new, memory_budget, with_memory_budget}`; compilation refusals report `DemResourceKind::CompiledBytes` and session storage refusals report `DemResourceKind::ActiveBatchBytes` |
| `DemSamplingPlan::{validate_replay_with_limits, validate_replay_work_units_with_limits, validate_detector_sample_work_units_with_limits, validate_sampled_error_work_units_with_limits}` | `DemSamplingPlan::validate_replay`, which only checks representability |
| `CircuitFlattenLimits` count fields, `DEFAULT_MAX_*` constants, and builders | `CircuitFlattenLimits::{new, memory_budget, with_memory_budget}` |
| `DemFlattenLimits` repeat-unroll, expanded-instruction, repeat-iteration, target, argument, and byte fields, `DEFAULT_MAX_*` constants, and builders | `DemFlattenLimits::{new, memory_budget, with_memory_budget}` |
| `CircuitPassLimits` item, target, argument, and payload fields, `DEFAULT_MAX_*` constants, and builders | `CircuitPassLimits::new(memory_budget, repeat_nesting)` with `memory_budget` and `with_memory_budget` |
| Fixed analyzer, feedback-inlining, Pauli-conjugation, time-reversal, flow-generator, missing-detector, detecting-region, and coordinate-query work caps | Removed; retained storage is admitted against a caller budget: `ErrorAnalyzerOptions::memory_budget`, `circuit_with_inlined_feedback_with_limits`, `DetectorErrorModel::detector_coordinates_with_memory_budget`, the generator parameters' `with_memory_budget`, `TimeReversedForFlowsOptions::memory_budget`, `InverseQecOptions::memory_budget`, `FlowGeneratorOptions::memory_budget`, `MissingDetectorOptions::memory_budget`, `DetectingRegionOptions::memory_budget`, `DetectingRegionTargetOptions::memory_budget`, `all_detecting_region_targets_with_budget`, and `all_detecting_region_ticks_with_budget`; the functions without one use `MemoryBudget::DEFAULT` |
| The generator's 131,072-physical-qubit cap | A projected one mebibyte plus 768 bytes per physical qubit admitted against the generator parameters' budget |
| Flow checking, Pauli conjugation, and `Flow` text parsing at the default budget only | `check_if_circuit_has_unsigned_stabilizer_flows_with_options`, `check_unsigned_stabilizer_flows_with_diagnostics_with_options`, `circuit_has_unsigned_stabilizer_flow_with_options`, and `circuit_has_all_unsigned_stabilizer_flows_with_options` with `UnsignedFlowCheckOptions`; `pauli_after_circuit_with_options` and `pauli_before_circuit_with_options` with `PauliConjugationOptions`; and `Flow::from_str_with_budget`; the existing functions delegate with the default budget |
| `sample_if_circuit_has_stabilizer_flows` at the default budget only | `sample_if_circuit_has_stabilizer_flows_with_options` with `SampledFlowOptions`, whose `memory_budget` admits each flow's augmented noiseless witness circuit, compiled sampling plan, witness simulator, and session; the existing function delegates with the default budget |

`ErrorAnalyzerOptions`, `TimeReversedForFlowsOptions`, `InverseQecOptions`, `FlowGeneratorOptions`, `MissingDetectorOptions`, `DetectingRegionOptions`, and `DetectingRegionTargetOptions` gained the public `memory_budget` field, so struct literals must add it, for example `memory_budget: MemoryBudget::DEFAULT`, or use `..Default::default()`.
One caller budget governs each of these operations end to end: flow generation passes it to the Tableau of a unitary circuit, missing-detector discovery to the flow generators it runs, time reversal to the dense Tableau that validates unitary input flows, and detecting regions to the Pauli strings of each output region.
Measurement-rich time reversal and the QEC inverse now refuse an expansion above the budget with `AnalysisError::ResourceLimit` for the new `stab_analysis::AnalysisOperation::TimeReversal` operation (`time-reversal`, resource `materialized-bytes`) instead of an invalid-conversion message.
The flow-check and Pauli-conjugation `*_with_options` functions return `AnalysisResult`: a Tableau the budget refuses falls back to the sparse tracker or the expanded instruction walk, and live sparse-tracker sensitivity state or an extracted or conjugated Pauli string the budget refuses is reported as `ResourceLimit` for the new `AnalysisOperation::FlowCheck` (`flow-check`) or for `AnalysisOperation::PauliConjugation`, so exhaustive matches on `AnalysisOperation` must add `FlowCheck`.
The unbudgeted `check_if_circuit_has_unsigned_stabilizer_flows` family still returns plain values and fails such a flow closed.
`Flow` text whose sparse term requests more than the budget, such as `X1000000000000 -> Z0`, now fails with `StabilizerError::MemoryBudgetExceeded` instead of `StabilizerError::InvalidFlowText`, and a detecting region the budget refuses reports `ResourceLimit` for `DetectingRegions` instead of an invalid-model message.
The CLI's fixed 64 MiB input caps and 1 MiB or 64 MiB per-record caps are replaced by `--memory-budget`, which bounds whole inputs at the budget and streamed records at half of it.
The CLI applies `--memory-budget` to a whole command instead of to each stage: plan compilation receives the budget less the active model's bytes, sessions and streaming runs receive the budget less the compiled plan's `retained_bytes()` (via `session_with_budget`, `streaming_run_with_budget`, and `DemSamplerLimits`), and input text, parsed models, and pre-inlining circuits are dropped as soon as no later stage reads them.
A command whose parse and compile stages, or whose compiled plan and session, each fit the budget alone but together exceed it is now refused before sampling; for example, a 147 MB flattened distance-7 surface-code circuit with 30,000 rounds is refused at compilation under the 1 GiB default and runs under `--memory-budget 2GiB`, where its peak resident memory falls from 0.90 to 0.76 GB for `sample` and from 1.11 to 0.86 GB for `detect`.
Such a refusal keeps its error class and exit status, and its message and JSON context name the bytes earlier stages hold; `Circuit::{from_stim_bytes_admitted, admitted_bytes}`, `DetectorErrorModel::{from_dem_bytes_admitted, admitted_bytes}`, and each compiled plan's `retained_bytes` report the conservative retained bytes held across stages.

## Cooperative Cancellation

Long-running operations share one cancellation signal, `stab_model::Cancellation`, with its error `stab_model::Cancelled`, both also exported from the `stab-core` root.
`Cancellation::new()` creates a settable signal and `Cancellation::never()`, also the `Default`, one that cannot be set, so code that never cancels keeps its behavior.
See the resource policy inventory's Cooperative Cancellation section for where each operation polls the signal and what it returns.

| Changed item | Migration |
| --- | --- |
| `SamplingCancellation`, `DemSamplingCancellation` | Removed; every session's `cancellation()` now returns `stab_model::Cancellation`, which keeps `cancel`, `reset`, and `is_cancelled` |
| `stab_decoder::DecodeCancellation`, also exported from the `stab-core` root | Removed; `decode_batch` and `DecoderSession::decode_validated_batch` take `&stab_model::Cancellation`, so replace `DecodeCancellation::new()` with `Cancellation::new()`; unlike the removed token the signal can be `reset` |
| `LogicalErrorSearchLimits`, `SatMaterializationLimits` | No longer `Copy`; limit accessors take `&self`, and `cancellation`/`with_cancellation` builders were added; clone a value that is used twice |
| `CircuitPassContext` | No longer `Copy`, and `limits` takes `&self`; `with_cancellation` and `cancellation` were added, and `CircuitPassError` gained the `Cancelled(Cancelled)` variant, which `run_circuit_pass` returns for any failure a pass reports while the context's signal is set |
| `SamplingCompiler`, `MeasurementToDetectionCompiler`, `DetectionSamplingCompiler`, `DemSamplingCompiler`, `DetectionConversionLimits`, `DemSamplerLimits`, `CircuitFlattenLimits`, `DemFlattenLimits` | No longer `Copy`; their `compile` methods take `&self`, limit accessors take `&self`, and `cancellation`/`with_cancellation` builders were added (`DemSamplingCompiler` also gained `limits` and `memory_budget`, and is no longer a unit struct); clone a value that is used twice |
| `ErrorAnalyzerOptions`, `MissingDetectorOptions`, `DetectingRegionOptions`, `DetectingRegionTargetOptions`, `TimeReversedForFlowsOptions`, `InverseQecOptions` | Gained a public `cancellation` field and are no longer `Copy`; struct literals add `cancellation: Cancellation::never()` or use `..Default::default()`, and `MissingDetectorOptions` now implements `Default` |
| `circuit_with_inlined_feedback_with_memory_budget(circuit, budget)` | `circuit_with_inlined_feedback_with_limits(circuit, FeedbackInliningLimits::new(budget))` |
| None | `circuit_flow_generators_with_options` with `FlowGeneratorOptions`, `DemFlattenedInstructionIter::with_cancellation`, the defaulted `FoldedDemVisitor::poll_cancellation` hook, and `stab_model::advanced::CancellationCountdown` |
| None | `UnsignedFlowCheckOptions::cancellation` for the flow-check `*_with_options` functions and `PauliConjugationOptions::cancellation` for `pauli_after_circuit_with_options` and `pauli_before_circuit_with_options`; a cancelled flow check returns `Cancelled` rather than failing its flows closed |
| None | `SampledFlowOptions::cancellation` for `sample_if_circuit_has_stabilizer_flows_with_options`, which returns the new `SampledFlowError::Cancelled` variant rather than a verdict from partial samples, so exhaustive matches on `SampledFlowError` must add it |
| Exhaustive matches on `ModelError`, `AnalysisError`, `SamplingCompileError`, `SamplingExecutionError`, `DetectionError`, `DetectionExecutionError`, and `DemError` | Add the new `Cancelled(Cancelled)` variant; `SamplingCompileErrorCode::Cancelled` reports `cancelled`, and `AnalysisError` converts a model `Cancelled` into its own variant |

Every session reports cooperative cancellation as the resumable `Cancelled` status with the sink finalized, wherever the cancellation is observed.
A circuit-sampling or detection-sampling session that observes it inside a batch discards that batch and rewinds its random stream, so after `Cancellation::reset` a run for the remaining shots produces exactly the seeded output of an uninterrupted run; such a session is no longer poisoned and its `run` no longer returns `SamplingExecutionError::Cancelled` or `DetectionExecutionError::Cancelled`, which remain for reference-sample computation and engine steps outside a run.
`SamplingPlan::streaming_run` and `DetectionSamplingPlan::streaming_run` also return the `Cancelled` status for an in-batch cancellation, but they consume themselves and cannot be resumed.
Only a sink failure or an internal invariant violation still poisons a sampling or detection-sampling session.

## Algebra Byte Admission

Every count and work cap in `stab-algebra` is removed by replacing the dense algorithms with word-parallel ones over packed rows, and admits the storage those caps bounded in bytes.
`stab_algebra::AlgebraMemoryBudget`, also exported from the `stab-core` root, is the algebra's own typed byte bound with the same one-gibibyte default as `MemoryBudget`, because `stab-algebra` does not depend on `stab-model`.
Constructors that turn a requested size into storage admit its bytes before allocating or drawing randomness; operations on values the caller already holds reserve a bounded multiple of them fallibly.

| Removed or changed item | Replacement |
| --- | --- |
| `StabilizerResource::{PauliQubits, CliffordQubits, TableauQubits, RandomTableauQubits, StabilizerSolveQubits, UnitaryMatrixDimension, FlowClassicalTerms, CircuitTableauRepeatWork}` and the `limit` and `ensure` methods | `StabilizerResource::{PauliString, CliffordString, Tableau, RandomTableau, StabilizerSolve, FlowTerms}`, which name the refused or failed storage and carry no limit |
| `StabilizerError::ResourceLimitExceeded { resource, requested, limit }` | `StabilizerError::MemoryBudgetExceeded { resource, requested_bytes, budget_bytes }` for a refused request and `StabilizerError::AllocationFailed { resource, bytes }` for a failed fallible reservation |
| `StabilizerError::OverconstrainedStabilizers` | Removed; as in Stim, a generator beyond the independent ones is reported as redundant or contradictory |
| `Tableau::x_output` and `Tableau::z_output` returning `&PauliString` | Both return an owned `PauliString`, because generator images are packed rows rather than stored strings |
| `stab_algebra::advanced::{tableau_identity_unchecked, tableau_from_output_columns_unchecked}` | `Tableau::identity_with_budget` and `Tableau::from_conjugated_generators`; a Tableau with mismatched generator shapes can no longer be constructed |
| None | `PauliString::{identity_with_budget, random_with_budget, from_bases_with_budget, from_str_with_budget}`, `FlexPauliString::{identity_with_budget, from_phase_and_bases_with_budget, from_str_with_budget}`, `CliffordString::{identity_with_budget, random_with_budget, from_gates_with_budget, concat_with_budget, repeat_with_budget}`, `PauliStringIterator::new_with_budget`, `Tableau::{from_pauli_string_with_budget, embedded_with_budget}`; each takes an `AlgebraMemoryBudget` and the unbudgeted constructor, method, or `FromStr` impl delegates to it with `AlgebraMemoryBudget::DEFAULT` |
| None | `Tableau::identity_with_budget`, `Tableau::random_with_budget`, `Tableau::from_stabilizers_with_budget`, `Tableau::is_identity`, `stab_analysis::circuit_to_tableau_with_options` with `CircuitToTableauOptions`, and `stab_analysis::AnalysisOperation::TableauConversion` |
| None | `Tableau::prepend(&mut self, gate: &Tableau, targets: &[usize])`, which rewrites only the target generator rows, and `stab_algebra::TableauAppender` with `identity_with_budget`, `new(Tableau)`, `append`, `x_output_basis`, `z_output_basis`, `x_output_sign`, `z_output_sign`, `len`, `is_empty`, and `finish`, which holds a Tableau transposed so each appended one- or two-qubit gate costs `O(n / 64)` word operations; `stab_bits::transpose_bit_tile` exposes the 64 by 64 block transpose it uses |

Behavior changes for workloads the removed caps admitted:

- `Tableau::from_stabilizers` now runs Stim's elimination, so its X outputs and the stabilizers it adds to an underconstrained input match Stim exactly; earlier builds returned valid but different destabilizers and completions for most inputs. Its Z outputs are unchanged. An anticommuting input reports the first anticommuting pair in input order, the later generator as `stabilizer` and the earlier as `conflict`, and both fields are indices into the input.
- `circuit_to_tableau` refuses a Tableau above the memory budget with `AnalysisError::ResourceLimit` for the `tableau-conversion` operation instead of an invalid-conversion message, and no longer caps repeat work.
- Unitary `circuit_time_reversed_for_flows` validates a wide circuit that the sparse tracker cannot fold against the dense Tableau instead of refusing it.
- Flow checking and the input-flow validation of unitary time reversal choose between the dense Tableau and the sparse reverse tracker by estimated cost instead of a 512-qubit threshold. `check_unsigned_stabilizer_flows_with_diagnostics` reports `OutputMismatch`, with the circuit's image of the input as wide as the widest of the circuit, the input, and the output, for every failing Pauli-only flow of a circuit made of unitaries, annotations, and noise; wide circuits, repeat blocks, and flows narrower than the circuit previously reported `InputMismatch` there. Other Pauli mismatches still report `InputMismatch`.
- Ignored-only and unitary `circuit_flow_generators` admit their flow rows in bytes instead of a 4096-qubit bound, and detecting regions no longer refuse an output region wider than 1,048,576 qubits by count.
- `Tableau::from_unitary` accepts any power-of-two dimension, as Stim does; a matrix that is not Clifford still reports `UnitaryMatrixNotClifford`.

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

`stab_records::RecordStreamReader` now yields packed records. `read_batch(max_records)` decodes up to `max_records` records into a reusable `PackedShotBatch` and returns a borrowed `PackedShotBatchView`, delivering every record decoded before a failure ahead of that failure; `read_ptb64_group()` decodes one 64-record `RecordFormat::Ptb64` group directly into a borrowed `BitPlaneBatchView` without transposing rows; `skip_records` and `has_more_records` consume records without returning them. `with_framing(RecordFraming::Records)`, `with_hits_duplicates(HitsDuplicates::Set)`, `with_empty_zero_width_input`, and `with_blank_dets_line_budget` select per-record framing and diagnostics, and `RecordStreamReadError` gains the matching structured variants. The per-record `next_record` and `next_b8_packed_record` methods and the `RecordStreamReader::ptb64` constructor are removed without aliases; use `RecordStreamReader::measurements` with `RecordFormat::Ptb64`. `DemReplayTransaction::write_packed_batch` and `DemReplayTransaction::write_bit_plane_batch` accept packed shot-major and bit-plane replay records alongside `write_batch`. `PackedShotBatch` adds `copy_shot_from_bits` and `copy_columns_from`, `PackedShotBatchView` adds `prefix`, and `stab_bits::BitMatrix` adds `row_words_mut`.

A replay caller that validates its whole input before writing output can keep the decoded records instead of decoding them again: `DemSamplingPlan::replay_record_store(&limits, shots)` creates a `DemReplayRecordStore` budgeted at what the replay session leaves of the same limits, `DemReplayRecordStore::try_retain` and `DemReplayRecordStore::try_retain_bit_planes` retain dense records as error-major bit planes and sparse records as set-error ordinals until a batch would exceed that budget, and `DemReplayTransaction::write_retained` replays every record a store holds, which produces the records that `write_packed_batch` or `write_bit_plane_batch` would. A batch that does not fit returns `DemReplayRetention::Full`, keeps none of its records, and leaves the store holding the whole batches retained before it, so the caller replays that prefix and then delivers the declined records itself, for example from a `RecordStreamReader` positioned at the `consumed_offset` recorded before the declined batch and created with `starting_at_offset`; `DemReplayRetention::Released` now means the store holds nothing, and `write_retained` rejects only a released store (`DemReplayRecordStore::is_released`).

The record visitors and record types without a product caller are removed without aliases: `for_each_record`, `for_each_packed_record`, `for_each_sparse_record`, `for_each_ptb64_record`, `for_each_ptb64_record_all`, `for_each_dets_record`, `for_each_dets_packed_record`, `for_each_dets_token_record`, `for_each_dets_sparse_shot`, their `try_for_each_*` counterparts, `RecordStreamError`, `MeasureRecord`, `MeasureRecordBatch`, `MeasureRecordBatchWriter`, `SparseShot`, and the public `DetsToken`. Use `RecordStreamReader` for streaming decode or `read_records`, `read_measurement_records`, and `read_dets_records` for materialized decode.

The removed `CompiledSampler`, `CompiledDetectionConverter`, and `CompiledDemSampler` types have no facade compatibility replacements.

## Wider Frame Batches

Detection sampling now runs on the same 256-lane Pauli-frame batches as circuit sampling and hands every sampled batch to its sink as bit planes without re-chunking. These changes are visible to library and CLI callers:

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

## Session Lane Widths

Circuit-sampling and detection-sampling sessions take an explicit Pauli-frame lane width, so a request for a few shots on a wide circuit no longer samples and stores a full 256-lane batch. Existing calls keep the 256-lane default and their seeded output.

| Changed item | Migration |
| --- | --- |
| `stab_engine::LaneWidth` | New closed, non-exhaustive width type with `Lanes64` and `Lanes256` (the default). `shots_per_batch` reports its batch size, `ALL` and `WIDEST` name the widths, and `for_total_shots(ShotCount)` picks the width for a known total: below `WIDEST_FROM_TOTAL_SHOTS` (512) the width that samples the fewest lanes for the whole request, preferring the wider on a tie, and 256 lanes from 512 shots on. With the shipped widths that is 64 lanes for totals of 1 to 192 and 257 to 448 shots. |
| `SamplingPlan::session_with_lane_width`, `DetectionSamplingPlan::session_with_lane_width` | New constructors that fix the session's lane width. `session` and `session_with_reference_mode` are unchanged and use `LaneWidth::default()`. `SamplingSession::lane_width` and `DetectionSamplingSession::lane_width` report it. |
| Seeded streams | A session's seeded output is a function of its plan, seed, and lane width. Partitioning a seeded run across calls on one session reproduces the combined call at either width, but two sessions with the same seed and different widths draw different streams with the same distribution. |
| Batch sizes | A 64-lane session delivers batches of at most 64 shots. `MAX_BATCH_SHOTS` remains the bound across every width. |
| Session memory | A 64-lane session admits and allocates 8 bytes per frame plane instead of 32, so a budget too small for a 256-lane session may admit a 64-lane one. |
| Executable-contract schema | Moves from `6` to `7` because the seeded stream now also depends on the session lane width, so every sampling plan fingerprint changes. The sampling request fingerprint and compiler schema are unchanged. |
| `detect` compiler schema | Reported by `capabilities` as `5` instead of `4`, to mark the CLI's width-dependent `detect` stream. |

The CLI's `sample` and `detect` create their session with `LaneWidth::for_total_shots` of `--shots`. Seeded `sample` and `detect` output therefore changes for totals of 1 to 192 and 257 to 448 shots on circuits with random results, and is byte-identical to the previous build for every other total and for every circuit whose results are deterministic. `m2d`, `sample_dem`, `convert`, and `analyze_errors` output is unchanged.

## Streaming Runs

Circuit sampling and detection sampling can run one request of a known shot count without materializing a plane per measurement for every lane, so wide records no longer cost the record width times the lane width. Existing sessions and their seeded output are unchanged.

| Changed item | Migration |
| --- | --- |
| `SamplingPlan::streaming_run(random_policy, reference_mode, lane_width, shots)` and `stab_engine::StreamingSamplingRun` | New. The run hands frame planes over in bounded windows and keeps shot-major records of only the `min(shots, lane_width)` lanes a batch delivers. `run(self, sink)` consumes it and returns the same summary and errors as `SamplingSession::run`; its output equals the first `run` of `shots` shots on `session_with_lane_width` with the same arguments. It cannot be resumed after a cooperative cancellation, because it keeps no undelivered lanes. |
| `DetectionSamplingPlan::streaming_run(random_policy, lane_width, shots)` and `stab_engine::StreamingDetectionRun` | New, with the same contract against `DetectionSamplingSession::run`. Detectors and observables arrive as shot-major records. |
| Batch layout | Streaming runs deliver shot-major `MeasurementBatchView` and `DetectionBatchView` batches; materializing sessions still deliver bit planes. Sinks must accept either layout, as `RecordBatchView` already allows. |
| Session and reference admission | A streaming run admits its frame, one window, its records, and transpose scratch, as listed in the [resource policy inventory](architecture/resource-policy-inventory.md). The reference sample and the reference simulator's output record are charged and stored at one bit per measurement instead of one byte, so `SessionStorageLimit` estimates for sessions and reference runs report smaller byte counts, and a budget that refused a reference run by less than the difference now admits it. |
| `stab_records::PackedShotBatch::shot_words_mut` | Now public, returning the shot's storage words with bits past the width cleared on drop. |
| `RecordEncoder` | Encodes a record part wider than 65,536 bits in segments and drains between them; its output is unchanged. |

The CLI's `sample` and `detect` use a streaming run for 1 to 63 shots and for larger non-PTB64 requests whose output planes fill at least 8 MiB per batch, and their output is byte-identical to the previous build. On a 5,000,000-round distance-3 repetition code (10,000,000 measurements) `sample --shots 1` peaks at about 7 MB instead of 249 MB, and `detect --shots 1` at about 7 MB instead of 317 MB. Budget refusals of `sample` and `detect` report the new estimates. A `MeasurementToDetectionSession` whose plan has no sweep correction converts each batch through bounded windows and delivers shot-major detector and observable records instead of bit planes, so a sink that asks `RecordBatchView::bit_planes` for them gets `None` and reads them through `shot_major_records` or `get`; its `retained_bytes` and budget refusals report the windowed storage, and `m2d` of one record of the repetition code above peaks at about 30 MB instead of 1.2 GB.

## On-Demand Plan Identity

Compiling a sampling plan no longer hashes its circuit. A plan does not retain its source, and hashing every instruction and target cost about half of compilation on large circuits (166 ms of 313 ms on a 147 MB flat circuit) while only `stab plan sample` and callers that ask for an identity read it, so the identity is computed from the compiler and the circuit when it is asked for. Every fingerprint byte is unchanged.

| Changed item | Migration |
| --- | --- |
| `SamplingPlan::fingerprint` and `SamplingPlan::request_fingerprint` | Removed. Call `SamplingCompiler::plan_fingerprint(&circuit)` on the compiler that compiled the plan; it returns the same `PlanFingerprint`, whose `request_fingerprint()` and its `model_fingerprint()` return the request and model identities, before or after compiling. |
| `SamplingCompiler::request_fingerprint(&circuit)` | New. It returns `CompilationRequestFingerprint::for_sampling(&circuit)`, the backend-neutral identity of that compilation request. |
| `Debug` for `SamplingPlan` | Prints the reference-sample loop policy instead of the fingerprint. |

`stab sample` no longer pays for an identity it never prints, and `stab plan sample` hashes the circuit once instead of twice; its output is byte-identical to the previous build.

## Packed Targets And Exact Circuit Admission

A parsed circuit stores each target in four bytes, as Stim's `GateTarget` does, and each item in 56 bytes instead of 96 on 64-bit targets, and parse admission charges what the circuit retains instead of two slots per target, so a parsed circuit's hold tracks its resident size. On a 147 MB flat surface-code circuit the parsed model's hold falls from 632 MB to 200 MB and its resident size from about 326 MB to about 188 MB.

| Changed item | Migration |
| --- | --- |
| `stab_model::Target` | Now an opaque four-byte `Copy` value instead of an enum with public variants, and it also implements `Hash`. The constructors and accessors are unchanged; match on `target.kind()`, which returns the new `stab_model::TargetKind` (also `stab_core::TargetKind`) with the old variants and fields, so `match target { Target::Qubit { id, inverted } => .. }` becomes `match target.kind() { TargetKind::Qubit { id, inverted } => .. }` with `id` and `inverted` bound by value. `Debug` prints the `TargetKind`. |
| `Target::sweep_bit(id)` | Returns `ModelResult<Target>` and refuses ids of `2^24` and above, which Stim cannot encode and the parser never produced. |
| Clones of targets | `target.clone()` and `.cloned()` become copies; `CircuitInstruction::targets` still returns `&[Target]`. |
| Circuit parse admission (`ParseLimits` memory budget) | A circuit item is charged its exact slot size, 56 bytes instead of 96 plus 32 bytes of heap bookkeeping; a target 4 bytes instead of 16 (two 8-byte slots); an argument 8 bytes instead of 16; a tag twice its bytes plus its 24-byte boxed record instead of twice its bytes. The spare capacity of a block's growing item list, of an instruction that later lines fuse into, and of the per-line scratch is charged as transient parse storage before it is reserved and returned when the block or fusion run ends, and a line that fuses into the previous instruction returns its item, argument, and tag charge after it is admitted, so `Circuit::from_stim_bytes_admitted` reports the same hold as `Circuit::admitted_bytes`. DEM parse admission is unchanged. Budget refusals quote smaller byte counts, and inputs that were refused by less than the difference now parse. |
| Byte projections derived from model sizes | Projections that multiply by the size of a `CircuitItem`, `CircuitInstruction`, or `Target`, or that use `advanced::circuit_declaration_admitted_bytes`, shrink with them: circuit-pass payload projections, flattening, feedback inlining, measurement-rich time reversal, and sampled-flow witness circuits admit less for the same circuit. |

Every `.stim` text, printed circuit, diagnostic, and fingerprint byte is unchanged.

## Repeat Nesting And Record Output Storage

Printing, counting, folded walking, and coordinate queries of a `Circuit`, and printing and counting of a `DetectorErrorModel`, no longer recurse per repeat level, so a model built through the API deeper than the parser's 256-level envelope can be printed and counted on a small stack.
The DEM parser and the folded DEM traversal (`FoldedDemTraversal::try_visit` and `try_visit_with_coordinates`) also keep explicit heap stacks, so `try_visit`, `try_visit_with_coordinates`, `DetectorErrorModel::try_visit_error_mechanisms`, `detector_coordinates`, `detector_coordinates_for`, and `coordinates_of_detector` accept a model built through the API at any nesting depth instead of refusing one deeper than 256 levels with `DEM traversal repeat nesting exceeds current limit 256` or `DEM error-mechanism repeat nesting exceeds current limit 256`.
The parser's `ParseLimits` repeat-nesting limit and its 256-level `RepeatNestingLimit::HARD_MAX` are unchanged, so every accepted and rejected `.dem` text parses, prints, and reports errors as before.
`FoldedDemTraversal::validate_repeat_depth` and `advanced::MAX_DEM_REPEAT_NESTING` remain for consumers whose own processing recurses per repeat level: DEM sampling-plan compilation, DEM flattening, and the logical-error and SAT searches still refuse a model nested deeper than 256 levels.
Circuit and DEM coordinate queries compute each repeat body's coordinate shift, and circuit detector-coordinate queries each body's detector count, once per query instead of once per enclosing level, so their time grows linearly with nesting depth instead of quadratically; results and errors are unchanged.
Parsing a circuit from bytes that are not valid UTF-8 charges the display text and record of each opaque tag, so such an input needs a larger `ParseLimits` memory budget than before and `Circuit::admitted_bytes` reports the larger hold.
Parsing a detector error model from such bytes charges the same display text and record for each opaque tag, so `DetectorErrorModel::from_dem_bytes`, `from_dem_bytes_with_limits`, and `from_dem_bytes_admitted` can refuse a budget that was accepted before, and `DetectorErrorModel::admitted_bytes` and the hold `from_dem_bytes_admitted` returns report the larger value.
The CLI's `sample`, `detect`, and `m2d` admit their record encoders' scratch and the buffers of named output files against `--memory-budget` before the session, and a request whose session fits but not beside that storage is refused with a new output-storage diagnostic.
`convert` charges the 64 KiB write buffer of each named `--out` and `--obs_out` file as well: record conversions add it to the record-conversion storage they already admit, and a circuit-to-circuit conversion admits it beside the parsed circuit and reports the output-storage diagnostic, so a budget within 64 KiB per named file of the old minimum is now refused before any output file is activated.
`stab_bits::BitMatrix::zeros_chunked` and `stab_records::PackedShotBatch::zeros_chunked` build the same storage as `zeros` in bounded chunks, calling a fallible callback before each chunk, and creating a streaming sampling or detection run now polls the plan's cancellation signal while zeroing its records and can return the existing `Cancelled` error.

## Feedback Inlining, Replay Input, And Budget Corrections

`stab_analysis::circuit_with_inlined_feedback` and `circuit_with_inlined_feedback_with_limits` refuse a circuit in which a reset or measurement randomizes a detector or observable, as Stim's `with_inlined_feedback` does; earlier builds returned a circuit that kept the undefined detector, and `m2d --ran_without_feedback` now exits 1 on such a circuit.
Circuit-to-DEM analysis charges the reverse tracker's growth against the bytes the analyzer already holds for its output and accumulated errors, so a circuit near the memory budget is refused when the growth is requested instead of at the end of the instruction, and some circuits that completed only because of that late check are now refused.
`analyze_errors --fold_loops` unrolls a repeat when the period search's two extra tracker copies do not fit the budget instead of refusing, so folding never needs a larger budget than not folding.
Circuit-to-DEM analysis no longer counts the circuit's ticks, so a circuit with more than `u64::MAX` ticks is analyzed, as Stim analyzes it, instead of being refused with a count overflow.
`sample_dem --replay_err_in` accepts a pipe or other non-seekable input whenever the memory budget retains every replay record, and otherwise fails before creating any output with the new `replay-input-not-rewindable` diagnostic, where earlier builds failed on every non-seekable input with a bare seek error.
Two file roles may name the same device or FIFO, such as `--out /dev/null --obs_out /dev/null`; the same-file refusal now applies only to regular files.
`m2d` admits the batches of its measurement and `--sweep` readers before opening any output and reads in smaller batches when the full 256-record batch does not fit, and `analyze_errors` admits the 64 KiB buffer of a named `--out` file, so a budget too small for one-record reader batches, or within 64 KiB of the old `analyze_errors` minimum, is now refused.
A `CliffordString` is charged its six packed planes and nothing else: `identity_with_budget`, `random_with_budget`, `from_gates_with_budget`, `concat_with_budget`, and `repeat_with_budget` no longer add one byte per gate, so they accept a budget smaller by the string's length, and concatenation and repetition build the result in that storage with fallible reservations only.
`stab_algebra::Flow::set_input_term` replaces one input term in place.
Creating a measurement-to-detection session with sweep correction polls the plan's cancellation signal while zeroing its planes and can return the existing `Cancelled` error.
A `sample_dem` replay session sizes its planes for one 64-record batch instead of 1,024 shots, so a replay needs less of the budget and retains more records under the same budget.

## Packed Reference Samples

A reference sample is the packed, shared `stab_engine::ReferenceSample`, one bit per measurement, instead of a `Vec<bool>`; the plan computes it once and every reader shares that storage, so the bytes admitted are the bytes allocated.

| Removed or changed item | Replacement |
| --- | --- |
| `SamplingPlan::try_reference_sample` returning `Vec<bool>` | Returns `ReferenceSample`, a handle to the plan's cached sample; `len`, `get`, `iter`, `as_bitslice`, and `words` read it, and `.iter().collect::<Vec<bool>>()` rebuilds a flag list where one is still wanted |
| `circuit_reference_sample` returning `Vec<bool>` | Returns `ReferenceSample` |
| None | `SweepRecord::try_for_plan(&SamplingPlan, BitVec)`, a packed sweep record whose width is checked against the plan, `SamplingPlan::sweep_width`, and `SamplingPlan::try_reference_sample_with_sweep(&SweepRecord)`, which returns the cached sample for an all-false record |
| `ReferenceSampleTree::try_new` taking `Vec<bool>` prefix bits, `prefix_bits` returning `&[bool]`, and `decompress` returning `Vec<bool>` | Each takes or returns `ReferenceSample`; build one with `ReferenceSample::from(BitVec)` |
| `ReferenceSampleTree::decompress_into` | `ReferenceSampleTree::decompress` |
| None | `stab_bits::BitIter`, returned by `BitSlice::iter` and `BitVec::iter`, and `BitSlice: Default` |

`MeasurementToDetectionPlan::retained_bytes` now includes the plan's packed reference sample and `MeasurementToDetectionSession::retained_bytes` no longer includes a byte per measurement, so `m2d` needs about seven eighths of a byte less per measurement; a sampling session that uses the reference sample admits 48 more bytes for the shared handle.

## Packed DEM Targets

`stab_model::DemTarget` is an eight-byte value instead of a sixteen-byte enum, and the offset of `shift_detectors` is no longer a target.
A target is a relative detector, a logical observable, or the `^` separator, and nothing else; its order, equality, and printed form are unchanged, so `.dem` text and `analyze_errors` output are byte for byte what they were.
An instruction's kind is fixed by what it holds: a `detector` holds one detector, a `logical_observable` one observable, an `error` its targets, and a `shift_detectors` its offset, so a model that pairs a kind with targets it cannot have can no longer be built.

| Removed or changed item | Replacement |
| --- | --- |
| Matching `DemTarget::{RelativeDetector, LogicalObservable, Separator}` | `DemTarget::kind()` returns the matchable `DemTargetKind`; `detector()`, `observable()`, `is_detector()`, `is_observable()`, and `is_separator()` answer the common questions directly |
| `DemTarget::Numeric` and `DemTarget::numeric` | Removed; `DemInstruction::detector_shift()` returns the offset of a `shift_detectors` instruction, whose `targets()` is empty |
| `DemInstruction::new(kind, args, targets, tag)` | The per-kind constructors, which cannot pair a kind with the wrong targets |
| `DemInstruction::detector(coords, DemTarget, tag)` | `DemInstruction::detector(coords, DemDetectorId, tag)` |
| `DemInstruction::logical_observable(DemTarget, tag)` returning `ModelResult` | `DemInstruction::logical_observable(DemObservableId, tag)`, which cannot fail |
| `advanced::dem_instruction_with_tag_bytes` and `advanced::dem_instruction_detector_shift` | `advanced::{dem_detector_instruction_with_tag_bytes, dem_logical_observable_instruction_with_tag_bytes, dem_shift_detectors_instruction_with_tag_bytes}` and `DemInstruction::detector_shift` |
| None | `DemTarget::{from_detector, from_observable, shifted, SEPARATOR}`, `From<DemDetectorId>`, `From<DemObservableId>`, and `From<DemTargetKind>` for `DemTarget`, and `Hash` on `DemTarget` |

Every byte charge derived from a target's or an item's size shrinks with it, so parsing a detector error model, circuit-to-DEM analysis, and the DEM searches accept smaller memory budgets than before, and nothing that fit before is refused.
A bare number in an `error` line is reported at the number itself instead of at the surrounding target region.
Flow checking labels each flow with an observable id the circuit does not use instead of a numeric target, which changes no result.

## Typed Batch Indices, Record Layouts, And Review Follow-Ups

These changes come from Phase D of the review fix plan and from its public API review.

| Removed or changed item | Replacement |
| --- | --- |
| `PackedShotBatch::get/set(usize, usize)`, `PackedShotBatchView::get`, `BitPlaneBatch::get/set(bit, shot)`, `BitPlaneBatchView::get`, `RecordBatchView::get`, `MeasurementBatchView::get` | Every batch accessor takes `(stab_records::ShotIndex, stab_records::BitIndex)` in that order, so swapped arguments no longer compile; the bit-plane types change their argument order from `(bit, shot)` to `(shot, bit)` |
| `RecordBatchView::{shot_major_records, bit_planes}` and `MeasurementBatchView::{shot_major_records, bit_planes}` returning `Option` | `layout()` returns `stab_records::RecordLayout`, a two-variant enum of `ShotMajor(PackedShotBatchView)` and `BitPlanes(BitPlaneBatchView)` |
| `ObservablePredictionBatch::records_mut()` | `ObservablePredictionBatch::view_mut()`, which cannot change the batch width |
| `CircuitReferenceSigns::detector_signs()` and `observable_signs()` returning `&[bool]` | Both return `stab_bits::BitSlice<'_>`; use `iter`, `get`, `len`, or `words` |
| `SamplingPlan::try_count_determined_measurements(bool)` and `count_determined_measurements(&Circuit, bool)` | Both take `stab_engine::InitialState`, either `Zero` or `Unknown` |
| `DemReplayRecordStore::{is_retaining, is_released}` | `DemReplayRecordStore::state()` returns `DemReplayStoreState`, one of `Retaining`, `Full`, or `Released` |
| `DemSamplingPlan::admit_replay_input` returning `u64` | Returns `MemoryBudget` |
| `SamplingCompileErrorCode` and `SamplingCompileError::code()` | Removed; the CLI diagnostic codes are the one code surface |
| `stab_engine::biased_randomize_bits` | Removed; it had no caller |
| None | `stab_model::DemInstructionView` and `DemInstruction::view()`, a borrowed enum with one variant per instruction kind carrying exactly that kind's data: `Error { probability, targets }`, `Detector { coordinates, detector }`, `LogicalObservable { observable }`, and `ShiftDetectors { coordinate_shift, detector_shift }` |
| `stabilizers_to_tableau(&[PauliString], bool, bool, bool)` and `stabilizers_to_tableau_with_budget` | `Tableau::from_stabilizers(&[PauliString], StabilizerTableauOptions)` and `Tableau::from_stabilizers_with_budget`; the options struct names `allow_redundant`, `allow_underconstrained`, and `inverse`, and its default is Stim's |
| `unitary_to_tableau(matrix, little_endian: bool)` | `Tableau::from_unitary(matrix, Endianness)` |
| `TableauIterator::new(usize, bool)` and `PauliStringIterator::new(n, min_weight, max_weight, bool, bool, bool)` | `TableauIterator::new(usize, SignIteration)` and `PauliStringIterator::new(n, RangeInclusive<usize>, PauliIterationBases)`, with `new_with_budget` changed the same way |
| `SingleQubitClifford::{multiply, inverse, apply_basis}` returning `StabilizerResult` | All three return their value directly, because the 24 single-qubit Cliffords are a closed group; `SingleQubitClifford` also implements `Mul`, and `StabilizerError::InvalidSingleQubitCliffordProduct` is removed |
| `PauliString::{intersects, commutes}` returning `StabilizerResult<bool>` | Both return `bool`; a shorter string is padded with identities |
| `PauliString::log_i_scalar_byproduct`, `right_multiply_in_place_returning_log_i_scalar`, and `PauliBasis::log_i_scalar_byproduct` returning a `u8` exponent | They return `PauliPhase`, which gains public `log_i` and `from_log_i`; the in-place multiply stays fallible and can report `AllocationFailed` when it grows |
| `Tableau::{x, y, z}_output_pauli_xyz` returning a `u8` | `Tableau::{x, y, z}_output_basis` returning `PauliBasis` |
| `StabilizerError::InvalidSparsePauliString { text }` and `InvalidFlowText { text }` holding the whole input | Both hold `excerpt: stab_algebra::TextExcerpt`, at most 64 characters |
| `StabilizerError::AntiCommutingStabilizer`, `RedundantStabilizer`, and `InconsistentStabilizer` holding formatted Pauli strings | They hold input indices; `InvalidFlowProduct` is a unit variant |
| `stab_algebra::advanced::{pauli_identity_unchecked, pauli_from_bases_unchecked}` | `pauli_identity_unbudgeted` and `pauli_from_bases_unbudgeted`; they skip only the budget admission |
| `stab_kernels_simd::pauli_right_multiply_words(&mut [u64], &mut [u64], &[u64], &[u64])` and `pauli_right_multiply_words_log_i` | Both take `(PauliPlanesMut, PauliPlanes)`, so the X and Z planes cannot be swapped |
| `RepetitionCodeParams`, `SurfaceCodeParams`, `ColorCodeParams`, and `generate_{repetition, surface, color}_code_circuit` | `CodeCircuitParams::try_new(CodeTask, CodeDistance, RoundCount)` and `CodeCircuitParams::generate()`; note the argument order, and that color-code distance and round constraints are refused by `try_new` |
| CLI task-name tables and `CliError::Unsupported{Repetition, Surface, Color}Task` | `stab_analysis::CodeFamily` and the task enums implement `FromStr`, `Display`, `ALL`, and `as_str`; the CLI error is `CliError::UnsupportedGenTask(UnknownCodeTask)` with unchanged text and codes |
| `stab_model::advanced::probability_from_valid` | Removed; use `Probability::try_new`, or `Probability::ZERO` and `Probability::ONE` |
| `stab_model::advanced::{dem_model_items_mut, dem_repeat_block_body_mut}` | `DetectorErrorModel::{reverse_items, item_mut, insert_item, try_retain_items_mut}` and the now-public `DemRepeatBlock::body_mut` |
| `Circuit::detector_coordinates()` without a budget | It admits the map against the default budget, and `Circuit::detector_coordinates_with_memory_budget` takes one; a refusal is `ModelError::ResourceLimit` with `ResourceKind::MaterializedBytes` |
| None | `stab_bits::fill_periodic_words`, `BitVec::try_zeros`, and `BitVec::try_resize_zeros` |
| None | `DemSamplingPlan::limits()`, `DetectionResourceKind::SessionStorageBytes`, and a `StorageAllocation { message }` variant on `DetectionError`, `DemError`, and `SamplingCompileError` |

`DemSamplingPlan::session` and `replay_session` now inherit the memory budget and cancellation of the compiler that built the plan, as `SamplingPlan` and the detection plans already did; a caller that budgeted the compiler can therefore see a session refused under that budget, and the `_with_limits` forms still override it.
`circuit_reference_signs_with_limits` also admits the packed signs it returns, so its minimum budget rises by eight bytes per started 64 detectors and per started 64 observables.

Allocator refusals that were reported as `InvalidSamplerCompilation` are now the `StorageAllocation` variant of the same error, with unchanged message text, and a session storage refusal converts to `DetectionError::ResourceLimit` with `DetectionResourceKind::SessionStorageBytes`.
On the command line, a session storage refusal in `sample`, `detect`, `m2d`, or `sample_dem` and every `StorageAllocation` error now carry the code `resource-limit-exceeded`; they previously carried `sampling-execution-failed`, `detection-execution-failed`, `dem-sampling-execution-failed`, or `invalid-sampler-compilation`.

`Tableau::from_unitary`, the former `unitary_to_tableau`, has a new acceptance rule.
With `b` the largest-magnitude entry of the first column, a matrix is accepted exactly when every entry is finite, every column's squared norm is within `1e-4` of one, every ratio of an entry to `b` is within squared distance `0.125` of one of `0`, `1`, `-1`, `i`, and `-i`, and the snapped matrix is exactly a scalar multiple of a Clifford unitary on every entry.
The snapping threshold is Stim v1.16.0's, so `RX(0.3)` is now accepted as the identity as Stim accepts it, where the earlier per-entry check rejected it; unlike Stim, Stab keeps the column-norm check and rejects a scaled matrix such as `2·I`.
Rejections keep their two classes, `MatrixNotUnitary` and `UnitaryMatrixNotClifford`.
Accepting a matrix of dimension `d` on `n` qubits now costs about `n·d` work per non-zero of a column plus `d²`, instead of `n·d³`.

A detector-coordinate map that does not fit is now refused before allocation for circuits as it already was for detector error models, and both refusals are `ModelError::ResourceLimit` instead of an invalid-model validation error; their text names the needed bytes and the budget and no longer starts with `invalid detector error model:`.
`ResourceLimitError::operation()` can therefore report the non-parse operations `CircuitDetectorCoordinates` and `DetectorErrorModelDetectorCoordinates`.
A DEM item is 88 bytes instead of 112, so every parse, flatten, and analysis-output charge per DEM declaration drops by 24 bytes.

Generation admits one mebibyte plus 768 bytes per physical qubit instead of 2048 bytes per qubit, so the default one-gibibyte budget now admits a rotated surface code up to distance 835, an unrotated one up to 591, and a color code up to 1113.
`gen` streams its text, and a named `--out` admits its 64 KiB write buffer before generation, so a budget that only just fit generation is refused when `--out` is added.

The algebra error messages for anticommuting, redundant, and inconsistent stabilizers name input indices instead of printing the Pauli strings, the flow-product message no longer prints both flows, and rejected sparse Pauli or flow text longer than 64 characters is cut and ends in `...`.

`Flow` text now follows Stim v1.16.0's rule for choosing the sparse form: a side whose last qubit is identity makes both sides print dense, so `___X_____ -> ________X xor rec[5]` is no longer printed as `X3 -> X8 xor rec[5]`.

`circuit_to_detector_error_model` and `analyze_errors` now accept a `PAULI_CHANNEL_2` with at most one non-zero probability without `approximate_disjoint_errors` and ignore the threshold for it, and refuse a target-less `PAULI_CHANNEL_2` with two or more non-zero probabilities unless approximation is allowed; both match Stim.
With `fold_loops`, a period search that fails for any reason other than cancellation now falls back to unrolling, so the error reported is the unrolled analysis's first error, as in Stim.
The search holds one tracker clone instead of two, and a budget that admits the analyzer and that clone but not their later growth is refused at the step that grows them.

## Analysis Error Taxonomy

`AnalysisError` used to funnel circuit refusals, unsupported input, arithmetic overflow, allocator refusals, and internal invariants into `InvalidDetectorErrorModel { message }`, so a circuit whose detector anticommutes with a reset was reported as an invalid detector error model and callers could not match on what happened.
Its variants now name the failure, and every message a user can reach keeps its text byte for byte.

| Removed variant | Replacement |
| --- | --- |
| `InvalidDetectorErrorModel { message }` | `NonDeterministicTargets`, `NoiseRequiresApproximation`, `UnanalyzableNoise`, `ErrorDecompositionFailed`, `NonGraphlikeError`, `NoLogicalErrorFound`, `InvalidRequest`, `AntiHermitianPauliProduct`, `MeasurementRecordOutOfRange`, `UnsupportedInput`, `CountOverflow`, `AllocationFailed`, or `InternalInvariant`, by meaning |
| `InvalidTableauConversion { message }` | `NonUnitaryOperation`, `PauliNotPreserved`, `GateDataUnavailable`, `InvalidRequest`, `AntiHermitianPauliProduct`, `MeasurementRecordOutOfRange`, `UnsupportedInput`, `CountOverflow`, or `InternalInvariant` |
| `InvalidCircuitSimplification { message }` | `AntiHermitianPauliProduct` or `InternalInvariant` |
| `InvalidDomainValue { kind, value }` | `InvalidGeneratorParameter { parameter, value }`, `ResourceLimit` for the generation budget, `CountOverflow`, `AllocationFailed`, or `InternalInvariant` |
| `InvalidResultFormat { message }` | `NonFiniteCoordinate { operation }` |
| `stab_analysis::ResourceOperation` | `stab_analysis::AnalysisOperation`, now `#[non_exhaustive]`, with the added operations `CircuitGeneration`, `CircuitInverse`, `CircuitSimplification`, `FlowMeasurementSolving`, `GateData`, `ReverseFrameTracking`, and `TableauSynthesis` |

The typed variants carry data instead of prose: `NonDeterministicTargets` holds the detector and observable targets, `NonUnitaryOperation` the gate and a `NonUnitaryReason`, `PauliNotPreserved` the gate and a `PauliCollapse`, `NoiseRequiresApproximation` the gate and an optional `ThresholdExcess`, `ErrorDecompositionFailed` the component, `InvalidRequest` an `AnalysisRequestError`, `InvalidGeneratorParameter` a `GeneratorParameter`, `GateDataUnavailable` the gate and a `GateData`, and `NoLogicalErrorFound` a `NoLogicalErrorReport`.
`CountOverflow`, `AllocationFailed`, `UnsupportedInput`, `MeasurementRecordOutOfRange`, and `InternalInvariant` carry their `AnalysisOperation` and a detail string, and `AnalysisError::operation()` returns the operation of any variant that has one.
`AnalysisError` is deliberately not `#[non_exhaustive]`, so a consumer that maps it to codes fails to compile when a variant is added.

The generation budget refusal is now `AnalysisError::ResourceLimit` for `AnalysisOperation::CircuitGeneration`, and `ResourceLimitError::is_memory_budget()` reports whether a refusal came from the memory budget.
`stab_model::ModelError::AllocationFailed { dialect, detail }` is new and replaces the allocator refusals that were reported as an invalid detector error model.

The command line's machine-readable codes follow the variants.
The analysis use of `invalid-detector-error-model` is split into `non-deterministic-targets`, `approximate-disjoint-errors-required`, `unanalyzable-noise`, `error-decomposition-failed`, `non-graphlike-error`, `no-logical-error-found`, `invalid-analysis-request`, `anti-hermitian-pauli-product`, `measurement-record-out-of-range`, `unsupported-analysis-input`, `analysis-count-overflow`, and `analysis-invariant-failed`, and that code now means only a malformed detector error model.
`invalid-tableau-conversion` and `invalid-circuit-simplification` are removed in favour of `non-unitary-operation`, `pauli-not-preserved`, `gate-data-unavailable`, and the codes above; the analysis uses of `invalid-domain-value` and `invalid-result-format` become `invalid-generator-parameter` and `non-finite-coordinate`; and allocator refusals from analysis and from the model carry `resource-limit-exceeded`.
The JSON `context` of these diagnostics, which was empty, now carries the typed fields, for example `targets` for non-deterministic targets and `operation`, `gate`, and `reason` for a non-unitary operation.

## DEM Sampled-Error Streams

Seeded `sample_dem` detector and observable output no longer depends on `--err_out`. A run that records sampled errors, through `--err_out` or `DemSamplingSession::run_with_sampled_errors`, now draws exactly the random streams of a run that does not, so its detector and observable records are byte-identical to the plain seeded run, and replaying its sampled-error file with `--replay_err_in` reproduces them. Pinned Stim's detector output is likewise independent of the flag.

Seeded sampled-error records, and the detector and observable records of a recording run, therefore change once for every model with a repeat that the detector stream skips, applies by parity, or folds; for other models they are byte-identical to the previous build. Errors that the detector stream samples directly keep their draws. Errors inside a repeat that cannot fire stay clear, and errors inside a deterministic zero-shift repeat are set. The copies of an error inside a stochastic zero-shift repeat, which the detector stream samples as one folded parity error, are drawn from separate streams derived from the seed and conditioned on that folded parity, so each copy still fires independently with its own probability. Seeded output without `--err_out`, every `--replay_err_in` replay, and all other commands are byte-identical to the previous build, and no public item was added, removed, or renamed. The `sample_dem` compiler schema reported by `capabilities` moves from `1` to `2` to mark the new sampled-error stream.

## Extension APIs

The typed circuit-pass seam is canonical in `stab-analysis`. It admits folded input, requires a conservative output-resource projection before proportional allocation, validates the returned closed-dialect circuit, and reports typed input, projection, or output failures. The separate Stable `stab-reference-noise-pass` crate proves this public boundary without depending on `stab-core`.

Decoder interoperability is canonical in `stab-decoder`. The separate `stab-reference-decoder` crate proves the reusable session boundary without depending on the facade.

Neither seam implies dynamic plugins, runtime gate registration, GPU placeholders, or a universal compiler registry.

## SIMD Kernels

SIMD leaf kernels are always on, and there is no Cargo feature to select them. `stab-bits` and `stab-algebra` depend on `stab-kernels-simd` unconditionally, so every consumer, direct or through `stab-core`, resolves one `stab-kernels-simd` package plus `fearless_simd` and `fearless_simd_macros`.

Unreleased 0.2 development revisions had an opt-in `portable-simd` feature on `stab-bits`, `stab-algebra`, `stab-engine`, `stab-core`, and `stab-cli`; it is removed without an alias, so delete `features = ["portable-simd"]` from any dependency declaration that copied it. Results are unchanged, because the kernels compute the same words as the scalar formulas they replace. The kernels select the instruction set once per call: statically on AArch64 and with a runtime check on x86-64. See [ADR 0008](architecture/adr-0008-unconditional-fearless-simd-kernels.md).

## Deferred Surfaces, Gate Vocabulary, And Folded Summaries

The exported `explain_errors` machinery is removed, because `explain_errors` is a deferred product surface, and gates and folded-circuit counts share one model-owned vocabulary.

| Removed or changed item | Replacement |
| --- | --- |
| `stab_analysis::explain_errors_from_circuit`, `explain_errors_from_circuit_with_options`, `ExplainErrorsOptions`, `ExplainedError`, `CircuitErrorLocation`, `CircuitErrorLocationStackFrame`, `CircuitTargetsInsideInstruction`, `DemTargetWithCoords`, `FlippedMeasurement`, `GateTargetWithCoords`, `canonicalize_circuit_error_location_parts`, `canonicalize_dem_error_terms`, and `stab_analysis::advanced::{CircuitErrorLocationView, CircuitTargetsInsideInstructionView, write_explained_error}`, also reachable as `stab_core::analysis::*` | Removed without a replacement; Stim's `ErrorMatcher` provenance stays deferred with `explain_errors` |
| `Gate::best_candidate_inverse()` returning `ModelResult<Gate>` | Removed; `Gate::generalized_inverse()` returns `Gate` directly, because every table row names its inverse |
| None | `stab_model::GateKind`, also exported from the `stab-core` root, a closed enum with one variant per canonical Stim v1.16.0 gate spelled like the gate (`GateKind::CX`, `GateKind::SQRT_X_DAG`), with `GateKind::ALL`, `GateKind::COUNT`, `GateKind::gate`, `GateKind::canonical_name`, and `From<GateKind> for Gate`; `Gate::kind` returns it, and aliases such as `CNOT` parse to their canonical kind |
| None | `Gate::controlled_pauli()`, the Pauli applied by `CX`, `CY`, `CZ`, `XCZ`, and `YCZ`, and `Gate::collapse_basis()`, the basis of `MX`, `MY`, `M`, `MRX`, `MRY`, `MR`, `RX`, `RY`, and `R` |
| None | `stab_model::FoldedCircuitSummary`, `FoldedCount`, and `FoldedWalkError`, plus `stab_model::advanced::{visit_folded_circuit, FoldedCircuitEvent, FoldedVisitError}` |

`GateKind` is deliberately not `#[non_exhaustive]`: the dialect is closed (ADR 0001), and code that selects behavior per gate should match it without a wildcard so that a new gate fails to compile. `Gate` keeps its size and its accessors; per-gate facts still come from the one gate table through `Gate`.

`FoldedCircuitSummary::of(circuit, nesting, cancellation)` visits each folded item once and reports expanded instruction, target, argument, tag-byte, and measurement counts per gate kind, expanded repeat iterations, folded item count, and maximum nesting. Each expanded count is a `FoldedCount` that is either exact or overflowed, so a consumer reports overflow of the count it reads with its own error through `FoldedCount::get`, `exact`, or `saturating`. `visit_folded_circuit` is the same walk for consumers that accumulate their own byte formulas incrementally. The walk refuses nesting beyond its `RepeatNestingLimit` with `FoldedWalkError::RepeatNesting`, polls its `Cancellation` every 1024 items, and never allocates in proportion to a repeat count.

Measurement-rich `circuit_time_reversed_for_flows` and `circuit_inverse_qec` now refuse a programmatically built circuit nested deeper than 256 repeat levels before projecting its expansion, instead of recursing without a bound; parsed circuits cannot exceed that envelope. `estimate_sampling_request` reports unknown item and operation counts for such a circuit.

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
