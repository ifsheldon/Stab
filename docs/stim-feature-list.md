# Stim v1.16.0 Feature And API Inventory

This inventory targets the pinned Stim submodule at `vendor/stim`, which is `v1.16.0` at commit `e2fc1eca7fd21684d433aa5f10f4504ea4860d07`.

Primary sources are Stim's upstream generated references and binding/source files: [README.md](../vendor/stim/README.md), [doc/python_api_reference_vDev.md](../vendor/stim/doc/python_api_reference_vDev.md), [doc/stim.pyi](../vendor/stim/doc/stim.pyi), [doc/usage_command_line.md](../vendor/stim/doc/usage_command_line.md), [doc/gates.md](../vendor/stim/doc/gates.md), [doc/file_format_stim_circuit.md](../vendor/stim/doc/file_format_stim_circuit.md), [doc/file_format_dem_detector_error_model.md](../vendor/stim/doc/file_format_dem_detector_error_model.md), [doc/result_formats.md](../vendor/stim/doc/result_formats.md), and the public-header umbrella [src/stim.h](../vendor/stim/src/stim.h).

Stim's stable 1.x compatibility contract covers the Python API and command-line API. The C++ API is exposed through headers but explicitly makes no compatibility guarantees, so this document treats it as an implementation and porting surface instead of a stable public contract.

The current Stab mapping is [stab-feature-checklist.md](stab-feature-checklist.md), while case-level correctness and feature-level performance qualification of the implemented selected surface are planned in [plans/comprehensive-correctness-qualification-plan.md](plans/comprehensive-correctness-qualification-plan.md) and [plans/comprehensive-stim-performance-qualification-plan.md](plans/comprehensive-stim-performance-qualification-plan.md).

## 1. Top-Level Product Surface

- **Main purpose:** high-performance stabilizer circuit simulation and QEC-oriented analysis, especially bulk sampling and detector error model generation.
- **Primary user surfaces:** Python package `stim`, CLI binary `stim`, text file formats `.stim` and `.dem`, result data formats, and generated reference docs.
- **Secondary surfaces:** C++ headers, JavaScript/WASM bindings, Crumble browser editor, `stimcirq`, `sinter`, `stimflow`, and ZX/lattice-surgery glue packages.
- **Core limitations:** no non-Clifford circuit operations such as T or Toffoli in `stim.Circuit`, no non-Pauli circuit noise such as amplitude damping in `stim.Circuit`, and only single-control Pauli feedback in circuit files.
- **Performance architecture:** SIMD bit kernels, reference-frame sampling, inverted tableau simulation, and specialized readers/writers for packed or sparse shot formats.

## 2. File Formats And IO Contracts

### 2.1 `.stim` Circuit Format

- **Encoding and structure:** UTF-8, line-based instructions, optional comments with `#`, optional indentation, optional instruction tags in square brackets, optional floating-point parens arguments, space-separated targets, and nested `REPEAT K { ... }` blocks.
- **Instruction components:** case-insensitive instruction names, tags with Stim-specific escapes, double-precision numeric arguments, and target lists.
- **Target kinds:** qubit targets such as `5`, inverted qubit measurement-result targets such as `!5`, measurement record targets such as `rec[-1]`, sweep bit targets such as `sweep[3]`, Pauli targets such as `X0`, `Y2`, and `Z4`, inverted Pauli product targets such as `!X0`, and combiner targets `*`.
- **Semantics:** implicit qubit count from target usage, all qubits start in `|0>`, measurement operations append to an immutable measurement record, `rec[-k]` references previous measurements, sweep bits default false when unavailable, `ELSE_CORRELATED_ERROR` observes a hidden correlated-error flag, and coordinate offsets affect coordinate annotations.
- **Broadcasting:** single-qubit operations broadcast over all targets; two-qubit operations broadcast over aligned target pairs; Pauli product instructions group terms using combiners.
- **Control-flow limits:** only repeat blocks are supported, and vacuous zero-repeat blocks are invalid.
- **Source:** [file_format_stim_circuit.md](../vendor/stim/doc/file_format_stim_circuit.md), [circuit.h](../vendor/stim/src/stim/circuit/circuit.h), [circuit_instruction.h](../vendor/stim/src/stim/circuit/circuit_instruction.h).

### 2.2 `.dem` Detector Error Model Format

- **Encoding and structure:** UTF-8, line-based instructions, optional comments, optional indentation, optional tags, optional numeric arguments, targets, and nested `repeat K { ... }` blocks.
- **Instruction types:** `error`, `detector`, `logical_observable`, `shift_detectors`, and `repeat`.
- **Target kinds:** relative detector targets `D#`, logical observable targets `L#`, unsigned numeric targets for shifts and repeats, and separator targets `^` for suggested decompositions.
- **Semantics:** a DEM is a sequence of independent error mechanisms with probabilities, detector symptoms, logical frame changes, optional coordinate annotations, detector-coordinate shifts, detector-index shifts, and decomposition hints.
- **Source:** [file_format_dem_detector_error_model.md](../vendor/stim/doc/file_format_dem_detector_error_model.md), [detector_error_model.h](../vendor/stim/src/stim/dem/detector_error_model.h), [dem_instruction.h](../vendor/stim/src/stim/dem/dem_instruction.h).

### 2.3 Shot And Result Data Formats

- **Dense text:** `01` stores each shot as a newline-terminated row of `0` and `1` characters.
- **Dense binary:** `b8` stores each shot as little-endian bit-packed bytes padded to a byte boundary.
- **Sparse text:** `dets` stores each shot as `shot` followed by sparse `M#`, `D#`, and `L#` terms.
- **Sparse binary:** `hits` and `r8` store sparse hit indices in compact binary encodings.
- **Transposed packed binary:** `ptb64` stores 64-shot groups in a transposed SIMD-friendly layout.
- **Context dependency:** formats intentionally omit metadata, so readers need external context such as bits per shot, detector count, observable count, result type order, and record count.
- **Result prefixes:** `M` means measurement, `D` means detector, and `L` means logical observable frame change.
- **IO implementation concepts:** measurement record readers, measurement record writers, batched writers, sparse shots, packed readers/writers, and RAII file wrappers.
- **Source:** [result_formats.md](../vendor/stim/doc/result_formats.md), [stim_data_formats.h](../vendor/stim/src/stim/io/stim_data_formats.h), [measure_record_reader.h](../vendor/stim/src/stim/io/measure_record_reader.h), [measure_record_writer.h](../vendor/stim/src/stim/io/measure_record_writer.h), [measure_record_batch.h](../vendor/stim/src/stim/io/measure_record_batch.h), [measure_record_batch_writer.h](../vendor/stim/src/stim/io/measure_record_batch_writer.h), [sparse_shot.h](../vendor/stim/src/stim/io/sparse_shot.h).

