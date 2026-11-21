# Open After Phase D: Details And Recommendations

Written 2026-11-21.
This expands the "Open After Phase D" list in [the review fix plan](review-fix-plan.md) into one section per item: what the code does today, why that is a problem, the proposed design, what it costs, how it relates to Stim, and a recommendation.
Nothing here is scheduled; each recommendation is a proposal for a decision.

Sizes: **S** stays inside one module, **M** changes one crate and its call sites, **L** crosses crates.
Call-site counts are `rg` hits at the time of writing and include tests.
Priorities follow the root `AGENTS.md`: API design, performance, and architecture rank above Stim compatibility, which ranks above backward compatibility.

## Summary

| # | Item | Size | Recommendation |
| --- | --- | --- | --- |
| D1 | One memory-budget type in a shared leaf crate | L | Done on 2026-11-21 as `stab-foundation`. |
| D2 | One representation of a refused budget | L | Done on 2026-11-21, with the fixed model caps of S5. |
| D3 | One observable-id domain | M | Done on 2026-11-21, as 32 bits everywhere. |
| D4 | Reject symlinks and unsafe path components on outputs | S | Decided on 2026-11-21: not rejected, and the CLI path trust boundary is documented. |
| R1 | A verdict from the flow checks | M | Done on 2026-11-21, as a two-way verdict with errors for everything else. |
| R2 | "No logical error found" as a search outcome, named search options | M | Done on 2026-11-21. |
| R3 | `Option` from the gate-data getters | M | Done on 2026-11-21, with R9. |
| R4 | One function per operation taking an options value | L | Do, after R1 and R2, and before any Python binding. |
| R5 | Engine run progress, status, summary, and error written once | M | Do, in the same pass as the engine half of R4. |
| R6 | Typed gate-argument and target-group views in `stab-model` | L | Done on 2026-11-21. |
| R7 | Nested option types for `ErrorAnalyzerOptions` | M | Do, in the same pass as R4 so call sites change once. |
| R8 | The `Vec<Vec<bool>>` record codec family | M | Remove from the public surface; keep as a test oracle. |
| R9 | Typed gate flow and decomposition metadata | M to L | Done on 2026-11-21. |
| R10 | Trim `stab_model::advanced` | S to M | Done on 2026-11-21. |
| S1 | `--fold_loops` circuits with a long transient are slower than Stim | M | Do as a measured performance task. |
| S2 | `ConversionOperation` is 40 bytes | S | Try it; keep only if `m2d` does not slow down. |
| S3 | `circuit_reference_signs` computes a byte per bit before packing | S | Do when that module is next touched. |
| S4 | A public `Tableau::to_unitary` | S to M | Defer until the library conversion surface is scheduled. |
| S5 | Fixed caps in `stab-model` reported an invalid detector error model | S | Done with D2. |
| N1 | Areas the review did not read | M | Run a second review pass before the next release. |

Remaining order: R4 with R5 and R7, then R8, with S1 as an independent performance task at any point.
R4 goes late on purpose: R1, R2, R3, and R7 each delete or reshape functions that R4 would otherwise rewrite twice.

## Decided And Done

These three were decided and implemented on 2026-11-21; the sections record what was changed and why.

### D1. One memory-budget type in a shared leaf crate

`stab_model::MemoryBudget` and `stab_algebra::AlgebraMemoryBudget` are the same idea, split only because `stab-algebra` cannot depend on `stab-model`.
`stab-analysis` converts one into the other by hand at every entry point that reaches the algebra, and two sites spell "no budget" as `from_bytes(u64::MAX)`.
The same split duplicates `ByteSpan` and `DiagnosticSeverity` (once in `stab-model`, once in `stab-records`) and the size estimate (`Estimate<T>` in the model, `EncodedSizeEstimate<T>` in records), so the CLI imports both spellings under aliases and converts between them.

The leaf product crate `stab-foundation` depends on no Stab crate and holds the budget, cancellation, the refusal of D2, the estimate, the byte span, and the diagnostic severity.
`stab-model`, `stab-algebra`, and `stab-records` depend on it and re-export the items that appear in their own signatures, so `stab_model::MemoryBudget` keeps working as a path while `AlgebraMemoryBudget` disappears.
It adds one edge to the dependency graph, one crate to the publish order, and one row to the product-contract table in `ops/architecture`.

