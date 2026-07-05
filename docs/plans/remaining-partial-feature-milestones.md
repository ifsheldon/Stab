# Remaining Partial Feature Milestones

## Summary

This historical plan covers every feature row marked `Partial` in `docs/stab-feature-checklist.md` whose remaining work is not intentionally deferred.
It turned the checklist into executable RPF milestones with owned tests, oracle evidence, benchmark rows, acceptance criteria, and explicit exclusions.

The current execution plan is now `docs/plans/non-deferred-partial-feature-milestones.md`.
Keep this document as historical RPF source material and update it only when old RPF references would otherwise mislead an implementation agent.

Use `docs/plans/lessons-learned.md` while executing every milestone.
The most important lesson for this plan is that a milestone is not actionable until it names exact subcases, executable comparators, resource behavior, benchmark class, and deferred edges.

Each milestone is a work packet for implementation agents.
A work packet is complete only when it has behavior changes or explicit rejections, targeted tests, oracle or benchmark metadata where relevant, documentation updates, a milestone progress or completion report, milestone-audit closure, and full-code-review closure.
Rows that are already substantially implemented still need this same closure pass before their checklist status changes because the evidence must prove the exact non-deferred surface, not merely show that some nearby implementation exists.

## Scope Rules

Included:

- Rust core APIs, transforms, analysis paths, search paths, DEM paths, result-format paths, and CLI behavior that already exist in partial form and are not explicitly future work.
- New tests, oracle rows, benchmark rows, profiler notes, reports, and checklist updates needed to prove those features.
- Explicit rejection behavior for unsupported shapes that remain outside the milestone.

Excluded:

- Python bindings and Python API ergonomics.
- JavaScript/WASM.
- Diagrams and visualization.
- `stim explain_errors` CLI.
- `stim repl`.
- QASM, Quirk, Crumble, and ecosystem integrations.
- GPU backends.
- Exact random-stream parity.
- C++ header compatibility.
- New public graph simulator, vector simulator, `TableauSimulator`, or `FlipSimulator` products.
- Deprecated `--detector_hypergraph` support.

If implementation reveals that an active row actually requires an excluded surface, stop and log the under-specification in `docs/plans/milestone-spec-gaps.md` instead of widening scope silently.

## Execution Order And Milestone Boundaries

Recommended order:

1. Finish or refresh RPF0 before starting new feature work so every `Partial` checklist row has a locked owner milestone, comparator class, oracle status, benchmark status, and explicit exclusion list.
2. Work on RPF1 through RPF7 in dependency order when possible: gate metadata informs execution contracts, circuit transforms inform feedback and flow work, sweep semantics inform CLI `m2d`, DEM traversal informs analyzer and search work, and flow utilities inform time-reversal and decomposition checks.
3. Use RPF8 only after one or more implementation milestones have fresh evidence that can be audited, benchmarked, documented, and reflected in rollup checklist rows.

Milestone boundaries:

- RPF1 owns gate metadata and gate execution support contracts, but not public simulator products or full circuit decomposition semantics.
- RPF2 owns circuit transforms and feedback-inlining transforms, but not QASM, Quirk, Crumble, diagrams, or Python ergonomics.
- RPF3 owns sweep-conditioned execution and legal-gate execution gaps in existing sampler, converter, detector, and analyzer surfaces, but not exact random-stream parity or public simulator APIs.
- RPF4 owns DEM Rust APIs, transforms, coordinates, counts, and folded traversal for selected consumers, but not DEM diagrams or Python class shape.
- RPF5 owns detector utility APIs and measurement-rich flows, including the flow semantics needed by RPF2 transforms.
- RPF6 owns analyzer, search, sparse reverse tracking, and active matched-error value-object hardening, but not full ErrorMatcher provenance or `stim explain_errors`.
- RPF7 owns visible CLI parity for `m2d`, `analyze_errors`, and accepted legacy dispatch, while keeping `--detector_hypergraph` excluded.
- RPF8 owns benchmark-gate, audit, review, and documentation closure for completed milestone slices.

## Required Milestone Packet

Every RPF1 through RPF7 implementation slice must produce the following packet before it can be marked complete:

- Scope note: exact subcases implemented, exact subcases rejected, exact subcases deferred, and upstream Stim files used only as semantic sources.
- Tests: targeted Rust or CLI tests for positive behavior, negative behavior, malformed inputs, resource boundaries, and compatibility-sensitive edge cases.
- Oracle evidence: exact, structural, statistical, semantic, or manifest-only rows updated to match the implemented surface.
- Benchmark evidence: source-owned benchmark rows for performance-sensitive work, including measurement work units, compare notes, runner coverage, and explicit primary-gate or report-only classification.
- Documentation: updates to `docs/stab-feature-checklist.md`, this plan or a milestone report, and any roadmap, README, CLI, oracle, or benchmark docs touched by the behavior.
- Closure: milestone-audit findings resolved or logged as under-specification, then full-code-review findings resolved or logged if they are outside the milestone.

If any packet item cannot be completed, leave the checklist row `Partial` and document why.

## Partial Row Coverage Matrix