## 3. Gate And Instruction Surface

### 3.1 Gate Families

- **Canonical registry size:** Stim v1.16.0 defines 81 canonical `GateType` entries excluding `NOT_A_GATE`, with additional parser aliases listed below.
- **Annotations:** `DETECTOR`, `OBSERVABLE_INCLUDE`, `TICK`, `QUBIT_COORDS`, `SHIFT_COORDS`, `MPAD`.
- **Control flow:** `REPEAT`.
- **Pauli gates:** `I`, `X`, `Y`, `Z`.
- **Single-qubit Clifford gates:** `C_NXYZ`, `C_NZYX`, `C_XNYZ`, `C_XYNZ`, `C_XYZ`, `C_ZNYX`, `C_ZYNX`, `C_ZYX`, `H`, `H_NXY`, `H_NXZ`, `H_NYZ`, `H_XY`, `H_YZ`, `S`, `S_DAG`, `SQRT_X`, `SQRT_X_DAG`, `SQRT_Y`, `SQRT_Y_DAG`.
- **Two-qubit Clifford gates:** `CX`, `CY`, `CZ`, `XCX`, `XCY`, `XCZ`, `YCX`, `YCY`, `YCZ`, `SWAP`, `ISWAP`, `ISWAP_DAG`, `CXSWAP`, `SWAPCX`, `CZSWAP`, `II`, `SQRT_XX`, `SQRT_XX_DAG`, `SQRT_YY`, `SQRT_YY_DAG`, `SQRT_ZZ`, `SQRT_ZZ_DAG`.
- **Noise channels:** `DEPOLARIZE1`, `DEPOLARIZE2`, `X_ERROR`, `Y_ERROR`, `Z_ERROR`, `I_ERROR`, `II_ERROR`, `PAULI_CHANNEL_1`, `PAULI_CHANNEL_2`, `E`, `ELSE_CORRELATED_ERROR`, `HERALDED_ERASE`, `HERALDED_PAULI_CHANNEL_1`.
- **Collapsing and reset gates:** `M`, `MX`, `MY`, `MR`, `MRX`, `MRY`, `R`, `RX`, `RY`.
- **Pair measurements:** `MXX`, `MYY`, `MZZ`.
- **Generalized Pauli product gates:** `MPP`, `SPP`, `SPP_DAG`.
- **Source:** [gates.md](../vendor/stim/doc/gates.md), [gates.h](../vendor/stim/src/stim/gates/gates.h), [gates.cc](../vendor/stim/src/stim/gates/gates.cc), and `gate_data_*.cc` files under [src/stim/gates](../vendor/stim/src/stim/gates).

### 3.2 Parser Aliases

- **Aliases:** `MZ` parses as `M`, `MRZ` parses as `MR`, `RZ` parses as `R`, `ZCX` and `CNOT` parse as `CX`, `ZCY` parses as `CY`, `ZCZ` parses as `CZ`, `H_XZ` parses as `H`, `SQRT_Z` parses as `S`, `SQRT_Z_DAG` parses as `S_DAG`, `SWAPCZ` parses as `CZSWAP`, and `CORRELATED_ERROR` parses as `E`.
- **Source:** [gates.h](../vendor/stim/src/stim/gates/gates.h), [gate_data_collapsing.cc](../vendor/stim/src/stim/gates/gate_data_collapsing.cc), [gate_data_controlled.cc](../vendor/stim/src/stim/gates/gate_data_controlled.cc), [gate_data_hada.cc](../vendor/stim/src/stim/gates/gate_data_hada.cc), [gate_data_period_4.cc](../vendor/stim/src/stim/gates/gate_data_period_4.cc), [gate_data_swaps.cc](../vendor/stim/src/stim/gates/gate_data_swaps.cc), [gate_data_noisy.cc](../vendor/stim/src/stim/gates/gate_data_noisy.cc).

### 3.3 Gate Metadata And Validation

- **Canonical metadata:** gate name, `GateType`, best candidate inverse, flags, argument count range, category, aliases, flow data, unitary matrix data, tableau data, and decomposition data.
- **Validation flags:** unitary, noisy, disjoint-probability arguments, result-producing, not fusible, block, paired targets, Pauli-string targets, measurement-record-only targets, bit targets, no targets, unsigned integer arguments, combiner targets, reset, no effect on qubits, and single-qubit broadcast.
- **Source:** [gates.h](../vendor/stim/src/stim/gates/gates.h), [gates.cc](../vendor/stim/src/stim/gates/gates.cc), [gates.pybind.cc](../vendor/stim/src/stim/gates/gates.pybind.cc).

## 4. Core Circuit Features

- **Construction and parsing:** construct circuits from text, files, and appended instructions; parse blocks, tags, targets, args, and comments; preserve canonical printed form.
- **Mutation:** append instructions, append text, insert instructions or blocks, pop operations, clear, concatenate, repeat, and copy.
- **Introspection:** count operations, qubits, measurements, detectors, observables, sweep bits, ticks, and determined measurements; inspect instruction targets and argument ranges.
- **Coordinates:** track final qubit coordinates, detector coordinates, coordinate shifts, and diagram coordinates.
- **Transforms:** flatten repeat blocks, decompose compound gates into supported lower-level operations, remove noise, remove tags, invert circuits, inline feedback, reverse time for flows, simplify circuits, build inverse-QEC and inverse-unitary circuits, and produce reference samples.
- **Exports:** write `.stim`, convert to tableaux, export OpenQASM, export Quirk URLs, export Crumble URLs, and generate diagrams.
- **Analysis:** produce detector error models, detecting regions, missing detector reports, flow generators, flow checks, shortest graphlike errors, hypergraph searches for undetectable logical errors, SAT encodings, and error explanations.
- **Built-in generation:** generate repetition-code, surface-code, and color-code circuits with configurable distances, rounds, task variants, and noise knobs.
- **Source:** [circuit.h](../vendor/stim/src/stim/circuit/circuit.h), [circuit.pybind.cc](../vendor/stim/src/stim/circuit/circuit.pybind.cc), [circuit2.pybind.cc](../vendor/stim/src/stim/circuit/circuit2.pybind.cc), [util_top](../vendor/stim/src/stim/util_top), [gen](../vendor/stim/src/stim/gen).

