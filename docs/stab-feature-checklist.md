# Stab Feature Checklist Against Stim v1.16.0

This checklist maps the Stim v1.16.0 inventory in [stim-feature-list.md](stim-feature-list.md) onto the current Stab codebase.
It is a feature-availability document, not a new roadmap.
Use [plans/non-deferred-partial-feature-milestones.md](plans/non-deferred-partial-feature-milestones.md), [plans/rust-stim-drop-in-rewrite.md](plans/rust-stim-drop-in-rewrite.md), and [plans/GOAL.md](plans/GOAL.md) for active implementation goals.

Status key:

- `Done`: implemented for the current Rust core or CLI surface and backed by source, tests, oracle rows, reports, or benchmark evidence.
- `Partial`: implemented for an explicit subset, with remaining Stim parity gaps or public API gaps named.
- `Deferred`: intentionally outside the current beta surface or future-plan scope.
- `Missing`: no implementation found in the current Stab tree and not clearly documented as deferred.

## 1. Top-Level Product Surface

| Stim feature area | Stab status | Evidence and notes |
| --- | --- | --- |
| Rust core library equivalent for core Stim semantics | Partial | `stab-core` exposes circuit, gate, target, result-format, sampler, detector-conversion, DEM, analyzer, search, and stabilizer APIs through [../crates/stab-core/src/lib.rs](../crates/stab-core/src/lib.rs). It is substantial but not a complete public API clone of Stim's Python API. |
| CLI binary | Partial | `stab-cli` implements `stab gen`, `stab convert`, `stab sample`, `stab detect`, `stab m2d`, `stab analyze_errors`, and `stab sample_dem` in [../crates/stab-cli/src/lib.rs](../crates/stab-cli/src/lib.rs). The binary is named `stab`, while drop-in `stim` packaging or aliasing remains outside this checklist unless the release plan adds it. |
| `.stim`, `.dem`, and result-format compatibility | Partial | `.stim` core is done, `.dem` core is partial for scoped analysis and sampling, and result formats are done for implemented CLI paths with some command-specific gaps. See sections 2, 5, 6, and 11. |
| Python package `stim` | Deferred | Python bindings are explicitly future work in [plans/rust-stim-drop-in-rewrite.md](plans/rust-stim-drop-in-rewrite.md). No `pyo3`, `maturin`, Python package, or Python API compatibility layer is present. |
| JavaScript/WASM API | Deferred | JS/WASM is explicitly future work in [plans/rust-stim-drop-in-rewrite.md](plans/rust-stim-drop-in-rewrite.md). |
| Ecosystem integrations | Deferred | `stimcirq`, `sinter`, Crumble, `stimflow`, ZX, lattice-surgery helpers, QASM, and Quirk are future or out-of-scope ecosystem surfaces. |
| GPU backend | Deferred | GPU acceleration is explicitly future work, and the current project focuses on CPU portable SIMD first. |

## 2. File Formats And IO Contracts

### 2.1 `.stim` Circuit Format

| Feature | Stab status | Evidence and notes |
| --- | --- | --- |
| Line-based `.stim` parsing, comments, tags, args, targets, and repeat blocks | Done | Implemented in [../crates/stab-core/src/circuit.rs](../crates/stab-core/src/circuit.rs), [../crates/stab-core/src/target.rs](../crates/stab-core/src/target.rs), and [../crates/stab-core/src/gate.rs](../crates/stab-core/src/gate.rs). M4 completion evidence is in [plans/m4-completion-report.md](plans/m4-completion-report.md). |
| Canonical `.stim` printing | Done | `Circuit::to_stim_string` is implemented in [../crates/stab-core/src/circuit.rs](../crates/stab-core/src/circuit.rs). M4 reports canonical printer oracle and benchmark evidence. |
| Target kinds | Partial | Qubit, inverted qubit, measurement record, sweep bit, Pauli, inverted Pauli, and combiner parsing are implemented in [../crates/stab-core/src/target.rs](../crates/stab-core/src/target.rs). Sweep-conditioned `m2d` conversion, non-frame and selected frame-path `detect` sampling with omitted all-false sweep bits, and selected analyzer sweep-control no-op plus invalid target-position rejection behavior are implemented for current execution subsets, while broader analyzer sweep behavior, typed `detect` sweep input, and broader sweep target shapes remain partial. |
| Broadcast and validation behavior | Done for parser and gate validation | Gate arity, target rules, Pauli product grouping, disjoint segments, probabilities, and typed arguments are covered by [../crates/stab-core/src/gate.rs](../crates/stab-core/src/gate.rs), [../crates/stab-core/src/circuit.rs](../crates/stab-core/src/circuit.rs), and M4 tests. |
| Full semantic execution of every legal circuit operation | Partial | Implemented where owned by sampler, detection, analyzer, generation, and algebra milestones. Unsupported or deferred semantics are documented in the relevant sections below. |

### 2.2 `.dem` Detector Error Model Format

