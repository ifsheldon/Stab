# Resource Policy Inventory

This inventory records the limit policy reset of 2026-11-14, which [ADR 0004](adr-0004-diagnostics-and-resources.md) adopts.

It classifies every limit-like constant in the product crates, as found by `rg -n "^\s*(pub(\([a-z]+\))? )?const [A-Z0-9_]*(MAX|LIMIT|CAP|BUDGET)[A-Z0-9_]*\s*:" crates/*/src --glob '!**/tests*'`, together with the parser defaults that the pattern does not match, and names every constant the reset removed.

## Policy

Hostile-input protection means memory safety, no panics, checked arithmetic, bounded allocation, and bounded recursion.

It does not mean bounded CPU time, which belongs to the caller; a compact input that fits the memory budget runs for as long as its work takes unless the caller cancels it through the operation's `stab_model::Cancellation` signal, described under [Cooperative Cancellation](#cooperative-cancellation).

Every materializing or executing operation has one admission dimension, a typed `stab_model::MemoryBudget` of conservatively counted retained bytes, admitted with checked arithmetic before allocation, RNG advancement, or output and followed by fallible reservation.

The budget defaults to one gibibyte, is caller-selectable through each operation's limits type, and is exposed on the CLI as the one Stab-native `--memory-budget SIZE` option.

Budgets are per operation: parsing, plan compilation, reference-simulator scratch, and session storage are each admitted against the same budget value, so one command's peak retained memory can be a small multiple of the budget.

## Classification

| Kind | Rule |
| --- | --- |
| Invariant | A format, representation, or Stim-semantic bound. Unchanged by the reset. |
| Nesting | The shared 256-level repeat-nesting envelope, or a tighter bound a recursive traversal still needs. |
| Byte budget | A `MemoryBudget` or a value derived from one. |
| Search budget | Explicit caller policy with a budget-exhausted result. Values unchanged by the reset. |
| Threshold | An implementation choice that selects storage, batching, or an algorithm and never rejects otherwise valid work. |
| Not a limit | Matched by the search pattern but not a resource limit. |

## Remaining Constants