## 5. Detector Error Model Features

- **Construction and parsing:** construct DEMs from text and files; parse instructions, tags, args, targets, repeat blocks, and comments.
- **Mutation:** append `error`, `detector`, `logical_observable`, `shift_detectors`, and repeat blocks; concatenate, repeat, clear, and copy.
- **Introspection:** count detectors, errors, observables, instructions, final detector shifts, final coordinate shifts, and detector coordinates.
- **Transforms:** flatten repeat blocks, round probabilities, strip tags, iterate flattened errors, and preserve or expose decomposition separators.
- **Sampling:** compile DEM samplers that sample detector events, logical observables, and optional sampled-error records, including replay of sampled-error records.
- **Analysis and diagrams:** compute shortest graphlike errors and render match-graph diagrams.
- **Source:** [detector_error_model.h](../vendor/stim/src/stim/dem/detector_error_model.h), [dem_instruction.h](../vendor/stim/src/stim/dem/dem_instruction.h), [detector_error_model.pybind.cc](../vendor/stim/src/stim/dem/detector_error_model.pybind.cc), [dem_sampler.h](../vendor/stim/src/stim/simulators/dem_sampler.h).

## 6. Sampling, Conversion, And Simulation Features

- **Measurement sampling:** compile a `stim.Circuit` into a measurement sampler using reference-frame sampling and write samples in supported result formats.
- **Detector sampling:** compile a `stim.Circuit` into a detector sampler that can return detector samples, append/prepend observables, separate observables to another output, and write result formats.
- **Measurement-to-detection conversion:** compile a conversion plan from a circuit and convert measurement records into detection-event records with optional sweep bits and observable output handling.
- **DEM sampling:** compile a detector error model into a DEM sampler that can emit detector records, observable records, sampled-error records, and replayed sampled-error records.
- **Single-shot tableau simulation:** execute stabilizer circuits, Clifford gates, measurements, resets, postselection, noise helpers, Pauli products, and arbitrary tableaux against an interactive tableau simulator.
- **Batched flip-frame simulation:** track measurement, detector, observable, and Pauli flips across batches with SIMD-backed frame simulation.
- **Randomness contract:** seeded sampling is reproducible enough for same-version workflows but Stim does not promise exact random streams across versions, SIMD widths, or different shot batching.
- **Source:** [compiled_measurement_sampler.pybind.cc](../vendor/stim/src/stim/py/compiled_measurement_sampler.pybind.cc), [compiled_detector_sampler.pybind.cc](../vendor/stim/src/stim/py/compiled_detector_sampler.pybind.cc), [measurements_to_detection_events.h](../vendor/stim/src/stim/simulators/measurements_to_detection_events.h), [dem_sampler.h](../vendor/stim/src/stim/simulators/dem_sampler.h), [tableau_simulator.h](../vendor/stim/src/stim/simulators/tableau_simulator.h), [frame_simulator.h](../vendor/stim/src/stim/simulators/frame_simulator.h).

## 7. Error Analysis, Search, And Decoder Configuration

- **Circuit-to-DEM analysis:** propagate errors through circuits, check detector determinism, support gauge detectors when requested, approximate disjoint errors when requested, fold loops, decompose hyperedges into graphlike components, and control remnant-edge introduction.
- **Error explanation:** match DEM error mechanisms back to physical circuit error locations, including tick offsets, nested repeat stack frames, resolved targets, flipped measurements, flipped Pauli products, detector coordinates, qubit coordinates, and noise tags.
- **Minimum-distance and logical-error search:** search graphlike and hypergraph representations for shortest undetectable logical errors, produce shortest graphlike errors, and build weighted MaxSAT/WCNF encodings.
- **Source:** [error_analyzer.h](../vendor/stim/src/stim/simulators/error_analyzer.h), [error_matcher.h](../vendor/stim/src/stim/simulators/error_matcher.h), [matched_error.h](../vendor/stim/src/stim/simulators/matched_error.h), [sparse_rev_frame_tracker.h](../vendor/stim/src/stim/simulators/sparse_rev_frame_tracker.h), [search](../vendor/stim/src/stim/search), [circuit_to_dem.h](../vendor/stim/src/stim/util_top/circuit_to_dem.h).

## 8. Stabilizer Algebra Features

- **Pauli strings:** construct, parse, multiply, divide, negate, slice, mutate, commute-test, apply before or after Clifford operations, convert to/from numpy/unitary/tableau, iterate all Paulis, randomize, compute weight, and inspect Pauli indices.
- **Clifford strings:** represent tensor products of single-qubit Clifford operations, concatenate, multiply, power, slice, mutate, generate all Clifford strings, randomize, and inspect X/Y/Z outputs.
- **Tableaux:** construct from sizes, named gates, circuits, generators, numpy arrays, stabilizers, state vectors, and unitaries; compose, invert, exponentiate, call on Pauli strings, randomize, iterate, and convert to circuit/numpy/Pauli/stabilizer/state-vector/unitary representations.
- **Flows:** represent stabilizer flows with input Pauli strings, output Pauli strings, measurement indices, and included observables; multiply flows and use them for circuit flow checks.
- **C++ extras:** header-exposed internals also include `PauliStringRef`, Pauli iterators, tableau iterators, `FlexPauliString`, graph-state simulator, vector simulator, sparse reverse frame tracker, and reference sample tree utilities.
- **Source:** [stabilizers](../vendor/stim/src/stim/stabilizers), [flow.pybind.cc](../vendor/stim/src/stim/stabilizers/flow.pybind.cc), [tableau_simulator.h](../vendor/stim/src/stim/simulators/tableau_simulator.h), [graph_simulator.h](../vendor/stim/src/stim/simulators/graph_simulator.h), [vector_simulator.h](../vendor/stim/src/stim/simulators/vector_simulator.h).