| Feature | Stab status | Evidence and notes |
| --- | --- | --- |
| DEM parser and canonical printer | Partial | Implemented in [../crates/stab-core/src/dem.rs](../crates/stab-core/src/dem.rs), with PF4-owned malformed-input and public-constructor validation coverage. M10 marks `.dem` parser and printer complete for the scoped M10 contract, but broader full API parity remains partial. |
| DEM instruction types | Done for core types | `error`, `detector`, `logical_observable`, `shift_detectors`, separators, and repeat blocks are implemented in [../crates/stab-core/src/dem.rs](../crates/stab-core/src/dem.rs), including Stim-compatible exact-one-target validation for `detector` and `logical_observable`. |
| DEM detector shifts, observables, coordinates, and counts | Partial with Rust count and coordinate subset | `count_detectors`, `count_observables`, `count_errors`, `total_detector_shift`, folded `final_coordinate_shift`, all-detector coordinate maps, selected-detector coordinate maps, and single-detector lookup are implemented with `DemDetectorId`. All-detector coordinate maps reject models above 1,000,000 detectors and point callers to selected-detector queries; selected and single-detector lookups use folded repeat indexing for non-overlapping repeat declarations, preserve first-declaration behavior for bounded overlapping repeats, algebraically find flat sparse overlapping repeat declarations beyond the previous candidate cap, return empty coordinates for valid flat sparse holes, and keep many-selected flat-overlap lookups on a one-pass body scan. Exact Python API shape and full folded traversal for every nested or non-flat ambiguous large-repeat coordinate-map case remain broader work. |
| DEM flattening and large repeat traversal | Partial | Public graphlike, hypergraph, SAT, analyzer, matcher, and sampler paths have explicit expansion limits. DEM graphlike, hypergraph, SAT, analyzer, ErrorMatcher, and sampler repeat handling now have PF4 source-owned cap tests and report-only benchmark evidence. M12 removed materialized CLI output limits for implemented streaming paths, but full folded traversal for every DEM operation is not complete. |

### 2.3 Shot And Result Data Formats

| Feature | Stab status | Evidence and notes |
| --- | --- | --- |
| `01` dense text | Done for implemented paths | Implemented in [../crates/stab-core/src/result_formats.rs](../crates/stab-core/src/result_formats.rs) and streaming readers in [../crates/stab-core/src/result_streaming.rs](../crates/stab-core/src/result_streaming.rs). |
| `b8` dense binary | Done for implemented paths | Implemented for sampling, detection, m2d input/output where accepted, and DEM sampling side streams. |
| `r8` sparse binary | Done for implemented paths | Implemented in result-format readers and writers and covered by M11 sample_dem side-stream tests. |
| `hits` sparse text or binary sparse index format | Done for implemented paths | Implemented by result-format readers and writers for measurement, detection, and DEM-sampling flows where accepted. |
| `dets` sparse text | Done for implemented paths | Implemented with command-specific measurement-only, typed `convert` layout, and detection-output behavior. `sample_dem --out_format=dets` is fixed to keep detector output separate from observable output. |
| `ptb64` transposed packed format | Done for implemented paths | Separate helper APIs support `ptb64`; CLI `sample`, `convert`, `detect`, `sample_dem`, and `m2d` input support relevant `ptb64` surfaces. `m2d --out_format=ptb64` and `--obs_out_format=ptb64` intentionally reject like pinned Stim v1.16.0. |
| Format conversion command coverage | Done for result-format CLI parity | `stab convert` supports `.stim -> .stim` canonicalization as a Stab extension and Stim-style result-format conversion for `01`, `b8`, `r8`, `hits`, `dets`, and `ptb64` with explicit counts, `--dem`, `--circuit`, `--types`, `--obs_out`, and `--obs_out_format`. |
| Streaming IO for large implemented CLI paths | Done for current post-beta surfaces | `stab sample`, `stab sample_dem`, `stab detect`, and implemented `stab m2d` paths stream output or input in bounded chunks. See [plans/post-beta-fix-report.md](plans/post-beta-fix-report.md). |

## 3. Gate And Instruction Surface

| Feature | Stab status | Evidence and notes |
| --- | --- | --- |
| 81 canonical Stim v1.16.0 gates | Done for registry and validation | `GATES` in [../crates/stab-core/src/gate.rs](../crates/stab-core/src/gate.rs) contains the canonical table excluding `NOT_A_GATE`, with categories, argument rules, target rules, and inverse names. |
| Parser aliases | Done | Aliases such as `MZ`, `MRZ`, `RZ`, `CNOT`, `ZCX`, `H_XZ`, `CORRELATED_ERROR`, `SQRT_Z`, and `SWAPCZ` are handled in [../crates/stab-core/src/gate.rs](../crates/stab-core/src/gate.rs). |
| Gate validation flags and categories | Done for current Rust metadata surface | Stab has typed validation for argument counts, target categories, grouping, probabilities, and selected metadata. Rust `Gate` exposes aliases, argument rules, target rules, target grouping, fusing, noisy/reset/measurement/unitary/single-qubit/two-qubit/target-capability/symmetry flags, unitary inverse, generalized inverse, local Clifford tableau metadata, Stim v1.16.0 `GateData.flows` metadata for tableau-backed unitary gates plus representative measurement-rich and variable-target gates, fixed-shape one- or two-qubit `GateUnitaryMatrix` metadata, and Stim v1.16.0 H/S/CX/M/R `GateDecomposition` metadata for the owned PF1 subset, with `pf1-gate-metadata-api` evidence. Full Python `GateData` object parity remains deferred, and `SPP` or `SPP_DAG` flow metadata does not imply sampler, detector-conversion, or analyzer execution support. |
| Gate semantic execution | Partial | Execution exists for the subsets owned by algebra, sampler, detector conversion, analyzer, and DEM work. The PF3 fixed-tableau contract covers all 46 fixed-tableau gates through inverse-canceling sampler, detection-conversion, and analyzer circuits via `pf3-gate-semantic-wide-rust`. `SPP` and `SPP_DAG` parse and have decomposition metadata but are explicit sampler, detection-conversion, and analyzer execution rejections until later gate-execution milestones implement them. Full public interactive simulator parity is not complete. |

