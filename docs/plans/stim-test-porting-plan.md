# Stim v1.16.0 Test Porting Plan

## Status

Created: 2026-06-26

Target: Stim v1.16.0, tag commit `e2fc1eca7fd21684d433aa5f10f4504ea4860d07`.

Pinned upstream source: `vendor/stim`.

Counts in this document are file-level counts, not individual test-case counts.

## Porting Policy

Use the following priorities when converting Stim tests into Stab tests.

| Priority | Meaning | Rust action |
| --- | --- | --- |
| P0 | Direct core port | Translate into Rust unit, property, or integration tests before or during the matching implementation milestone. |
| P1 | Oracle parity fixture | Run pinned C++ Stim and Rust Stab on the same input and compare exact, structural, or statistical output. |
| P2 | Semantic mining | Extract the behavioral contract into Rust tests, but do not mirror binding-specific Python or JS mechanics. |
| P3 | Deferred ecosystem surface | Track in the compatibility matrix, but do not port until Stab deliberately supports that ecosystem surface. |
| Bench | Benchmark source | Convert into Criterion, CLI benchmark, or `ops` benchmark workloads. |
| Skip | Not useful as a Rust test | Replace with Cargo, Rust API, documentation, or CI guarantees instead of porting literally. |

## Oracle Fixture Execution

Manifest rows that are proven by direct Rust tests can use an `argv` value starting with `cargo-test`.
The remaining `argv` tokens are passed to `cargo test` from the repository root, for example `cargo-test|-p|stab-core|--test|stim_format|target`.
Use this mode only when the row's upstream behavior has been ported into explicit Rust tests and the row's comparator is structural, property, or statistical.
Keep exact CLI or file-output compatibility rows on oracle fixtures that compare pinned Stim output against Stab output.

## Summary

| Upstream bucket | Count | Primary action |
| --- | ---: | --- |
| C++ GTest files from `vendor/stim/file_lists/test_files` | 103 | P0 and P1 for core, CLI, formats, simulators, stabilizers, DEM, and search behavior. |
| Python pytest files found by `*_test.py` | 91 | P2 for Rust-relevant core semantics, P3 for Python binding and ecosystem behavior. |
| JavaScript test files found by `*.test.js` | 19 | P3 until JS/WASM and Crumble are in scope. |
| Visual snapshot assets under `vendor/stim/testdata` | 58 | P3 until diagrams are in scope. |
| C++ perf files from `vendor/stim/file_lists/perf_files` | 23 | Bench for M3 baseline and M12 hardening. |
| Test support headers and harness files | 9 | Reuse concepts only when useful. |

## Recommended Port Order

1. M1 should create a machine-readable compatibility matrix from the hierarchy below, including priority, milestone, parity mode, and upstream path.
2. M2 should create red or manifest-only oracle cases for all P0 and P1 files needed by M4 through M11.
3. M3 should convert the `Bench` files into a pinned C++ Stim baseline before Rust implementations exist.
4. M4 through M11 should flip the relevant red parity tests green as each feature lands.
5. The Future Plan in `docs/plans/rust-stim-drop-in-rewrite.md` should revisit P3 Python, JS, Crumble, and diagram snapshot tests after the core CLI and Rust library are stable.

## C++ GTest Hierarchy

Source of truth: `vendor/stim/file_lists/test_files`.

### Top-Level Library And Build Smoke

Action: port the behavioral smoke tests, skip C++ include/linkage specifics.

```text
src/stim.test.cc
src/stim/main_namespaced.test.cc
src/stim_included_twice.test.cc
```

Priority:

- `src/stim.test.cc`: P0 library smoke and gate-data availability.
- `src/stim/main_namespaced.test.cc`: P1 CLI entry smoke.
- `src/stim_included_twice.test.cc`: Skip direct port because Cargo already protects Rust crate inclusion semantics.

### Circuit Model, Parser, Targets, And Decomposition

Action: P0 for M4 parser/model behavior, P1 for canonical print and invalid-input fixtures, P0 or P2 for decomposition behavior used by later simulators.

```text
src/stim/circuit/circuit.test.cc
src/stim/circuit/circuit_instruction.test.cc
src/stim/circuit/gate_decomposition.test.cc
src/stim/circuit/gate_target.test.cc
```