## 9. Diagrams And Visualization

- **CLI diagram types:** `timeline-text`, `timeline-svg`, `timeline-3d`, `timeline-3d-html`, `timeslice-svg`, `detslice-text`, `detslice-svg`, `detslice-with-ops-svg`, `matchgraph-svg`, `matchgraph-3d`, `matchgraph-3d-html`, and `interactive-html`.
- **CLI diagram aliases:** `time-slice-svg`, `time+detector-slice-svg`, `interactive`, `detector-slice-text`, `detector-slice-svg`, `match-graph-svg`, `match-graph-3d`, and `match-graph-3d-html`.
- **Python circuit diagram aliases:** `timeline`, `timeline-html`, `timeline-svg-html`, `detslice-svg-html`, `detslice-with-ops-svg-html`, `matchgraph-svg-html`, and hyphenated match-graph variants.
- **Python DEM diagrams:** match-graph SVG, SVG HTML, 3D GLTF, and 3D HTML renderings.
- **Rendering backends:** ASCII timelines, SVG timelines, detector slices, time-slice diagrams, combined time/detector-slice diagrams, match-graph SVG, GLTF/3D, GLTF HTML viewers, Crumble interactive HTML, JSON helpers, base64 embedding, lattice maps, and coordinate helpers.
- **Source:** [command_diagram.cc](../vendor/stim/src/stim/cmd/command_diagram.cc), [command_diagram.pybind.cc](../vendor/stim/src/stim/cmd/command_diagram.pybind.cc), [diagram](../vendor/stim/src/stim/diagram), [crumble](../vendor/stim/glue/crumble).

## 10. Built-In Circuit Generation

- **CLI and Python generator entry points:** `stim gen` and `stim.Circuit.generated`.
- **Codes and tasks:** `repetition_code:memory`, `surface_code:rotated_memory_x`, `surface_code:rotated_memory_z`, `surface_code:unrotated_memory_x`, `surface_code:unrotated_memory_z`, and `color_code:memory_xyz`.
- **Generator parameters:** code, task, distance, rounds, output path, after-Clifford depolarization, before-round data depolarization, before-measure flip probability, and after-reset flip probability.
- **Generated circuit content:** layout comments, qubit coordinates, detector coordinates, observables, ticks, reset/measurement rounds, and optional noise.
- **Source:** [command_gen.cc](../vendor/stim/src/stim/cmd/command_gen.cc), [circuit_gen_params.h](../vendor/stim/src/stim/gen/circuit_gen_params.h), [gen_rep_code.cc](../vendor/stim/src/stim/gen/gen_rep_code.cc), [gen_surface_code.cc](../vendor/stim/src/stim/gen/gen_surface_code.cc), [gen_color_code.cc](../vendor/stim/src/stim/gen/gen_color_code.cc).

## 11. Command-Line API

### 11.1 Command Index

| Command | Main feature surface |
| --- | --- |
| `stim analyze_errors` | Convert a `.stim` circuit into a `.dem` detector error model. |
| `stim convert` | Convert shot/result data between supported formats and type selections. |
| `stim detect` | Sample a circuit into detection-event data and optional observable output. |
| `stim diagram` | Render circuit or DEM diagrams. |
| `stim explain_errors` | Explain DEM errors in terms of circuit error locations. |
| `stim gen` | Generate standard QEC circuits. |
| `stim help` | Print command, gate, and format help. |
| `stim m2d` | Convert measurement records into detection-event records. |
| `stim repl` | Run an interactive Stim REPL. |
| `stim sample` | Sample circuit measurement records. |
| `stim sample_dem` | Sample detector error models. |

### 11.2 Command Options

- **`analyze_errors`:** `--allow_gauge_detectors`, `--approximate_disjoint_errors [probability]`, `--block_decompose_from_introducing_remnant_edges`, `--decompose_errors`, `--fold_loops`, `--ignore_decomposition_failures`, `--in`, `--out`.
- **`convert`:** `--bits_per_shot`, `--circuit`, `--in`, `--in_format`, `--num_detectors`, `--num_measurements`, `--num_observables`, `--obs_out`, `--obs_out_format`, `--out`, `--out_format`, `--types`.
- **`detect`:** `--append_observables`, `--in`, `--obs_out`, `--obs_out_format`, `--out`, `--out_format`, `--seed`, `--shots`.
- **`diagram`:** `--filter_coords`, `--in`, `--out`, `--remove_noise`, `--tick`, `--type`.
- **`explain_errors`:** `--dem_filter`, `--in`, `--out`, `--single`.
- **`gen`:** `--after_clifford_depolarization`, `--after_reset_flip_probability`, `--before_measure_flip_probability`, `--before_round_data_depolarization`, `--code`, `--distance`, `--out`, `--rounds`, `--task`.
- **`m2d`:** `--append_observables`, `--circuit`, `--in`, `--in_format`, `--obs_out`, `--obs_out_format`, `--out`, `--out_format`, `--ran_without_feedback`, `--skip_reference_sample`, `--sweep`, `--sweep_format`.
- **`sample`:** `--in`, `--out`, `--out_format`, `--seed`, `--shots`, `--skip_loop_folding`, `--skip_reference_sample`.
- **`sample_dem`:** `--err_out`, `--err_out_format`, `--in`, `--obs_out`, `--obs_out_format`, `--out`, `--out_format`, `--replay_err_in`, `--replay_err_in_format`, `--seed`, `--shots`.
- **Legacy dispatch modes:** `--sample`, `--detect`, `--gen`, `--m2d`, `--detector_hypergraph`, `--frame0`, and `--prepend_observables` exist in upstream Stim command dispatch. This inventory records their existence; Stab's checklist is the source of truth for which deprecated legacy modes are intentionally kept or excluded from Stab CLI parity.
- **Source:** [usage_command_line.md](../vendor/stim/doc/usage_command_line.md), [main_namespaced.cc](../vendor/stim/src/stim/main_namespaced.cc), [cmd](../vendor/stim/src/stim/cmd).

