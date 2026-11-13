# Resource Policy Inventory

This inventory records the E4 limit policy reset of the [engine speed and architecture plan](../plans/engine-speed-and-architecture-plan.md), which replaced the A2 policy record that previously lived here.

It classifies every limit-like constant in the product crates, as found by `rg -n "^\s*(pub(\([a-z]+\))? )?const [A-Z0-9_]*(MAX|LIMIT|CAP|BUDGET)[A-Z0-9_]*\s*:" crates/*/src --glob '!**/tests*'`, together with the parser defaults that the pattern does not match, and names every constant E4 removed.

## Policy

Hostile-input protection means memory safety, no panics, checked arithmetic, bounded allocation, and bounded recursion.

It does not mean bounded CPU time, which belongs to the caller; a compact input that fits the memory budget runs for as long as its work takes until cooperative cancellation lands.

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
| Algebra | A bound on a slow dense algorithm in `stab-algebra` or its direct consumers, left for the run that replaces those algorithms. |
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
| `SatMaterializationLimits::DEFAULT_MAX_*` (eight constants) | `stab-analysis/src/dem/sat/limits.rs` | Search budget | Explicit SAT construction policy, values unchanged. |
| `DEFAULT_MAX_*` (fourteen constants) | `stab-analysis/src/dem/search/budget.rs` | Search budget | `LogicalErrorSearchLimits` defaults, values unchanged. |
| `MAX_PAULI_QUBITS`, `MAX_CLIFFORD_QUBITS`, `MAX_TABLEAU_QUBITS`, `MAX_RANDOM_TABLEAU_QUBITS`, `MAX_STABILIZER_SOLVE_QUBITS`, `MAX_UNITARY_MATRIX_DIMENSION`, `MAX_FLOW_CLASSICAL_TERMS`, `MAX_CIRCUIT_TABLEAU_REPEAT_WORK` | `stab-algebra/src/limits.rs` | Algebra | Bounds of the dense algebra algorithms that a later run replaces. |
| `MAX_TIME_REVERSE_TABLEAU_QUBITS` | `stab-analysis/src/circuit_inverse.rs` | Algebra | Alias of the dense tableau bound that selects the dense validation path. |
| `MAX_BATCH_FLOW_TABLEAU_QUBITS` | `stab-analysis/src/circuit_flow/checker.rs` | Algebra | Alias of the dense tableau bound. |
| `MAX_IGNORED_ONLY_FLOW_GENERATOR_PROJECTED_BYTES` | `stab-analysis/src/circuit_flow/generators/resources.rs` | Algebra | The ignored-only flow generator is quadratic in qubits; this bounds that dense algorithm until it is replaced. |
| `MAX_DEM_SELECTED_COORDINATE_FLATTENED_DECLARATIONS` | `stab-model/src/dem/coordinate_scan.rs` | Threshold | Selects the bounded flattened scan over selected-iteration search; it never rejects a query. |
| `MAX_BATCH_SHOTS` (three definitions) | `stab-engine` sampling, detection, and DEM sessions | Threshold | The 64-shot delivery batch. |
| `MAX_POOLED_SETS` | `stab-analysis/src/sparse_rev_frame_tracker.rs` | Threshold | Bounds the recycled-set pool; it never rejects work. |
| `INITIAL_NESTED_DEM_CAPACITY` | `stab-model/src/dem/parser/items.rs` | Threshold | An initial vector capacity. |
| `CAPABILITIES_OUTPUT_SCHEMA_VERSION` | `stab-cli/src/agent.rs` | Not a limit | A schema version. |
| `MEMORY_BUDGET_HELP` | `stab-cli/src/diagnostics.rs` | Not a limit | Help text for budget refusals. |

E4 also introduced unmatched cost-model and threshold constants: `REFERENCE_SNAPSHOT_GENERATOR_BYTES` keeps the reference-snapshot admission formula unchanged, `PREFERRED_ACTIVE_BATCH_BYTES` keeps DEM batch shape, and therefore seeded streams, independent of a larger budget, `FOLDED_FINAL_REPEAT_THRESHOLD` keeps missing-detector discovery on its count-independent proof above one million expanded instructions, and `GENERATED_BYTES_PER_PHYSICAL_QUBIT`, `COORDINATE_MAP_ENTRY_BYTES`, `SELECTED_COORDINATE_CANDIDATE_BYTES`, `DETECTING_REGION_LIVE_STATE_UNIT_BYTES`, `DECLARED_ROW_*`, `FEEDBACK_RETAINED_COPIES`, and `MEASUREMENT_RICH_RETAINED_COPIES` are conservative bytes-per-unit factors of byte admission.

## Operation Budgets

| Operation | Caller selection | What the budget bounds |
| --- | --- | --- |
| Circuit and DEM parsing | `ParseLimits::with_memory_budget` | Source bytes plus a conservative retained size per declaration and target, charged before retention. |
| Circuit sampling | `SamplingCompiler::memory_budget` | The resolved program, refused with `SamplingCompileError::CompiledBytesLimit` before materialization; each session's reusable storage; and the reference-simulator scratch. There is no general-frame fallback. |
| Detection conversion and detection sampling | `DetectionConversionLimits::with_memory_budget` | The compact conversion plan plus one byte per measurement, sweep, detector, and observable record bit, the resolved program, the reference scratch, and each session's storage. |
| DEM sampling and replay | `DemSamplerLimits::with_memory_budget` | Each session's reusable storage. Batch shape follows the historical 64 MiB target so seeded streams do not depend on a larger budget, and a one-shot batch is admitted whenever it fits the budget. |
| Circuit flattening | `CircuitFlattenLimits::with_memory_budget` | The conservative materialized circuit. |
| DEM flattening | `DemFlattenLimits::with_memory_budget` | The conservative materialized model. |
| Circuit passes | `CircuitPassLimits::with_memory_budget` | Projected logical payload of the input, the declared output projection, and the returned circuit. |
| Circuit-to-DEM analysis | `ErrorAnalyzerOptions::memory_budget` | Each retained output item, charged before it is retained. |
| Feedback inlining | `circuit_with_inlined_feedback_with_memory_budget` | The unrolled intermediate and output circuits, projected before unrolling. |
| Circuit generation | `*CodeParams::with_memory_budget` | 2048 conservative bytes per physical qubit, admitted before generation. |
| DEM coordinate maps | `DetectorErrorModel::detector_coordinates_with_memory_budget` | The all-detector map. |
| CLI inputs | `--memory-budget` | Whole inputs at the budget and each streamed text record at half of it, because a record's buffer doubles while it is framed. |

Measurement-rich time reversal, missing-detector discovery, flow generation, detecting regions, and the ErrorMatcher filter admit their retained storage against the default budget and do not yet take a caller budget.

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
| `MAX_TIME_REVERSE_TABLEAU_EXPANDED_INSTRUCTIONS` | Work cap | Removed; the dense path is still bounded by the algebra tableau width. |
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

## Evidence

The parity ledger families under `resource-safety.*` own the byte-budget boundaries, and `stab-extensions.memory-budget` owns the CLI option; `docs/stim-parity.md` renders their selectors.

Boundary tests admit the exact budget and refuse the first excess byte before any allocation, RNG advance, or sink mutation, using the allocation counter and sink observation where the owner already did.

Hostile-shape tests keep a typed refusal for each removed cap's shape when it exceeds the budget, such as a compact repeat with an astronomical record width or flattened size.