Small features:

- Circuit parsing, canonical printing, append behavior, repeat blocks, tags, coordinate annotations, copy/concat/slicing-style operations, and error reporting.
- Circuit instruction validation, argument and target rules, target separators, measurement-record targets, detector/observable targets, sweep targets, and combiner targets.
- Gate decomposition rules needed for `decomposed`, generated circuits, simulator analysis, and later QASM/export decisions.

### CLI Commands

Action: P1 oracle fixtures by command, with direct Rust integration tests around argument parsing and file I/O boundaries.

```text
src/stim/cmd/command_analyze_errors.test.cc
src/stim/cmd/command_convert.test.cc
src/stim/cmd/command_detect.test.cc
src/stim/cmd/command_diagram.test.cc
src/stim/cmd/command_explain_errors.test.cc
src/stim/cmd/command_gen.test.cc
src/stim/cmd/command_m2d.test.cc
src/stim/cmd/command_sample.test.cc
src/stim/cmd/command_sample_dem.test.cc
```

Priority:

- P1 M4/M7: `command_convert.test.cc`.
- P1 M7: `command_gen.test.cc`.
- P1 M8: `command_sample.test.cc`.
- P1 M9: `command_detect.test.cc` and `command_m2d.test.cc`.
- P1 M10: `command_analyze_errors.test.cc`.
- P1 M11: `command_sample_dem.test.cc`.
- P3: `command_diagram.test.cc` and `command_explain_errors.test.cc` because those commands are deferred.

### Detector Error Model

Action: P0 and P1 for M10.

```text
src/stim/dem/dem_instruction.test.cc
src/stim/dem/detector_error_model.test.cc
```

Small features:

- `.dem` parsing, canonical printing, repeat blocks, detector shifts, coordinates, observables, separators, probability validation, flattening, approximate equality, and structural equivalence.
- PFM-B3 adds seven independently selectable Rust tests under `crates/stab-core/tests/dem_folded_traversal.rs` for counts and shifts, coordinates, compact transforms, sampler behavior, graphlike or hypergraph search collection, SAT/WCNF collection, and ErrorMatcher filter keys. The count selector owns a 96-case deterministic Proptest corpus with seed `[0xB3; 32]` and a fixed generated domain covering nested repeats, shifts, tags, annotations, separators, and zero or deterministic active errors; it compares summaries, coordinates, transforms, deterministic sampling, search, and matcher filtering against explicitly unrolled models. Separate evidence asserts literal pinned WCNF text, ports `DemSampler.resample_combinations`, applies `1e-12` fractional-coordinate tolerance, and covers neutral repeats, declaration-count overflow, coordinate-work limits, visitor errors, and inherent-materialization caps.
- The corresponding `pfm-b3-dem-traversal-*` oracle rows are implemented Rust-test proxies. They supplement, rather than replace, the exact `.dem` parse and print oracle rows.

### Diagrams And Rendering

Action: P3 until diagram support is explicitly in scope. Preserve these as visual snapshot sources and structural examples.

```text
src/stim/diagram/ascii_diagram.test.cc
src/stim/diagram/base64.test.cc
src/stim/diagram/coord.test.cc
src/stim/diagram/detector_slice/detector_slice_set.test.cc
src/stim/diagram/graph/match_graph_3d_drawer.test.cc
src/stim/diagram/graph/match_graph_svg_drawer.test.cc
src/stim/diagram/json_obj.test.cc
src/stim/diagram/timeline/timeline_3d_drawer.test.cc
src/stim/diagram/timeline/timeline_ascii_drawer.test.cc
src/stim/diagram/timeline/timeline_svg_drawer.test.cc
```

Small features:

- ASCII, SVG, GLTF, JSON object utilities, coordinates, timeline diagrams, detector slices, and match graph diagrams.

### Gates And Gate Metadata

Action: P0 for M4 and M6.

```text
src/stim/gates/gates.test.cc
```

Small features:

- Gate names, aliases, categories, arity, argument rules, target rules, inverse metadata, local Clifford tableau metadata, tableau-backed unitary flow metadata, fixed-shape unitary matrix metadata, H/S/CX/M/R decomposition metadata, and generated gate tables.

