# Partial Feature Closure Inventory

## Purpose

This inventory is the source-owned map from `docs/stab-feature-checklist.md` partial rows to the active remaining partial feature milestones in `docs/plans/remaining-partial-feature-milestones.md`.
It still records the earlier PF milestone ids because existing oracle rows, benchmark rows, and progress reports use those ids.
It exists so later implementation agents do not have to infer active scope from broad Stim source files or from checklist prose.

Every active item below has an owner milestone, owner crate or surface, upstream source, first test strategy, oracle status, benchmark plan, and explicit exclusions.
Rows that are partial only because deferred products are absent are recorded in the exclusion table and must not block the current Rust and CLI beta closure work.

## Status Classes

- `Active`: implement or tighten the named Rust or CLI surface during the matching RPF milestone in `docs/plans/remaining-partial-feature-milestones.md`.
- `Rollup`: parent checklist row that becomes complete only after its active child rows are complete.
- `Deferred-only`: partial or missing checklist row whose remaining work is intentionally outside this plan.
- `Decision`: RPF0 must lock the subcases before implementation because the checklist currently mixes active and deferred work.

## RPF Mapping

| Historical inventory milestone | Active RPF milestone | Notes |
| --- | --- | --- |
| PF1 circuit and DEM Rust API rows | RPF0 classification, RPF4 DEM follow-up | The current Rust circuit and basic DEM API slices are implemented; RPF0 keeps them classified and RPF4 owns remaining DEM transform and folded-traversal work. |
| PF1 gate metadata rows | RPF1 | RPF1 owns unsupported metadata accessors and gate execution contract documentation after decomposition metadata; RPF5 owns measurement-rich flow metadata. |
| PF2 transform rows | RPF2 | RPF2 owns circuit flattening, noise removal, decomposition, feedback inlining, time reversal for flows, and repeat traversal. |
| PF3 sweep and gate execution rows | RPF3 | RPF3 owns sweep-conditioned execution and legal-gate execution gaps. |
| PF4 DEM rows | RPF4 | RPF4 owns DEM APIs, transforms, coordinates, and folded traversal. |
| PF5 utility and flow rows | RPF5 | RPF5 owns detecting regions, missing detectors, and measurement-rich flows. |
| PF6 analyzer, search, and sparse tracker rows | RPF6 | RPF6 owns analyzer, search, sparse reverse tracker, and active matched-error value-object hardening. |
| PF7 CLI rows | RPF7 | RPF7 owns `stab m2d`, `stab analyze_errors`, accepted legacy aliases, and explicit `--detector_hypergraph` exclusion. |
| PF0/PF8 evidence rules | RPF0 and RPF8 | RPF0 owns inventory lock; RPF8 owns benchmark, audit, documentation, and rollup closure. |

## Locked RPF Subcases

This section is the RPF0 split from broad upstream files into owned, semantic-mining, deferred, and out-of-scope subcases.
Manifest-only oracle rows stay broad extraction contracts, but implementation milestones must use the locked subcases below instead of treating a whole upstream file as acceptance.

### RPF1 Gate Metadata

Owned:

- Gate decomposition metadata accessor behavior for the H/S/CX/M/R decomposition table, including representative exact text, the full supported gate set, parseability as `.stim` text, and fail-closed behavior for gates without decomposition metadata. Implemented by `pf1-gate-decomposition-metadata`; keep this row current if the public API changes.
- Measurement-rich or variable-target gate flow metadata decision, either implemented with typed Rust APIs or rejected with precise unsupported-accessor errors.
- A canonical gate support table separating validation support, tableau metadata, unitary metadata, flow metadata, decomposition metadata, sampler execution, detector conversion, analyzer propagation, and explicit rejection. Implemented as `docs/plans/rpf1-gate-execution-support-contract.md`; keep it synchronized with execution behavior.

Semantic-mining:

- Python `GateData` tests are used only to infer semantics for Rust accessors.

Deferred or out of scope:

- Python `GateData` object shape and binding ergonomics.
- Full circuit decomposition behavior, which belongs to RPF2.

### RPF2 Circuit Transforms

Owned:

- `flattened` and flattened-operation traversal for repeat blocks, tags, annotations, detectors, observables, measurement references, coordinate shifts, and large-repeat resource behavior. Implemented for Rust `Circuit::flattened` and `Circuit::flattened_operations` with a one-million-output materialization cap, folded shift-only repeats, exact canonical-output tests, structural operation tests, an implemented oracle row, and report-only benchmark runners.
- `without_noise` for noise removal while preserving deterministic operations, coordinates, ticks, detectors, observables, and measurement-record semantics. Implemented for Rust `Circuit::without_noise` with exact tests for noisy measurements, ordinary noise removal, heralded-noise `MPAD` replacement, tags, annotations, detectors, observables, and measurement-record preservation.
- `decomposed` for compound gates, MPP, SPP, pair measurements, grouped targets, base-gate lowering, tableau or flow semantic checks, and unsupported decomposition target errors. Rust `Circuit::decomposed` now covers public ISWAP, MPP, SPP, pair-measurement, tag-preservation, noise-preservation, annotation-preservation, constant-MPP, and anti-Hermitian rejection cases; flow-driven decomposition checks remain open where they depend on RPF5.
- Feedback inlining for selected single-control Pauli feedback and MPP feedback cases, including repeat-block support or explicit repeat-block rejection. A scoped Rust `Circuit::with_inlined_feedback` API is implemented for the supported top-level Pauli and MPP subset and rejects repeat blocks plus unsupported classical controlled gates; full loop-refolding parity remains open.
- `time_reversed_for_flows` only for the measurement-rich flow cases locked by RPF5.

Semantic-mining:

- Python transform tests are used only to infer Rust transform semantics and canonical-output expectations.

Deferred or out of scope:

- QASM, Quirk, Crumble, diagrams, Python operator ergonomics, and exact loop refolding unless RPF2 explicitly implements it.

### RPF3 Sweep-Conditioned Execution And Gate Semantics

Owned:

- `m2d` sweep records in accepted formats, including `01`, `b8`, and `ptb64` input where supported, omitted-sweep default-false behavior, width mismatches, invalid record counts, side outputs, and streaming resource behavior.
- Sweep-conditioned detection sampling if the converter and sampler can prove semantic equivalence to explicit small-circuit expansion.
- Sweep-aware analyzer behavior for public `analyze_errors` or Rust DEM generation cases selected by the analyzer milestone.
- Legal-gate execution support classification across sampler, converter, detection, and analyzer paths, with tests for accepted execution and explicit unsupported-shape errors.

Semantic-mining:

- Frame-simulator and error-analyzer tests are used to mine legal-gate execution behavior, not to claim public simulator API parity.

Deferred or out of scope:

- Exact random-stream parity and public interactive simulator products.

### RPF4 DEM APIs And Folded Traversal

Owned:

- Public materialized DEM `flattened`, `rounded`, and `without_tags` behavior for tags, separators, detector shifts, coordinate shifts, logical observables, and repeats.
- DEM count, coordinate, final-shift, and selected-coordinate queries for large and nested repeats, with folded traversal or documented caps.
- Folded or capped traversal for DEM sampler, graphlike search, hypergraph search, SAT or WCNF encoding, matcher-adjacent operations, and analyzer-adjacent operations.
- Resource-boundary tests for huge repeats, nested repeats, high detector shifts, high observable counts, malformed DEMs, and unsafe transform expansion.

Semantic-mining:

- Python DEM tests are used to infer Rust API semantics only.

Deferred or out of scope:

- DEM diagrams, Python class operators, and Python API shape.

### RPF5 Detector Utilities And Measurement-Rich Flows

Owned:

- Detecting-region repeat traversal, broader Clifford gate support, target-shape support, tick windows, detector filtering, multi-detector regions, anticommutation behavior, and gauge behavior.
- Missing-detector multi-record row reduction, repeated MPP stabilizer-product cases, observable-interaction cases, honeycomb suffix cases, and toric suffix cases, with precise errors for any unpromoted family.
- Measurement-rich `Flow`, `has_flow`, `has_all_flows`, `flow_generators`, flow multiplication, included observables, measurement indices, failure diagnostics, and transform integration.

Semantic-mining:

- Python flow tests are used only to infer Rust flow semantics.

Deferred or out of scope:

- Python flow binding ergonomics.

### RPF6 Analyzer Search And Sparse Reverse Tracking

Owned:

- Generated-circuit analyzer behavior, loop folding, gauge detectors, approximate disjoint errors, decomposition options, remnant-edge blocking, ignored decomposition failures, and exact or structural `.dem` comparators.
- Generated-circuit graphlike search, hypergraph search, shortest errors, SAT or WCNF encoding, ordering-insensitive result comparison, and resource behavior.
- Sparse reverse detector-frame tracker optimized loop folding, all-unitary fuzz cases, analyzer and search consumption, and active matched-error value-object hardening needed by analyzer/search outputs.

Semantic-mining:

- ErrorMatcher and matched-error tests are used only for value-object behavior required by active analyzer and search paths.

Deferred or out of scope:

- Full ErrorMatcher provenance, heralded matching, repeat-contained noise stack frames, and `stim explain_errors` CLI.

### RPF7 Visible CLI Parity

Owned:

- `stab m2d` accepted flags, sweep records, feedback inlining, skip-reference behavior, append observables, side outputs, input formats, output formats, path errors, writer errors, stdout behavior, stderr class, exit status, and resource boundaries.
- `stab analyze_errors` accepted flags, decomposition behavior, gauge behavior, approximate disjoint errors, fold-loop behavior, input and output paths, stdout behavior, stderr class, exit status, and malformed input behavior.
- Accepted legacy aliases `--gen`, `--convert`, `--sample`, `--detect`, `--m2d`, and `--analyze_errors`, including conflicts between multiple modes.
- Explicit rejection or absence of deprecated `--detector_hypergraph`.

Semantic-mining:

- Usage examples are used to infer command shape and error-class expectations.

Deferred or out of scope:

- `stim explain_errors`, diagrams, `stim repl`, and deprecated aliases not selected for Stab.

## Rollup Rows

| Checklist row | Status class | Completion rule |
| --- | --- | --- |
| Rust core library equivalent for core Stim semantics | Rollup | Complete only after RPF1 through RPF6 close the active Rust API, transform, DEM, utility, flow, analyzer, and search rows. |
| CLI binary | Rollup | Complete only after RPF7 closes active `m2d`, `analyze_errors`, and accepted legacy-dispatch rows, or records exact exclusions. |
| `.stim`, `.dem`, and result-format compatibility | Rollup | Complete only after RPF2, RPF3, RPF4, and RPF7 close active transform, sweep, DEM, and command-specific format gaps. |
| Full semantic execution of every legal circuit operation | Rollup | Complete only after RPF3 records which legal gates execute in sampler, converter, detection, and analyzer paths, with explicit rejection tests for unsupported shapes. |
| Highest-priority remaining feature gaps | Rollup | Complete only after the child rows for circuit transforms, DEM API, sweep-conditioned behavior, and feedback-inlining parity are implemented or explicitly deferred. |

## Active Work Items

