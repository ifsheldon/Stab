# M10 Progress Report

## Milestone

M10: Detector Error Model Core

## Status

Partial progress, not milestone-complete.
This slice implements the `.dem` format core and staged `analyze_errors` paths needed by the existing deterministic oracle rows, correlated-error rows, the first single-qubit and `CX` Pauli-propagation rows, the first `--decompose_errors` fallback and known-components rows, the high-repeat loop-folding row, the first `--allow_gauge_detectors` rows, and the first approximate disjoint-error threshold rows.
M10 still requires graphlike algorithm internals, hypergraph analyzer internals, general loop folding, broader two-qubit Clifford propagation, broader decomposition behavior, broader gauge-detector behavior, broader approximation behavior, sparse reverse detector-frame tracking, structural DEM equivalence for generated QEC circuits, and complete benchmark coverage before milestone audit can accept it.

## Tests Ported Or Created

- `cargo test -p stab-core dem` covers DEM target limits, DEM instruction separated target groups, repeat blocks, detector shifts, coordinates, observables, probabilities, separators, invalid input, deterministic detector declarations, measurement-flip analyzer output, identical-symptom merging and cancellation, unconditional and conditional correlated-error output, graphlike decomposition fallback, known-component decomposition, remnant-edge blocking, ignored decomposition failures, and undecomposable triple rejection under `--decompose_errors`, identity-noise no-ops, reset cutoff of pending single-qubit errors and channels, simple Pauli-error analyzer output, propagation of pending Pauli errors through single-qubit Clifford gates and `CX`, propagation through pair measurements, `HERALDED_PAULI_CHANNEL_1` basis behavior, `HERALDED_ERASE` threshold and basis behavior, propagation of approximate `PAULI_CHANNEL_1` basis probabilities through `H_XY`, `--allow_gauge_detectors` on upstream Bell-correlation gauge examples through `H`, `H_XY`, and `CX`, default gauge rejection, gauge-observable rejection even with gauge detectors allowed, single-qubit `DEPOLARIZE1` analyzer output using Stim's independent-channel conversion, two-qubit `DEPOLARIZE2` analyzer output and over-mixing rejection, exact-solved and approximate single-qubit `PAULI_CHANNEL_1` analyzer output, thresholded approximate two-qubit `PAULI_CHANNEL_2` analyzer output, shifted detector coordinates, and top-level folded repeat output.
- `cargo test -p stab-core error_decomp` covers the M10-owned independent/disjoint XYZ conversion and depolarizing-channel conversion subset ported from `src/stim/util_bot/error_decomp.test.cc`.
- `cargo test -p stab-core graphlike` covers the M10-owned graphlike edge, node, graph, and search-state construction, canonicalization, equality, ordering, hashing, display, graph target conversion, separator handling, repeat flattening, and DEM transition subset ported from `src/stim/search/graphlike/edge.test.cc`, `src/stim/search/graphlike/node.test.cc`, `src/stim/search/graphlike/graph.test.cc`, and `src/stim/search/graphlike/search_state.test.cc`.
- `cargo test -p stab-cli m10` covers `stab analyze_errors`, the legacy `--analyze_errors` alias, measurement-flip output, unconditional and conditional correlated-error output, `H`, `H_XY`, and `CX` Pauli-error propagation fixtures, `--decompose_errors` fallback output, `--block_decompose_from_introducing_remnant_edges`, `--ignore_decomposition_failures`, `--allow_gauge_detectors` on upstream CLI and `H_XY` gauge examples, default gauge rejection, gauge-observable rejection even with gauge detectors allowed, identity-noise no-op output, reset cutoff output, simple Pauli-error output, `DEPOLARIZE1`, `DEPOLARIZE2`, exact-solved `PAULI_CHANNEL_1`, bare and numeric-threshold `--approximate_disjoint_errors` for `PAULI_CHANNEL_1`, numeric-threshold `--approximate_disjoint_errors` for `PAULI_CHANNEL_2`, `HERALDED_ERASE`, `ELSE_CORRELATED_ERROR` fixtures, `--fold_loops` on a high-repeat fixture, and current flag parsing on supported circuits.
- `cargo test -p stab-bench m10_dem_benchmark_rows_have_stab_compare_runners` covers Stab-side M10 `.dem` parse, `.dem` print, and loop-folding analyzer benchmark runners.

## Implementation Areas