### Circuit Generation

Action: P1 for M7 golden CLI output and P0 for Rust generator validation.

```text
src/stim/gen/circuit_gen_params.test.cc
src/stim/gen/gen_color_code.test.cc
src/stim/gen/gen_rep_code.test.cc
src/stim/gen/gen_surface_code.test.cc
```

Small features:

- Generator parameter validation, repetition code generation, rotated and unrotated surface code generation, color code generation, noise parameter placement, detector and observable structure, and repeat-block shape.

### Input And Output Formats

Action: P0 for typed format parsers and writers, P1 for CLI byte-level fixtures.

```text
src/stim/io/measure_record.test.cc
src/stim/io/measure_record_batch.test.cc
src/stim/io/measure_record_batch_writer.test.cc
src/stim/io/measure_record_reader.test.cc
src/stim/io/measure_record_writer.test.cc
src/stim/io/sparse_shot.test.cc
```

Small features:

- Measurement record formats, bit-packed formats, text formats, sparse shots, batch reading and writing, padding, endian conventions, and invalid input handling.

### Memory And Portable SIMD

Action: P0 for M5, translated into Rust scalar-vs-portable-SIMD property tests plus focused unit tests.
M5 owns the subcases that correspond to Stab's initial bit-core API: bit references, packed bit vectors, row operations, masked XOR, range XOR, transposition, copy/load-store boundaries, popcount-like helpers, twiddle helpers, and sparse XOR vectors.
Upstream subcases for APIs not introduced in M5, such as randomization, shifts, addition, table text parsing, table slicing/concatenation/resizing, lower-triangular inversion, subset/intersection predicates, and custom allocation/storage utilities, must stay explicitly deferred until Stab introduces equivalent public or simulator-facing APIs.

```text
src/stim/mem/bit_ref.test.cc
src/stim/mem/fixed_cap_vector.test.cc
src/stim/mem/monotonic_buffer.test.cc
src/stim/mem/simd_bit_table.test.cc
src/stim/mem/simd_bits.test.cc
src/stim/mem/simd_bits_range_ref.test.cc
src/stim/mem/simd_util.test.cc
src/stim/mem/simd_word.test.cc
src/stim/mem/sparse_xor_vec.test.cc
```

Small features:

- Bit references, bit ranges, packed bit tables, row operations, XOR vectors, SIMD words, alignment, boundary sizes, sparse XOR behavior, and storage utilities.

Skip or adapt:

- `fixed_cap_vector.test.cc` and `monotonic_buffer.test.cc` should only be ported if Stab introduces equivalent containers.

### Search Algorithms

Action: P0 when implementing analyzer/search internals, P1 when exposed through `shortest_graphlike_error`, `analyze_errors`, or related CLI behavior.

```text
src/stim/search/graphlike/algo.test.cc
src/stim/search/graphlike/edge.test.cc
src/stim/search/graphlike/graph.test.cc
src/stim/search/graphlike/node.test.cc
src/stim/search/graphlike/search_state.test.cc
src/stim/search/hyper/algo.test.cc
src/stim/search/hyper/edge.test.cc
src/stim/search/hyper/graph.test.cc
src/stim/search/hyper/node.test.cc
src/stim/search/hyper/search_state.test.cc
src/stim/search/sat/wcnf.test.cc
```

Small features:

- Graphlike and hypergraph search nodes, edges, graph construction, search-state transitions, logical-error search, and WCNF/SAT problem output.
- Current PFM-B5 closure is executable in `crates/stab-core/tests/dem_search_pfm_b5.rs`: seven graphlike cases, eight hypergraph cases, and ten exact shortest or weighted WDIMACS cases port the selected `algo.test.cc` and `wcnf.test.cc` behavior with independent selectors. Generated tie-sensitive results use minimum-distance and canonical target-signature invariants; deterministic direct results and WCNF text use exact assertions.

### Simulators

Action: P0 for M8 through M11, P1 for statistical and structural equivalence against C++ Stim.