## 4. Core Circuit Features

| Feature | Stab status | Evidence and notes |
| --- | --- | --- |
| Circuit construction from parsed text | Done | `Circuit::from_stim_str`, item storage, repeat blocks, instructions, tags, and canonical printing are implemented in [../crates/stab-core/src/circuit.rs](../crates/stab-core/src/circuit.rs). |
| Programmatic mutation | Done for current Rust API surface | `append_instruction`, `append_repeat_block`, `append_from_stim_text`, `append_from_stim_program_text`, bounded path-based `from_stim_file`, streaming `write_stim_file`, `append_circuit`, `concatenated`, `repeated`, `repeat_in_place`, `insert_item`, `insert_instruction`, `insert_repeat_block`, `insert_circuit`, `pop_item`, `pop_last_item`, derived `Clone`, and `clear` exist and are covered by `pf1-circuit-rust-api`. Python operator ergonomics remain deferred. |
| Core introspection | Done for current Rust API surface | `count_qubits`, top-level `len`/`is_empty`, `iter_items`, `item_range`, `instruction_range`, lazy flattened forward and reverse instruction iterators, `count_measurements`, `count_detectors`, `count_observables`, `count_ticks`, `count_sweep_bits`, `measurement_record_count`, `detection_record_width`, and related helpers exist and are covered by `pf1-circuit-rust-api`. Python binding-style indexing and full Python property parity are deferred. |
| Circuit coordinate queries | Done for current Rust API surface | Coordinates are parsed and used in analyzer and detection workflows. Rust `Circuit` exposes folded `final_coordinate_shift`, `final_qubit_coordinates`, `detector_coordinates`, `detector_coordinates_for`, and `coordinates_of_detector` using `CircuitDetectorId`, with `pf1-circuit-rust-api` coverage. Rust coordinate queries reject non-finite folded coordinate results as documented hardening; exact Python-style API shape and exact C++ infinity behavior remain deferred or logged. |
| Repeat handling | Partial | Repeat blocks parse, print, sample, analyze, convert, and participate in the Rust `Circuit::flattened`, `Circuit::flattened_operations`, and `Circuit::without_noise` transform subset. Materialized flattening is capped at one million output operations while shift-only repeats are folded; fully folded traversal across every transform remains partial. |
| Circuit transforms | Partial | `without_tags`, `flattened`, `flattened_operations`, `without_noise`, `decomposed`, `to_tableau`, `inverse_unitary`, `inverse_qec`, scoped unitary and selected single-instruction measurement-rich `time_reversed_for_flows`, `simplified`, and a scoped `circuit_with_inlined_feedback` helper for `m2d --ran_without_feedback` are implemented, including the supported MPP feedback-transform case. `flattened` applies Stim-style coordinate shifts, drops `SHIFT_COORDS`, preserves instruction tags, drops repeat tags, and rejects excessive materialized expansion; `without_noise` strips measurement noise, drops ordinary noise, and converts heralded noise to deterministic `MPAD` records; `decomposed` covers public ISWAP, MPP, SPP, pair-measurement, tag-preservation, noise-preservation, annotation-preservation, constant-MPP, and anti-Hermitian rejection cases. The feedback helper rejects repeat blocks and unsupported classical controlled gates until exact loop refolding is planned. The flow time-reversal helper validates unsigned Pauli-only flows with bounded tableau validation or folded sparse validation for supported large repeats, then returns the inverse circuit with swapped flow endpoints for unitary circuits, including idle far-qubit and folded large-repeat H, SQRT_X, and CY cases; folded validation supports the full single-qubit Clifford gate set, `CX`/`CY`/`CZ`, and fixed two-qubit tableau-backed Clifford gates with plain qubit-pair targets inside promoted repeat bodies. It also validates selected measurement-rich flows through the sparse tracker and reverses flows for one noiseless plain `M`, `MX`, `MY`, `MXX`, `MYY`, or `MZZ` instruction group. Broader measurement-rich QEC transforms, resets, detectors, feedback, noise, repeats, and multi-instruction rewrites remain incomplete. |
| Reference samples and determined measurements | Done for current Rust API surface | `Circuit::reference_sample`, `Circuit::reference_sample_tree`, `Circuit::count_determined_measurements`, `CompiledSampler::reference_sample`, `ReferenceSampleTree`, and the free `count_determined_measurements` helper expose the current Rust subset and are covered by `pf1-circuit-rust-api`. Python bit-packed return shapes and full Python API parity are deferred. |
| Circuit exports | Deferred | QASM, Quirk, Crumble, and diagram exports are explicitly future work. |
| Built-in circuit generation | Done for current CLI and core generator surface | Repetition-code, rotated/unrotated surface-code, and color-code generation are implemented in [../crates/stab-core/src/circuit_generation.rs](../crates/stab-core/src/circuit_generation.rs) and `stab gen`. See [plans/m7-completion-report.md](plans/m7-completion-report.md). |