| Constant | Owner | Kind | Justification |
| --- | --- | --- | --- |
| `STIM_TARGET_VALUE_LIMIT` (two definitions) | `stab-model/src/ids.rs`, `lib.rs` | Invariant | Stim's target values are below $2^{24}$. |
| `MAX_STIM_NUMBER_TOKEN_BYTES` (two definitions) | `stab-model` circuit and DEM parsers | Invariant | Pinned Stim's numeric lexer accepts at most 63 bytes. |
| `MAX_CIRCUIT_REPEAT_COUNT_EXCLUSIVE` | `stab-model/src/circuit/parser.rs` | Invariant | Circuit repeat counts are below $2^{63}$ in pinned Stim. |
| `MAX_DEM_TEXT_INTEGER` | `stab-model/src/dem/parser.rs` | Invariant | DEM textual integers are bounded as in pinned Stim. |
| `MAX_DEM_DETECTOR_ID` | `stab-model/src/dem.rs` | Invariant | DEM detector IDs are bounded by the DEM representation. |
| `MAX_PARSE_DIAGNOSTIC_TEXT_BYTES` | `stab-model/src/diagnostics.rs` | Invariant | Diagnostic excerpts are bounded representation, not admitted work. |
| `MAX_PROBLEM_TERMS` | `stab-analysis/src/circuit_to_dem/decompose.rs` | Invariant | Stim's exact decomposition rejects problems above 64 terms. |
| `MAX_LOCAL_DETECTORS` | `stab-analysis/.../local_decomposition.rs` | Invariant | Stim's local decomposition has a 16-symptom mask. |
| `MAX_COMBINATION_BASES` | `stab-analysis/src/circuit_to_dem/reverse_fold.rs` | Invariant | A composite channel combines at most four basis errors. |
| `RepeatNestingLimit::HARD_MAX` | `stab-model/src/parse_limits.rs` | Nesting | The shared 256-level envelope that callers may tighten but not raise. |
| `MAX_DEM_REPEAT_NESTING` | `stab-model/src/dem.rs` | Nesting | Re-export of the shared envelope for DEM consumers. |
| `ReferenceSampleTree::MAX_NESTING` | `stab-engine/src/reference_sample_tree.rs` | Nesting | The shared envelope. |
| `ParseLimits::DEFAULT_REPEAT_NESTING` | `stab-model/src/parse_limits.rs` | Nesting | The shared envelope as a `RepeatNestingLimit`. The model's folded circuit walk (`stab_model::advanced::visit_folded_circuit` and `FoldedCircuitSummary`) enforces it for the analyzer, flattening, feedback inlining, flow generation, missing-detector discovery, measurement-rich time reversal, the CLI herald count, and the sampling request estimate, each reporting its own nesting error. |
| `MAX_DETECTING_REGION_REPEAT_NESTING` | `stab-analysis/src/circuit_detecting_regions.rs` | Nesting | 128. Raising it to 256 overflowed a 2 MiB test-thread stack in a debug build, because the snapshot walk and the tracker's recurrence probes recurse several frames per level, so it stays until that traversal is iterative. |
| `ParseLimits::DEFAULT_MEMORY_BUDGET` | `stab-model/src/parse_limits.rs` | Byte budget | The parse budget for source bytes plus the conservative retained model. |
| `MAX_DEM_SELECTED_COORDINATE_REPEAT_CANDIDATES` | `stab-model/src/dem/api.rs` | Byte budget | The default budget divided by 48 conservative bytes per retained candidate; truncation beyond it reports a typed error naming the budget. |
| `AlgebraMemoryBudget::DEFAULT` | `stab-algebra/src/budget.rs` | Byte budget | One gibibyte, the same default as `MemoryBudget`, used by the unbudgeted algebra constructors and parsers, each of which delegates to a `*_with_budget` sibling; `stab-analysis` maps its `MemoryBudget` onto it. |
| `SatMaterializationLimits::DEFAULT_MAX_*` (eight constants) | `stab-analysis/src/dem/sat/limits.rs` | Search budget | Explicit SAT construction policy, values unchanged. |
| `DEFAULT_MAX_*` (fourteen constants) | `stab-analysis/src/dem/search/budget.rs` | Search budget | `LogicalErrorSearchLimits` defaults, values unchanged. |
| `MAX_DEM_SELECTED_COORDINATE_FLATTENED_DECLARATIONS` | `stab-model/src/dem/coordinate_scan.rs` | Threshold | Selects the bounded flattened scan over selected-iteration search; it never rejects a query. |
| `MAX_BATCH_SHOTS` | `stab-engine/src/lib.rs` | Threshold | The public 256-shot delivery bound: one Pauli-frame batch at the widest `LaneWidth` for circuit sampling and detection sampling, the only batch width of measurement-to-detection conversion, and the generated DEM sample window delivered per batch. A session created with the 64-lane `LaneWidth` delivers at most 64 shots per batch and charges 8 instead of 32 bytes per frame plane against its memory budget. |
| `WINDOW_TARGET_BYTES` | `stab-engine/src/detection/frame/window.rs` | Threshold | 1 MiB of frame planes per window of a streaming sampling or detection run: the planes recorded between two hand-overs, rounded down to whole 64-plane tiles. It bounds retained planes independently of the circuit and never rejects work or changes output. |
| `SEGMENT_BITS` | `stab-records/src/encoder.rs` | Threshold | 65,536 bits of one record part encoded between two drain checks, so a wide record never holds its whole encoding; it never changes output. |
| `LaneWidth::WIDEST_FROM_TOTAL_SHOTS` | `stab-engine/src/lane_width.rs` | Threshold | 512 shots, two widest batches: `LaneWidth::for_total_shots` picks the widest lane width from this total on and otherwise the width that samples the fewest lanes; it never rejects work. |
| `MAX_REPLAY_BATCH_SHOTS` | `stab-engine/src/dem_sampling/session.rs` | Threshold | The 64-record shot-major DEM replay batch, which also caps the DEM session's batch-shape search. |
| `MAX_POOLED_SETS` | `stab-analysis/src/sparse_rev_frame_tracker.rs` | Threshold | Bounds the recycled-set pool; it never rejects work. |
| `INITIAL_NESTED_DEM_CAPACITY` | `stab-model/src/dem/parser/items.rs` | Threshold | An initial vector capacity. |
| `CAPABILITIES_OUTPUT_SCHEMA_VERSION` | `stab-cli/src/agent.rs` | Not a limit | A schema version. |
| `MEMORY_BUDGET_HELP` | `stab-cli/src/diagnostics.rs` | Not a limit | Help text for budget refusals. |