```text
src/stim/simulators/dem_sampler.test.cc
src/stim/simulators/error_analyzer.test.cc
src/stim/simulators/error_matcher.test.cc
src/stim/simulators/frame_simulator.test.cc
src/stim/simulators/frame_simulator_util.test.cc
src/stim/simulators/graph_simulator.test.cc
src/stim/simulators/matched_error.test.cc
src/stim/simulators/measurements_to_detection_events.test.cc
src/stim/simulators/sparse_rev_frame_tracker.test.cc
src/stim/simulators/tableau_simulator.test.cc
src/stim/simulators/vector_simulator.test.cc
```

Small features:

- Frame simulation, frame-simulator detection-output helpers, Pauli-target observable include behavior, tableau simulation, vector simulation, graph simulation, error analysis, error matching, DEM sampling, measurement-to-detection conversion, matched errors, and sparse reverse-frame tracking.
- Current PFM-B5 analyzer closure is executable in `crates/stab-core/tests/dem_analyzer_pfm_b5.rs` plus the existing giant-loop tests. It ports nested, coordinate, gauge, generated repetition-code, loop-carried observable, period-8, period-127, and cross-iteration rejection behavior and adds sixteen seeded folded-versus-unrolled loops plus one nested coordinate differential case.
- Current M12 graph/vector coverage lives in `crates/stab-core/tests/simulator_cross_checks.rs` as scoped tableau and amplitude semantic checks adapted from `graph_simulator.test.cc` and `vector_simulator.test.cc`; broader public graph/vector simulator APIs remain deferred until Stab exposes equivalent surfaces.

### Stabilizers And Algebra

Action: P0 for M6, with additional property tests beyond literal translation.

```text
src/stim/stabilizers/clifford_string.test.cc
src/stim/stabilizers/flex_pauli_string.test.cc
src/stim/stabilizers/flow.test.cc
src/stim/stabilizers/pauli_string.test.cc
src/stim/stabilizers/pauli_string_iter.test.cc
src/stim/stabilizers/pauli_string_ref.test.cc
src/stim/stabilizers/tableau.test.cc
src/stim/stabilizers/tableau_iter.test.cc
```

Small features:

- Pauli strings, Clifford strings, flows, tableaus, iterators, reference-derived owned-API behavior, signs, commutation, products, inverses, random generation, text round trips, and conjugation behavior.

### Low-Level Utilities

Action: port only when the Rust project has equivalent behavior; prefer typed Rust APIs over C++ utility parity.

```text
src/stim/util_bot/arg_parse.test.cc
src/stim/util_bot/error_decomp.test.cc
src/stim/util_bot/probability_util.test.cc
src/stim/util_bot/str_util.test.cc
src/stim/util_bot/test_util.test.cc
src/stim/util_bot/twiddle.test.cc
```

Priority:

- P0/P1: `arg_parse.test.cc` for CLI boundary behavior, `error_decomp.test.cc` for M10, and `probability_util.test.cc` for probability validation.
- P0: `twiddle.test.cc` only if Stab implements equivalent bit tricks.
- Skip or adapt: `str_util.test.cc` and `test_util.test.cc` unless equivalent Rust helpers become public or risky.

### Top-Level Algorithms And Exports

Action: P0 or P1 when the behavior feeds supported CLI or library surfaces, P3 for deferred exports.

```text
src/stim/util_top/circuit_flow_generators.test.cc
src/stim/util_top/circuit_inverse_qec.test.cc
src/stim/util_top/circuit_inverse_unitary.test.cc
src/stim/util_top/circuit_to_dem.test.cc
src/stim/util_top/circuit_to_detecting_regions.test.cc
src/stim/util_top/circuit_vs_amplitudes.test.cc
src/stim/util_top/circuit_vs_tableau.test.cc
src/stim/util_top/count_determined_measurements.test.cc
src/stim/util_top/export_crumble_url.test.cc
src/stim/util_top/export_qasm.test.cc
src/stim/util_top/export_quirk_url.test.cc
src/stim/util_top/has_flow.test.cc
src/stim/util_top/mbqc_decomposition.test.cc
src/stim/util_top/missing_detectors.test.cc
src/stim/util_top/reference_sample_tree.test.cc
src/stim/util_top/simplified_circuit.test.cc
src/stim/util_top/stabilizers_to_tableau.test.cc
src/stim/util_top/stabilizers_vs_amplitudes.test.cc
src/stim/util_top/transform_without_feedback.test.cc
```

