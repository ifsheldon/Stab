# Resource Policy Inventory

This inventory records the E4 limit policy reset of the [engine speed and architecture plan](../plans/engine-speed-and-architecture-plan.md), which replaced the A2 policy record that previously lived here.

It classifies every limit-like constant in the product crates, as found by `rg -n "^\s*(pub(\([a-z]+\))? )?const [A-Z0-9_]*(MAX|LIMIT|CAP|BUDGET)[A-Z0-9_]*\s*:" crates/*/src --glob '!**/tests*'`, together with the parser defaults that the pattern does not match, and names every constant E4 removed.

## Policy

Hostile-input protection means memory safety, no panics, checked arithmetic, bounded allocation, and bounded recursion.

It does not mean bounded CPU time, which belongs to the caller; a compact input that fits the memory budget runs for as long as its work takes unless the caller cancels it through the operation's `stab_model::Cancellation` signal, described under [Cooperative Cancellation](#cooperative-cancellation).

Every materializing or executing operation has one admission dimension, a typed `stab_model::MemoryBudget` of conservatively counted retained bytes, admitted with checked arithmetic before allocation, RNG advancement, or output and followed by fallible reservation.

The budget defaults to one gibibyte, is caller-selectable through each operation's limits type, and is exposed on the CLI as the one Stab-native `--memory-budget SIZE` option.

Budgets are per operation: parsing, plan compilation, reference-simulator scratch, and session storage are each admitted against the same budget value, so one command's peak retained memory can be a small multiple of the budget.

## Classification

| Kind | Rule |
| --- | --- |
| Invariant | A format, representation, or Stim-semantic bound. Unchanged by E4. |
| Nesting | The shared 256-level repeat-nesting envelope, or a tighter bound a recursive traversal still needs. |
| Byte budget | A `MemoryBudget` or a value derived from one. |
| Search budget | Explicit caller policy with a budget-exhausted result. Values unchanged by E4. |
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
| `MAX_FEEDBACK_REPEAT_NESTING` | `stab-analysis/src/circuit_feedback.rs` | Nesting | 256, the shared envelope. |
| `MAX_MISSING_DETECTOR_REPEAT_NESTING` | `stab-analysis/src/circuit_missing_detectors.rs` | Nesting | 256, the shared envelope. |
| `MAX_FLOW_GENERATOR_REPEAT_NESTING` | `stab-analysis/src/circuit_flow/generators/resources.rs` | Nesting | 256, the shared envelope. |
| `MAX_DETECTING_REGION_REPEAT_NESTING` | `stab-analysis/src/circuit_detecting_regions.rs` | Nesting | 128. Raising it to 256 overflowed a 2 MiB test-thread stack in a debug build, because the snapshot walk and the tracker's recurrence probes recurse several frames per level, so it stays until that traversal is iterative. |
| `ParseLimits::DEFAULT_MEMORY_BUDGET` | `stab-model/src/parse_limits.rs` | Byte budget | The parse budget for source bytes plus the conservative retained model. |
| `DETECTING_REGION_MEMORY_BUDGET` | `stab-analysis/src/circuit_detecting_regions.rs` | Byte budget | Live tracker state, output regions, and helper vectors; the default budget because the API takes no budget yet. |
| `FLOW_GENERATOR_MEMORY_BUDGET` | `stab-analysis/src/circuit_flow/generators/resources.rs` | Byte budget | Owned measurement-rich flow rows, also used by missing-detector discovery; the default budget. |
| `ERROR_MATCHER_FILTER_MEMORY_BUDGET` | `stab-analysis/src/error_matcher/filter.rs` | Byte budget | Retained filter keys; the default budget. |
| `MAX_DEM_SELECTED_COORDINATE_REPEAT_CANDIDATES` | `stab-model/src/dem/api.rs` | Byte budget | The default budget divided by 48 conservative bytes per retained candidate; truncation beyond it reports a typed error naming the budget. |
| `AlgebraMemoryBudget::DEFAULT` | `stab-algebra/src/budget.rs` | Byte budget | One gibibyte, the same default as `MemoryBudget`, for algebra values built from a requested size; `stab-analysis` maps its `MemoryBudget` onto it. |
| `SatMaterializationLimits::DEFAULT_MAX_*` (eight constants) | `stab-analysis/src/dem/sat/limits.rs` | Search budget | Explicit SAT construction policy, values unchanged. |
| `DEFAULT_MAX_*` (fourteen constants) | `stab-analysis/src/dem/search/budget.rs` | Search budget | `LogicalErrorSearchLimits` defaults, values unchanged. |
| `MAX_DEM_SELECTED_COORDINATE_FLATTENED_DECLARATIONS` | `stab-model/src/dem/coordinate_scan.rs` | Threshold | Selects the bounded flattened scan over selected-iteration search; it never rejects a query. |
| `MAX_BATCH_SHOTS` (three definitions) | `stab-engine` sampling, detection, and DEM sessions | Threshold | The 64-shot delivery batch. |
| `MAX_POOLED_SETS` | `stab-analysis/src/sparse_rev_frame_tracker.rs` | Threshold | Bounds the recycled-set pool; it never rejects work. |
| `INITIAL_NESTED_DEM_CAPACITY` | `stab-model/src/dem/parser/items.rs` | Threshold | An initial vector capacity. |
| `CAPABILITIES_OUTPUT_SCHEMA_VERSION` | `stab-cli/src/agent.rs` | Not a limit | A schema version. |
| `MEMORY_BUDGET_HELP` | `stab-cli/src/diagnostics.rs` | Not a limit | Help text for budget refusals. |