## 12. Python API

### 12.1 Import And Runtime Behavior

- **Import dispatch:** `import stim` detects machine architecture and imports either `_stim_sse2` for AVX2/SSE2 machines or `_stim_polyfill` otherwise; AVX2-specific binding import is disabled in this version.
- **Package entry point:** `python -m stim` routes to `stim.main(command_line_args=sys.argv[1:])`.
- **Exposed metadata:** `__version__` and internal `_UNSTABLE_raw_format_data`.
- **Source:** [glue/python/src/stim/__init__.py](../vendor/stim/glue/python/src/stim/__init__.py), [glue/python/src/stim/_main_argv.py](../vendor/stim/glue/python/src/stim/_main_argv.py), [stim.pybind.cc](../vendor/stim/src/stim/py/stim.pybind.cc), [march.pybind.cc](../vendor/stim/src/stim/py/march.pybind.cc).

### 12.2 Top-Level Functions

- **General:** `gate_data`, `main`, `read_shot_data_file`, `write_shot_data_file`.
- **Circuit target factories:** `target_rec`, `target_sweep_bit`, `target_inv`, `target_x`, `target_y`, `target_z`, `target_pauli`, `target_combiner`, `target_combined_paulis`.
- **DEM target factories:** `target_relative_detector_id`, `target_logical_observable_id`, `target_separator`.
- **Source:** [doc/stim.pyi](../vendor/stim/doc/stim.pyi), [stim.pybind.cc](../vendor/stim/src/stim/py/stim.pybind.cc), [read_write.pybind.cc](../vendor/stim/src/stim/io/read_write.pybind.cc).

### 12.3 Circuit Classes

- **`Circuit`:** constructor `__init__`; operators `__add__`, `__eq__`, `__getitem__`, `__iadd__`, `__imul__`, `__len__`, `__mul__`, `__ne__`, `__repr__`, `__rmul__`, and `__str__`; mutation and construction methods `append`, `append_operation`, `append_from_stim_program_text`, `insert`, `pop`, `clear`, `copy`, `from_file`, `to_file`; sampler/compiler methods `compile_sampler`, `compile_detector_sampler`, `compile_m2d_converter`; analysis methods `count_determined_measurements`, `detector_error_model`, `detecting_regions`, `explain_detector_error_model_errors`, `flow_generators`, `get_detector_coordinates`, `get_final_qubit_coordinates`, `has_all_flows`, `has_flow`, `likeliest_error_sat_problem`, `missing_detectors`, `reference_detector_and_observable_signs`, `reference_sample`, `search_for_undetectable_logical_errors`, `shortest_error_sat_problem`, `shortest_graphlike_error`, `solve_flow_measurements`; transforms `approx_equals`, `decomposed`, `flattened`, `flattened_operations`, `generated`, `inverse`, `time_reversed_for_flows`, `to_crumble_url`, `to_qasm`, `to_quirk_url`, `to_tableau`, `with_inlined_feedback`, `without_noise`, `without_tags`; properties `num_detectors`, `num_measurements`, `num_observables`, `num_qubits`, `num_sweep_bits`, and `num_ticks`.
- **`CircuitInstruction`:** `__eq__`, `__init__`, `__ne__`, `__repr__`, `__str__`, `gate_args_copy`, `target_groups`, `targets_copy`, `name`, `num_measurements`, and `tag`.
- **`CircuitRepeatBlock`:** `__eq__`, `__init__`, `__ne__`, `__repr__`, `body_copy`, `name`, `num_measurements`, `repeat_count`, and `tag`.
- **`CircuitTargetsInsideInstruction`:** `args`, `gate`, `tag`, `target_range_start`, `target_range_end`, and `targets_in_range`.
- **`GateTarget`:** `__eq__`, `__init__`, `__ne__`, `__repr__`, `is_combiner`, `is_inverted_result_target`, `is_measurement_record_target`, `is_qubit_target`, `is_sweep_bit_target`, `is_x_target`, `is_y_target`, `is_z_target`, `pauli_type`, `qubit_value`, and `value`.
- **`GateTargetWithCoords`:** `coords` and `gate_target`.
- **Source:** [circuit.pybind.cc](../vendor/stim/src/stim/circuit/circuit.pybind.cc), [circuit_instruction.pybind.cc](../vendor/stim/src/stim/circuit/circuit_instruction.pybind.cc), [circuit_repeat_block.pybind.cc](../vendor/stim/src/stim/circuit/circuit_repeat_block.pybind.cc), [gate_target.pybind.cc](../vendor/stim/src/stim/circuit/gate_target.pybind.cc).

### 12.4 DEM Classes

- **`DetectorErrorModel`:** constructor `__init__`; operators `__add__`, `__eq__`, `__getitem__`, `__iadd__`, `__imul__`, `__len__`, `__mul__`, `__ne__`, `__repr__`, `__rmul__`, and `__str__`; methods `append`, `approx_equals`, `clear`, `compile_sampler`, `copy`, `diagram`, `flattened`, `from_file`, `get_detector_coordinates`, `rounded`, `shortest_graphlike_error`, `to_file`, and `without_tags`; properties `num_detectors`, `num_errors`, and `num_observables`.
- **`DemInstruction`:** `__eq__`, `__init__`, `__ne__`, `__repr__`, `__str__`, `args_copy`, `target_groups`, `targets_copy`, `tag`, and `type`.
- **`DemRepeatBlock`:** `__eq__`, `__init__`, `__ne__`, `__repr__`, `body_copy`, `repeat_count`, and `type`.
- **`DemTarget`:** `__eq__`, `__init__`, `__ne__`, `__repr__`, `__str__`, `is_logical_observable_id`, `is_relative_detector_id`, `is_separator`, `logical_observable_id`, `relative_detector_id`, `separator`, and `val`.
- **`DemTargetWithCoords`:** `coords` and `dem_target`.
- **Source:** [detector_error_model.pybind.cc](../vendor/stim/src/stim/dem/detector_error_model.pybind.cc), [dem_instruction.pybind.cc](../vendor/stim/src/stim/dem/dem_instruction.pybind.cc), [detector_error_model_repeat_block.pybind.cc](../vendor/stim/src/stim/dem/detector_error_model_repeat_block.pybind.cc), [detector_error_model_target.pybind.cc](../vendor/stim/src/stim/dem/detector_error_model_target.pybind.cc).