The reset removed every algebra bound. The two dense-tableau aliases were first replaced by 512-qubit thresholds and then by a measured cost model, which the search pattern does not match either, so it is listed here:

| Constant | Owner | Kind | Justification |
| --- | --- | --- | --- |
| `DENSE_*_NS`, `SPARSE_*_NS` | `stab-analysis/src/circuit_flow/check_algorithm.rs` | Cost model | Pauli-only flows of a measurement-free, repeat-free circuit are checked against one dense Tableau or with the sparse reverse tracker, whichever `FlowCheckCost` estimates to be cheaper from the qubit count, the top-level gate-target count, the flow count, and the flows' Pauli weights. `check_if_circuit_has_unsigned_stabilizer_flows`, its diagnostic, single, and batch variants, and the input-flow validation of unitary `circuit_time_reversed_for_flows` share it; time reversal of a circuit with repeat blocks keeps the sparse tracker when it folds every repeat body and otherwise uses the dense Tableau. Both algorithms return identical results and diagnostics, so the choice never changes output and never refuses work. The coefficients were fitted on a grid of surface-code, shallow random, 1D brickwork, and deep random circuits of 16 to 4096 qubits with few, all single-qubit, random full-width, and heavy flow sets. |

The algebra kernels choose layouts with private thresholds that never change results: `BLOCK_ROWS` and `BLOCKED_MIN_ROWS` in `stab-algebra/src/tableau/action.rs` make composition and inversion tabulate eight generators at a time for blocks of 1024 rows once there are at least 256 rows, `random.rs` blocks its triangular products by 64 rows above 256 rows, and `BITWISE_TRANSPOSE_WIDTH` transposes matrices at most 16 wide bit by bit.

The reset also introduced unmatched cost-model and threshold constants: `REFERENCE_SNAPSHOT_GENERATOR_BYTES` keeps the reference-snapshot admission formula unchanged, `PREFERRED_ACTIVE_BATCH_BYTES` keeps DEM batch shape, and therefore seeded streams, independent of a larger budget, `FOLDED_FINAL_REPEAT_THRESHOLD` keeps missing-detector discovery on its count-independent proof above one million expanded instructions, and `GENERATED_BYTES_PER_PHYSICAL_QUBIT`, `COORDINATE_MAP_ENTRY_BYTES`, `SELECTED_COORDINATE_CANDIDATE_BYTES`, `DETECTING_REGION_LIVE_STATE_UNIT_BYTES`, `DECLARED_ROW_*`, `FEEDBACK_RETAINED_COPIES`, and `MEASUREMENT_RICH_RETAINED_COPIES` are conservative bytes-per-unit factors of byte admission.

## Operation Budgets