| Inventory id | Checklist rows | Milestone | Owner crate or surface | Upstream sources | First tests and comparator | Oracle status | Benchmark plan | Exclusions |
| --- | --- | --- | --- | --- | --- | --- | --- | --- |
| PF1-CIRCUIT-RUST-API | Programmatic mutation; Core introspection; Circuit coordinate queries; Reference samples and determined measurements | PF1 | `stab-core` circuit API | `src/stim/circuit/circuit_pybind_test.py`; `src/stim/circuit/circuit.test.cc`; `src/stim/util_top/reference_sample_tree.test.cc`; `src/stim/util_top/count_determined_measurements.test.cc` | Structural Rust tests cover the implemented stats, coordinate, and mutation-helper subset for clear, top-level length, top-level item iteration, validated top-level item ranges, instruction-only ranges with repeat-block rejection, lazy flattened forward and reverse instruction iteration through nested repeat blocks, append-from-Stim-text helpers, atomic parse-failure handling, bounded path-based `.stim` file reads, streaming `.stim` file writes, circuit append and concatenation, repeat special cases, nested repeat-count fusion, repeat overflow rejection, insertion with boundary fusion, repeat-block insertion, pop behavior without neighbor refusion, circuit reference samples, reference-sample trees, deterministic-measurement counts, sweep-controlled reference/count helpers under default-false semantics, measurement counts, detector counts, observable counts, tick counts, sweep-bit counts, folded final coordinate shifts, final qubit coordinates through repeats, folded detector-coordinate lookup through repeats, all-detector coordinate maps including empty-coordinate detectors, missing-detector rejection, and non-finite folded-shift rejection. No active PF1 circuit API test subcase remains before the broad row can close against current Rust scope. | `pf1-circuit-stats-coordinates`, `pf1-circuit-append-text`, `pf1-circuit-file-helpers`, `pf1-circuit-concat`, `pf1-circuit-repeat`, `pf1-circuit-insert-pop`, `pf1-circuit-iterators`, `pf1-circuit-reference-determined`, and `pf1-circuit-detector-coordinates` implemented rows supplement the broad `pf1-circuit-rust-api` manifest-only row. | `pf1-circuit-coordinate-query` has a non-primary report-only Rust runner with measurement work and compare notes. Existing reference-sample and count-determined rows remain separate historical coverage evidence for lower-level implementations; iterator/range views are covered structurally and are not separate PF1 benchmark gates. | Python operators, Python bit-packed reference-sample return shapes, file-like objects, binding-specific slicing, and unbounded streaming `.stim` file reads remain deferred; exact C++ infinity behavior for folded coordinate overflow is logged in `docs/plans/milestone-spec-gaps.md`. |
| PF1-GATE-METADATA | Gate validation flags and categories | PF1 | `stab-core` gate registry | `src/stim/gates/gates_test.py`; `src/stim/gates/gates.test.cc`; `src/stim/gates/gates.perf.cc` | Structural Rust tests cover the implemented accessor subset for aliases, argument rules, target rules, target grouping, fusing, noisy/reset/measurement/unitary/single-qubit/two-qubit/target-capability/symmetry flags, unitary inverse, generalized inverse, local Clifford tableau metadata, tableau-backed unitary flow metadata, fixed-shape one- or two-qubit `GateUnitaryMatrix` metadata, H/S/CX/M/R `GateDecomposition` metadata, and parser-versus-execution fail-closed behavior for `SPP` and `SPP_DAG`. Remaining PF5 tests must cover measurement-rich or variable-target flow metadata decisions before the broad row closes. | `pf1-gate-metadata-rust-accessors`, `pf1-gate-tableau-metadata`, `pf1-gate-flow-metadata`, `pf1-gate-unitary-matrix-metadata`, and `pf1-gate-decomposition-metadata` implemented rows supplement the broad `pf1-gate-metadata-api` manifest-only row. | `pf1-gate-metadata-lookup` has a non-primary report-only Rust runner with measurement work and compare notes, including tableau-supported gate, flow, fixed-shape unitary matrix, H/S/CX/M/R decomposition, and alias reads. | Python `GateData` object shape remains deferred. |
| PF1-DEM-RUST-API | DEM construction and mutation; DEM detector shifts, observables, coordinates, and counts | PF1 | `stab-core` DEM API | `src/stim/dem/detector_error_model_pybind_test.py`; `src/stim/dem/detector_error_model_repeat_block_pybind_test.py`; `src/stim/dem/dem_instruction_pybind_test.py`; `src/stim/dem/detector_error_model.test.cc` | Structural Rust tests cover the implemented basic and introspection subset for length, emptiness, clear, append-from-text atomic parse failure, recursive tag stripping, folded final coordinate shifts, non-finite folded-shift rejection, error counts through repeats, all-detector coordinate maps, selected-detector coordinate maps, single-detector coordinate lookup, top-level item iteration, validated item ranges, instruction-only ranges with repeat-block rejection, typed item downcasts, lazy adjusted flattened instruction iteration through repeats, huge-repeat lazy traversal for yielding bodies, and exact-one-target validation for detector and logical observable declarations. Remaining PF4 tests must cover public materialized `flattened`, `rounded`, copy ergonomics beyond `Clone` if still useful, and transform resource boundaries before the broad rows close. | `pf1-dem-basic-rust-api`, `pf1-dem-counts-coordinates`, and `pf1-dem-iterators` implemented rows supplement the broad `pf1-dem-rust-api` manifest-only row. | `pf1-dem-counts-repeat` and `pf1-dem-without-tags` have non-primary report-only Rust runners with measurement work and compare notes; `pf1-dem-counts-repeat` now includes count, final-coordinate, and selected detector-coordinate submeasurements. | Python list operators, binding-only mutation ergonomics, exact Python API shape, and full folded traversal for every large-repeat coordinate-map or transform case remain deferred. |
| PF2-FLATTEN-WITHOUT-NOISE | Repeat handling; Circuit transforms; Full circuit transform API parity | PF2 | `stab-core` circuit transforms | `src/stim/circuit/circuit_pybind_test.py`; `src/stim/circuit/circuit.test.cc` | `cargo test -p stab-core --test circuit_transforms` covers Rust `Circuit::flattened`, `Circuit::flattened_operations`, and `Circuit::without_noise` for tags, annotations, detectors, observables, measurement references, coordinate shifts, heralded-noise `MPAD` preservation, excessive materialized expansion rejection, and folded shift-only repeats. | `pf2-circuit-flatten-without-noise-rust` is implemented and supplements the broad `pf2-circuit-flatten-without-noise` manifest-only row. | `pf2-circuit-flatten-repeat` and `pf2-circuit-without-noise` have non-primary report-only Rust runners with measurement work and compare notes. | QASM, Quirk, Crumble, diagrams, Python iterator ergonomics, and broader transform parity remain deferred or owned by later PF2 rows. |
| PF2-DECOMPOSED | Circuit transforms; Gate semantic execution | PF2 | `stab-core` decomposition and algebra integration | `src/stim/circuit/gate_decomposition.test.cc`; `src/stim/util_top/simplified_circuit.test.cc`; `src/stim/util_top/circuit_vs_tableau.test.cc` | `cargo test -p stab-core --test circuit_transforms decomposed` covers the Rust `Circuit::decomposed` API for public ISWAP and MPP output, tag preservation across RX, noise, MPP, detector, and SPP, constant-MPP products, and anti-Hermitian MPP/SPP rejection. Broader flow-semantic decomposition checks remain open where they depend on RPF5. | `pf2-circuit-decomposed-public-rust` is implemented and supplements the broad `pf2-circuit-decomposed` manifest-only row. | `pf2-circuit-decompose-mpp-spp` has a non-primary report-only Rust runner with measurement work and compare notes. | Any decomposition requiring deferred simulator products must fail closed. |
| PF2-FEEDBACK-TIME-REVERSE | Circuit transforms; Measurement-to-detection conversion; Full feedback-inlining transform parity | PF2 | `stab-core` feedback and flow transforms | `src/stim/util_top/transform_without_feedback.test.cc`; `src/stim/util_top/circuit_flow_generators.test.cc`; `src/stim/util_top/has_flow.test.cc` | `cargo test -p stab-core --test circuit_transforms feedback` covers the scoped Rust `Circuit::with_inlined_feedback` API, exact supported top-level feedback output, MPP feedback DEM preservation, repeat-block rejection, and unsupported classical-control rejection. Full tests for exact loop refolding, repeat-block feedback support, `time_reversed_for_flows`, and flow-transform preservation remain open. | `pf2-feedback-inline-scoped-rust` is implemented and supplements the broad `pf2-feedback-time-reverse` manifest-only row plus implemented M9 feedback rows. | `pf2-feedback-inline-batch` now has a non-primary report-only Rust runner with measurement work and compare notes; `pf2-time-reverse-flow` remains a placeholder. | Exact loop refolding remains open; `time_reversed_for_flows` remains blocked on RPF5 measurement-rich flow semantics. |
| PF3-SWEEP-CONVERSION-DETECTION | Target kinds; Measurement-to-detection conversion; Broader sweep-conditioned simulator and analysis parity | PF3 | `stab-core` detection converter and `stab-cli` detect or m2d support | `src/stim/simulators/measurements_to_detection_events.test.cc`; `src/stim/cmd/command_m2d.test.cc`; `src/stim/cmd/command_detect.test.cc` | Exact CLI and structural core tests for sweep records in accepted formats, omitted-sweep defaults, width mismatches, unsupported sweep shapes, side outputs, and streaming resource behavior. | `pf3-sweep-m2d-detect` manifest-only row plus existing M9 sweep rows. | `pf3-m2d-sweep-b8`, `pf3-m2d-sweep-ptb64-input`, and `pf3-detect-sweep-sampling` non-primary placeholders. | Python converter class parity remains deferred. |
| PF3-SWEEP-ANALYZER-GATES | Target kinds; Gate semantic execution; Circuit-to-DEM analysis | PF3 | `stab-core` sampler, converter, detection, and analyzer semantics | `src/stim/simulators/error_analyzer.test.cc`; `src/stim/simulators/frame_simulator.test.cc`; `src/stim/simulators/tableau_simulator.test.cc` | Structural and exact tests for sweep-controlled analyzer behavior, legal-gate execution gaps, unsupported target shapes, and semantic equivalence to explicit small-circuit expansion. | `pf3-sweep-analyzer` and `pf3-gate-semantic-execution` manifest-only rows. | `pf3-analyze-errors-sweep` and `pf3-gate-semantic-wide` non-primary placeholders. | Exact random-stream parity and public interactive simulator APIs remain deferred. |
| PF4-DEM-INTROSPECTION-TRANSFORMS | DEM parser and canonical printer; DEM introspection; DEM transforms; Full DEM public API parity | PF4 | `stab-core` DEM API | `src/stim/dem/detector_error_model.test.cc`; `src/stim/dem/detector_error_model_pybind_test.py`; `src/stim/dem/dem_instruction.test.cc` | Exact and structural tests for error counts, instruction counts, coordinate maps, final shifts, `flattened`, `rounded`, `without_tags`, separators, tags, and repeat blocks. | `pf4-dem-introspection-transforms` and `pf4-dem-coordinate-api` manifest-only rows. | `pf4-dem-flatten-repeat`, `pf4-dem-rounded`, and `pf4-dem-coordinate-map` non-primary placeholders. | DEM diagrams and Python operator ergonomics remain deferred. |
| PF4-DEM-FOLDED-TRAVERSAL | DEM flattening and large repeat traversal; DEM analysis and shortest graphlike error | PF4 | `stab-core` DEM traversal consumers | `src/stim/simulators/dem_sampler.test.cc`; `src/stim/search/graphlike/algo.test.cc`; `src/stim/search/hyper/algo.test.cc`; `src/stim/search/sat/wcnf.test.cc` | Resource-boundary tests for large and nested repeats across sampler, search, SAT, matcher-adjacent, and analyzer-adjacent traversal. Tests must prove folded traversal or a documented cap. | `pf4-dem-folded-traversal` manifest-only row. | `pf4-dem-folded-traversal`, `pf4-dem-folded-graphlike-traversal`, and `pf4-dem-sampler-folded-repeat` non-primary placeholders. | Match-graph rendering remains deferred. |
| PF5-DETECTING-REGIONS | Detector-analysis utility APIs | PF5 | `stab-core` detector utility API | `src/stim/util_top/circuit_to_detecting_regions.test.cc`; `src/stim/util_top/circuit_to_detecting_regions_test.py` | Structural tests for the detecting-region subcases locked by RPF5: repeat traversal, broader Clifford gates, target shapes, tick windows, detector filtering, multi-detector regions, anticommutation behavior, and gauge behavior. | `pf5-detecting-regions-extended` manifest-only row plus M9 simple implemented row. | `pf5-detecting-regions-repeat` non-primary placeholder. | Unpromoted gauge or anticommutation behavior must fail closed and stay partial. |
| PF5-MISSING-DETECTORS | Detector-analysis utility APIs | PF5 | `stab-core` detector utility API | `src/stim/util_top/missing_detectors.test.cc` | Structural tests for the missing-detector subcases locked by RPF5: multi-record row reduction, repeated MPP stabilizer products, observable interactions, honeycomb suffixes, toric suffixes, and clear errors for unpromoted families. | `pf5-missing-detectors-extended` manifest-only row plus M9 future rows for row reduction, MPP, observable, honeycomb, and toric families. | `pf5-missing-detectors-mpp` and `pf5-missing-detectors-generated-code` non-primary placeholders. | Any missing-detector family not implemented by RPF5 must remain explicitly deferred or fail closed. |
| PF5-MEASUREMENT-RICH-FLOWS | Flows; Circuit transforms | PF5 | `stab-core` flow API | `src/stim/stabilizers/flow.test.cc`; `src/stim/stabilizers/flow_pybind_test.py`; `src/stim/util_top/circuit_flow_generators.test.cc`; `src/stim/util_top/has_flow.test.cc` | Structural tests for measurement indices, observables, flow multiplication, flow validation, `has_flow`, `has_all_flows`, `flow_generators`, failure explanations, and transform integration. | `pf5-measurement-rich-flows` manifest-only row. | `pf5-flow-solve-measurement-rich` and `pf5-has-all-flows-batch` non-primary placeholders. | Python flow binding ergonomics remain deferred. |
| PF6-ANALYZER-GENERATED | Circuit-to-DEM analysis; `analyze_errors --decompose_errors` and related flags; Sparse reverse detector-frame tracking | PF6 | `stab-core` analyzer | `src/stim/simulators/error_analyzer.test.cc`; `src/stim/util_top/circuit_to_dem.test.cc`; `src/stim/cmd/command_analyze_errors.test.cc` | Exact `.dem` and structural tests for generated circuits, loop folding, gauge detectors, approximate disjoint errors, decomposition options, remnant-edge blocking, and ignored failures. | `pf6-analyzer-generated-looping` manifest-only row plus existing M10 analyzer rows. | `pf6-analyze-errors-generated-surface` and `pf6-error-decomp-loop-folded` non-primary placeholders. | Full ErrorMatcher provenance remains deferred unless needed for analyzer correctness. |
| PF6-SEARCH | Shortest graphlike and hypergraph logical-error search; DEM analysis and shortest graphlike error | PF6 | `stab-core` search APIs | `src/stim/search/graphlike/algo.test.cc`; `src/stim/search/hyper/algo.test.cc`; `src/stim/search/sat/wcnf.test.cc` | Structural tests for generated-circuit graphlike search, hypergraph search, shortest errors, SAT or WCNF encoding, ordering-insensitive results, and resource behavior. | `pf6-search-generated` manifest-only row. | `pf6-graphlike-search-generated` and `pf6-hypergraph-search-generated` non-primary placeholders. | New public graph or vector simulator APIs remain deferred. |
| PF6-SPARSE-REV-TRACKER | Sparse reverse detector-frame tracking; Error explanation value objects | PF6 | `stab-core` sparse reverse tracker and analyzer support | `src/stim/simulators/sparse_rev_frame_tracker.test.cc`; `src/stim/simulators/error_matcher.test.cc`; `src/stim/simulators/matched_error.test.cc` | Structural and generated tests for optimized loop folding, all-unitary fuzz cases, analyzer/search consumption, and matched-error value-object support needed by active analyzer paths. | `pf6-sparse-rev-tracker` manifest-only row. | `pf6-sparse-rev-frame-loop` non-primary placeholder. | Heralded matching, repeat-contained noise stack frames, and `stim explain_errors` CLI remain deferred. |
| PF7-M2D-CLI | `stim m2d`; Measurement-to-detection conversion | PF7 | `stab-cli` m2d | `src/stim/cmd/command_m2d.test.cc`; `src/stim/simulators/measurements_to_detection_events.test.cc`; `doc/usage_command_line.md` | Exact and structural CLI oracle rows for formats, sweep, feedback inlining, skip-reference, append observables, side outputs, invalid paths, invalid formats, writer failure, and resource boundaries. | `pf7-m2d-cli-parity` manifest-only row plus existing M9 rows. | `pf7-cli-m2d-sweep-b8` and `pf7-cli-m2d-feedback-inline` non-primary placeholders. | `--detector_hypergraph` remains excluded. |
| PF7-ANALYZE-ERRORS-CLI | `stim analyze_errors`; `analyze_errors --decompose_errors` and related flags; Circuit-to-DEM analysis | PF7 | `stab-cli` analyze_errors | `src/stim/cmd/command_analyze_errors.test.cc`; `src/stim/simulators/error_analyzer.test.cc`; `doc/usage_command_line.md` | Exact and structural CLI oracle rows for flags, decomposition, gauge handling, approximate disjoint errors, fold loops, input and output paths, stdout behavior, stderr class, and exit status. | `pf7-analyze-errors-cli-parity` manifest-only row plus existing M10 rows. | `pf7-cli-analyze-errors-generated` and `pf7-cli-analyze-errors-decompose` non-primary placeholders. | `stim explain_errors` and diagram outputs remain deferred. |
| PF7-LEGACY-DISPATCH | Legacy top-level command flags | PF7 | `stab-cli` dispatch | `src/stim/main_namespaced.test.cc`; `src/stim/main_namespaced.perf.cc`; `doc/usage_command_line.md` | CLI tests for accepted aliases, conflicts between multiple modes, unknown legacy flags, help normalization, and explicit rejection or absence of excluded aliases. | `pf7-legacy-dispatch-parity` manifest-only row. | `pf7-cli-legacy-dispatch-startup` non-primary placeholder. | Deprecated aliases not selected for Stab remain excluded, including `--detector_hypergraph`. |