## 5. Detector Error Model Features

| Feature | Stab status | Evidence and notes |
| --- | --- | --- |
| DEM construction and mutation | Done for current Rust API surface | `DetectorErrorModel::new`, `len`, `is_empty`, `clear`, `append_from_dem_text`, `push_instruction`, `push_repeat_block`, `DemInstruction` constructors, `DemRepeatBlock::new`, typed `Probability`, typed `RepeatCount`, and derived `Clone` exist and are covered by `pf1-dem-rust-api`. Python-style list operations, concatenation operators, repetition operators, and exact Python API shape remain deferred. |
| DEM introspection | Partial with Rust count, coordinate, and iterator subset | Detector counts, observable counts, error counts, final detector shifts, folded final coordinate-shift vectors, detector-coordinate maps, top-level item iteration, validated item ranges, instruction-only ranges, typed item downcasts, and lazy adjusted flattened instruction iteration are implemented. Exact Python API parity and complete folded large-repeat traversal across every DEM consumer are not complete. |
| DEM transforms | Partial with Rust transform subset | Scoped flattening and repeat traversal exist for search, SAT, analyzer, matcher, and sampler paths, `DetectorErrorModel::without_tags`, `DetectorErrorModel::flattened`, and `DetectorErrorModel::rounded` are available, and `iter_flattened_instructions` exposes a lazy adjusted traversal for Rust callers. Materialized `flattened` preserves instruction tags, drops repeat tags and shift instructions, applies detector and coordinate shifts, and rejects excessive repeat expansion with a documented cap; `rounded` rounds only error probabilities and preserves repeat structure. Full transform API parity and folded traversal across every DEM consumer are not complete. |
| DEM sampling | Done for current Rust and CLI surface | `CompiledDemSampler` and `stab sample_dem` support deterministic, noisy statistical, observable side output, sampled-error output, replay, and current result formats. See [../crates/stab-core/src/dem_sampler.rs](../crates/stab-core/src/dem_sampler.rs), [../crates/stab-cli/src/sample_dem.rs](../crates/stab-cli/src/sample_dem.rs), and [plans/m11-progress-report.md](plans/m11-progress-report.md). |
| Streaming DEM sampling | Done for CLI; partial for public API parity | Post-beta Stab added visitor APIs and moved CLI `sample_dem` to streaming writers. Existing materialized APIs remain and keep limits. |
| DEM analysis and shortest graphlike error | Partial | Graphlike, hypergraph, SAT, and shortest-error APIs exist for scoped M10 cases in [../crates/stab-core/src/dem.rs](../crates/stab-core/src/dem/analyze](../crates/stab-core/src/dem/analyze). Full DEM diagram and public API parity is not complete. |
| DEM diagrams | Deferred | Match-graph SVG, 3D, and HTML diagrams remain future work. |

## 6. Sampling, Conversion, And Simulation Features

| Feature | Stab status | Evidence and notes |
| --- | --- | --- |
| Compiled measurement sampling | Done for current Rust and CLI surface | `CompiledSampler` in [../crates/stab-core/src/sampling](../crates/stab-core/src/sampling) and `stab sample` support deterministic and statistical sampling, output formats, seeds, skip-reference, and skip-loop-folding semantics owned by M8. |
| Measurement-sampler Python API parity | Deferred | No Python `CompiledMeasurementSampler` binding is implemented. |
| Detector sampling | Done for current CLI surface; partial public API parity | `sample_detection_events`, `try_for_each_sampled_detection_event`, and `stab detect` are implemented in [../crates/stab-core/src/detection.rs](../crates/stab-core/src/detection.rs) and [../crates/stab-cli/src/detection.rs](../crates/stab-cli/src/detection.rs). Non-frame and selected frame-path sweep-conditioned circuits use omitted all-false sweep bits for the current execution subset, including frame-path Pauli-observable circuits with sweep-controlled `CX` and `CY` qubit targets, sweep-controlled `CZ`, and `CZ` bit/bit no-op groups. Full Python `CompiledDetectorSampler` API parity and typed `detect` sweep input are deferred. |
| Measurement-to-detection conversion | Partial | `CompiledDetectionConverter`, `convert_measurements_to_detection_events`, additive sweep-aware conversion, streaming visitor conversion, and `stab m2d` are implemented for the current detector-conversion subset. `m2d --sweep`, `--sweep_format`, and scoped `--ran_without_feedback` are supported, while full transform API parity, exact loop refolding, and broader sweep-conditioned simulator surfaces remain partial. |
| Detector-analysis utility APIs | Partial | `circuit_detecting_regions` supports the simple H/CX/MXX detecting-region case with typed detector and tick inputs, bounded repeat-tick traversal, additive detector and logical-observable `DemTarget` filters, dense-capped default-like all-target/all-tick helpers, the pinned `MX` and `MZZ` detecting-region examples, generated repetition-code all-target and all-tick selection with selected exact detector and observable regions, the full single-qubit Clifford gate set with plain qubit targets, fixed two-qubit tableau-backed Clifford gates with plain qubit-pair targets, and ignored anticommutation mode for the current supported detector-region subset. `missing_detectors` supports basic reset/measure suggestions plus Gaussian row reduction for multi-record detector rows, repeated MPP and pair-measurement stabilizer-product cases, record-only observable rows, ignored Pauli observable rows, tableau-backed single-qubit and fixed two-qubit Clifford propagation with plain qubit target groups, bounded repeat traversal with explicit expansion caps, and the pinned honeycomb and toric generated-code suffix cases, and `circuit_with_inlined_feedback` covers the supported MPP feedback-transform case. Broader detecting-region target shapes, broader generated-code regions, gauge handling, broader generated-code missing-detector suffix analysis, folded large-repeat traversal beyond current caps, broader composed flow generator solving, transform integration, and exact loop refolding remain future work. |
| DEM sampling | Done for current Rust and CLI surface | See section 5 and M11 evidence. |
| Single-shot interactive tableau simulator | Deferred | Core tableau algebra and some simulator semantics exist for implemented workflows, but a public Python-style `TableauSimulator` API or new public Rust simulator product is outside the active non-deferred scope. Scoped simulator cross-checks exist in M12 for evidence, not as a public simulator product. |
| Batched flip-frame simulator | Deferred | Internal frame-simulator and packed-frame paths support sampling, detection, and Pauli-target observable cases, but a full public Python-style `FlipSimulator` API or new public Rust simulator product is outside the active non-deferred scope. |
| Exact random-stream parity with Stim | Deferred | The project explicitly requires statistical and semantic equivalence, not exact random-stream reproduction. |