| Operation | Caller selection | What the budget bounds |
| --- | --- | --- |
| Circuit and DEM parsing | `ParseLimits::with_memory_budget` | Source bytes plus a conservative retained size per declaration, target, and parenthesized argument, each target and argument charged for two slots and each tag byte twice, charged before retention. |
| Circuit sampling | `SamplingCompiler::memory_budget` | The resolved program, refused with `SamplingCompileError::CompiledBytesLimit` before materialization; each session's reusable storage; and the reference-simulator scratch, whose output record is packed at one bit per measurement. A materializing session is charged `P * (2q + m)` bytes for `q` frame qubits, `m` measurements, and `P`-byte planes (8 at 64 lanes, 32 at 256), plus `8 * ceil(m / 64)` for the packed reference sample when it uses one. A streaming run of `s` shots is charged `P * (2q + w)`, where the window capacity `w` is the smaller of `m` and one window plus 63 planes, the deepest classical-control lookback, and the most measurements of one instruction; plus `8 * min(s, lanes) * ceil(m / 64)` for its records, 32 KiB of transpose scratch, and the packed reference sample. There is no general-frame fallback. |
| Detection conversion and detection sampling | `DetectionConversionLimits::with_memory_budget` | The compact conversion plan plus one byte per measurement, sweep, detector, and observable record bit, the resolved program, the reference scratch, and each session's storage. A materializing detection session is charged `P * (2q + m + d + 2o)` bytes for `d` detectors and `o` observables. A streaming run of `s` shots is charged `P * (2q + w + d_w + 2o)`, where `w` also covers the deepest detector or observable lookback and the detector window `d_w` is the smaller of `d` and one window, plus `8 * min(s, lanes) * (ceil(d / 64) + ceil(o / 64))` for its records and 64 KiB of transpose scratch. |
| DEM sampling and replay | `DemSamplerLimits::with_memory_budget` | Each session's reusable storage. Batch shape follows the historical 64 MiB target so seeded streams do not depend on a larger budget, and a one-shot batch is admitted whenever it fits the budget. |
| Circuit flattening | `CircuitFlattenLimits::with_memory_budget` | The conservative materialized circuit. |
| DEM flattening | `DemFlattenLimits::with_memory_budget` | The conservative materialized model. |
| Circuit passes | `CircuitPassLimits::with_memory_budget` | Projected logical payload of the input, the declared output projection, and the returned circuit. |
| Circuit-to-DEM analysis | `ErrorAnalyzerOptions::memory_budget` | Each retained output item, charged before it is retained. |
| Feedback inlining | `FeedbackInliningLimits::with_memory_budget` through `circuit_with_inlined_feedback_with_limits` | The unrolled intermediate and output circuits, projected before unrolling. |
| Circuit generation | `*CodeParams::with_memory_budget` | 2048 conservative bytes per physical qubit, admitted before generation. |
| Stabilizer algebra | `Tableau::{identity_with_budget, random_with_budget, from_pauli_string_with_budget, embedded_with_budget}`, `stabilizers_to_tableau_with_budget`, `PauliString::{identity_with_budget, random_with_budget, from_bases_with_budget, from_str_with_budget}`, `FlexPauliString::{identity_with_budget, from_phase_and_bases_with_budget, from_str_with_budget}`, `CliffordString::{identity_with_budget, random_with_budget, from_gates_with_budget, concat_with_budget, repeat_with_budget}`, and `PauliStringIterator::new_with_budget`; the unbudgeted forms and the `FromStr` impls use `AlgebraMemoryBudget::DEFAULT` | The packed rows of a Tableau, or of Pauli and Clifford strings, built from a requested width, plus the random sampler's matrices and the solver's transposed rows and inversion scratch, admitted before allocation or RNG use. Composition, inversion, powers, direct sums, canonicalization, unitary conversion, and flow terms allocate a bounded multiple of operands the caller already holds and reserve it fallibly. |
| Circuit-to-Tableau conversion | `CircuitToTableauOptions::memory_budget` | The circuit's Tableau; repeat bodies and their powers have the same width. |
| Measurement-rich time reversal and QEC inverse | `TimeReversedForFlowsOptions::memory_budget`, `InverseQecOptions::memory_budget` | Three retained copies of the expanded reversal, refused as `time-reversal` `materialized-bytes`, and the dense Tableau that validates unitary input flows. |
| Flow generation | `FlowGeneratorOptions::memory_budget` through `circuit_flow_generators_with_options` | The projected owned flow rows, `2n + m` rows of a `Flow` plus `2n` Pauli bytes and four bytes per measurement record each, and the Tableau of a unitary circuit. |
| Missing-detector discovery | `MissingDetectorOptions::memory_budget` | The declared rows, the projected flow rows, and the flow generators it runs. |
| Detecting regions | `DetectingRegionOptions::memory_budget`, `DetectingRegionTargetOptions::memory_budget`, `all_detecting_region_targets_with_budget`, `all_detecting_region_ticks_with_budget` | Live tracker state at 64 bytes per unit, recurrence-probe clones, the output map and the Pauli strings of each region, and the dense helper vectors. |
| DEM coordinate maps | `DetectorErrorModel::detector_coordinates_with_memory_budget` | The all-detector map. |
| CLI inputs | `--memory-budget` | Whole inputs at the budget and each streamed text record at half of it, because a record's buffer doubles while it is framed. |

Each of these operations admits every retained allocation it makes against the one caller budget, including the algebra values it builds, which receive the same byte count as an `AlgebraMemoryBudget`; the functions without a budget parameter use `MemoryBudget::DEFAULT`.
Flow checking, Pauli conjugation (`pauli_after_circuit`, `pauli_before_circuit`), and `Flow` text parsing take no caller budget and admit their sized values against the default budget; `sample_if_circuit_has_stabilizer_flows` in `stab-engine` admits its witness simulator storage and compiles each augmented circuit against the default budget too.

## Cooperative Cancellation