| Checklist row or row group | Disposition | Owner milestone | Completion rule |
| --- | --- | --- | --- |
| Rust core library equivalent for core Stim semantics | Rollup | RPF8 | Complete only after RPF1 through RPF6 close active Rust rows and docs no longer imply Python or simulator product parity. |
| CLI binary | Rollup | RPF7 and RPF8 | Complete only after active `m2d`, `analyze_errors`, and accepted legacy alias behavior is proven. |
| `.stim`, `.dem`, and result-format compatibility | Rollup | RPF2, RPF4, RPF7, RPF8 | Complete only after active transform, DEM, and CLI format gaps are closed or explicitly deferred. |
| Target kinds | Active | RPF3 | Broader sweep-conditioned execution and analysis behavior must be implemented or explicitly rejected with tests. |
| Full semantic execution of every legal circuit operation | Active rollup | RPF1, RPF3, RPF6 | Parser acceptance must not imply execution support; execution coverage and unsupported legal shapes must be documented. |
| DEM parser and canonical printer | Active | RPF4 | Parser/printer rows close only after DEM API transform and folded traversal limits are resolved for non-deferred paths. |
| DEM detector shifts, observables, coordinates, and counts | Active | RPF4 | Finish folded coordinate/count behavior and large-repeat resource policy. |
| DEM flattening and large repeat traversal | Active | RPF4 and RPF6 | Add public transform APIs and folded traversal for selected consumers, or documented caps with tests. |
| Gate validation flags and categories | Implemented for current Rust metadata surface | RPF0/PF1 | `pf1-gate-metadata-api` now provides executable closure evidence; Python `GateData` shape stays deferred and execution support remains separately tracked. |
| Gate semantic execution | Active | RPF3 and RPF6 | Fill accepted legal-gate execution gaps in sampler, detection, converter, and analyzer paths, or reject unsupported shapes precisely. |
| Programmatic mutation | Implemented for current Rust API surface | RPF0/PF1 | `pf1-circuit-rust-api` now provides executable closure evidence; remaining Python operator ergonomics stay deferred. |
| Core introspection | Implemented for current Rust API surface | RPF0/PF1 | `pf1-circuit-rust-api` now provides executable closure evidence; remaining Python-style indexing and property parity stay deferred. |
| Circuit coordinate queries | Implemented for current Rust API surface | RPF0/PF1 | `pf1-circuit-rust-api` now provides executable closure evidence; exact Python API shape and exact C++ infinity behavior stay deferred or logged. |
| Repeat handling | Active | RPF2, RPF4, RPF6 | Complete folded traversal or caps across transforms, DEM consumers, analyzer, and search. |
| Circuit transforms | Active | RPF2 and RPF5 | Finish `flattened`, `decomposed`, `without_noise`, feedback inlining, time reversal for flows, and measurement-rich flow transforms. |
| Reference samples and determined measurements | Implemented for current Rust API surface | RPF0/PF1 | `pf1-circuit-rust-api` now provides executable closure evidence; remaining Python bit-packed return shapes and Python API parity stay deferred. |
| DEM construction and mutation | Implemented for current Rust API surface | RPF0/PF1 | `pf1-dem-rust-api` now provides executable closure evidence; Python-style list operations, operators, and exact Python API shape stay deferred. |
| DEM introspection | Active | RPF4 | Finish folded large-repeat traversal and resource behavior across every public DEM query selected for Rust scope. |
| DEM transforms | Active | RPF4 | Finish public `flattened`, `rounded`, and transform resource boundaries. |
| DEM analysis and shortest graphlike error | Active | RPF4 and RPF6 | Finish folded traversal and generated-circuit evidence for graphlike, hypergraph, SAT, sampler-adjacent, and analyzer-adjacent consumers. |
| Measurement-to-detection conversion | Active | RPF2, RPF3, RPF7 | Finish feedback transform semantics, broader sweep-conditioned conversion, and visible CLI parity. |
| Detector-analysis utility APIs | Active | RPF5 | Finish promoted detecting-region, missing-detector, feedback, and flow utility subcases. |
| Single-shot interactive tableau simulator | Deferred-only | None | Public simulator product remains out of scope. |
| Batched flip-frame simulator | Deferred-only | None | Public simulator product remains out of scope. |
| Circuit-to-DEM analysis | Active | RPF6 and RPF7 | Finish analyzer subcases, generated-circuit evidence, loop folding, gauge behavior, and CLI proof. |
| `analyze_errors --decompose_errors` and related flags | Active | RPF6 and RPF7 | Finish core and CLI analyzer parity for selected decomposition and flag behavior. |
| Error explanation value objects | Mixed | RPF6 | Harden only value-object behavior required by active analyzer/search paths; full ErrorMatcher provenance and CLI remain deferred. |
| Shortest graphlike and hypergraph logical-error search | Active | RPF6 | Finish generated-circuit search, ordering-insensitive structural comparators, and resource behavior. |
| Sparse reverse detector-frame tracking | Active | RPF6 | Finish optimized loop folding and analyzer/search integration where needed; deterministic generated supported-unitary repeat coverage is implemented for the current promoted subset. |
| Flows | Active | RPF1 and RPF5 | Finish measurement-rich flows, variable-target flow metadata decisions, and transform integration. |
| `stim m2d` | Implemented for selected PF7 CLI surface | RPF7 | The selected CLI closure is implemented by `pf7-m2d-cli-parity`; only newly selected CLI cases should reopen this row. Broader detector-converter API semantics remain active under measurement-to-detection conversion rows. |
| `stim analyze_errors` | Implemented for selected PF7 CLI surface | RPF7 | The selected CLI closure is implemented by `pf7-analyze-errors-cli-parity`; only newly selected CLI cases should reopen this row. Broader analyzer semantics remain active under core analyzer rows. |
| Legacy top-level command flags | Implemented for selected PF7 CLI surface | RPF7 | The selected legacy-dispatch closure is implemented by `pf7-legacy-dispatch-parity`; only newly selected legacy spellings or failure modes should reopen this row. |
| Highest-priority remaining feature gaps | Rollup | RPF8 | Complete only after the named child milestones reach their acceptance criteria. |

`Generated API docs` and `Generated API reference or machine-readable feature matrix` are `Missing`, not `Partial`.
They are useful follow-up work but are not part of this partial-feature milestone plan.

## Milestone RPF0: Inventory And Comparator Lock

Objective: freeze the exact owned subcases before more implementation work starts.

Owned checklist rows:

- Every `Partial` row in `docs/stab-feature-checklist.md`.
- Every row that currently looks partial only because of deferred Python, JS, diagram, ecosystem, simulator product, or generated-doc gaps.

Tasks:

- Re-read `docs/stab-feature-checklist.md`, `docs/stim-feature-list.md`, `docs/plans/stim-test-porting-plan.md`, `docs/plans/lessons-learned.md`, `docs/plans/milestone-spec-gaps.md`, and the matching pinned Stim v1.16.0 source files under `vendor/stim`.
- Update `docs/plans/partial-feature-inventory.md` so every active row has an owner milestone, owner crate or CLI surface, upstream subcase list, comparator class, oracle row status, benchmark row status, and explicit exclusion list.
- Split upstream test files into owned, semantic-mining-only, deferred, and out-of-scope subcases.
- Add or update manifest-only oracle rows for every active owned subcase that does not yet have executable evidence.
- Add or update non-primary benchmark placeholders for every milestone that claims performance-sensitive behavior.
- Update `docs/stab-feature-checklist.md` if a row is partial only because of explicitly deferred work and should say that clearly.
- Log any unresolved scope question in `docs/plans/milestone-spec-gaps.md`.

Tests:

- `cargo test -p stab-oracle fixtures --quiet`
- `just oracle::list`
- `just oracle::matrix --check`
- `just bench::list`

Benchmarks:

- No timing run is required.
- Benchmark metadata must parse and list placeholder rows without adding them to the primary gate.

Acceptance criteria:

- No active milestone uses a whole upstream file as its acceptance criterion.
- Every active row has at least one executable test plan or a prerequisite comparator implementation task.
- Every deferred-only row is named with a deferral reason.
- Rollup rows have child-owner rules and no direct implementation task.

## Milestone RPF1: Gate Metadata And Gate Execution Contract

Objective: keep parser acceptance versus execution support explicit after the current Rust gate metadata surface has closed.

Owned checklist rows:

- Gate semantic execution.
- Full semantic execution of every legal circuit operation, for the gate-table and metadata portions.
- Flows, for execution and transform integration beyond gate-level metadata.

Implementation tasks:

- Treat current Rust gate metadata accessors, unsupported-accessor errors, and metadata-column support-contract synchronization as closed by `pf1-gate-metadata-api`.
- Keep the resolved decision that measurement-rich and variable-target gate flow metadata belongs in `Gate::flows` for Stim v1.16.0 `GateData.flows` shapes, while execution support remains tracked separately.
- Keep `docs/plans/rpf1-gate-execution-support-contract.md`, `docs/stab-feature-checklist.md`, and `docs/plans/pf1-gate-metadata-progress-report.md` synchronized whenever execution support changes.

Tests:

- Port owned metadata cases from `vendor/stim/src/stim/gates/gates.test.cc`, `vendor/stim/src/stim/gates/gates_test.py`, and `vendor/stim/src/stim/gates/gate_data_*.cc`.
- Add exact tests for representative decomposition strings and full support-set tests for every gate with decomposition metadata.
- Add semantic tests proving supported decomposition metadata parses and matches tableau or flow behavior where those comparators are valid.
- Add tests proving `has_*` helpers and fallible accessors agree for tableau, unitary, flow, and decomposition metadata.
- Add negative tests for measurement-rich gates, variable-target gates, annotation gates, noisy gates, and unsupported metadata shapes.
- Add contract tests for the canonical gate execution support table so parser support cannot drift from execution documentation.

Oracle rows:

- Keep `pf1-gate-metadata-api` current if public metadata API names or behavior change.
- Keep rows for gate execution support structural unless there is a faithful pinned Stim CLI or exact-output comparator.

Benchmarks:

- Extend `pf1-gate-metadata-lookup` with decomposition and any newly implemented flow metadata submeasurements.
- Keep `pf3-gate-semantic-wide` current when gate execution support changes.
- Keep these rows report-only unless a faithful direct Stim baseline and stable repeated evidence exist.

Acceptance criteria:

- Every canonical gate has explicit metadata support or explicit fail-closed behavior.
- Decomposition metadata does not imply full circuit decomposition unless RPF2 implements and tests it.
- Gate execution support is documented separately from parser acceptance.
- Benchmark rows have measurement work units, compare notes, and no primary gate promotion without repeated stable evidence.

## Milestone RPF2: Circuit Transforms And Repeat Traversal

Objective: finish active transform APIs for circuits while preserving explicit deferrals for exports, diagrams, and Python ergonomics.

Owned checklist rows:

- Repeat handling.
- Circuit transforms.
- Full circuit transform API parity, except QASM, Quirk, Crumble, diagrams, and Python ergonomics.
- Measurement-to-detection conversion, for feedback-inlining prerequisites.
- Full feedback-inlining transform parity.

Implementation tasks:

- Implement public Rust `flattened` and flattened-operation traversal for repeat blocks, tags, annotations, coordinate shifts, detectors, observables, and measurement references. The Rust `Circuit::flattened` and `Circuit::flattened_operations` subset is implemented with tests, oracle metadata, benchmarks, and progress evidence in `docs/plans/rpf2-circuit-transform-progress-report.md`.
- Implement `without_noise` while preserving deterministic operations, coordinates, ticks, detectors, observables, and measurement-record semantics. The Rust `Circuit::without_noise` subset is implemented with tests, oracle metadata, benchmarks, and progress evidence in `docs/plans/rpf2-circuit-transform-progress-report.md`.
- Implement full or explicitly scoped `decomposed` behavior for compound gates, pair measurements, MPP, SPP, target grouping, and base-gate lowering. Rust `Circuit::decomposed` now covers public ISWAP, MPP, SPP, pair-measurement, tag-preservation, noise-preservation, annotation-preservation, constant-MPP, and anti-Hermitian rejection cases; flow-dependent semantic checks remain open until RPF5.
- Extend `circuit_with_inlined_feedback` into the selected public feedback-inlining surface, including repeat-block behavior or precise rejection. The scoped Rust `Circuit::with_inlined_feedback` API is implemented for the current top-level Pauli and MPP feedback subset, with explicit rejection for repeat blocks and unsupported classical controlled gates; full loop-refolding parity remains open.
- Implement `time_reversed_for_flows` only after RPF5 defines required flow semantics.
- Prefer folded traversal over full expansion for large repeats; when expansion remains necessary, add a documented cap and rejection tests.