- Added `DetectorErrorModel`, DEM instruction, target, repeat block, detector id, observable id, parser, canonical printer, detector counting, observable counting, and detector-shift helpers in `stab-core`.
- Added the first graphlike search internal types for observable masks, edges, nodes, graphs, and canonical search states, including DEM flattening into graphlike edges and DEM error-instruction emission for search-state transitions.
- Added a staged circuit-to-DEM analyzer for deterministic detector declarations, measurement-flip errors, unconditional and conditional correlated Pauli errors, identity-noise no-ops, reset cutoff of pending single-qubit noise, identical-symptom error merging, and simple single-qubit `X_ERROR`, `Y_ERROR`, and `Z_ERROR` effects feeding measurement-record detectors and observables.
- Added single-qubit `DEPOLARIZE1` handling for probabilities up to `3/4` using Stim's independent per-channel probability conversion, with over-mixing rejection.
- Added two-qubit `DEPOLARIZE2` handling for probabilities up to `15/16`, using Stim's independent two-qubit Pauli-channel decomposition and identical-symptom XOR merging.
- Added exact-solved `PAULI_CHANNEL_1` handling for single-qubit channels that can be represented as independent errors without `--approximate_disjoint_errors`.
- Added approximate `PAULI_CHANNEL_1` handling for remaining single-qubit channels under bare and numeric-threshold `--approximate_disjoint_errors`, including rejection when approximation is not explicitly enabled or when a component exceeds the threshold.
- Added approximate `PAULI_CHANNEL_2` handling for two-qubit channels under numeric-threshold `--approximate_disjoint_errors`, including per-channel disjoint component summation before independent error merging.
- Added pair-measurement analyzer handling for `MXX`, `MYY`, and `MZZ`, including pending Pauli-error parity updates and matching backward gauge-tracker sensitivity for deterministic detector checks.
- Added `HERALDED_PAULI_CHANNEL_1` analyzer handling for single-component basis errors and thresholded disjoint multi-component errors.
- Added `HERALDED_ERASE` analyzer handling as thresholded disjoint heralded Pauli components with one herald measurement per target.
- Added scoped Clifford propagation for pending Pauli errors through single-qubit Clifford gates and `CX`, propagation of pending single-qubit `PAULI_CHANNEL_1` basis probabilities through single-qubit Clifford gates, cancellation of duplicate Pauli effects after propagation, and an explicit error instead of silent misanalysis when pending single-qubit Pauli channels would need to cross `CX`.
- Added `--decompose_errors` target-level decomposition paths that first try exact decomposition using known graphlike components, then optionally introduce a graphlike remnant edge, respect `--block_decompose_from_introducing_remnant_edges`, support `--ignore_decomposition_failures`, and reject simple undecomposable detector triples.
- Added a backward detector-sensitivity pass for the first `--allow_gauge_detectors` cases, including default rejection of non-deterministic detectors, rejection of non-deterministic observables even with gauge detectors allowed, and 50 percent gauge-error emission for detector-only gauge sets through `H`, `H_XY`, and `CX`.
- Added top-level `--fold_loops` handling for repeat blocks by analyzing one body, wrapping it in a DEM repeat block, and appending the body detector shift.
- Added `stab analyze_errors` CLI dispatch, including the legacy `--analyze_errors` alias and current staged flag parsing.
- Extended the oracle core fixture runner to support `core-dem-parse-print`.
- Implemented the current M10 exact and structural rows for `.dem` parse-print and basic `analyze_errors`.
- Added Stab-side benchmark compare runners for the M10 `.dem` parse, `.dem` print, and loop-folding analyzer rows.

## Current Evidence