`stab_model::Cancellation` is one cloneable signal: `Cancellation::new` creates a settable signal, `cancel` and `reset` set and clear it from any thread, and `Cancellation::never`, also the default, cannot be set and costs no allocation.
Operations given a set signal stop with a typed `Cancelled` outcome, either the error variant `Cancelled(stab_model::Cancelled)` of the operation's error type or a session's `Cancelled` run status, and the CLI reports the stable diagnostic code `cancelled`.
The CLI passes never-cancelled signals, because the process already stops on `SIGINT` and `SIGTERM`.

Loops poll through `stab_model::advanced::CancellationCountdown`, which loads the signal once every 1024 ticks; a tick is one interpreted instruction, repeat iteration, sampled or replayed error, or comparable bounded unit, never one gate target or one word-parallel step.
One tick can still be a large instruction, such as a measurement with many targets in the reference simulator, so the poll interval is bounded in units of work rather than in time.

| Operation | How the caller supplies the signal | Where it is polled | Outcome |
| --- | --- | --- | --- |
| Circuit sampling | `SamplingCompiler::cancellation`, or `SamplingSession::cancellation` for a session-local signal | Reference-sample repeat interpretation and every frame-program instruction and repeat iteration, plus between batches | Between batches, `SamplingRunStatus::Cancelled` with a finalized sink and a resumable seeded stream; inside a batch, `SamplingExecutionError::Cancelled` with no partial batch, no finalization, and a poisoned session; during reference computation, `SamplingExecutionError::Cancelled` |
| Detection sampling | `DetectionConversionLimits::with_cancellation` on `DetectionSamplingCompiler::limits`, or `DetectionSamplingSession::cancellation` | Frame program and conversion program, plus between batches | As for circuit sampling, with `DetectionRunStatus::Cancelled` and `DetectionExecutionError::Cancelled` |
| Measurement-to-detection conversion | `DetectionConversionLimits::with_cancellation` on `MeasurementToDetectionCompiler::limits`, or `MeasurementToDetectionSession::cancellation` | Reference-sample computation at compilation, sweep-correction frame program, and conversion program | `DetectionError::Cancelled` from compilation; a batch cancelled before or during conversion returns `DetectionRunStatus::Cancelled`, delivers nothing, and leaves the session resumable because conversion consumes no random stream |
| DEM sampling and replay | `DemSamplerLimits::with_cancellation`, or the session's `cancellation` | Every sampled or replayed error and repeat iteration, plus between batches | `DemSamplingRunStatus::Cancelled` or `DemReplayBatchStatus::Cancelled`; counter-based streams make an in-batch cancellation resumable with the same seeded output |
| Circuit-to-DEM analysis | `ErrorAnalyzerOptions::cancellation` | Every analyzed instruction and repeat iteration, folded or unrolled, including recurrence probes | `AnalysisError::Cancelled` |
| Circuit and DEM flattening | `CircuitFlattenLimits::with_cancellation`, `DemFlattenLimits::with_cancellation` | Every materialized instruction; every visited DEM item and repeat iteration, also through `DemFlattenedInstructionIter::with_cancellation` | `AnalysisError::Cancelled`, or `ModelError::Cancelled` from the iterator |
| Feedback inlining | `FeedbackInliningLimits::with_cancellation` | Every undone instruction and unrolled iteration | `AnalysisError::Cancelled` |
| Time reversal and QEC inverse | `TimeReversedForFlowsOptions::cancellation`, `InverseQecOptions::cancellation` | Every reversed instruction of the expanded measurement-rich reversal | `AnalysisError::Cancelled` |
| Flow generation and missing detectors | `FlowGeneratorOptions::cancellation` through `circuit_flow_generators_with_options`, `MissingDetectorOptions::cancellation` | Every undone instruction of the measurement-rich solver, and the unitary path's Tableau conversion | `AnalysisError::Cancelled` |
| Circuit-to-Tableau conversion | `CircuitToTableauOptions::cancellation` through `circuit_to_tableau_with_options`, also reached from `TimeReversedForFlowsOptions::cancellation` for the dense validation of unitary time reversal | Every instruction, and before every Tableau composition of a repeat power, so a repeat count near `2^63` is cancellable between its at most 126 compositions | `AnalysisError::Cancelled` |
| Detecting regions | `DetectingRegionOptions::cancellation`, `DetectingRegionTargetOptions::cancellation` | Every undone instruction and repeat iteration of the reverse tracker | `AnalysisError::Cancelled` |
| Folded DEM traversal | `FoldedDemVisitor::poll_cancellation`, a defaulted visitor hook | Every expanded or selected repeat iteration | The visitor's error type |