E4 task 6 removed every algebra bound; the search pattern also no longer matches the two analysis thresholds that replaced the dense-tableau aliases, so they are listed here:

| Constant | Owner | Kind | Justification |
| --- | --- | --- | --- |
| `BATCH_FLOW_TABLEAU_QUBIT_THRESHOLD` | `stab-analysis/src/circuit_flow/checker.rs` | Threshold | 512. Repeat-free circuits up to this width check full-width flows against one dense Tableau and wider ones use the sparse reverse tracker. The two paths report different mismatch diagnostics, and the sparse cost follows the flows' support rather than the quadratic Tableau, so keeping the boundary keeps diagnostic output and wide-circuit cost unchanged; it never refuses work. |
| `DENSE_TIME_REVERSE_VALIDATION_QUBIT_THRESHOLD` | `stab-analysis/src/circuit_inverse.rs` | Threshold | 512. Repeat-free unitary circuits up to this width validate reversed flows against the dense Tableau; wider circuits use the sparse tracker when it supports them and otherwise the dense Tableau admitted in bytes, which the former alias refused. |

The algebra kernels choose layouts with private thresholds that never change results: `BLOCK_ROWS` and `BLOCKED_MIN_ROWS` in `stab-algebra/src/tableau/action.rs` make composition and inversion tabulate eight generators at a time for blocks of 1024 rows once there are at least 256 rows, `random.rs` blocks its triangular products by 64 rows above 256 rows, and `BITWISE_TRANSPOSE_WIDTH` transposes matrices at most 16 wide bit by bit.

E4 also introduced unmatched cost-model and threshold constants: `REFERENCE_SNAPSHOT_GENERATOR_BYTES` keeps the reference-snapshot admission formula unchanged, `PREFERRED_ACTIVE_BATCH_BYTES` keeps DEM batch shape, and therefore seeded streams, independent of a larger budget, `FOLDED_FINAL_REPEAT_THRESHOLD` keeps missing-detector discovery on its count-independent proof above one million expanded instructions, and `GENERATED_BYTES_PER_PHYSICAL_QUBIT`, `COORDINATE_MAP_ENTRY_BYTES`, `SELECTED_COORDINATE_CANDIDATE_BYTES`, `DETECTING_REGION_LIVE_STATE_UNIT_BYTES`, `DECLARED_ROW_*`, `FEEDBACK_RETAINED_COPIES`, and `MEASUREMENT_RICH_RETAINED_COPIES` are conservative bytes-per-unit factors of byte admission.

## Operation Budgets