Priority:

- P0/P1 M6-M10: circuit-to-DEM, detecting regions, flow generators, inverse behavior, circuit-vs-tableau, count determined measurements, missing detectors, reference samples, simplification, stabilizers-to-tableau, and transform without feedback.
- PFM-B4 closes the selected detector-utility and flow-engine extraction with two focused detecting-region cases, twelve focused `missing_detectors.circuit` subcases plus honeycomb and toric cases, twenty-four independently selectable `circuit_flow_generators.various` examples, the pinned signed 40-flow `all_operations` set, split C++ and Python solver examples with every attributed subcase in its owning selector, a fixed-seed general GF(2) solver corpus, a 255-query bounded exhaustive checker differential, and pinned regressions for measurement-free mixed sweep/plain groups, folded ignored-only traversal, mixed unitary-plus-noise propagation, `MPAD` simulated-qubit width and asymmetric record association, duplicate reset and measure-reset generator semantics, gate-specific sweep-only feedback handling, Stim-compatible flow ordering, and touched-qubit unitary-repeat transforms over 65,536-wide idle or sparse high-index trackers. Every PFM-B4 case uses an exact one-test ledger selector; sources outside those ledger-owned cases remain semantic-mining inputs until a future plan selects them.
- P3: Crumble, QASM, and Quirk exports unless they become explicit drop-in goals.
- P2 Future: amplitude and state-vector checks are deferred unless a later matrix/state-vector parity plan selects exact subcases. Current M12 graph/vector coverage is limited to the scoped `simulator_cross_checks` semantic evidence and does not imply public simulator or amplitude API parity.

Current `src/stim/util_top/circuit_inverse_qec.test.cc` ownership is split across structural rows for the unitary QEC inverse subset, selected reset-measure-detector target-list rewrites, the exact two-to-one detector-flow packet, the exact `m_det` two-detector packet, the exact noiseless MPP all-record identity-parity detector-flow packet, the exact noisy `MZZ` detector-flow packet, the exact observable Pauli include packet, selected noisy `M`/`MX`/`MY` measurement-only reversal, selected noisy `MR`/`MRX`/`MRY` measure-reset-only reversal, selected exact noisy measure-reset detector-flow, selected measure-reset pass-through rewrites, selected measurement-rich `time_reversed_for_flows` including the selected `dont_turn_measurements_into_resets` single-measurement option, the selected `MZZ` plus plain-qubit unitary suffix packet, and exact pinned `flow_flip` packet, and explicit fail-closed nearby measurement-rewrite shapes.
The selected MPAD record-tail inverse plus Pauli-only, measurement-record, and observable time-reversal evidence is tracked by `pf2-inverse-qec-mpad-rust` and is derived from pinned `time_reversed_for_flows` probes plus upstream `CircuitFlowReverser::do_measuring_instruction` source inspection, with the exact probe transcript recorded in `docs/plans/pfm2-inverse-qec-mpad-scope.md`.

## Python Test Hierarchy

Source: all `*_test.py` files under `vendor/stim`.

### Stim Python Binding Tests

Action: P2 before Python bindings exist, then P3-to-P0 when the Python API milestone starts.

```text
src/stim/circuit/circuit_instruction_pybind_test.py
src/stim/circuit/circuit_pybind_test.py
src/stim/circuit/circuit_repeat_block_test.py
src/stim/circuit/gate_target_pybind_test.py
src/stim/dem/dem_instruction_pybind_test.py
src/stim/dem/detector_error_model_pybind_test.py
src/stim/dem/detector_error_model_repeat_block_pybind_test.py
src/stim/dem/detector_error_model_target_pybind_test.py
src/stim/gates/gates_test.py
src/stim/py/compiled_detector_sampler_pybind_test.py
src/stim/py/compiled_measurement_sampler_pybind_test.py
src/stim/py/stim_pybind_test.py
src/stim/simulators/dem_sampler_pybind_test.py
src/stim/simulators/frame_simulator_pybind_test.py
src/stim/simulators/matched_error_pybind_test.py
src/stim/simulators/measurements_to_detection_events_test.py
src/stim/simulators/tableau_simulator_pybind_test.py
src/stim/stabilizers/clifford_string_pybind_test.py
src/stim/stabilizers/flow_pybind_test.py
src/stim/stabilizers/pauli_string_pybind_test.py
src/stim/stabilizers/tableau_pybind_test.py
src/stim/util_top/circuit_flow_generators_test.py
src/stim/util_top/circuit_inverse_qec_test.py
src/stim/util_top/circuit_to_detecting_regions_test.py
src/stim/util_top/export_crumble_url_pybind_test.py
src/stim/util_top/export_qasm_pybind_test.py
src/stim/util_top/export_quirk_url_pybind_test.py
```