## 7. Error Analysis, Search, And Decoder Configuration

| Feature | Stab status | Evidence and notes |
| --- | --- | --- |
| Circuit-to-DEM analysis | Partial | `circuit_to_detector_error_model` and `stab analyze_errors` support the staged M10 surface, including default analysis, decomposition options, fold loops, gauge detectors, approximate disjoint errors, selected analyzer sweep-control no-op behavior, and invalid controlled-Pauli sweep target-position rejection. The RPF6 generated-QEC semantic subset covers noisy generated repetition-code and rotated-surface-code circuits through `pf6-analyzer-generated-qec-rust`, and the selected loop-folded decomposition subset covers repeated composite-error decomposition plus remnant-edge blocking through `pf6-error-decomp-loop-folded-rust`. Broader generated-loop analyzer behavior remains active. See [../crates/stab-cli/src/analyze_errors.rs](../crates/stab-cli/src/analyze_errors.rs), [plans/m10-progress-report.md](plans/m10-progress-report.md), and [plans/rpf6-analyzer-progress-report.md](plans/rpf6-analyzer-progress-report.md). |
| `analyze_errors --decompose_errors` and related flags | Partial | Implemented for scoped M10 cases, including `--block_decompose_from_introducing_remnant_edges`, `--ignore_decomposition_failures`, approximate disjoint options, and the selected `--fold_loops` plus `--decompose_errors` repeated composite-error and remnant-edge blocking cases. Full upstream analyzer parity remains broader than the current proof set. |
| Error explanation value objects | Partial | Matched-error value objects and `explain_errors_from_circuit` exist in `stab-core`, but full ErrorMatcher provenance, generated surface-code repeat matching, heralded matching, and repeat-contained noise stack frames remain deferred. |
| `stim explain_errors` CLI | Deferred | Explicitly deferred in [plans/rust-stim-drop-in-rewrite.md](plans/rust-stim-drop-in-rewrite.md) and [plans/post-beta-fix-report.md](plans/post-beta-fix-report.md). |
| Shortest graphlike and hypergraph logical-error search | Partial | Scoped direct DEM graphlike and hypergraph search are implemented and tested under M10. The PF6 generated-QEC search subset covers rotated-surface-code and repetition-code graphlike and hypergraph search instruction counts plus ungraphlike generated DEM rejection, and selected generated-QEC SAT/WCNF structural encoding is covered by `pf6-search-generated-sat-wcnf-rust`. Broader generated-circuit search families, broader generated SAT or WCNF families, loop-folded generated search, ordering-insensitive result comparators beyond instruction counts, and full provenance remain future work. |
| SAT or WCNF encoding | Done for scoped API | `shortest_error_sat_problem` and `likeliest_error_sat_problem` are exposed through [../crates/stab-core/src/lib.rs](../crates/stab-core/src/lib.rs) and covered by M10. |
| Sparse reverse detector-frame tracking | Partial | A staged internal subset is implemented for M10, PF5, and PF6, including generic reverse propagation for the full single-qubit Clifford gate set, fixed two-qubit tableau-backed Clifford gates, `CX`/`CY`/`CZ` feedback-capable reverse propagation, unsigned `SPP` and `SPP_DAG` product propagation, and supported-Clifford unitary-repeat folding for the full single-qubit Clifford gate set plus fixed two-qubit tableau-backed Clifford gates with plain qubit-pair targets. The PF6 evidence now includes deterministic generated supported-unitary loops, nested repeats, multi-target single-qubit instructions, multi-pair two-qubit instructions, no-fold traversal comparisons, public unsigned-flow consumption, unsigned `SPP` or `SPP_DAG` flow propagation through the public helper path, anti-Hermitian `SPP` rejection, and the report-only `pf6-sparse-rev-frame-loop` benchmark. Analyzer/search consumption beyond the promoted unsigned-flow path, broader variable-target unitary execution semantics outside this unsigned tracker path, active matched-error hardening, and full ErrorMatcher provenance remain future work. |

