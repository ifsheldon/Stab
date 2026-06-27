# M10 Progress Report

## Milestone

M10: Detector Error Model Core

## Status

Partial progress, not milestone-complete.
This slice implements the `.dem` format core and staged `analyze_errors` paths needed by the existing deterministic oracle rows, correlated-error rows, the first `--decompose_errors` fallback and known-components rows, the high-repeat loop-folding row, and the first approximate disjoint-error threshold rows.
M10 still requires graphlike and hypergraph analyzer internals, general loop folding, decomposition behavior, gauge-detector handling, broader approximation behavior, sparse reverse detector-frame tracking, structural DEM equivalence for generated QEC circuits, and complete benchmark coverage before milestone audit can accept it.

## Tests Ported Or Created

- `cargo test -p stab-core dem` covers DEM target limits, repeat blocks, detector shifts, coordinates, observables, probabilities, separators, invalid input, deterministic detector declarations, measurement-flip analyzer output, identical-symptom merging and cancellation, unconditional and conditional correlated-error output, graphlike decomposition fallback, known-component decomposition, remnant-edge blocking, ignored decomposition failures, and undecomposable triple rejection under `--decompose_errors`, identity-noise no-ops, reset cutoff of pending single-qubit errors and channels, simple Pauli-error analyzer output, single-qubit `DEPOLARIZE1` analyzer output, two-qubit `DEPOLARIZE2` analyzer output and over-mixing rejection, exact-solved and approximate single-qubit `PAULI_CHANNEL_1` analyzer output, thresholded approximate two-qubit `PAULI_CHANNEL_2` analyzer output, shifted detector coordinates, and top-level folded repeat output.
- `cargo test -p stab-cli m10` covers `stab analyze_errors`, the legacy `--analyze_errors` alias, measurement-flip output, unconditional and conditional correlated-error output, `--decompose_errors` fallback output, `--block_decompose_from_introducing_remnant_edges`, `--ignore_decomposition_failures`, identity-noise no-op output, reset cutoff output, simple Pauli-error output, `DEPOLARIZE1`, `DEPOLARIZE2`, exact-solved `PAULI_CHANNEL_1`, bare and numeric-threshold `--approximate_disjoint_errors` for `PAULI_CHANNEL_1`, numeric-threshold `--approximate_disjoint_errors` for `PAULI_CHANNEL_2`, `ELSE_CORRELATED_ERROR` fixtures, `--fold_loops` on a high-repeat fixture, and current flag parsing on supported circuits.
- `cargo test -p stab-bench m10_dem_benchmark_rows_have_stab_compare_runners` covers Stab-side M10 `.dem` parse, `.dem` print, and loop-folding analyzer benchmark runners.

## Implementation Areas

- Added `DetectorErrorModel`, DEM instruction, target, repeat block, detector id, observable id, parser, canonical printer, detector counting, observable counting, and detector-shift helpers in `stab-core`.
- Added a staged circuit-to-DEM analyzer for deterministic detector declarations, measurement-flip errors, unconditional and conditional correlated Pauli errors, identity-noise no-ops, reset cutoff of pending single-qubit noise, identical-symptom error merging, and simple single-qubit `X_ERROR`, `Y_ERROR`, and `Z_ERROR` effects feeding measurement-record detectors and observables.
- Added single-qubit `DEPOLARIZE1` handling for probabilities up to `3/4`, with over-mixing rejection.
- Added two-qubit `DEPOLARIZE2` handling for probabilities up to `15/16`, using Stim's independent two-qubit Pauli-channel decomposition and identical-symptom XOR merging.
- Added exact-solved `PAULI_CHANNEL_1` handling for single-qubit channels that can be represented as independent errors without `--approximate_disjoint_errors`.
- Added approximate `PAULI_CHANNEL_1` handling for remaining single-qubit channels under bare and numeric-threshold `--approximate_disjoint_errors`, including rejection when approximation is not explicitly enabled or when a component exceeds the threshold.
- Added approximate `PAULI_CHANNEL_2` handling for two-qubit channels under numeric-threshold `--approximate_disjoint_errors`, including per-channel disjoint component summation before independent error merging.
- Added `--decompose_errors` target-level decomposition paths that first try exact decomposition using known graphlike components, then optionally introduce a graphlike remnant edge, respect `--block_decompose_from_introducing_remnant_edges`, support `--ignore_decomposition_failures`, and reject simple undecomposable detector triples.
- Added top-level `--fold_loops` handling for repeat blocks by analyzing one body, wrapping it in a DEM repeat block, and appending the body detector shift.
- Added `stab analyze_errors` CLI dispatch, including the legacy `--analyze_errors` alias and current staged flag parsing.
- Extended the oracle core fixture runner to support `core-dem-parse-print`.
- Implemented the current M10 exact and structural rows for `.dem` parse-print and basic `analyze_errors`.
- Added Stab-side benchmark compare runners for the M10 `.dem` parse, `.dem` print, and loop-folding analyzer rows.