| Requirement | Status | Evidence |
| --- | --- | --- |
| `.dem` parser and canonical printer | Partial | `DetectorErrorModel::from_dem_str`, `DetectorErrorModel::to_dem_string`, `just oracle::run --milestone M10 --exact` |
| DEM core types, repeats, coordinates, detector shifts, observables, separators, probability validation | Partial | `coverage-dem-dem-instruction`, `cargo test -p stab-core dem_instruction`, `cargo test -p stab-core dem` |
| Graphlike search graph structures | Partial | `coverage-search-graphlike-edge`, `coverage-search-graphlike-node`, `coverage-search-graphlike-graph`, `coverage-search-graphlike-search-state`, `cargo test -p stab-core graphlike`, `just oracle::run --milestone M10 --structural` |
| `stim analyze_errors` staged default behavior | Partial | `cargo test -p stab-cli m10`, `just oracle::run --milestone M10 --structural` |
| `stim analyze_errors` unconditional correlated Pauli errors | Partial | `m10-analyze-errors-correlated-error`, `cargo test -p stab-core correlated_error`, `cargo test -p stab-cli correlated_error`, `just oracle::run --milestone M10 --exact`, `just oracle::record --check-clean` |
| `stim analyze_errors --decompose_errors` graphlike decomposition | Partial | `m10-analyze-errors-decompose-fallback`, `m10-analyze-errors-decompose-known-components`, `cargo test -p stab-core decompose`, `cargo test -p stab-cli decompose`, `cargo test -p stab-cli ignore_decomposition`, `just oracle::run --milestone M10 --exact` |
| `stim analyze_errors --allow_gauge_detectors` | Partial | `m10-analyze-errors-allow-gauge-detectors`, `m10-analyze-errors-allow-gauge-detectors-hxy`, `cargo test -p stab-core gauge`, `cargo test -p stab-cli gauge`, `just oracle::run --milestone M10 --exact`, `just oracle::record --check-clean` |
| `stim analyze_errors --approximate_disjoint_errors` conditional correlated Pauli errors | Partial | `m10-analyze-errors-else-correlated-error`, `cargo test -p stab-core else_correlated`, `cargo test -p stab-cli else_correlated`, `just oracle::run --milestone M10 --exact`, `just oracle::record --check-clean` |
| `stim analyze_errors` identity-noise no-ops | Partial | `m10-analyze-errors-identity-noise`, `cargo test -p stab-core identity_noise`, `cargo test -p stab-cli identity_noise`, `just oracle::run --milestone M10 --exact` |
| `stim analyze_errors` measurement-flip errors and identical-symptom merging | Partial | `m10-analyze-errors-measurement-flip`, `cargo test -p stab-core dem_analyzer_maps_measurement_flip_probability_to_error`, `cargo test -p stab-core dem_analyzer_merges_identical_error_symptoms`, `cargo test -p stab-cli measurement_flip`, `just oracle::run --milestone M10 --exact` |
| `stim analyze_errors` reset cutoff behavior | Partial | `m10-analyze-errors-reset-clears-error`, `cargo test -p stab-core reset`, `cargo test -p stab-cli reset_clears_error`, `just oracle::run --milestone M10 --exact` |
| `stim analyze_errors` Clifford propagation | Partial | `m10-analyze-errors-h-propagates-pauli-error`, `m10-analyze-errors-hxy-propagates-pauli-error`, `m10-analyze-errors-cnot-propagates-pauli-error`, `cargo test -p stab-core propagates_pauli`, `cargo test -p stab-cli propagates_pauli`, `just oracle::run --milestone M10 --exact`, `just oracle::record --check-clean` |
| `stim analyze_errors` single-qubit `DEPOLARIZE1` | Partial | `m10-analyze-errors-depolarize1`, `coverage-util-bot-error-decomp`, `cargo test -p stab-core depolarize1`, `cargo test -p stab-core error_decomp`, `cargo test -p stab-cli depolarize1`, `just oracle::run --milestone M10 --exact` |
| `stim analyze_errors` two-qubit `DEPOLARIZE2` | Partial | `m10-analyze-errors-depolarize2`, `cargo test -p stab-core depolarize2`, `cargo test -p stab-cli depolarize2`, `just oracle::run --milestone M10 --exact`, `just oracle::record --check-clean` |
| `stim analyze_errors` exact-solved single-qubit `PAULI_CHANNEL_1` | Partial | `m10-analyze-errors-exact-pauli-channel1`, `cargo test -p stab-core exact_solved_pauli_channel1`, `cargo test -p stab-cli exact_pauli_channel1`, `just oracle::run --milestone M10 --exact` |
| `stim analyze_errors --approximate_disjoint_errors` | Partial | `m10-analyze-errors-approx-pauli-channel1`, `m10-analyze-errors-approx-pauli-channel1-threshold`, `m10-analyze-errors-approx-pauli-channel2-threshold`, `cargo test -p stab-core threshold`, `cargo test -p stab-core pauli_channel1`, `cargo test -p stab-core pauli_channel2`, `cargo test -p stab-cli numeric_threshold`, `cargo test -p stab-cli pauli_channel2`, `just oracle::run --milestone M10 --exact` |
| `stim analyze_errors --approximate_disjoint_errors` heralded erasure | Partial | `m10-analyze-errors-heralded-erase`, `cargo test -p stab-core heralded_erase`, `cargo test -p stab-cli heralded_erase`, `just oracle::run --milestone M10 --exact` |
| `circuit_to_dem` heralded Pauli channel basis behavior | Partial | `coverage-util-top-circuit-to-dem`, `cargo test -p stab-core circuit_to_dem`, `just oracle::run --milestone M10 --structural` |
| Structural DEM comparators for generated QEC circuits | Missing | Remaining M10 manifest-only structural rows |
| Loop folding without flattening high-repeat circuits | Partial | `m10-analyze-errors-fold-repeat`, `cargo test -p stab-core dem_analyzer_fold`, `just bench::compare --milestone M10` |
| M10 benchmark reporting | Partial | `just bench::compare --milestone M10` measures `.dem` parse, `.dem` print, and loop-folding rows; decomposition, graphlike, and full analyzer rows remain pending or missing baseline |

## Verification Commands

- `cargo fmt --check --all`
- `cargo clippy --workspace --all-targets -- -D warnings`
- `cargo test --workspace`
- `just oracle::run --milestone M10 --exact`
- `just oracle::run --milestone M10 --structural`
- `just bench::compare --milestone M10`