## 8. Stabilizer Algebra Features

| Feature | Stab status | Evidence and notes |
| --- | --- | --- |
| Owned Pauli strings | Done for scoped Rust algebra surface | `PauliString`, parsing, multiplication, commutation, iteration, and text round trips are implemented in [../crates/stab-core/src/stabilizers](../crates/stab-core/src/stabilizers). See [plans/m6-completion-report.md](plans/m6-completion-report.md). |
| Flexible Pauli strings | Done for scoped Rust algebra surface | `FlexPauliString` is exposed and covered by M6. |
| Clifford strings | Done for scoped Rust algebra surface | `CliffordString` is exposed and covered by M6, with M12 direct benchmark optimizations. |
| Tableaux and tableau iterators | Done for scoped Rust algebra surface | `Tableau`, `TableauIterator`, composition, inversion, named gates, and conversions needed by current workflows are implemented. |
| Flows | Partial | `Flow`, unitary circuit flow generators, scoped measurement, reset, pair-measurement, nonconstant and constant `MPP`, composed measurement-rich instruction sequences without ordinary unitary mixing, feedback, `MPAD`, and promoted heralded-noise MPP flow generators, scoped `solve_for_flow_measurements` examples, scoped unitary and selected single-instruction measurement-rich `time_reversed_for_flows`, Stim v1.16.0 `GateData.flows` metadata for implemented gate-table metadata shapes, unsigned `check_if_circuit_has_unsigned_stabilizer_flows` measurement-record and observable dependency cases, and unsigned `circuit_has_unsigned_stabilizer_flow` plus `circuit_has_all_unsigned_stabilizer_flows` helpers are implemented for the scoped Rust semantics. Broader all-operation composed measurement-rich `flow_generators`, broader heralded-noise generator synthesis, full generator-table measurement solving, broader measurement-rich transform integration, signed sampled flow checking, and Python API parity remain deferred or active follow-up work. See [plans/rpf5-flow-progress-report.md](plans/rpf5-flow-progress-report.md). |
| Public borrowed Pauli string refs | Deferred | M6 explicitly uses owned Rust APIs instead of cloning Stim's borrowed binding-oriented shape. |
| Conversion to or from numpy, state vectors, and arbitrary unitaries | Deferred or partial | Some unitary-to-tableau and small cross-check subsets exist. Full numpy, state-vector, arbitrary unitary, and `tableau_to_unitary` parity is deferred. |
| Graph simulator and vector simulator public APIs | Deferred | M12 added scoped cross-check tests, but new public graph/vector simulator APIs are explicitly deferred. |

## 9. Diagrams And Visualization

| Feature | Stab status | Evidence and notes |
| --- | --- | --- |
| Circuit timeline diagrams | Deferred | `stim diagram` is intentionally deferred. |
| Detector-slice diagrams | Deferred | Diagram surfaces are future work. |
| DEM match-graph diagrams | Deferred | DEM diagram surfaces are future work. |
| Interactive HTML and Crumble diagram surfaces | Deferred | Crumble and interactive diagrams are future ecosystem or visualization work. |

## 10. Built-In Circuit Generation

| Feature | Stab status | Evidence and notes |
| --- | --- | --- |
| `repetition_code:memory` | Done | Implemented in core generation and `stab gen`, with exact oracle rows and structural generator tests. |
| `surface_code:rotated_memory_x` and `surface_code:rotated_memory_z` | Done | Implemented and covered by M7 exact and structural evidence. |
| `surface_code:unrotated_memory_x` and `surface_code:unrotated_memory_z` | Done | Implemented and covered by M7 exact and structural evidence. |
| `color_code:memory_xyz` | Done | Implemented and covered by M7 exact and structural evidence. |
| Generator parameters and noise knobs | Done for M7 surface | Distance, rounds, output path, and supported noise knobs are implemented through typed generator parameters in [../crates/stab-core/src/circuit_generation.rs](../crates/stab-core/src/circuit_generation.rs). |
| Python `stim.Circuit.generated` | Deferred | No Python bindings yet. |

## 11. Command-Line API