### D2. One representation of a refused budget

A refused memory budget is reported today in at least six shapes: the analysis `ResourceLimitError`, `ModelError::ResourceLimit`, the engine's `DetectionResourceLimitError` and `DemResourceLimitError`, `SamplingCompileError::CompiledBytesLimit`, and the two `SessionStorageLimit` variants, plus the algebra's `StabilizerError::MemoryBudgetExceeded`.
Only the engine can say whether the required amount is exact or a lower bound.
The CLI decides whether to print the "raise `--memory`" help from typed data for analysis errors and from message text for the rest.

The change puts one value in the leaf crate of D1 that carries the required amount (exact or a lower bound) and the budget.
Each crate's error keeps its own typed description of what was being built and embeds that one value, and each error type answers "is this a budget refusal, and for how much" through one accessor.
The CLI then asks that question instead of reading message text.
S5 goes with it, because the two fixed model caps are the same misclassification.

### D3. One observable-id domain

A circuit `ObservableId` holds 64 bits and a `DemObservableId` holds 32, so `OBSERVABLE_INCLUDE(4294967296)` parses and then fails inside circuit-to-DEM conversion with "invalid detector error model".

The question was whether to make everything 64 bits.
Widening would cost almost nothing in memory or speed: a `DemTarget` already packs into eight bytes with 62 bits of room, and the engine indexes observables with `usize`.
It is still the wrong direction, for three reasons.

- The DEM text format is a public contract, and pinned Stim refuses `L4294967296` with "id > 0xFFFFFFFF". A 64-bit domain would make Stab write detector error models that Stim cannot read.
- An observable id is a dense index into the observable record of every shot. An id above `2^32` needs more than 512 MiB of observable record per shot, so no sampler or decoder can use one.
- Stim itself does not handle large circuit ids. `OBSERVABLE_INCLUDE(4294967295)` comes out of `stim analyze_errors` as `logical_observable L2147483647` on this host, and `stim sample_dem` on `L4294967295` crashes.

The change is one `ObservableId` type whose domain is `0..=u32::MAX`, used by circuits and detector error models alike, with the range checked where a circuit is parsed or built.
`DemObservableId` is removed.
The divergence is that Stab refuses `OBSERVABLE_INCLUDE` arguments above `4294967295` at parse time where Stim's parser accepts them, and it is recorded in the parity ledger.

## Decided Without A Code Change

### D4. Symlinks and unsafe path components on outputs