### 12.5 Sampler And Converter Classes

- **`CompiledMeasurementSampler`:** `__init__`, `__repr__`, `sample`, `sample_bit_packed`, and `sample_write`.
- **`CompiledDetectorSampler`:** `__init__`, `__repr__`, `sample`, `sample_bit_packed`, and `sample_write`.
- **`CompiledMeasurementsToDetectionEventsConverter`:** `__init__`, `__repr__`, `convert`, and `convert_file`.
- **`CompiledDemSampler`:** `sample` and `sample_write`.
- **Source:** [compiled_measurement_sampler.pybind.cc](../vendor/stim/src/stim/py/compiled_measurement_sampler.pybind.cc), [compiled_detector_sampler.pybind.cc](../vendor/stim/src/stim/py/compiled_detector_sampler.pybind.cc), [measurements_to_detection_events.pybind.cc](../vendor/stim/src/stim/simulators/measurements_to_detection_events.pybind.cc), [dem_sampler.pybind.cc](../vendor/stim/src/stim/simulators/dem_sampler.pybind.cc).

### 12.6 Error Explanation Classes

- **`ExplainedError`:** `circuit_error_locations` and `dem_error_terms`.
- **`CircuitErrorLocation`:** `flipped_measurement`, `flipped_pauli_product`, `instruction_targets`, `noise_tag`, `stack_frames`, and `tick_offset`.
- **`CircuitErrorLocationStackFrame`:** `instruction_offset`, `instruction_repetitions_arg`, and `iteration_index`.
- **`FlippedMeasurement`:** `observable` and `record_index`.
- **Source:** [matched_error.pybind.cc](../vendor/stim/src/stim/simulators/matched_error.pybind.cc).

### 12.7 Stabilizer, Gate Metadata, And Simulator Classes

- **`GateData`:** `__eq__`, `__init__`, `__ne__`, `__repr__`, `__str__`, `aliases`, `flows`, `generalized_inverse`, `hadamard_conjugated`, `inverse`, `is_noisy_gate`, `is_reset`, `is_single_qubit_gate`, `is_symmetric_gate`, `is_two_qubit_gate`, `is_unitary`, `name`, `num_parens_arguments_range`, `produces_measurements`, `tableau`, `takes_measurement_record_targets`, `takes_pauli_targets`, and `unitary_matrix`.
- **`PauliString`:** constructor `__init__`; operators `__add__`, `__eq__`, `__getitem__`, `__iadd__`, `__imul__`, `__itruediv__`, `__len__`, `__mul__`, `__ne__`, `__neg__`, `__pos__`, `__repr__`, `__rmul__`, `__setitem__`, `__str__`, and `__truediv__`; methods `after`, `before`, `commutes`, `copy`, `extended_product`, `from_numpy`, `from_unitary_matrix`, `iter_all`, `pauli_indices`, `random`, `to_numpy`, `to_tableau`, `to_unitary_matrix`; properties `sign` and `weight`.
- **`PauliStringIterator`:** `__iter__` and `__next__`.
- **`CliffordString`:** constructor `__init__`; operators `__add__`, `__eq__`, `__getitem__`, `__iadd__`, `__imul__`, `__ipow__`, `__len__`, `__mul__`, `__ne__`, `__pow__`, `__repr__`, `__rmul__`, `__setitem__`, and `__str__`; methods `all_cliffords_string`, `copy`, `random`, `x_outputs`, `y_outputs`, and `z_outputs`.
- **`Tableau`:** constructor `__init__`; operators `__add__`, `__call__`, `__eq__`, `__iadd__`, `__len__`, `__mul__`, `__ne__`, `__pow__`, `__repr__`, and `__str__`; methods `append`, `copy`, `from_circuit`, `from_conjugated_generators`, `from_named_gate`, `from_numpy`, `from_stabilizers`, `from_state_vector`, `from_unitary_matrix`, `inverse`, `inverse_x_output`, `inverse_x_output_pauli`, `inverse_y_output`, `inverse_y_output_pauli`, `inverse_z_output`, `inverse_z_output_pauli`, `iter_all`, `prepend`, `random`, `then`, `to_circuit`, `to_numpy`, `to_pauli_string`, `to_stabilizers`, `to_state_vector`, `to_unitary_matrix`, `x_output`, `x_output_pauli`, `x_sign`, `y_output`, `y_output_pauli`, `y_sign`, `z_output`, `z_output_pauli`, and `z_sign`.
- **`TableauIterator`:** `__iter__` and `__next__`.
- **`Flow`:** `__eq__`, `__init__`, `__mul__`, `__ne__`, `__repr__`, `__str__`, `included_observables_copy`, `input_copy`, `measurements_copy`, and `output_copy`.
- **`TableauSimulator`:** `__init__`, `c_xyz`, `c_zyx`, `canonical_stabilizers`, `cnot`, `copy`, `current_inverse_tableau`, `current_measurement_record`, `cx`, `cy`, `cz`, `depolarize1`, `depolarize2`, `do`, `do_circuit`, `do_pauli_string`, `do_tableau`, `h`, `h_xy`, `h_xz`, `h_yz`, `iswap`, `iswap_dag`, `measure`, `measure_kickback`, `measure_many`, `measure_observable`, `num_qubits`, `peek_bloch`, `peek_observable_expectation`, `peek_x`, `peek_y`, `peek_z`, `postselect_observable`, `postselect_x`, `postselect_y`, `postselect_z`, `reset`, `reset_x`, `reset_y`, `reset_z`, `s`, `s_dag`, `set_inverse_tableau`, `set_num_qubits`, `set_state_from_stabilizers`, `set_state_from_state_vector`, `sqrt_x`, `sqrt_x_dag`, `sqrt_y`, `sqrt_y_dag`, `state_vector`, `swap`, `x`, `x_error`, `xcx`, `xcy`, `xcz`, `y`, `y_error`, `ycx`, `ycy`, `ycz`, `z`, `z_error`, `zcx`, `zcy`, and `zcz`.
- **`FlipSimulator`:** `__init__`, `append_measurement_flips`, `batch_size`, `broadcast_pauli_errors`, `clear`, `copy`, `do`, `generate_bernoulli_samples`, `get_detector_flips`, `get_measurement_flips`, `get_observable_flips`, `num_detectors`, `num_measurements`, `num_observables`, `num_qubits`, `peek_pauli_flips`, `set_pauli_flip`, and `to_numpy`.
- **Source:** [gates.pybind.cc](../vendor/stim/src/stim/gates/gates.pybind.cc), [pauli_string.pybind.cc](../vendor/stim/src/stim/stabilizers/pauli_string.pybind.cc), [clifford_string.pybind.cc](../vendor/stim/src/stim/stabilizers/clifford_string.pybind.cc), [tableau.pybind.cc](../vendor/stim/src/stim/stabilizers/tableau.pybind.cc), [flow.pybind.cc](../vendor/stim/src/stim/stabilizers/flow.pybind.cc), [tableau_simulator.pybind.cc](../vendor/stim/src/stim/simulators/tableau_simulator.pybind.cc), [frame_simulator.pybind.cc](../vendor/stim/src/stim/simulators/frame_simulator.pybind.cc).