| Stim command | Stab status | Evidence and notes |
| --- | --- | --- |
| `stim gen` | Done for supported M7 families and tasks | `stab gen` and legacy `--gen` are implemented in [../crates/stab-cli/src/lib.rs](../crates/stab-cli/src/lib.rs). |
| `stim convert` | Done for result-format CLI parity plus Stab `.stim` extension | `stab convert` supports `01`, `b8`, `r8`, `hits`, `dets`, and `ptb64` conversions with explicit counts, `--dem`, `--circuit`, unique `--types` letters, `--obs_out`, and `--obs_out_format`. The Stab-specific `.stim -> .stim` canonical conversion remains supported only for `--in_format=stim --out_format=stim`. |
| `stim sample` | Done for current M8/M12 surface | `stab sample` supports current flags, deterministic and noisy statistical behavior, output formats, seed handling, skip-reference, skip-loop-folding, and streaming output. |
| `stim detect` | Done for current M9/M12 surface | `stab detect` supports shots, input/output paths, detector/observable routing, relevant formats including supported `ptb64`, seeds, and streaming output. |
| `stim m2d` | Done for selected PF7 CLI surface | `stab m2d` supports text, `b8`, `r8`, `hits`, `dets`, and `ptb64` input where accepted, plus detector and observable outputs where accepted. `--sweep` and `--sweep_format` stream sweep records for the current sweep-conditioned conversion subset, scoped `--ran_without_feedback` applies feedback inlining before conversion, and `ptb64` output is rejected like pinned Stim v1.16.0. PF7 selected CLI evidence covers `--circuit`, `--in`, `--out`, `--sweep`, and `--obs_out` path IO, path-error precedence before converter setup, selected default detector-only `dets` output, `--append_observables`, `--skip_reference_sample`, observable side-output widths, Pauli-target observable annotations, selected format and width failures, writer failure propagation, stdout behavior, stderr class, exit status, and existing M9 sweep, feedback, format, and resource behavior. Broader detector-converter API parity remains tracked by the measurement-to-detection conversion row. |
| `stim analyze_errors` | Done for selected PF7 CLI surface | `stab analyze_errors` implements the staged M10 flag surface and current analyzer subset, with PF7 path-IO evidence for `--in` and `--out` success, missing input paths, output-open precedence, stdout behavior, stderr class, and exit status. PF7 flag evidence covers selected default, `--fold_loops`, `--allow_gauge_detectors`, `--approximate_disjoint_errors`, `--decompose_errors`, remnant-edge blocking, ignored decomposition failures, invalid threshold arguments, malformed stdin, stdout behavior, stderr class, and exit status. Broader generated-loop analyzer behavior, search behavior, full ErrorMatcher provenance, `stim explain_errors`, and deprecated `--detector_hypergraph` support remain outside this CLI closure. |
| `stim sample_dem` | Done for current M11/M12 surface | `stab sample_dem` supports detector output, observable side output, error output, replay input, result formats including `ptb64` where accepted, and streaming CLI writers. Hidden or compatibility observable-routing aliases are implementation conveniences and are not counted as additional Stim parity surface. |
| `stim diagram` | Deferred | No Stab command. |
| `stim explain_errors` | Deferred | No Stab command, despite scoped core matched-error support. |
| `stim repl` | Deferred | No Stab command. |
| `stim help` command | Done for Stab-native structural help | `stab help [topic]` and top-level `stab --help [topic]` are normalized to Stab-native help for implemented commands, result formats, and gate names. Exact byte-for-byte pinned-Stim help text remains intentionally out of scope. |
| Legacy top-level command flags | Done for selected PF7 surface | `--gen`, `--convert`, `--sample`, `--detect`, `--m2d`, and `--analyze_errors` aliases are normalized in [../crates/stab-cli/src/lib.rs](../crates/stab-cli/src/lib.rs), with PF7 executable coverage for accepted aliases, selected multiple-mode conflicts, explicit `--detector_hypergraph` rejection, help-topic absence, and fail-closed behavior for selected unimplemented legacy-style flags. Other legacy and deprecated Stim spellings are intentionally unsupported unless a future plan selects them. Users should use `stab analyze_errors` instead of deprecated analyzer spellings. |

## 12. Python API

| Python feature area | Stab status | Evidence and notes |
| --- | --- | --- |
| Top-level `stim` functions | Deferred | No Python module exists. Rust equivalents exist for some target constructors and helpers but are not exposed as Python. |
| `stim.Circuit` class | Deferred with Rust subset | `Circuit` exists as a Rust type, but Python class operators, binding-style mutation API, file-like object helpers, transforms, diagram/export methods, and properties are not bound. |
| `stim.DetectorErrorModel` class | Deferred with Rust subset | `DetectorErrorModel` exists as a Rust type with current Rust construction, introspection, coordinate, tag-stripping, materialized `flattened`, and `rounded` APIs, but Python class operators, diagrams, Python binding shape, and full method parity are not bound. |
| Sampler and converter classes | Deferred with Rust subset | Rust `CompiledSampler`, `CompiledDetectionConverter`, and `CompiledDemSampler` exist, but Python classes do not. |
| Error explanation classes | Deferred with Rust subset | Rust matched-error value objects exist for scoped M10 support, but Python `ExplainedError` and related classes are not exposed. |
| Stabilizer, gate metadata, and simulator classes | Deferred with Rust subset | Rust Pauli, Clifford, Tableau, Flow, and bounded `Gate` metadata APIs exist. Python `GateData`, `TableauSimulator`, `FlipSimulator`, numpy conversions, and full class operator parity are not exposed. |

## 13. JavaScript And WASM API

| JS/WASM feature area | Stab status | Evidence and notes |
| --- | --- | --- |
| WASM build and JS package | Deferred | No WASM build or JS package exists. |
| JS `Circuit`, `Tableau`, `PauliString`, and `TableauSimulator` | Deferred | Rust core APIs can inform future bindings, but no JS bindings are present. |
| Crumble browser editor | Deferred | Future ecosystem work. |

## 14. Ecosystem And Glue Packages