## Deferred-Only Or Excluded Partial Rows

| Checklist row | Reason this is not active PF work | Evidence to keep current |
| --- | --- | --- |
| Single-shot interactive tableau simulator | The remaining public `TableauSimulator` product is a Python-style simulator API, while current internals only support evidence and implemented workflows. | Keep checklist row partial or deferred until a public Rust simulator API plan exists. |
| Batched flip-frame simulator | The remaining public `FlipSimulator` product is binding-style API parity, while current internals support sampling and detection. | Keep checklist row partial or deferred until a public simulator API plan exists. |
| Error explanation value objects | Active RPF6 may harden value objects only when analyzer or search needs them, but full ErrorMatcher provenance and `stim explain_errors` CLI are explicitly deferred. | Keep full provenance and CLI rows deferred. |
| Public borrowed Pauli string refs | M6 intentionally chose owned Rust APIs instead of binding-oriented borrowed refs. | Keep deferred. |
| Graph simulator and vector simulator public APIs | M12 uses scoped cross-checks only. New public APIs are out of scope. | Keep deferred. |
| Circuit exports | QASM, Quirk, Crumble, and diagrams are future surfaces. | Keep deferred. |
| DEM diagrams | Rendering surfaces are future work. | Keep deferred. |
| Generated API docs or machine-readable feature matrix | This row is `Missing`, not `Partial`; it is useful tooling work but not part of the current partial-feature closure set. | Plan separately if documentation generation becomes a product requirement. |