Tests:

- Port owned cases from `vendor/stim/src/stim/circuit/circuit.test.cc`, `vendor/stim/src/stim/circuit/gate_decomposition.test.cc`, `vendor/stim/src/stim/util_top/transform_without_feedback.test.cc`, `vendor/stim/src/stim/util_top/circuit_flow_generators.test.cc`, and `vendor/stim/src/stim/util_top/has_flow.test.cc`.
- Add exact canonical-output tests for `flattened`, `without_noise`, supported `decomposed`, and feedback-inlining outputs.
- Add semantic tests comparing tableau action, sampling distributions, detector error models, or flow satisfaction before and after transforms.
- Add negative tests for unsupported repeat refolding, unsupported feedback controls, unsupported decomposition target shapes, invalid measurement-record rewrites, and excessive expansion.
- Add resource-boundary tests for nested large repeats.

Oracle rows:

- Replace or supplement `pf2-circuit-flatten-without-noise`, `pf2-circuit-decomposed`, and `pf2-feedback-time-reverse`. The broad feedback row is now supplemented by `pf2-feedback-inline-scoped-rust`.
- Exact-output rows should cite pinned Stim examples when canonical text is stable.
- Structural rows should state why exact text is not a faithful comparator.

Benchmarks:

- Add or implement `pf2-circuit-flatten-repeat`, `pf2-circuit-without-noise`, `pf2-circuit-decompose-mpp-spp`, `pf2-feedback-inline-batch`, and `pf2-time-reverse-flow`. The feedback-inline batch row now has report-only runner coverage for the scoped method.
- Use submeasurement thresholds when one row bundles flattening, decomposition, and feedback work.
- Promote only faithful direct or CLI-comparable rows with stable repeated evidence.

Acceptance criteria:

- Each transform either matches pinned Stim v1.16.0 on owned cases or rejects unowned cases with a precise domain error.
- Measurement, detector, observable, sweep, and coordinate references are proven correct by tests.
- Large-repeat behavior is folded, capped, or explicitly rejected with tests.

## Milestone RPF3: Sweep-Conditioned Execution And Legal Gate Semantics

Objective: close active sweep-conditioned behavior and legal gate execution gaps in sampler, detector conversion, detection sampling, and analyzer paths.

Progress: non-frame `detect` sampling with omitted all-false sweep bits is implemented and tracked in [rpf3-sweep-gate-progress-report.md](rpf3-sweep-gate-progress-report.md). Frame-path sweep sampling, analyzer sweep behavior, and broad gate execution classification remain active.

Owned checklist rows:

- Target kinds.
- Full semantic execution of every legal circuit operation.
- Gate semantic execution.
- Measurement-to-detection conversion, for sweep-conditioned conversion and default behavior.
- Broader sweep-conditioned simulator and analysis parity.

Implementation tasks:

- Extend sweep-conditioned semantics beyond the current detector-conversion subset only for exact subcases selected by RPF0.
- Add sweep-aware behavior for `detect` and analyzer paths when selected as active Rust or CLI scope.
- Add legal-gate execution for parser-accepted gates in sampler, converter, detection, and analyzer paths where Stim semantics are non-deferred.
- Maintain explicit rejection for sweep target shapes, gate families, or mixed feedback/sweep cases not selected by the milestone.
- Keep streaming or bounded behavior for all public inputs and outputs.

Tests:

- Port owned cases from `vendor/stim/src/stim/simulators/measurements_to_detection_events.test.cc`, `vendor/stim/src/stim/simulators/frame_simulator.test.cc`, `vendor/stim/src/stim/simulators/error_analyzer.test.cc`, `vendor/stim/src/stim/cmd/command_detect.test.cc`, and `vendor/stim/src/stim/cmd/command_m2d.test.cc`.
- Add sweep record tests for `01`, `b8`, `r8`, `hits`, `dets`, and `ptb64` wherever accepted.
- Add semantic tests comparing sweep-conditioned circuits to explicit small-circuit expansions.
- Add tests for omitted sweep defaults, width mismatch, invalid sweep record counts, unsupported formats, unsupported sweep target shapes, and writer errors.
- Add gate execution tests that separate parser validation, sampler execution, detector conversion, and analyzer propagation.

Oracle rows:

- Supplement `pf3-sweep-m2d-detect`, `pf3-sweep-analyzer`, and `pf3-gate-semantic-execution` with executable rows for selected subcases.
- Exact CLI rows should prove stdout, stderr class, exit status, accepted flags, rejected flags, and path handling.

Benchmarks:

- Implement `pf3-m2d-sweep-b8`, `pf3-m2d-sweep-ptb64-input`, `pf3-detect-sweep-sampling`, and `pf3-analyze-errors-sweep` when their corresponding behavior is active, and keep `pf3-gate-semantic-wide` current as the fixed-tableau gate execution contract expands.
- Classify CLI rows as `cli-baseline` when Stim v1.16.0 exposes the same public command shape.
- Keep core-only rows `contract-representative` or `report-only` unless a faithful direct baseline exists.

Acceptance criteria:

- Sweep-conditioned behavior is tested across accepted formats and rejected precisely for unsupported shapes.
- Parser acceptance never implies untested execution support.
- Public paths stream or enforce documented caps.

## Milestone RPF4: DEM API, Transforms, And Folded Traversal

Objective: finish non-deferred DEM Rust API gaps and remove avoidable large-repeat expansion limits from DEM operations where practical.

Owned checklist rows:

- DEM parser and canonical printer.
- DEM detector shifts, observables, coordinates, and counts.
- DEM flattening and large repeat traversal.
- DEM introspection.
- DEM transforms.
- Full DEM public API parity, except diagrams, Python ergonomics, and the already closed current Rust construction and mutation helper subset.

Implementation tasks:

- Implement public materialized `flattened` for selected DEM cases with tags, separators, detector shifts, coordinate shifts, logical observables, and repeats. The current Rust `DetectorErrorModel::flattened` subset is implemented with tests, oracle metadata, benchmark metadata, and progress evidence in `docs/plans/rpf4-dem-transform-progress-report.md`.
- Implement public `rounded` for probability rounding with explicit numerical behavior. The current Rust `DetectorErrorModel::rounded` subset is implemented with tests, oracle metadata, benchmark metadata, and progress evidence in `docs/plans/rpf4-dem-transform-progress-report.md`.
- Treat the current Rust construction and mutation helper subset as closed by `pf1-dem-rust-api`; add copy, concat, repetition, or mutation helpers only if a later plan extracts a concrete non-Python Rust API gap.
- Finish folded traversal for coordinate maps where current APIs still require caps or do not prove nested or non-flat ambiguous overlapping selected-coordinate lookup through very large repeats. The current all-detector coordinate-map cap, folded non-overlapping selected-query lookup, flat sparse-overlap selected-query fast path, valid flat sparse-hole behavior, and many-selected flat-overlap scan are implemented with tests, oracle metadata, benchmark metadata, and progress evidence in `docs/plans/rpf4-dem-coordinate-progress-report.md`; final shifts, final detector shifts, counts, recursive `without_tags`, and selected coordinates through shifted repeats have PF4 query evidence in `docs/plans/rpf4-dem-transform-progress-report.md`.
- Add folded or capped traversal behavior for graphlike, hypergraph, SAT, matcher-adjacent, sampler-adjacent, and analyzer-adjacent DEM consumers where owned by RPF4. The current capped graphlike, hypergraph, SAT, analyzer, ErrorMatcher, and DEM sampler repeat subsets are implemented with tests, oracle metadata, benchmark metadata, and progress evidence in `docs/plans/rpf4-dem-search-sat-progress-report.md` and `docs/plans/rpf4-dem-sampler-progress-report.md`.
- Preserve decomposition separators and tags according to the transform contract.

Tests:

- Port owned cases from `vendor/stim/src/stim/dem/detector_error_model.test.cc`, `vendor/stim/src/stim/dem/dem_instruction.test.cc`, and Python DEM tests as semantic-mining sources.
- Add exact canonical-output tests for `flattened`, `rounded`, `without_tags`, tags, separators, coordinate shifts, detector shifts, and repeats.
- Add structural tests for all-detector coordinate maps, selected-detector coordinate maps, final coordinate shifts, final detector shifts, and error counts.
- Add resource-boundary tests for huge repeats, nested repeats, high detector shifts, high observable counts, malformed DEM text, and unsafe transform expansion. The current malformed DEM, high-id, detector-shift overflow, and invalid constructor subset is implemented with tests, oracle metadata, and progress evidence in `docs/plans/rpf4-dem-transform-progress-report.md`.
- Add negative tests for invalid probabilities, invalid separator use, invalid coordinate values, and unsupported transform shapes. The current invalid probability, separator, target, repeat-count, tag, programmatic non-finite coordinate, and repeat-block instruction-only range subset is implemented with tests, oracle metadata, and progress evidence in `docs/plans/rpf4-dem-transform-progress-report.md`.

Oracle rows:

- Supplement `pf4-dem-introspection-transforms`, `pf4-dem-coordinate-api`, and `pf4-dem-folded-traversal`. The materialized transform subset is supplemented by `pf4-dem-materialized-transforms-rust`; the introspection query subset is supplemented by `pf4-dem-introspection-query-rust`; the validation negative subset is supplemented by `pf4-dem-validation-negative-rust`; the coordinate resource subset is supplemented by `pf4-dem-coordinate-resource-rust`; the search and SAT repeat-resource subset is supplemented by `pf4-dem-search-sat-repeat-resource-rust`; the analyzer and matcher repeat-resource subset is supplemented by `pf4-dem-analyzer-matcher-repeat-resource-rust`; the sampler repeat-resource subset is supplemented by `pf4-dem-sampler-repeat-resource-rust`.
- Exact rows should cover stable `.dem` text outputs.
- Structural rows should cover resource behavior and large-repeat non-materialization.

Benchmarks:

- Implement `pf4-dem-flatten-repeat`, `pf4-dem-rounded`, `pf4-dem-coordinate-map`, `pf4-dem-folded-traversal`, `pf4-dem-folded-graphlike-traversal`, and `pf4-dem-sampler-folded-repeat`. These rows now have report-only runner coverage for current materialized transforms, coordinate resource behavior, and capped-repeat traversal behavior. Full folded traversal remains active beyond these report-only rows.
- Use `direct-match` only when Stim v1.16.0 exposes a faithful timing surface.
- Keep rows report-only when they prove Stab resource behavior without a faithful Stim ratio.

Acceptance criteria:

- DEM public APIs use typed detector ids, observable ids, coordinates, probabilities, repeat counts, and domain errors.
- Public DEM transforms have explicit behavior for large repeats.
- Any remaining expansion cap is documented in the checklist and tested.

## Milestone RPF5: Detector Utilities And Measurement-Rich Flows

Objective: finish active utility APIs for detecting regions, missing detectors, feedback-related transforms, and flow solving.

Owned checklist rows:

- Detector-analysis utility APIs.
- Flows.
- Circuit transforms, for flow-aware transforms.
- Gate validation flags and categories, only when flow execution or transform integration reveals a metadata-contract drift.

Implementation tasks:

- Extend `circuit_detecting_regions` for selected repeat traversal, Clifford gates, target shapes, tick windows, detector filtering, multi-detector regions, anticommutation behavior, and gauge behavior. The bounded repeat-tick traversal, detector/logical-observable target-filter, promoted single-qubit and fixed two-qubit Clifford propagation, and ignored-anticommutation subsets are implemented with tests, oracle metadata, benchmark metadata where performance-sensitive, and progress evidence in `docs/plans/rpf5-detecting-regions-progress-report.md`.
- Extend `missing_detectors` for selected multi-record row reduction, repeated MPP stabilizer products, observable interaction, honeycomb suffix, and toric suffix cases. The Gaussian row-reduction, repeated MPP and pair-measurement stabilizer-product, record-only observable-row, ignored Pauli observable-row, and pinned honeycomb and toric generated-code suffix subset is implemented with tests, oracle metadata, benchmark metadata, and progress evidence in `docs/plans/rpf5-missing-detectors-progress-report.md`.
- Implement measurement-rich flow solving for `Flow`, `has_flow`, `has_all_flows`, `flow_generators`, and failure explanations. The unsigned `has_flow` and unsigned `has_all_flows` Rust helper subset with measurement-record and observable dependencies and the exact `circuit_flow_generators` subset for measurement, reset, pair-measurement, nonconstant and constant `MPP`, feedback, `MPAD`, and the promoted heralded-noise MPP fixture are implemented and tracked in [rpf5-flow-progress-report.md](rpf5-flow-progress-report.md).
- Integrate flow semantics with `time_reversed_for_flows`, feedback inlining, and gate metadata where selected.
- Add precise errors for unpromoted utility families.

Tests:

- Port owned cases from `vendor/stim/src/stim/util_top/circuit_to_detecting_regions.test.cc`, `vendor/stim/src/stim/util_top/missing_detectors.test.cc`, `vendor/stim/src/stim/stabilizers/flow.test.cc`, `vendor/stim/src/stim/util_top/circuit_flow_generators.test.cc`, and `vendor/stim/src/stim/util_top/has_flow.test.cc`.
- Add positive tests for every promoted detecting-region gate and target shape.
- Add positive and negative tests for missing-detector row reduction, repeated MPP products, observables, gauge detectors, honeycomb suffixes, and toric suffixes when promoted.
- Add flow tests for measurement indices, observables, multiplication, validation, generator solving, negative cases, and diagnostics.
- Add transform-integration tests proving flow-aware transforms preserve or intentionally rewrite flow data.

Oracle rows:

- Supplement `pf5-detecting-regions-extended`, `pf5-missing-detectors-extended`, and `pf5-measurement-rich-flows`. The detecting-regions subset is supplemented by `pf5-detecting-regions-repeat-rust`, `pf5-detecting-regions-targets-rust`, `pf5-detecting-regions-clifford-rust`, and `pf5-detecting-regions-anticommutation-rust`; the promoted missing-detectors subset is supplemented by `pf5-missing-detectors-row-reduction-rust`, `pf5-missing-detectors-mpp-observable-rust`, `pf5-missing-detectors-generated-honeycomb-rust`, and `pf5-missing-detectors-generated-toric-rust`; the unsigned has-flow record and observable subset is supplemented by `pf5-has-flow-record-observable-rust`; the unsigned has-all helper subset is supplemented by `pf5-has-all-flows-rust`; the promoted measurement-rich generator subset is supplemented by `pf5-flow-generators-measurement-rust`; and the promoted solve-for-measurements examples are supplemented by `pf5-flow-solve-measurement-rust`.
- Use structural comparators when exact text is unstable or when result ordering is intentionally set-like.

Benchmarks:

- Implement `pf5-detecting-regions-repeat`, `pf5-detecting-regions-targets`, `pf5-detecting-regions-clifford`, `pf5-missing-detectors-mpp`, `pf5-missing-detectors-generated-code`, `pf5-flow-solve-measurement-rich`, `pf5-has-all-flows-batch`, and `pf5-flow-generators-measurement-rich`. The detecting-regions repeat, target-filter, and Clifford rows now have report-only runner coverage for the promoted performance-sensitive subsets; the ignored-anticommutation mode is not separately benchmarked because it changes only the sparse-tracker error policy. `pf5-missing-detectors-mpp` has report-only runner coverage for the promoted MPP and observable-row subset, `pf5-missing-detectors-generated-code` has report-only runner coverage for the promoted honeycomb and toric generated-code suffix subset, `pf5-has-all-flows-batch` has report-only runner coverage for the promoted unsigned has-all-flow helper over record and observable batches, `pf5-flow-generators-measurement-rich` has report-only runner coverage for the promoted measurement, reset, pair-measurement, nonconstant and constant `MPP`, feedback, `MPAD`, and heralded-noise MPP generator subset, and `pf5-flow-solve-measurement-rich` has report-only runner coverage for the promoted solve-for-measurements examples.
- Keep complex utility rows report-only unless faithful Stim comparison and repeated stable ratios exist.

Acceptance criteria:

- Every promoted utility subfamily has positive, negative, and resource-boundary tests.
- Unpromoted utility subfamilies are rejected precisely or documented as deferred.
- Measurement-rich flows include observables and measurement records in both success and failure tests. The promoted unsigned has-flow subset satisfies this criterion only for the checker cases listed in [rpf5-flow-progress-report.md](rpf5-flow-progress-report.md).

## Milestone RPF6: Analyzer, Search, And Sparse Reverse Tracking

Objective: close non-deferred analyzer and logical-error search gaps without taking on full ErrorMatcher provenance or `stim explain_errors`.

Owned checklist rows:

- Circuit-to-DEM analysis.
- `analyze_errors --decompose_errors` and related flags.
- DEM analysis and shortest graphlike error.
- Shortest graphlike and hypergraph logical-error search.
- Sparse reverse detector-frame tracking.
- Error explanation value objects only where active analyzer or search paths need them.

Implementation tasks:

- Extend `circuit_to_detector_error_model` for selected generated circuits, loop folding, gauge detectors, approximate disjoint errors, decomposition options, remnant-edge blocking, and ignored decomposition failures. The generated-QEC semantic subset for noisy repetition-code and rotated-surface-code circuits is implemented and tracked in [rpf6-analyzer-progress-report.md](rpf6-analyzer-progress-report.md).
- Extend graphlike, hypergraph, shortest-error, SAT, and WCNF search behavior for selected generated-circuit and DEM cases.
- Improve sparse reverse detector-frame tracking for optimized loop folding where it affects analyzer or search correctness. The supported-Clifford repeat-folding subset is implemented for the full single-qubit Clifford gate set and fixed two-qubit tableau-backed Clifford gates with plain qubit-pair targets, with deterministic generated repeat tests covering nested and grouped target bodies; analyzer/search-specific consumption and unsupported variable-target unitary semantics remain active.
- Harden matched-error value objects only when required by active analyzer/search outputs.
- Keep full stack-frame provenance, heralded matching, repeat-contained noise provenance, and `explain_errors` CLI deferred.