`stab-algebra` operations take no signal: each call is polynomial in bytes already admitted, at most `O(n^3 / 64)` word operations for an `n`-qubit Tableau, or `O(log |k|)` such compositions for `Tableau::pow(k)`, and callers that loop over them poll between calls.
Parsing, circuit generation, circuit passes, logical-error search, and SAT materialization take no signal: parsing and generation are linear in their admitted bytes, the built-in passes are linear in the folded circuit, and search and SAT keep explicit search budgets.
`stab_decoder::DecodeCancellation` remains the decoder session's own one-way signal.

## Removed Constants

| Removed constant | Former kind | Reason and replacement |
| --- | --- | --- |
| `ParseLimits::DEFAULT_SOURCE_BYTES` and `SourceByteLimit` | Byte cap | Converted: the source bytes are charged to `ParseLimits::DEFAULT_MEMORY_BUDGET`. |
| `ParseLimits::DEFAULT_SOURCE_LINES`, `DEFAULT_REPRESENTED_INSTRUCTIONS`, `DEFAULT_REPRESENTED_TARGETS` and their limit types | Count caps beside a byte budget | Removed; declarations and targets are charged in bytes. |
| `MAX_CIRCUIT_INPUT_BYTES`, `MAX_SAMPLE_DEM_INPUT_BYTES`, `MAX_ANALYZE_ERRORS_INPUT_BYTES` | CLI input caps | Replaced by `--memory-budget`. |
| `MAX_M2D_TEXT_RECORD_BYTES`, `MAX_SAMPLE_DEM_REPLAY_TEXT_RECORD_BYTES`, `MAX_CONVERT_RECORD_BYTES` | CLI per-record caps | Replaced by half the `--memory-budget`. |
| `MAX_EXPANDED_OPERATIONS_PER_SHOT` | Work cap | Removed. |
| `MAX_SAMPLING_SESSION_STORAGE_BYTES` | Byte budget | Converted to the sampling plan's memory budget. |
| `MAX_DETECTION_SESSION_STORAGE_BYTES`, `MAX_REFERENCE_SCRATCH_BYTES` | Byte budgets | Converted to `DetectionConversionLimits::memory_budget`. |
| Detection `DEFAULT_MAX_COMPILED_BYTES` | Byte budget | Converted to `DetectionConversionLimits::memory_budget`. |
| Detection `DEFAULT_MAX_RECORD_BITS` | Count cap | Removed; record bits are charged one byte each to the plan budget. |
| Detection `DEFAULT_MAX_EXPANDED_INSTRUCTIONS`, `DEFAULT_MAX_REPEAT_ITERATIONS` | Work caps | Removed. |
| Detection `DEFAULT_MAX_COMPILED_TERMS` | Count cap | Removed; terms are charged in the plan bytes. |
| `MAX_DEM_SAMPLER_ACTIVE_BATCH_BYTES`, `MAX_DEM_SESSION_STORAGE_BYTES` | Byte budgets | Converted to `DemSamplerLimits::memory_budget`, with batch shape kept by `PREFERRED_ACTIVE_BATCH_BYTES`. |
| `MAX_DEM_SAMPLER_SAMPLE_ERROR_APPLICATIONS_PER_SHOT`, `MAX_DEM_SAMPLER_REPLAY_WORK_UNITS_PER_SHOT` | Work caps | Removed. |
| `MAX_MATERIALIZED_FLATTENED_BYTES` and `CircuitFlattenLimits::DEFAULT_MAX_MATERIALIZED_BYTES` | Byte budget | Converted to `CircuitFlattenLimits::memory_budget`. |
| `MAX_MATERIALIZED_FLATTENED_OPERATIONS`, `_TARGETS`, `_ARGUMENTS` and their `CircuitFlattenLimits::DEFAULT_MAX_*` aliases | Count caps beside a byte budget | Removed. |
| `MAX_DEM_FLATTEN_MATERIALIZED_BYTES` and its `DemFlattenLimits` alias | Byte budget | Converted to `DemFlattenLimits::memory_budget`. |
| `MAX_DEM_FLATTEN_REPEAT_UNROLL`, `_EXPANDED_INSTRUCTIONS`, `_REPEAT_ITERATIONS`, `_TARGET_OCCURRENCES`, `_ARGUMENT_VALUES` and their `DemFlattenLimits` aliases | Count and work caps | Removed. |
| `MAX_PROJECTED_PASS_PAYLOAD_BYTES` and `CircuitPassLimits::DEFAULT_MAX_PROJECTED_PAYLOAD_BYTES` | Byte budget | Converted to `CircuitPassLimits::memory_budget`. |
| `CircuitPassLimits::DEFAULT_MAX_REPRESENTED_ITEMS`, `_TARGET_OCCURRENCES`, `_ARGUMENT_VALUES` | Count caps beside a byte budget | Removed. |
| `MAX_ANALYZER_EXPANDED_INSTRUCTIONS`, `MAX_ANALYZER_REPEAT_ITERATIONS`, `MAX_LOOP_CYCLE_STEPS` (both configurations) | Work caps | Removed; retained analyzer output is charged to `ErrorAnalyzerOptions::memory_budget`. |
| `MAX_FEEDBACK_REPEAT_WORK_UNITS` | Work cap | Removed; the unrolled circuits are projected in bytes before unrolling. |
| `MAX_NONUNITARY_PAULI_CONJUGATION_INSTRUCTIONS` | Work cap | Removed; conjugation retains one fixed-width Pauli string. |
| `MAX_TIME_REVERSE_TABLEAU_EXPANDED_INSTRUCTIONS` | Work cap | Removed; the dense Tableau is admitted in bytes and the conversion is cancellable. |
| `MAX_MEASUREMENT_RICH_EXPANDED_INSTRUCTIONS` | Work cap | Removed; the expanded reversal is projected in bytes against `TimeReversedForFlowsOptions::memory_budget`. |
| `MAX_MISSING_DETECTOR_SCANNED_INSTRUCTIONS` | Work cap | Removed; declared rows are projected in bytes, and `FOLDED_FINAL_REPEAT_THRESHOLD` keeps the count-independent path selection. |
| `MAX_FLOW_GENERATOR_PROJECTED_BYTES` | Byte budget | Converted to `FlowGeneratorOptions::memory_budget` and `MissingDetectorOptions::memory_budget`, through the private `FLOW_GENERATOR_MEMORY_BUDGET`, which was removed once both took a caller budget. |
| `MAX_FLOW_GENERATOR_EXPANDED_OPERATIONS` | Work cap | Removed; solver work mutates rows that the byte budget bounds. |
| `MAX_DETECTING_REGION_REPRESENTED_WORK`, `_TRAVERSAL_WORK` | Work caps | Removed. |
| `MAX_DETECTING_REGION_LIVE_STATE_UNITS`, `_OUTPUT_BYTES` | Byte proxies | Converted to the detecting-region options' `memory_budget` at 64 bytes per live-state unit, through the private `DETECTING_REGION_MEMORY_BUDGET`, which was removed once the options took a caller budget. |
| `MAX_DETECTING_REGION_OUTPUT_REGIONS`, `_HELPER_TARGETS`, `_HELPER_TICKS` | Count caps | Removed; regions and helper vectors are charged in bytes. |
| `MAX_ERROR_MATCHER_REPEAT_UNROLL`, `_EXPANDED_INSTRUCTIONS`, `_REPEAT_ITERATIONS` | Work caps | Removed. |
| `MAX_ERROR_MATCHER_FILTER_REPEAT_UNROLL`, `_EXPANDED_INSTRUCTIONS`, `_REPEAT_ITERATIONS` | Work and count caps | Removed; retained keys were first charged to a filter byte budget, which was then deleted with the deferred `explain_errors` surface. |
| `ERROR_MATCHER_FILTER_MEMORY_BUDGET` | Byte budget | Removed with `ErrorMatcher`, because `explain_errors` is a deferred product surface. |
| `MAX_FEEDBACK_REPEAT_NESTING`, `MAX_MISSING_DETECTOR_REPEAT_NESTING`, `MAX_FLOW_GENERATOR_REPEAT_NESTING` | Nesting | Removed; each was 256, and the shared folded walk now applies `ParseLimits::DEFAULT_REPEAT_NESTING` with the same value and the same reported depth. |
| `MAX_DEM_COORDINATE_SCALAR_WORK` | Work cap | Removed; coordinate vectors are bounded by the represented model. |
| `MAX_DEM_COORDINATE_MAP_DETECTORS` | Count cap | Converted to byte admission of the all-detector map. |
| `MAX_GENERATED_PHYSICAL_QUBITS` | Generator cap | Converted to 2048 bytes per physical qubit against the parameters' budget, because generation is linear in its output. |
| `MAX_PAULI_QUBITS` | Algebra count cap | Converted: Pauli strings and Pauli iterator state built from a requested width, and bases collected from an iterator, are admitted in bytes against a caller `AlgebraMemoryBudget`, `AlgebraMemoryBudget::DEFAULT` for the unbudgeted forms, so a compact sparse literal such as `+X1000000000000` is refused before allocation. Pauli products, parsing of dense text, and right multiplication grow only to an operand's width. |
| `MAX_CLIFFORD_QUBITS` | Algebra count cap | Converted: Clifford strings from a requested width, `repeat`, `concat`, and collected gates admit their six planes and table indices in bytes against a caller `AlgebraMemoryBudget`, the default one for the unbudgeted forms, with overflow-checked sizes. |
| `MAX_TABLEAU_QUBITS` | Algebra cap on a dense algorithm | Removed by fixing the algorithm: Tableau rows are packed X and Z words with one sign plane, composition and inversion are word-parallel in `O(n^3 / 64)`, and identities, embeddings, and `from_pauli_string` admit their packed rows in bytes; composition, inversion, powers, and direct sums allocate a bounded multiple of their operands fallibly. |
| `MAX_RANDOM_TABLEAU_QUBITS` | Algebra cap on a slow algorithm | Removed by fixing the algorithm: Bravyi and Maslov's sampler runs on packed GF(2) rows in `O(n^3 / 64)`, and the output plus every sampling matrix is admitted in bytes before any randomness is drawn. |
| `MAX_STABILIZER_SOLVE_QUBITS` | Algebra cap on a slow algorithm | Removed by fixing the algorithm: Stim's elimination runs over transposed packed rows with the elimination Tableau carried beside the stabilizers, and the working rows, the output, and the inversion scratch are admitted in bytes. |
| `MAX_UNITARY_MATRIX_DIMENSION` | Algebra count cap | Removed: unitary conversion is bounded by its input, because each per-generator scratch matrix has the caller's matrix size and is reserved fallibly, and like Stim it accepts any power-of-two dimension; the `O(d^3)` checks are the caller's CPU time. The power-of-two invariant stays. |
| `MAX_FLOW_CLASSICAL_TERMS` | Algebra count cap | Removed: every stored measurement or observable term is one item the caller supplied, as an iterator item, a text token, or an operand's term, so storage is bounded by the input and grows by fallible reservation. |
| `MAX_CIRCUIT_TABLEAU_REPEAT_WORK` | Algebra work cap | Removed by fixing the algorithm: a repeat body is raised to its count by squaring, at most 126 compositions for a count below `2^63`, and the conversion polls cancellation before each composition. |
| `MAX_TIME_REVERSE_TABLEAU_QUBITS` | Alias of the dense Tableau cap | Replaced by a validation-algorithm choice that never refuses work, now the `FlowCheckCost` model; wide circuits use the dense Tableau admitted in bytes when it is cheaper or when the sparse tracker cannot fold a repeat. |
| `MAX_BATCH_FLOW_TABLEAU_QUBITS` | Alias of the dense Tableau cap | Replaced by an algorithm choice that never refuses work, now the `FlowCheckCost` model. |
| `BATCH_FLOW_TABLEAU_QUBIT_THRESHOLD`, `DENSE_TIME_REVERSE_VALIDATION_QUBIT_THRESHOLD` | Unmeasured 512-qubit algorithm thresholds | Replaced by the measured `FlowCheckCost` model: qubit count alone mispredicted both ways, and the flow checker's two paths reported different mismatch kinds. |
| `MAX_IGNORED_ONLY_FLOW_GENERATOR_PROJECTED_BYTES` | Fixed byte cap on a quadratic algorithm | Converted: ignored-only and unitary flow rows are admitted in bytes against `FlowGeneratorOptions::memory_budget` with the measurement-rich row formula, and building them is linear in those bytes. |

## Evidence

The parity ledger families under `resource-safety.*` own the byte-budget boundaries, and `stab-extensions.memory-budget` owns the CLI option; `docs/stim-parity.md` renders their selectors.

Boundary tests admit the exact budget and refuse the first excess byte before any allocation, RNG advance, or sink mutation, using the allocation counter and sink observation where the owner already did.

Hostile-shape tests keep a typed refusal for each removed cap's shape when it exceeds the budget, such as a compact repeat with an astronomical record width or flattened size.