Decided on 2026-11-21 as recommended below: the [path trust boundary](../architecture/README.md#path-trust-boundary) is written down, the root `AGENTS.md` points at it, and no command changed.
The section keeps the reasoning.

**Today.** `--out`, `--obs_out`, and the other output flags open the path the caller typed.
No command checks whether a component is `..`, whether the path is a symlink, or whether the target is outside some directory.
Stim behaves the same way.

**Why it was raised.** The root `AGENTS.md` says to reject path traversal, unsafe components, unexpected symlinks, and writes outside intended output roots.

**Why rejecting is the wrong fix here.** That rule protects a program that derives a path from untrusted content, such as an archive entry, a fixture manifest, or a workspace file.
The CLI derives nothing: the caller names the exact path on the command line and already holds the authority to write there with any other tool.
There is no "intended output root" to escape, so a check could only refuse things the caller asked for.
It would also break ordinary use: `/dev/stdout` is a symlink, the plan's item A3 deliberately lets two outputs name the same device, and writing through a symlinked scratch directory is routine on clusters.
A service that runs Stab on behalf of an untrusted user needs a real sandbox, and a path filter inside the CLI would not be one.

**Recommendation: do not reject.**
Write the trust boundary down instead, which `AGENTS.md` allows for ("unless a tighter trust boundary is documented"): command-line paths carry the caller's authority, and the hostile-path rules apply to paths that Stab derives from input, which today means only `ops/` tooling.
The library crates already take readers and writers instead of paths, so an embedding service owns its own path policy.
Size S: one paragraph in the architecture README and one row in the resource policy inventory.
No Stim divergence.

## Larger Redesigns

### R1. Three-way verdict from the flow checks

**Outcome, 2026-11-21.** Done, with one change from the proposal below.
`check_unsigned_stabilizer_flows` is the only flow check and returns one `FlowVerdict` per flow.
Implementing it showed that the checker is complete for valid circuits: every case the old code called unsupported was either an invalid circuit, which pinned Stim also refuses and which is now an error, or a flow with terms on qubits no circuit can address, which is now decided by treating those qubits as idle.
`FlowVerdict` is therefore `Holds` or `Violated(FlowViolation)` with no `Unsupported` variant, and it stays `#[non_exhaustive]`.
A refused budget, a cancellation, and an invalid circuit are errors.
The rest of this section is the proposal as written before the work.

**Today.** `stab-analysis` has eight public flow-check functions in `circuit_flow/checker.rs`: `check_if_circuit_has_unsigned_stabilizer_flows`, `check_unsigned_stabilizer_flows_with_diagnostics`, `circuit_has_unsigned_stabilizer_flow`, `circuit_has_all_unsigned_stabilizer_flows`, and a `_with_options` twin of each.
The plain forms end in `.unwrap_or_else(|_| vec![false; flows.len()])`.
The diagnostic form turns any error, including a budget refusal or a cancellation, into `UnsupportedCircuit { reason: String }`.

**Problem.** A caller who asks "does this circuit have flow F" gets a confident `false` when the memory budget refused the work, when the run was cancelled, or when the circuit is outside the checker's supported subset.
That is a wrong answer, and it comes from the default entry point.
The result type also stores one three-way fact in two fields, `has_flow: bool` and `failure: Option<_>`.

**Proposed design.**

```rust
pub enum FlowVerdict {
    Holds,
    Violated(FlowViolation),
    Unsupported(UnsupportedFlowQuery),
}

pub fn check_unsigned_stabilizer_flows(
    circuit: &Circuit,
    flows: &[Flow],
    options: &UnsignedFlowCheckOptions,
) -> AnalysisResult<Vec<FlowVerdict>>;
```

`FlowViolation` names the mismatch (output, input, record out of range, anticommuting collapse), `UnsupportedFlowQuery` carries a typed reason, and a budget refusal or cancellation is an `Err`.
The eight functions become one, plus a small helper for "all hold".

**Cost.** M. About 77 call sites across 8 files, most of them tests that compare against `true` or `false`.

**Stim.** Stim's `has_flow` returns a bool because its checker is complete for the circuits it accepts.
Stab's unsigned checker covers a subset, which is why "cannot tell" must not read as "no".
The CLI has no command on this path, so only the library surface diverges; it needs a ledger entry as a design choice.

**Recommendation: do first.**
It is the only open item where the current API returns something false.

### R2. "No logical error found" as a search outcome

**Outcome, 2026-11-21.** Done as proposed below, with `SymptomGrowth::{Explore, Skip}` and a matching `UngraphlikeErrors::{Ignore, Reject}` for the graphlike search.
Three statements in the proposal were wrong: the report already held typed facts, no CLI command reaches the searches, and the removed error added an `invalid detector error model: ` prefix that the report does not carry.
One question is left open: a hypergraph search with both maxima at 2 runs the graphlike search, as in Stim, and that can report "not found" where an error within the stated maxima exists, because the graphlike search drops errors that contain a `^` separator.
Stab keeps Stim's behavior and documents it on the options; removing the reroute would be a deliberate divergence that trades Stim parity and speed for a search that honors its stated maxima.
The rest of this section is the proposal as written before the work.

**Today.** `find_undetectable_logical_error(model, usize, usize, bool)` and `shortest_graphlike_undetectable_logical_error(model, bool)`, each with a `_with_limits` twin, return `AnalysisResult<DetectorErrorModel>`.
When the search finishes without finding an error they return `Err(AnalysisError::NoLogicalErrorFound)`, with the reasons (no observables, no detectors, no errors, no graphlike edges) flattened into message text.

**Problem.** A normal outcome of a search is reported as a failure.
The two `usize` arguments have the same type and long negative names (`dont_explore_detection_event_sets_with_size_above`, `dont_explore_edges_with_degree_above`), so they are easy to swap, and the pair `(2, 2)` silently reroutes to the graphlike search.

**Proposed design.**

```rust
pub struct HypergraphSearchOptions {
    pub max_detection_event_set_size: usize,
    pub max_edge_degree: usize,
    pub symptom_growth: SymptomGrowth,          // Explore | SkipIncreasing
    pub limits: LogicalErrorSearchLimits,
}

pub enum LogicalErrorSearch {
    Found(DetectorErrorModel),
    NotFound(NoLogicalErrorReport),
}
```

`NoLogicalErrorReport` holds the four facts as fields and its `Display` renders Stim's exact warning text, so the CLI output does not change.

**Cost.** M. About 90 hits in 12 files.

**Stim.** Stim throws in the "not found" case, and the CLI keeps Stim's stderr text and exit status by rendering the report.
The library divergence is a design choice for the ledger.

**Recommendation: do.**
The Phase D taxonomy already gave "not found" its own error variant, so this is now a contained change.

### R3. `Option` from the gate-data getters

**Outcome, 2026-11-21.** Done with R9 as proposed below.
`gate_tableau`, `gate_flows`, `gate_unitary_matrix`, and `single_qubit_clifford_for_gate` return `Option`, the `gate_has_*` functions and `GateDataUnavailable` are removed, and the decomposition is read from `Gate::h_s_cx_m_r_decomposition()` in the model.
The rest of this section is the proposal as written before the work.

**Today.** `stab-analysis/src/gate.rs` has four pairs: `gate_tableau` and `gate_has_tableau`, `gate_flows` and `gate_has_flows`, `gate_unitary_matrix` and `gate_has_unitary_matrix`, `gate_h_s_cx_m_r_decomposition` and `gate_has_h_s_cx_m_r_decomposition`.
Each getter returns `AnalysisResult` and fails with `GateDataUnavailable` when the gate has no such data.
`single_qubit_clifford_for_gate` returns `StabilizerResult` instead.

**Problem.** "This gate has no tableau" is an ordinary answer about a closed gate table, and a caller who checked `gate_has_tableau` first still has to handle an error that cannot occur.

**Proposed design.** Each getter returns `Option<T>`, the four `gate_has_*` functions are removed, and `gate_decomposition_to_circuit` becomes `GateDecomposition::to_circuit` in `stab-model`.

**Cost.** M. About 100 hits in 15 files, including the engine's reference-sample kernels.

**Dependency.** The getters can only fail for a second reason today: they parse the strings of R9 at run time.
With R9 done, the parse disappears and `Option` is the whole truth; without R9 the `Option` form has to hide an internal parse failure.

**Recommendation: do, together with R9.**

### R4. One function per operation taking an options value

**Today.** `stab-analysis` exports about twenty operations twice, as `f` and as `f_with_options`, `f_with_limits`, or `f_with_budget`, where the plain form is the other with defaults.
The options follow four conventions: by value or by reference, public fields or private fields with `with_*` builders, a budget inside domain parameters, or a bare `MemoryBudget` with no cancellation.
The engine has the same shape in its session constructors: `session`, `session_with_reference_mode`, `session_with_lane_width`, `session_with_budget`, `streaming_run`, `streaming_run_with_budget`, `session_with_limits`, `replay_session_with_limits`, and four free functions with `_with_limits` twins.
Some operations on hostile input have neither a budget nor a cancellation path, for example `solve_for_flow_measurements`.

**Problem.** The surface is doubled, "which knobs does this operation have" is a per-function lookup, the default forms are where errors got swallowed (R1), and a Python binding would have to wrap every twin.

**Proposed design.** One function per operation taking `&XOptions` where `XOptions: Default`.
Every options type has public fields and embeds the same two limit fields, `memory_budget` and `cancellation`, under the same names.
In the engine, `plan.session(&SessionOptions)` and `plan.streaming_run(&SessionOptions, shots)` replace the telescoping constructors, with `SessionOptions { random, reference, lane_width, memory_budget: Option<MemoryBudget> }` where `None` inherits the plan's budget.
The engine free functions that compile a plan and discard it are removed, since the plan already exposes the widths they return.

**Cost.** L in total, but each operation is a small mechanical step: about 20 analysis pairs and about 80 engine hits in 18 files.

**Stim.** None; the CLI flags do not change.

**Recommendation: do, after R1 and R2, and before any Python binding work starts.**
Callers write `&Default::default()` where they called the plain form.

### R5. Engine run types written once

**Today.** The engine defines the same four types three times: `SamplingRunProgress`, `DetectionRunProgress`, `DemSamplingRunProgress`; the three `*RunStatus`; the three `*RunSummary`; and `RunError`, `DetectionRunError`, `DemSamplingRunError`.
Their fields are identical.
`DemReplayBatchStatus` and the decoder's `DecodeBatchStatus` are a fifth and sixth `Completed | Cancelled`.

**Proposed design.** `RunProgress`, `RunStatus`, `RunSummary`, and `RunError<E, SinkError>` once, where `E` is the family's engine error.
No per-family aliases, so there is one name to learn.

**Cost.** M. Mostly renames; the generic `RunError` is the only part with design content.

**Recommendation: do, in the same pass as the engine half of R4.**
It halves what a binding has to wrap.

### R6. Typed gate-argument and target-group views in `stab-model`

**Outcome, 2026-11-21.** Done.
`CircuitInstruction::gate_arguments()` and `gate_targets()` return infallible, allocation-free views with one variant per rule, and `target_groups()` returns a named iterator.
The views yield typed items through small iterator types instead of slices, because arguments are stored as `f64` and targets as packed values.
About 120 impossible branches were removed, far more than the 37 counted below, and every construction path was checked to validate.
What remains is one narrowing step per instruction where code dispatches on `GateKind`, since the relation between a gate kind and its rule is table data; R9 can remove some of those.
A diagnostic timing of nine `analyze_errors`, `sample`, and `detect` runs showed no measurable difference, and their outputs were byte-identical.
The rest of this section is the proposal as written before the work.

**Today.** `CircuitInstruction` exposes `probability_argument() -> ModelResult<Option<Probability>>`, `probability_arguments() -> ModelResult<Option<Vec<Probability>>>`, `observable_id_argument() -> ModelResult<Option<ObservableId>>`, and `target_groups() -> Vec<&[Target]>`.
The instruction was validated against its gate's rules when it was built, so the `Err` arms cannot occur.
`probability_arguments` and `target_groups` allocate on every call, and `target_groups` is called from lowering and analysis loops.

**Problem.** Downstream code re-checks what the model already guarantees.
`stab-analysis` contains about 37 branches that return errors such as "target is not a qubit", "expected paired targets", or "is not a measurement record" for shapes a validated circuit cannot have.
AGENTS.md asks for impossible states to be removed by construction.

**Proposed design.** Two matchable views on `CircuitInstruction`, each keyed by the gate's existing rule:

```rust
pub enum GateArguments<'a> {
    None,
    Probability(Probability),
    OptionalProbability(Option<Probability>),
    Probabilities(&'a [Probability]),
    Observable(ObservableId),
    Coordinates(&'a [f64]),
    Exact(&'a [f64]),
}

pub enum GateTargets<'a> {
    Qubits(&'a [QubitTarget]),
    QubitPairs(QubitPairs<'a>),
    PauliProducts(PauliProducts<'a>),
    Records(&'a [RecordTarget]),
    // ...one variant per target rule
}
```

`target_groups()` returns a named `ExactSizeIterator` instead of a `Vec`.
`DemInstruction::view()` from Phase D is the model for this.

**Cost.** L. The model side is M; removing the impossible branches in analysis and engine is another M, and it can follow in pieces.

**Performance.** Removing a per-instruction allocation from lowering loops may be measurable; it has to be measured before anything is claimed.

**Recommendation: do.**
It removes the most code downstream of any model item, and R9 builds on it.

### R7. Nested option types for `ErrorAnalyzerOptions`

**Today.** Five independent booleans: `fold_loops`, `decompose_errors`, `allow_gauge_detectors`, `ignore_decomposition_failures`, and `block_decomposition_from_introducing_remnant_edges`.
The last two mean nothing unless `decompose_errors` is set, and the type does not say so.

**Proposed design.**

```rust
pub struct ErrorAnalyzerOptions {
    pub loops: LoopFolding,                          // Unroll | Fold
    pub decomposition: Option<ErrorDecomposition>,   // None: do not decompose
    pub gauge_detectors: GaugeDetectors,             // Reject | Allow
    pub approximate_disjoint_errors: Option<Probability>,
    pub memory_budget: MemoryBudget,
    pub cancellation: Cancellation,
}

pub struct ErrorDecomposition {
    pub on_failure: DecompositionFailure,            // Fail | Ignore
    pub remnant_edges: RemnantEdges,                 // Allow | Block
}
```

**Cost.** M and wide: 277 hits in 43 files, nearly all struct literals in tests.

**Stim.** Stim accepts `--ignore_decomposition_failures` without `--decompose_errors` and ignores it.
The CLI mapping keeps that, checked against the oracle.

**Recommendation: do, in the same pass as R4**, so those 43 files are edited once.
On its own it is the lowest-value redesign here: the meaningless combinations are harmless today.

### R8. The `Vec<Vec<bool>>` record codec family

**Today.** `stab-records/src/result_formats.rs` exports `write_records`, `read_records`, `read_measurement_records`, `write_ptb64_records_checked`, `read_ptb64_records`, `read_ptb64_records_all`, `ptb64_record_count`, and `MeasureRecordWriter`.
They use one byte per bit, take no memory budget, and `read_ptb64_records` allocates `bits_per_record × max_shots` booleans from caller integers before it looks at the input.
Outside `stab-records` they are used by three CLI test files and one benchmark data helper in `ops/bench`.

**Problem.** It is an unbounded decode path over hostile bytes that runs beside the bounded `RecordStreamReader` and `RecordEncoder`, which every product route uses.
A library user who reaches for the obvious-looking `read_records` gets the unbudgeted one.

**Proposed design.** Take the family off the public surface and keep it as the reference codec for tests, inside `stab-records` under `#[cfg(test)]` or in `test-support`.
If a one-call convenience is wanted, add `RecordFormat::decode_all(input, layout, budget) -> RecordResult<PackedShotBatch>` and an `encode_all` on top of the streaming codecs.
`MeasureRecordWriter` mirrors Stim's writer contract and can stay as a documented, bit-at-a-time writer if that mirror is still wanted.

**Cost.** M. The work is moving tests, not changing behaviour.

**Recommendation: remove from the public surface.**
Budgeting it would mean maintaining two codec implementations to the same standard for no product caller.

### R9. Typed gate flow and decomposition metadata

**Outcome, 2026-11-21.** Done.
Flows are `GateFlowDescriptor` values with signed dense Pauli strings and typed record offsets, and a decomposition is a list of `DecompositionStep::{H, S, Cx, M, R}`, so an odd-length `CX` or an unknown gate name cannot be written.
Two-qubit tableaux are a `TwoQubitClifford` table whose entries are checked to be Cliffords at compile time.
A model test renders every typed flow and decomposition and compares it with the vendored Stim gate data, and the analysis owner tests compare all tableaux and flows with the same data.
No product crate parses gate text at run time; `Tableau::gate1` and `gate2` remain for caller-supplied text.
Timings and outputs of `analyze_errors`, `sample`, and `detect` were unchanged.
The rest of this section is the proposal as written before the work.

**Today.** `stab-model` publishes each gate's flows as `&'static [&'static str]` and its decomposition as `.stim` text in a `GateDecomposition`.
`stab-analysis` parses the flow strings back with `Flow::from_str` and `Tableau::gate2(&str, &str, &str, &str)`, and the engine parses the decomposition text again in two lowering sites.

**Problem.** These are stringly typed domain values inside the product, and each consumer owns a parse step whose errors cannot happen.
Those unreachable errors are what keeps the getters of R3 fallible.

**Proposed design.** Plain-data descriptors in `stab-model`, because the model cannot name algebra types:

```rust
pub struct GateFlowDescriptor {
    pub input: &'static [PauliTerm],
    pub output: &'static [PauliTerm],
    pub measurements: &'static [i32],
}

pub struct DecompositionStep {
    pub gate: GateKind,
    pub targets: &'static [u8],
}
```

The string forms stay as `Display` output and as a test oracle that checks the tables against Stim's text.

**Cost.** M to L. The tables are large and must be generated or transcribed carefully; a test that renders them back to Stim's strings protects the transcription.

**Recommendation: do after R6**, and land R3 with it.

### R10. Trim `stab_model::advanced`

**Outcome, 2026-11-21.** Done: the module went from 57 exports to 27.
Seventeen items were deleted or made crate-private and thirteen became inherent methods or associated items, as the migration guide lists.
`STIM_TARGET_VALUE_LIMIT` was not dead, since the flow checker reads it, and is now `QubitId::MAX`.
Left as proposals: splitting what remains into `traversal`, `tagged`, `gate_tables`, and `admission` submodules, and giving the two free wrappers in `stab_algebra::advanced` the same treatment.
The rest of this section is the proposal as written before the work.

**Today.** `stab_model::advanced` exports about sixteen items that have no caller outside `stab-model`: the `plain_*_gate` helpers, `lookup_gate`, `lookup_simple_plain_gate`, three `measure_record_offset_*` functions, `bounded_parse_diagnostic_text`, `parse_error_with_human_message`, `invalid_utf8_parse_error`, two hidden retained-bytes functions, and `STIM_TARGET_VALUE_LIMIT`.
Several live items are free functions that wrap a method, such as `advanced::circuit_simulated_qubit_count(&circuit)`, used in 13 files.

**Problem.** The public surface is padded with internals, and the live wrappers break the project's own rule that a type method is the natural home.

**Proposed design.** Delete the dead items.
Turn the live wrappers into inherent methods, `Circuit::simulated_qubit_count()` and `CircuitInstruction::measurement_result_count()`.
`advanced` keeps only operations that rely on an invariant the caller has already established: `CircuitBuilder`, the folded traversal, and the tag-byte constructors.

**Cost.** S for the deletions, M with the renames.

**Recommendation: do now.**
The exact list should be re-derived with `rg` at the time, since D1 and D3 move items.

## Smaller Items

### S1. `--fold_loops` circuits with a long transient

`analyze_errors --fold_loops` is 2.6 to 4.8 times slower than pinned Stim (shared-host diagnostic) on circuits whose loop takes many iterations to become periodic.
Successful output is identical; the time goes to the sparse reverse tracker's set operations during the unfolded iterations.
This is the one known place where Stab is slower than Stim by more than a small factor, and the first tier asks for comparable performance.

Recommendation: do it as its own performance task.
Profile first, since `perf` is unavailable on this host and the hot operation has only been located by elimination.
The likely candidates are the per-qubit sorted-set XOR and its allocation pattern.
Add a benchmark-suite row before making any claim.

### S2. `ConversionOperation` is 40 bytes

The detection conversion plan's operation enum is 40 bytes because the repeat variant carries its fields inline.
Moving the repeat fields to a side table would bring it to 24 bytes, about 16 MB per million detectors.

Recommendation: try it, and keep it only if `m2d` does not slow down.
The Phase D arena showed that an extra indirection in this loop costs about 3 percent until the lookup is hoisted, so measure before and after.

### S3. `circuit_reference_signs` computes a byte per bit before packing

The public result is packed since Phase D, but the computation still fills one byte per sign and packs at the end, so its transient peak is eight times the result.

Recommendation: do it when that module is next touched; it is not worth a change of its own.

### S4. A public `Tableau::to_unitary`

`Tableau::from_unitary` is public and fast after Phase D.
The reverse direction exists only as a test helper that is too slow to publish.
Stim offers both directions.

Recommendation: defer until the library conversion surface is scheduled, and then build it from the stabilizer state vector instead of promoting the test helper.
No CLI command needs it.

### S5. Two fixed caps in `stab-model`

Fixed limits in `stab-model` reported `invalid detector error model` when they refused, so a caller could not tell a malformed model from a limit.

Done with D2: three caps (traversal repeat depth, repeat expansion, and coordinate-query candidates) are typed fixed limits, and the model's refusal no longer needs a parse position.

## Not Reviewed

### N1. Areas the review did not read

The 2026-11-19 review did not read `ops/` outside part of `ops/release`, the detecting-regions, missing-detector, and DEM-search parts of `stab-analysis`, or `circuit/parser.rs` beyond black-box fuzzing.
Phase D touched some of that code (the search error variants, the parser diagnostics split) without reviewing it.

Recommendation: run a second review pass over those areas before the next release.
The DEM search and the circuit parser both take hostile input directly, so they come first; `ops/` is operator tooling and can come last.