Tests:

- Port owned cases from `vendor/stim/src/stim/simulators/error_analyzer.test.cc`, `vendor/stim/src/stim/simulators/error_matcher.test.cc`, `vendor/stim/src/stim/simulators/matched_error.test.cc`, `vendor/stim/src/stim/search/*_test.cc`, and `vendor/stim/src/stim/util_top/circuit_to_dem.test.cc`.
- Add exact `.dem` output tests for deterministic analyzer cases.
- Add structural tests for gauge detectors, approximate disjoint errors, decomposed errors, generated circuits, loop folding, search result sets, and ordering-insensitive outputs.
- Add generated or property-style tests for sparse reverse tracking when new supported unitary families, repeated loops, detectors with coordinates, observables, noise decomposition, analyzer consumption, or search consumption are promoted.
- Add negative tests for unsupported decomposition failure handling and invalid analyzer options.

Oracle rows:

- Supplement `pf6-analyzer-generated-looping`, `pf6-search-generated`, and `pf6-sparse-rev-tracker`. The generated-QEC analyzer subset is supplemented by `pf6-analyzer-generated-qec-rust`, and the selected loop-folded decomposition and remnant-edge blocking subset is supplemented by `pf6-error-decomp-loop-folded-rust`; the broader generated-looping row remains manifest-only.
- Use exact `.dem` comparators where output order is stable and structural comparators otherwise.

Benchmarks:

- Implement or extend `pf6-analyze-errors-generated-surface`, `pf6-error-decomp-loop-folded`, `pf6-graphlike-search-generated`, `pf6-hypergraph-search-generated`, and `pf6-sparse-rev-frame-loop`. The generated-surface analyzer, loop-folded decomposition, generated graphlike search, generated hypergraph search, and supported-Clifford sparse reverse frame loop rows have report-only Rust runners.
- Use schema-version-2 submeasurement thresholds for bundled analyzer or search rows.
- Promote only rows with faithful pinned Stim evidence and repeated stable ratios.

Acceptance criteria:

- Analyzer and search outputs match pinned Stim for owned exact cases and satisfy structural comparators for allowed-ordering cases. The generated-QEC semantic subset satisfies this for the promoted noisy repetition-code and rotated-surface-code cases only.
- Loop folding is proven by tests and benchmarks, not only by small-output equality.
- Deferred provenance and CLI explanation surfaces stay explicitly outside the claim.

## Milestone RPF7: Visible CLI Parity Closure

Objective: finish active command-line gaps for `stab m2d`, `stab analyze_errors`, and accepted legacy dispatch.

Owned checklist rows:

- `stim m2d`.
- `stim analyze_errors`.
- Legacy top-level command flags.
- CLI binary, as a rollup.
- Measurement-to-detection conversion, for public command behavior.

Implementation tasks:

- Finish `stab m2d` parity for selected `--sweep`, `--sweep_format`, `--ran_without_feedback`, `--skip_reference_sample`, `--append_observables`, `--obs_out`, `--obs_out_format`, input formats, output formats, path errors, writer errors, and resource boundaries. The selected `m2d` CLI closure is now implemented by `pf7-m2d-cli-parity`; only newly selected command shapes, path failure modes, format failures, or resource failures should reopen this slice.
- Finish `stab analyze_errors` parity for selected flags, decomposition behavior, gauge behavior, approximate disjoint errors, fold-loop behavior, input and output paths, stdout behavior, stderr class, and exit status. The selected `analyze_errors` CLI closure is now implemented by `pf7-analyze-errors-cli-parity`; only newly selected flags, malformed inputs, or analyzer failure modes should reopen this slice.
- Finish accepted legacy alias behavior for `--gen`, `--convert`, `--sample`, `--detect`, `--m2d`, and `--analyze_errors`. The selected legacy-dispatch closure is now implemented by `pf7-legacy-dispatch-parity`; only newly selected legacy spellings or failure modes should reopen this slice.
- Add conflict tests for multiple legacy modes. The selected `--convert`, `--sample`, `--detect`, `--m2d`, `--analyze_errors`, and `--gen=...` conflict subset is implemented with tests and oracle metadata in `pf7-legacy-dispatch-conflicts-rust`.
- Keep deprecated `--detector_hypergraph` rejected or absent, and document that users should use `stab analyze_errors`. The explicit mode and help-topic exclusion subset is implemented with tests and oracle metadata in `pf7-detector-hypergraph-excluded-rust`, and the selected unimplemented legacy-style `--diagram`, `--explain_errors`, `--repl`, and `--sample_dem` flags fail closed in `pf7-legacy-unselected-modes-rust`.

Tests:

- Port owned cases from `vendor/stim/src/stim/cmd/command_m2d.test.cc`, `vendor/stim/src/stim/cmd/command_analyze_errors.test.cc`, `vendor/stim/src/stim/main_namespaced.test.cc`, and selected `vendor/stim/doc/usage_command_line.md` examples.
- Add exact oracle rows for accepted command shapes.
- Add malformed-input oracle rows for stderr class and exit status.
- Add CLI tests for nonexistent `--circuit`, `--in`, `--out`, `--obs_out`, and scratch paths.
- Add resource-boundary tests for large measurement inputs, large sweep inputs, writer failure, unsupported `ptb64` output, invalid observable side-output formats, and feedback-inlining failures.
- Add tests proving `--detector_hypergraph` is not supported.

Oracle rows:

- Keep `pf7-m2d-cli-parity` implemented as the selected `m2d` CLI closure. The selected `analyze_errors` CLI closure is implemented by `pf7-analyze-errors-cli-parity`, and the selected legacy-dispatch closure is implemented by `pf7-legacy-dispatch-parity`, with supporting evidence from `pf7-m2d-path-io-rust`, `pf7-m2d-command-contract-rust`, `pf7-analyze-errors-path-io-rust`, `pf7-analyze-errors-flags-rust`, `pf7-legacy-dispatch-accepted-rust`, `pf7-legacy-dispatch-conflicts-rust`, `pf7-detector-hypergraph-excluded-rust`, and `pf7-legacy-unselected-modes-rust`.
- Exact-output rows must run against pinned Stim v1.16.0 when the command shape is shared.
- Stab-only explicit rejections must still have Stab CLI tests or oracle rows.

Benchmarks:

- Implement `pf7-cli-m2d-sweep-b8`, `pf7-cli-m2d-feedback-inline`, `pf7-cli-analyze-errors-generated`, `pf7-cli-analyze-errors-decompose`, and `pf7-cli-legacy-dispatch-startup`.
- Promote only faithful pinned-Stim CLI rows with stable repeated evidence into the 1.25x threshold file.
- Keep startup or rejection-only rows report-only unless they are a documented operational performance contract.

Acceptance criteria:

- CLI behavior is proven through oracle rows or CLI tests, not only core unit tests.
- Public command paths stream or enforce documented caps.
- Help, README, roadmap, feature checklist, oracle manifest, benchmark manifest, and progress reports agree on the supported command surface.

## Milestone RPF8: Benchmark Gate, Audit, And Documentation Closure

Objective: turn milestone evidence into durable acceptance evidence and update rollup rows without overstating parity.

Owned checklist rows:

- Rust core library equivalent for core Stim semantics.
- CLI binary.
- `.stim`, `.dem`, and result-format compatibility.
- Full semantic execution of every legal circuit operation.
- Highest-priority remaining feature gaps.
- Any active row completed by RPF1 through RPF7.

Tasks:

- Run strict benchmark probes for every newly benchmarked row and record reports under `target/benchmarks/`.
- For each benchmark report, record machine metadata, Stim metadata, Stab metadata, local-modification state, warmup state, measurement-run count, variance, ratios, and row status.
- Promote only stable direct-match or CLI-baseline rows into `benchmarks/m12-primary-thresholds.json` at `max_relative_ratio: 1.25`.
- Add or update schema-version-2 submeasurement thresholds for bundled rows.
- Keep report-only, contract-representative, proxy, tiny, no-ratio, and no-faithful-baseline rows out of the primary threshold file unless a separate source-owned waiver file says otherwise.
- Update profiler notes in the same change set as any threshold promotion.
- Update `docs/stab-feature-checklist.md`, `docs/plans/rust-stim-drop-in-rewrite.md`, `docs/plans/stim-test-porting-plan.md`, oracle metadata, benchmark metadata, and milestone progress or completion reports to match behavior.
- Run milestone-audit for each completed milestone.
- Run full-code-review for each completed milestone and spawn GPT-5.5/xhigh subagents during the review when executing from Codex.
- Fix implementation, test, benchmark, documentation, audit, and review findings before claiming completion.
- Log under-specification findings in `docs/plans/milestone-spec-gaps.md` instead of hiding them in checklist wording.

Tests:

- `cargo test -p stab-bench --quiet`
- `cargo test -p stab-oracle fixtures --quiet`
- `just oracle::run --implemented-only`
- `just bench::smoke`
- Every milestone-specific test listed in RPF1 through RPF7.

Benchmarks:

- `just bench::baseline --primary --out target/benchmarks/remaining-partial-primary-baseline`
- `just bench::compare --primary --warmup --measurement-runs 3 --require-profiler-notes --baseline target/benchmarks/remaining-partial-primary-baseline/baseline.json --report target/benchmarks/remaining-partial-primary-compare`
- `just bench::primary-regression --baseline target/benchmarks/remaining-partial-primary-baseline/baseline.json --report target/benchmarks/remaining-partial-primary-regression`
- `just bench::primary-memory-regression --baseline target/benchmarks/remaining-partial-primary-baseline/baseline.json`

Acceptance criteria:

- Every completed milestone has a report naming tests, oracle rows, benchmark rows, audit outcome, review outcome, and remaining exclusions.
- Rollup checklist rows change status only after child-row evidence exists.
- No completion report cites stale local evidence as authoritative release evidence.
- Every remaining partial row either has an active follow-up owner or is partial only because of a documented deferred surface.

## Required Final Verification

Before claiming this plan complete, run:

```sh
cargo fmt --all --check
cargo clippy -p stab-core -p stab-cli -p stab-oracle -p stab-bench --all-targets -- -D warnings
cargo test --workspace --quiet
just oracle::run --implemented-only
just bench::smoke
just maintenance::pre-commit
```

If a milestone changes benchmark gates, also run the RPF8 primary benchmark evidence commands from current `HEAD`.
If a milestone changes public CLI behavior, include the relevant CLI unit tests and oracle rows in the final evidence.
If a milestone changes public Rust APIs, update Rust docs or matching project docs in the same change set.

## Stop And Log Conditions

Stop implementation work and write a `docs/plans/milestone-spec-gaps.md` entry when:

- A promoted subcase requires Python bindings, JS/WASM, diagrams, `explain_errors` CLI, `repl`, QASM, Quirk, Crumble, GPU, ecosystem integrations, exact random-stream parity, C++ header compatibility, or a public simulator product.
- A whole upstream file is still being treated as acceptance criteria.
- A CLI feature cannot define accepted flags, rejected flags, input formats, output formats, stdout behavior, stderr class, exit status, path handling, and resource behavior.
- A benchmark row cannot be assigned a comparability class.
- A performance claim would require stale reports, unrecorded local modifications, missing profiler notes, or informal waivers.
- A public parser, converter, sampler, analyzer, transformer, search, or writer path has neither streaming behavior nor a documented cap.
- A checklist update would need to hide a known limitation to mark a row done.