## 13. C++ Header Surface

- **Compatibility status:** C++ headers are available but unstable across minor versions, as stated in [src/stim.h](../vendor/stim/src/stim.h).
- **Umbrella modules:** circuit, gates, command implementations, DEMs, diagrams, generators, IO, memory/SIMD kernels, searches, simulators, stabilizers, utility parsing/math, and top-level circuit transforms.
- **Core circuit headers:** `Circuit`, `CircuitInstruction`, `CircuitStats`, `GateTarget`, and gate decomposition helpers.
- **Core DEM headers:** `DetectorErrorModel`, `DemInstruction`, repeat blocks, DEM targets, flattened iteration, coordinate lookup, and DEM sampling.
- **Memory and SIMD headers:** bit refs, bitwords for 64/SSE/AVX widths, fixed-capacity vectors, monotonic buffers, SIMD bit tables, SIMD bit ranges, SIMD words, span refs, and sparse XOR vectors.
- **Search headers:** graphlike graph/node/edge/search state, hypergraph graph/node/edge/search state, SAT/WCNF generation, and shared search helpers.
- **Simulator headers:** DEM sampler, error analyzer, error matcher, force-streaming helpers, frame simulator, graph simulator, matched error structs, measurement-to-detection converter, sparse reverse frame tracker, tableau simulator, and vector simulator.
- **Top-level utility headers:** flow generators, inverse QEC circuits, inverse unitary circuits, circuit-to-DEM conversion, detecting-region conversion, circuit-vs-amplitudes/tableau checks, count determined measurements, Crumble/Quirk/QASM exports, flow checks, MBQC decomposition, missing detectors, reference sample tree, circuit simplification, stabilizer-to-tableau conversion, and transform-without-feedback utilities.
- **Source:** [src/stim.h](../vendor/stim/src/stim.h), [src/stim](../vendor/stim/src/stim).

## 14. JavaScript And WASM API

- **Build surface:** JavaScript bindings are Emscripten/WASM glue under [glue/javascript](../vendor/stim/glue/javascript), with build helper [build_wasm.sh](../vendor/stim/glue/javascript/build_wasm.sh).
- **`stim.Circuit`:** constructor, `append_operation`, `append_from_stim_program_text`, `copy`, `isEqualTo`, `repeated`, and `toString`.
- **`stim.Tableau`:** constructor, static `random`, static `from_named_gate`, static `from_conjugated_generators_xs_zs`, `x_output`, `y_output`, `z_output`, `toString`, `isEqualTo`, and `length`.
- **`stim.TableauSimulator`:** constructor, `CNOT`, `CY`, `CZ`, `H`, `SWAP`, `X`, `Y`, `Z`, `copy`, `current_inverse_tableau`, `do_circuit`, `do_pauli_string`, `do_tableau`, `measure`, `measure_kickback`, and `set_inverse_tableau`.
- **`stim.PauliString`:** constructor, static `random`, `commutes`, `isEqualTo`, `length`, `neg`, `pauli`, `sign`, `times`, and `toString`.
- **Source:** [glue/javascript/README.md](../vendor/stim/glue/javascript/README.md), [circuit.js.cc](../vendor/stim/glue/javascript/circuit.js.cc), [tableau.js.cc](../vendor/stim/glue/javascript/tableau.js.cc), [tableau_simulator.js.cc](../vendor/stim/glue/javascript/tableau_simulator.js.cc), [pauli_string.js.cc](../vendor/stim/glue/javascript/pauli_string.js.cc).

## 15. Ecosystem And Glue Packages

### 15.1 `stimcirq`

- **Main features:** convert Cirq circuits to Stim circuits, convert Stim circuits to Cirq circuits, sample compatible Cirq circuits through Stim, preserve tags, handle classically controlled Pauli feedback, handle annotations, and define custom Cirq gates for Stim concepts.
- **Custom types:** `StimSampler`, `CXSwapGate`, `CZSwapGate`, `DetAnnotation`, `CumulativeObservableAnnotation`, `ShiftCoordsAnnotation`, `FeedbackPauli`, `SweepPauli`, `MeasureAndOrResetGate`, `IErrorGate`, `IIErrorGate`, `IIGate`, and `TwoQubitAsymmetricDepolarizingChannel`.
- **Source:** [glue/cirq/README.md](../vendor/stim/glue/cirq/README.md), [glue/cirq/stimcirq](../vendor/stim/glue/cirq/stimcirq).

### 15.2 `sinter`