| Operation | Caller selection | What the budget bounds |
| --- | --- | --- |
| Circuit and DEM parsing | `ParseLimits::with_memory_budget` | Source bytes plus a conservative retained size per declaration, target, and parenthesized argument, each target and argument charged for two slots and each tag byte twice, charged before retention. |
| Circuit sampling | `SamplingCompiler::memory_budget` | The resolved program, refused with `SamplingCompileError::CompiledBytesLimit` before materialization; each session's reusable storage; and the reference-simulator scratch. There is no general-frame fallback. |
| Detection conversion and detection sampling | `DetectionConversionLimits::with_memory_budget` | The compact conversion plan plus one byte per measurement, sweep, detector, and observable record bit, the resolved program, the reference scratch, and each session's storage. |
| DEM sampling and replay | `DemSamplerLimits::with_memory_budget` | Each session's reusable storage. Batch shape follows the historical 64 MiB target so seeded streams do not depend on a larger budget, and a one-shot batch is admitted whenever it fits the budget. |
| Circuit flattening | `CircuitFlattenLimits::with_memory_budget` | The conservative materialized circuit. |
| DEM flattening | `DemFlattenLimits::with_memory_budget` | The conservative materialized model. |
| Circuit passes | `CircuitPassLimits::with_memory_budget` | Projected logical payload of the input, the declared output projection, and the returned circuit. |
| Circuit-to-DEM analysis | `ErrorAnalyzerOptions::memory_budget` | Each retained output item, charged before it is retained. |
| Feedback inlining | `FeedbackInliningLimits::with_memory_budget` through `circuit_with_inlined_feedback_with_limits` | The unrolled intermediate and output circuits, projected before unrolling. |
| Circuit generation | `*CodeParams::with_memory_budget` | 2048 conservative bytes per physical qubit, admitted before generation. |
| Stabilizer algebra | `Tableau::identity_with_budget`, `Tableau::random_with_budget`, `stabilizers_to_tableau_with_budget`; `AlgebraMemoryBudget::DEFAULT` for the other size-requested constructors | The packed rows of a Tableau, or of Pauli and Clifford strings, built from a requested width, plus the random sampler's matrices and the solver's transposed rows and inversion scratch, admitted before allocation or RNG use. Composition, inversion, powers, direct sums, canonicalization, unitary conversion, and flow terms allocate a bounded multiple of operands the caller already holds and reserve it fallibly. |
| Circuit-to-Tableau conversion | `CircuitToTableauOptions::memory_budget` | The circuit's Tableau; repeat bodies and their powers have the same width. |
| DEM coordinate maps | `DetectorErrorModel::detector_coordinates_with_memory_budget` | The all-detector map. |
| CLI inputs | `--memory-budget` | Whole inputs at the budget and each streamed text record at half of it, because a record's buffer doubles while it is framed. |