## RPF0 Manifest Rows

The earlier PF0 pass added manifest-only oracle rows with ids beginning `pf1-` through `pf7-` and non-primary benchmark placeholders with ids beginning `pf1-` through `pf7-`.
RPF0 keeps those historical ids as source-owned extraction contracts and adds any missing placeholders promised by `docs/plans/remaining-partial-feature-milestones.md`.
These rows are intentionally not implementation evidence.
They are extraction contracts: before a matching RPF milestone claims implementation, the matching manifest-only row must be replaced or supplemented by executable exact, structural, property, statistical, or benchmark evidence.

Benchmark PF rows use `non-primary-report-only` and `contract-only`.
They must not enter the primary 1.25x gate until a later implementation milestone adds a real runner, classifies the benchmark, collects stable comparable evidence, and updates profiler notes.

## RPF0 Done Criteria

RPF0 is complete when:

- This inventory covers every active partial row and every deferred-only partial row in `docs/stab-feature-checklist.md`.
- `oracle/fixtures/manifest.csv` has manifest-only PF rows for each active implementation work item.
- `benchmarks/manifest.csv` has non-primary PF benchmark placeholders for each milestone that will need performance evidence.
- Oracle and benchmark tooling can parse PF milestones without adding them to the current M12 primary gate.
- `just oracle::list`, `just oracle::matrix --check`, `cargo test -p stab-oracle fixtures --quiet`, and `just bench::list` pass.