## Current Evidence

| Requirement | Status | Evidence |
| --- | --- | --- |
| `.dem` parser and canonical printer | Partial | `DetectorErrorModel::from_dem_str`, `DetectorErrorModel::to_dem_string`, `just oracle::run --milestone M10 --exact` |
| DEM core types, repeats, coordinates, detector shifts, observables, separators, probability validation | Partial | `cargo test -p stab-core dem` |
| `stim analyze_errors` staged default behavior | Partial | `cargo test -p stab-cli m10`, `just oracle::run --milestone M10 --structural` |
| `stim analyze_errors` unconditional correlated Pauli errors | Partial | `m10-analyze-errors-correlated-error`, `cargo test -p stab-core correlated_error`, `cargo test -p stab-cli correlated_error`, `just oracle::run --milestone M10 --exact`, `just oracle::record --check-clean` |
| `stim analyze_errors --decompose_errors` graphlike decomposition | Partial | `m10-analyze-errors-decompose-fallback`, `m10-analyze-errors-decompose-known-components`, `cargo test -p stab-core decompose`, `cargo test -p stab-cli decompose`, `cargo test -p stab-cli ignore_decomposition`, `just oracle::run --milestone M10 --exact` |
| `stim analyze_errors --approximate_disjoint_errors` conditional correlated Pauli errors | Partial | `m10-analyze-errors-else-correlated-error`, `cargo test -p stab-core else_correlated`, `cargo test -p stab-cli else_correlated`, `just oracle::run --milestone M10 --exact`, `just oracle::record --check-clean` |
| `stim analyze_errors` identity-noise no-ops | Partial | `m10-analyze-errors-identity-noise`, `cargo test -p stab-core identity_noise`, `cargo test -p stab-cli identity_noise`, `just oracle::run --milestone M10 --exact` |
| `stim analyze_errors` measurement-flip errors and identical-symptom merging | Partial | `m10-analyze-errors-measurement-flip`, `cargo test -p stab-core dem_analyzer_maps_measurement_flip_probability_to_error`, `cargo test -p stab-core dem_analyzer_merges_identical_error_symptoms`, `cargo test -p stab-cli measurement_flip`, `just oracle::run --milestone M10 --exact` |
| `stim analyze_errors` reset cutoff behavior | Partial | `m10-analyze-errors-reset-clears-error`, `cargo test -p stab-core reset`, `cargo test -p stab-cli reset_clears_error`, `just oracle::run --milestone M10 --exact` |
| `stim analyze_errors` single-qubit `DEPOLARIZE1` | Partial | `m10-analyze-errors-depolarize1`, `cargo test -p stab-core depolarize1`, `cargo test -p stab-cli depolarize1`, `just oracle::run --milestone M10 --exact` |
| `stim analyze_errors` two-qubit `DEPOLARIZE2` | Partial | `m10-analyze-errors-depolarize2`, `cargo test -p stab-core depolarize2`, `cargo test -p stab-cli depolarize2`, `just oracle::run --milestone M10 --exact`, `just oracle::record --check-clean` |
| `stim analyze_errors` exact-solved single-qubit `PAULI_CHANNEL_1` | Partial | `m10-analyze-errors-exact-pauli-channel1`, `cargo test -p stab-core exact_solved_pauli_channel1`, `cargo test -p stab-cli exact_pauli_channel1`, `just oracle::run --milestone M10 --exact` |
| `stim analyze_errors --approximate_disjoint_errors` | Partial | `m10-analyze-errors-approx-pauli-channel1`, `m10-analyze-errors-approx-pauli-channel1-threshold`, `m10-analyze-errors-approx-pauli-channel2-threshold`, `cargo test -p stab-core threshold`, `cargo test -p stab-core pauli_channel2`, `cargo test -p stab-cli numeric_threshold`, `cargo test -p stab-cli pauli_channel2`, `just oracle::run --milestone M10 --exact` |
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