Measurement-rich time reversal, missing-detector discovery, flow generation (whose ignored-only and unitary rows are admitted like measurement-rich ones), detecting regions, and the ErrorMatcher filter admit their retained storage against the default budget and do not yet take a caller budget; the dense Tableau of unitary time-reversal validation and unitary flow generation is admitted against it too.

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
| ErrorMatcher | `ExplainErrorsOptions::cancellation` through `explain_errors_from_circuit_with_options` | Every scanned instruction and repeat iteration, the filter traversal, and each candidate's analysis | `AnalysisError::Cancelled` |
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
| `MAX_MEASUREMENT_RICH_EXPANDED_INSTRUCTIONS` | Work cap | Removed; the expanded reversal is projected in bytes against the default budget. |
| `MAX_MISSING_DETECTOR_SCANNED_INSTRUCTIONS` | Work cap | Removed; declared rows are projected in bytes, and `FOLDED_FINAL_REPEAT_THRESHOLD` keeps the count-independent path selection. |
| `MAX_FLOW_GENERATOR_PROJECTED_BYTES` | Byte budget | Converted to `FLOW_GENERATOR_MEMORY_BUDGET`. |
| `MAX_FLOW_GENERATOR_EXPANDED_OPERATIONS` | Work cap | Removed; solver work mutates rows that the byte budget bounds. |
| `MAX_DETECTING_REGION_REPRESENTED_WORK`, `_TRAVERSAL_WORK` | Work caps | Removed. |
| `MAX_DETECTING_REGION_LIVE_STATE_UNITS`, `_OUTPUT_BYTES` | Byte proxies | Converted to `DETECTING_REGION_MEMORY_BUDGET` at 64 bytes per live-state unit. |
| `MAX_DETECTING_REGION_OUTPUT_REGIONS`, `_HELPER_TARGETS`, `_HELPER_TICKS` | Count caps | Removed; regions and helper vectors are charged in bytes. |
| `MAX_ERROR_MATCHER_REPEAT_UNROLL`, `_EXPANDED_INSTRUCTIONS`, `_REPEAT_ITERATIONS` | Work caps | Removed. |
| `MAX_ERROR_MATCHER_FILTER_REPEAT_UNROLL`, `_EXPANDED_INSTRUCTIONS`, `_REPEAT_ITERATIONS` | Work and count caps | Removed; retained keys are charged to `ERROR_MATCHER_FILTER_MEMORY_BUDGET`. |
| `MAX_DEM_COORDINATE_SCALAR_WORK` | Work cap | Removed; coordinate vectors are bounded by the represented model. |
| `MAX_DEM_COORDINATE_MAP_DETECTORS` | Count cap | Converted to byte admission of the all-detector map. |
| `MAX_GENERATED_PHYSICAL_QUBITS` | Generator cap | Converted to 2048 bytes per physical qubit against the parameters' budget, because generation is linear in its output. |
| `MAX_PAULI_QUBITS` | Algebra count cap | Converted: Pauli strings and Pauli iterator state built from a requested width, and bases collected from an iterator, are admitted in bytes against `AlgebraMemoryBudget::DEFAULT`, so a compact sparse literal such as `+X1000000000000` is refused before allocation. Pauli products, parsing of dense text, and right multiplication grow only to an operand's width. |
| `MAX_CLIFFORD_QUBITS` | Algebra count cap | Converted: Clifford strings from a requested width, `repeat`, `concat`, and collected gates admit their six planes and table indices in bytes against the default budget, with overflow-checked sizes. |
| `MAX_TABLEAU_QUBITS` | Algebra cap on a dense algorithm | Removed by fixing the algorithm: Tableau rows are packed X and Z words with one sign plane, composition and inversion are word-parallel in `O(n^3 / 64)`, and identities, embeddings, and `from_pauli_string` admit their packed rows in bytes; composition, inversion, powers, and direct sums allocate a bounded multiple of their operands fallibly. |
| `MAX_RANDOM_TABLEAU_QUBITS` | Algebra cap on a slow algorithm | Removed by fixing the algorithm: Bravyi and Maslov's sampler runs on packed GF(2) rows in `O(n^3 / 64)`, and the output plus every sampling matrix is admitted in bytes before any randomness is drawn. |
| `MAX_STABILIZER_SOLVE_QUBITS` | Algebra cap on a slow algorithm | Removed by fixing the algorithm: Stim's elimination runs over transposed packed rows with the elimination Tableau carried beside the stabilizers, and the working rows, the output, and the inversion scratch are admitted in bytes. |
| `MAX_UNITARY_MATRIX_DIMENSION` | Algebra count cap | Removed: unitary conversion is bounded by its input, because each per-generator scratch matrix has the caller's matrix size and is reserved fallibly, and like Stim it accepts any power-of-two dimension; the `O(d^3)` checks are the caller's CPU time. The power-of-two invariant stays. |
| `MAX_FLOW_CLASSICAL_TERMS` | Algebra count cap | Removed: every stored measurement or observable term is one item the caller supplied, as an iterator item, a text token, or an operand's term, so storage is bounded by the input and grows by fallible reservation. |
| `MAX_CIRCUIT_TABLEAU_REPEAT_WORK` | Algebra work cap | Removed by fixing the algorithm: a repeat body is raised to its count by squaring, at most 126 compositions for a count below `2^63`, and the conversion polls cancellation before each composition. |
| `MAX_TIME_REVERSE_TABLEAU_QUBITS` | Alias of the dense Tableau cap | Replaced by `DENSE_TIME_REVERSE_VALIDATION_QUBIT_THRESHOLD`, which only selects a validation algorithm; wide circuits the sparse tracker cannot validate now use the dense Tableau admitted in bytes instead of being refused. |
| `MAX_BATCH_FLOW_TABLEAU_QUBITS` | Alias of the dense Tableau cap | Replaced by `BATCH_FLOW_TABLEAU_QUBIT_THRESHOLD`, which keeps the same algorithm selection. |
| `MAX_IGNORED_ONLY_FLOW_GENERATOR_PROJECTED_BYTES` | Fixed byte cap on a quadratic algorithm | Converted: ignored-only and unitary flow rows are admitted in bytes against `FLOW_GENERATOR_MEMORY_BUDGET` with the measurement-rich row formula, and building them is linear in those bytes. |

## Evidence

The parity ledger families under `resource-safety.*` own the byte-budget boundaries, and `stab-extensions.memory-budget` owns the CLI option; `docs/stim-parity.md` renders their selectors.

Boundary tests admit the exact budget and refuse the first excess byte before any allocation, RNG advance, or sink mutation, using the allocation counter and sink observation where the owner already did.

Hostile-shape tests keep a typed refusal for each removed cap's shape when it exceeds the budget, such as a compact repeat with an astronomical record width or flattened size.