Rust extraction targets:

- Mine object semantics, constructors, parse and print behavior, equality, approximate equality, iteration, indexing, slicing, error cases, and sampler contracts into Rust tests.
- Do not port Python-specific repr, pickle, NumPy array layout, Python exception exact text, or Python mutability behavior until `pyo3` bindings exist.
- Defer export-specific tests for Crumble, QASM, and Quirk unless those surfaces become drop-in requirements.

### Cirq Integration Tests

Action: P3. Use selected examples as parser/sampler fixtures only when they cover core Stim behavior better than C++ tests do.

```text
glue/cirq/stimcirq/_cirq_to_stim_test.py
glue/cirq/stimcirq/_cx_swap_test.py
glue/cirq/stimcirq/_cz_swap_test.py
glue/cirq/stimcirq/_det_annotation_test.py
glue/cirq/stimcirq/_feedback_pauli_test.py
glue/cirq/stimcirq/_i_error_gate_test.py
glue/cirq/stimcirq/_ii_error_gate_test.py
glue/cirq/stimcirq/_ii_gate_test.py
glue/cirq/stimcirq/_measure_and_or_reset_gate_test.py
glue/cirq/stimcirq/_obs_annotation_test.py
glue/cirq/stimcirq/_shift_coords_annotation_test.py
glue/cirq/stimcirq/_stim_sampler_test.py
glue/cirq/stimcirq/_stim_to_cirq_test.py
glue/cirq/stimcirq/_sweep_pauli_test.py
glue/cirq/stimcirq/_two_qubit_asymmetric_depolarize_test.py
```

### Sinter Sampling Ecosystem Tests

Action: P3. Track as ecosystem integration, not as initial Stab core parity.

```text
glue/sample/src/sinter/_collection/_collection_manager_test.py
glue/sample/src/sinter/_collection/_collection_test.py
glue/sample/src/sinter/_collection/_collection_worker_test.py
glue/sample/src/sinter/_collection/_sampler_ramp_throttled_test.py
glue/sample/src/sinter/_command/_main_collect_test.py
glue/sample/src/sinter/_command/_main_combine_test.py
glue/sample/src/sinter/_command/_main_plot_test.py
glue/sample/src/sinter/_command/_main_predict_test.py
glue/sample/src/sinter/_data/_anon_task_stats_test.py
glue/sample/src/sinter/_data/_collection_options_test.py
glue/sample/src/sinter/_data/_existing_data_test.py
glue/sample/src/sinter/_data/_task_stats_test.py
glue/sample/src/sinter/_data/_task_test.py
glue/sample/src/sinter/_decoding/_decoding_test.py
glue/sample/src/sinter/_decoding/_stim_then_decode_sampler_test.py
glue/sample/src/sinter/_plotting_test.py
glue/sample/src/sinter/_predict_test.py
glue/sample/src/sinter/_probability_util_test.py
```

### StimFlow Tests

Action: P3. Mine circuit/noise examples only after the core Stim-compatible surface is stable.