- **Main features:** collect decoding statistics, combine CSV statistics, plot error/discard/custom curves, predict observables through decoders, manage multiprocessing collection, throttle batch sizes, resume collection, postselect detectors/observables, count observable error combinations, count detection events, and integrate built-in decoders.
- **Public API classes:** `AnonTaskStats`, `CollectionOptions`, `CompiledDecoder`, `CompiledSampler`, `Decoder`, `Fit`, `Progress`, `Sampler`, `Task`, and `TaskStats`.
- **Public API functions:** `collect`, `iter_collect`, `fit_binomial`, `fit_line_slope`, `fit_line_y_at_x`, `group_by`, `log_binomial`, `log_factorial`, `plot_custom`, `plot_discard_rate`, `plot_error_rate`, `post_selection_mask_from_4th_coord`, `predict_discards_bit_packed`, `predict_observables`, `predict_observables_bit_packed`, `predict_on_disk`, `read_stats_from_csv_files`, `shot_error_rate_to_piece_error_rate`, `stats_from_csv_files`, `better_sorted_str_terms`, and `comma_separated_key_values`.
- **CLI commands:** `sinter collect`, `sinter combine`, and `sinter plot`.
- **Source:** [doc/sinter_api.md](../vendor/stim/doc/sinter_api.md), [doc/sinter_command_line.md](../vendor/stim/doc/sinter_command_line.md), [glue/sample/src/sinter](../vendor/stim/glue/sample/src/sinter).

### 15.3 Crumble

- **Main features:** interactive browser editor for Stim circuits, circuit/layer editing, Pauli-frame propagation, gatesets matching Stim gates, keyboard/toolbox handling, URL/state sync, drawing utilities, timeline viewer, generated gate-name tests, and bundled single-page HTML embedding for diagrams.
- **Source:** [glue/crumble/README.md](../vendor/stim/glue/crumble/README.md), [glue/crumble](../vendor/stim/glue/crumble), [diagram/crumble.h](../vendor/stim/src/stim/diagram/crumble.h).

### 15.4 `stimflow`

- **Main features:** higher-level circuit/chunk/flow construction, patch and stabilizer-code utilities, noise helpers, layered circuit representation, layer transpilation, feedback layers, interaction/swap/iswap/sqrt-PP layers, tag layers, reset/measure/MPP layers, and HTML/SVG/3D visualization helpers.
- **Source:** [glue/stimflow](../vendor/stim/glue/stimflow).

### 15.5 ZX And Lattice-Surgery Glue

- **Main features:** external stabilizer utilities, text diagram parsing, ZX graph solving, lattice-surgery SAT synthesis, rewrite passes, networkx/GLTF/text translators, and verification helpers.
- **Source:** [glue/zx](../vendor/stim/glue/zx), [glue/lattice_surgery](../vendor/stim/glue/lattice_surgery).

## 16. Packaging, Build, And Documentation Surfaces

- **Python packaging:** `setup.py`, `pyproject.toml`, pybind11 extension builds, Python package data, `stim.__version__`, and CPU-feature-selected extension imports.
- **C++ builds:** CMake and Bazel workspace files.
- **JavaScript packaging:** root `package.json`, Crumble `package.json`, and Emscripten build helpers.
- **Generated docs:** Python API reference, Python stub file, Sinter API reference, gate reference, result format reference, command-line reference, circuit file format reference, DEM file format reference, developer documentation, and notebooks.
- **Developer generators:** doc regeneration scripts, stub generators, file-list regeneration, known-gate generation for JS, Crumble single-page compilation, and version overwrite helpers.
- **Source:** [setup.py](../vendor/stim/setup.py), [pyproject.toml](../vendor/stim/pyproject.toml), [CMakeLists.txt](../vendor/stim/CMakeLists.txt), [WORKSPACE](../vendor/stim/WORKSPACE), [package.json](../vendor/stim/package.json), [dev](../vendor/stim/dev), [doc](../vendor/stim/doc).

## 17. Test And Benchmark Surface

- **Core C++ tests:** tests cover circuits, circuit instructions, gate targets, commands, DEM parsing/modeling, diagrams, gates, generators, IO, memory/SIMD kernels, search algorithms, simulators, stabilizer primitives, low-level utilities, and top-level circuit transforms.
- **Core perf tests:** perf files cover circuits, gates, main command dispatch, readers, memory kernels, sparse XOR, error analysis, DEM sampling, frame simulation, tableau simulation, Clifford strings, Pauli strings, tableau iteration, stabilizer-to-tableau conversion, and reference samples.
- **Python tests:** pybind tests cover circuits, instructions, repeat blocks, targets, DEMs, samplers, frame/tableau simulators, matched errors, measurements-to-detection conversion, stabilizer primitives, exports, flow utilities, inverse utilities, detecting regions, and generated APIs.
- **JavaScript tests:** tests cover JS `Circuit`, `PauliString`, `Tableau`, and `TableauSimulator`.
- **Glue tests:** `stimcirq`, Crumble, Sinter, `stimflow`, ZX, and lattice-surgery glue packages each include their own language-specific test suites.
- **Source:** [src/stim](../vendor/stim/src/stim), [glue](../vendor/stim/glue), [file_lists/test_files](../vendor/stim/file_lists/test_files).

## 18. Porting-Oriented Priority Notes

- **Highest-priority stable contracts:** `.stim`, `.dem`, result formats, CLI commands/options/stdout/stderr/exit status, Python API classes/functions, gate names, parser aliases, and seeded/statistical behavior contracts.
- **Important implementation surfaces:** C++ headers, SIMD bit kernels, frame simulation, tableau simulation, DEM analysis, measurement-to-detection conversion, packed IO, and diagram backends.
- **Ecosystem surfaces to schedule separately:** JavaScript/WASM, Crumble, `stimcirq`, `sinter`, `stimflow`, ZX/lattice-surgery glue, generated docs, package metadata, and research-data references.
- **Known compatibility traps:** parser aliases, optional tags, Stim-specific tag escaping, relative `rec[-k]` indexing, sweep-bit defaulting, DEM decomposition separators, `ptb64` 64-shot grouping, little-endian packed bits, CLI legacy modes, Python deprecated methods that still exist, and non-stability of exact seeded streams across architectures.