| Ecosystem surface | Stab status | Evidence and notes |
| --- | --- | --- |
| `stimcirq` | Deferred | No Cirq integration exists. |
| `sinter` | Deferred | No decoding-statistics package exists. |
| Crumble | Deferred | No browser editor integration exists. |
| `stimflow` | Deferred | No separate flow toolkit exists. |
| ZX and lattice-surgery glue | Deferred | No ZX or lattice-surgery integration exists. |
| QASM and Quirk exports | Deferred | Explicitly future work unless drop-in scope expands. |

## 15. Packaging, Build, And Documentation Surfaces

| Feature | Stab status | Evidence and notes |
| --- | --- | --- |
| Cargo workspace | Done | The repo is a Cargo workspace with `stab-core`, `stab-cli`, `stab-oracle`, `stab-bench`, and `ops` crates. |
| Operational command surface | Done | The repo uses `just` and modular `justfiles`, with complex logic in Rust ops crates. |
| Python packaging | Deferred | No `pyproject.toml` or Python package exists for Stab bindings. |
| JS packaging | Deferred | No JS package exists. |
| Generated API docs | Missing | No generated Rust API reference, Python stub, or compatibility matrix generated from public API source of truth currently exists. Existing docs and reports are handwritten. |
| Feature and roadmap docs | Done for current planning surface | Roadmap, test-porting plan, completion reports, lessons learned, GOAL, and this checklist live under [plans](plans) and [docs](.). |

## 16. Test And Benchmark Surface

| Feature | Stab status | Evidence and notes |
| --- | --- | --- |
| Upstream test inventory and porting plan | Done | [plans/stim-test-porting-plan.md](plans/stim-test-porting-plan.md) groups upstream tests and planned Rust ports. |
| Oracle fixture matrix | Done for implemented surfaces | `oracle/fixtures/manifest.csv`, `ops/oracle`, and milestone reports track exact, statistical, structural, and semantic-mining rows. |
| Benchmark manifest and primary beta gate | Done for current performance infrastructure | `benchmarks/manifest.csv`, `benchmarks/m12-primary-thresholds.json`, `benchmarks/m12-primary-beta-waivers.json`, and M12/post-beta reports provide primary benchmark evidence. |
| Current beta performance gate | Done for current report state | The expanded clean 1.25x beta evidence records 80 comparable rows passing and 5 checked no-ratio waivers across 85 primary rows. |
| Tests for deferred Python, JS, diagrams, and ecosystem packages | Deferred | These are intentionally future work and should not be used as blockers for the current Rust/CLI beta surface. |

## 17. Highest-Priority Remaining Feature Gaps

This section is a short triage view of gaps that are visible after mapping the Stim inventory onto Stab.

| Gap | Status | Why it matters |
| --- | --- | --- |
| Full circuit transform API parity | Partial | Rust `flattened`, `flattened_operations`, `without_noise`, `decomposed`, scoped unitary and selected single-instruction measurement-rich `time_reversed_for_flows`, and scoped `with_inlined_feedback` are implemented for the owned RPF2 subset, but exact feedback loop refolding, broader measurement-rich QEC flow rewrites, QASM/Quirk/Crumble export, and richer flow transforms remain absent or scoped. |
| Full DEM public API parity | Partial | Rust rounded, flattened, tag-stripping, coordinate, count, and iterator subsets exist, but diagram APIs, Python-style ergonomics, and full folded traversal across every DEM consumer remain absent or scoped. |
| Broader sweep-conditioned simulator and analysis parity | Partial | Public `m2d --sweep`, non-frame and selected frame-path `detect` omitted-sweep default-false sampling, and selected analyzer sweep-control no-op plus invalid target-position rejection behavior are implemented for current subsets, but broader analyzer sweep behavior, typed `detect` sweep input, Python APIs, and every sweep target shape remain broader work. |
| Full feedback-inlining transform parity | Partial | `m2d --ran_without_feedback`, `circuit_with_inlined_feedback`, and scoped Rust `Circuit::with_inlined_feedback` cover the supported top-level Pauli and MPP feedback subset while rejecting repeat blocks and unsupported classical controlled gates. Exact loop refolding and broader repeat-block feedback parity remain open. |
| Full ErrorMatcher provenance and `explain_errors` CLI | Deferred | Core value objects exist, but full stack-frame provenance and CLI UX remain future work. |
| Public interactive `TableauSimulator` and `FlipSimulator` APIs | Deferred | Current internals support implemented workflows, but public simulator API parity is not exposed. |
| Diagrams and visualization | Deferred | Stim's diagram command and Python diagram APIs are a large independent rendering surface. |
| Python bindings | Deferred | Needed for eventual Stim drop-in package parity, but intentionally after Rust and CLI surfaces mature. |
| JS/WASM and ecosystem packages | Deferred | Separate product and integration projects, not part of current core Rust/CLI beta. |
| Generated API reference or machine-readable feature matrix | Missing | Helpful for preventing drift as Stab grows, but not yet implemented. |

## 18. Maintenance Notes

- When a Stab feature moves from `Partial`, `Deferred`, or `Missing` to `Done`, update this checklist in the same change set as the implementation and tests.
- When a deferred surface becomes in scope, first update [plans/rust-stim-drop-in-rewrite.md](plans/rust-stim-drop-in-rewrite.md) and [plans/GOAL.md](plans/GOAL.md), then add tests or oracle rows before claiming checklist completion.
- Do not use this checklist to weaken existing milestone done criteria; milestone-specific reports remain the evidence source for whether a planned slice is complete.