```text
glue/stimflow/src/stimflow/_chunk/_chunk_builder_test.py
glue/stimflow/src/stimflow/_chunk/_chunk_compiler_test.py
glue/stimflow/src/stimflow/_chunk/_chunk_reflow_test.py
glue/stimflow/src/stimflow/_chunk/_chunk_test.py
glue/stimflow/src/stimflow/_chunk/_code_util_test.py
glue/stimflow/src/stimflow/_chunk/_flow_util_test.py
glue/stimflow/src/stimflow/_chunk/_patch_test.py
glue/stimflow/src/stimflow/_chunk/_stabilizer_code_test.py
glue/stimflow/src/stimflow/_chunk/_weave_test.py
glue/stimflow/src/stimflow/_core/_circuit_util_test.py
glue/stimflow/src/stimflow/_core/_complex_util_test.py
glue/stimflow/src/stimflow/_core/_flow_test.py
glue/stimflow/src/stimflow/_core/_noise_test.py
glue/stimflow/src/stimflow/_core/_pauli_map_test.py
glue/stimflow/src/stimflow/_core/_tile_test.py
glue/stimflow/src/stimflow/_layers/_data_test.py
glue/stimflow/src/stimflow/_layers/_layer_circuit_test.py
glue/stimflow/src/stimflow/_layers/_layer_feedback_test.py
glue/stimflow/src/stimflow/_layers/_layer_interact_swap_test.py
glue/stimflow/src/stimflow/_layers/_layer_rotation_test.py
glue/stimflow/src/stimflow/_layers/_layer_tag_test.py
glue/stimflow/src/stimflow/_layers/_transpile_test.py
glue/stimflow/src/stimflow/_viz/_3d_model_test.py
glue/stimflow/src/stimflow/_viz/_viz_circuit_html_test.py
glue/stimflow/src/stimflow/_viz/_viz_patch_svg_test.py
```

### ZX And Lattice Surgery Tests

Action: P3. These are ecosystem tools around Stim, not initial drop-in CLI/core parity.

```text
glue/lattice_surgery/stimzx/_external_stabilizer_test.py
glue/lattice_surgery/stimzx/_text_diagram_parsing_test.py
glue/lattice_surgery/stimzx/_zx_graph_solver_test.py
glue/zx/stimzx/_external_stabilizer_test.py
glue/zx/stimzx/_text_diagram_parsing_test.py
glue/zx/stimzx/_zx_graph_solver_test.py
```

## JavaScript Test Hierarchy

Source: all `*.test.js` files under `vendor/stim`.

### Stim JavaScript Bindings

Action: P3 until JS/WASM support is deliberately added.

```text
glue/javascript/circuit.test.js
glue/javascript/pauli_string.test.js
glue/javascript/tableau.test.js
glue/javascript/tableau_simulator.test.js
```

### Crumble Browser Application

Action: P3. Do not port to Rust during the core rewrite. Selected circuit strings may become parser fixtures if useful.

```text
glue/crumble/base/describe.test.js
glue/crumble/base/equate.test.js
glue/crumble/base/obs.test.js
glue/crumble/base/revision.test.js
glue/crumble/base/seq.test.js
glue/crumble/circuit/circuit.test.js
glue/crumble/circuit/layer.test.js
glue/crumble/circuit/pauli_frame.test.js
glue/crumble/circuit/propagated_pauli_frames.test.js
glue/crumble/draw/main_draw.test.js
glue/crumble/editor/editor_state.test.js
glue/crumble/gates/gateset.test.js
glue/crumble/keyboard/chord.test.js
glue/crumble/test/generated_gate_name_list.test.js
glue/crumble/test/test_util.test.js
```

## Visual Snapshot Assets

Source: `vendor/stim/testdata`.

Action: P3 until diagram support is in scope. Keep as snapshot inputs for future SVG and GLTF rendering parity.

```text
testdata/anticommuting_detslice.svg
testdata/bezier_time_slice.svg
testdata/circuit_all_ops_3d.gltf
testdata/circuit_all_ops_detslice.svg
testdata/circuit_all_ops_timeline.svg
testdata/circuit_all_ops_timeslice.svg
testdata/circuit_diagram_timeline_svg_chained_loops.svg
testdata/classical_feedback.gltf
testdata/classical_feedback.svg
testdata/colinear_detector_slice.svg
testdata/collapsing.gltf
testdata/collapsing.svg
testdata/command_diagram_timeline.svg
testdata/command_diagram_timeline_tick0.svg
testdata/command_diagram_timeline_tick1.svg
testdata/command_diagram_timeline_tick1_3.svg
testdata/command_diagram_timeline_tick2.svg
testdata/detector_pseudo_targets.gltf
testdata/detector_pseudo_targets.svg
testdata/detslice-with-ops_surface_code.svg
testdata/empty_match_graph.gltf
testdata/lattice_surgery_cnot.gltf
testdata/lattice_surgery_cnot.svg
testdata/long_range_detector.svg
testdata/match_graph_no_coords.gltf
testdata/match_graph_no_coords.svg
testdata/match_graph_repetition_code.gltf
testdata/match_graph_repetition_code.svg
testdata/match_graph_surface_code.gltf
testdata/match_graph_surface_code.svg
testdata/measurement_looping.gltf
testdata/measurement_looping.svg
testdata/noise_gates_1.gltf
testdata/noise_gates_1.svg
testdata/noise_gates_2.gltf
testdata/noise_gates_2.svg
testdata/noise_gates_3.gltf
testdata/noise_gates_3.svg
testdata/observable_slices.svg
testdata/repeat.gltf
testdata/repeat.svg
testdata/repetition_code.gltf
testdata/repetition_code.svg
testdata/rotated_memory_z_detector_slice.svg
testdata/shifted_coords.svg
testdata/single_qubits_gates.gltf
testdata/single_qubits_gates.svg
testdata/surface_code.gltf
testdata/surface_code.svg
testdata/surface_code_full_time_detector_slice.svg
testdata/surface_code_time_detector_slice.svg
testdata/surface_code_time_slice.svg
testdata/svg_ids.svg
testdata/test_circuit_all_ops.gltf
testdata/tick.gltf
testdata/tick.svg
testdata/two_qubits_gates.gltf
testdata/two_qubits_gates.svg
```

## Benchmark Source Hierarchy

Source of truth: `vendor/stim/file_lists/perf_files`.

Action: Bench. Use these files to define M3 C++ baseline workloads and M12 Rust-vs-C++ comparisons.

```text
src/stim/circuit/circuit.perf.cc
src/stim/gates/gates.perf.cc
src/stim/io/measure_record_reader.perf.cc
src/stim/main.perf.cc
src/stim/main_namespaced.perf.cc
src/stim/mem/simd_bit_table.perf.cc
src/stim/mem/simd_bits.perf.cc
src/stim/mem/simd_word.perf.cc
src/stim/mem/sparse_xor_vec.perf.cc
src/stim/search/graphlike/algo.perf.cc
src/stim/simulators/dem_sampler.perf.cc
src/stim/simulators/error_analyzer.perf.cc
src/stim/simulators/frame_simulator.perf.cc
src/stim/simulators/tableau_simulator.perf.cc
src/stim/stabilizers/clifford_string.perf.cc
src/stim/stabilizers/pauli_string.perf.cc
src/stim/stabilizers/pauli_string_iter.perf.cc
src/stim/stabilizers/tableau.perf.cc
src/stim/stabilizers/tableau_iter.perf.cc
src/stim/util_bot/error_decomp.perf.cc
src/stim/util_bot/probability_util.perf.cc
src/stim/util_top/reference_sample_tree.perf.cc
src/stim/util_top/stabilizers_to_tableau.perf.cc
```

Benchmark grouping:

- M3 baseline: parse/print, gate lookup, result reading, CLI startup, bit kernels, graphlike search, simulator sampling, DEM sampling, analyzer, stabilizer algebra, probability utilities, reference sampling, and stabilizers-to-tableau conversion.
- M12 hardening: keep the same matrix stable and add Stab-specific regressions only when a profiler identifies a hot path not covered by Stim's perf files.

## Test Support Assets

These files are test infrastructure, not standalone tests.

```text
src/stim/circuit/circuit.test.h
src/stim/main_namespaced.test.h
src/stim/mem/simd_word.test.h
src/stim/util_bot/test_util.test.h
dev/doctest_proper.py
glue/crumble/test/test.html
glue/crumble/test/test_import_all.js
glue/crumble/test/test_main.js
glue/crumble/test/test_util.js
```

Action:

- Recreate only the fixtures and assertions needed by Rust tests.
- Avoid copying C++ helper shapes directly when Rust property tests or typed fixtures express the invariant more clearly.
- Do not port upstream shell scripts; Stab operational workflows should remain `just` plus `ops` Rust binaries.
