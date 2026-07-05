# RPF5 Detecting Regions Progress Report

## Summary

This RPF5 report covers bounded repeat traversal, additive detector or logical-observable target filters, promoted unsigned Clifford propagation, and ignored-anticommutation mode in the Rust `circuit_detecting_regions` utility for the currently supported gate subset.
The target-filter slice adds a `DemTarget`-based detecting-region API that can query detector and logical-observable targets, default-like helpers for all detector and logical-observable targets and all ticks, and the pinned Stim `MX` and `MZZ` detecting-region examples.
The unsigned Clifford slice now adds the full single-qubit Clifford gate set with plain qubit targets plus fixed two-qubit tableau-backed Clifford gates with plain qubit-pair targets.
It is not an RPF5 completion report because detecting-region target shapes beyond plain qubit pairs, multi-detector generated-code cases, gauge behavior, missing-detector families, and measurement-rich flow-transform integration remain active work.

## Implemented Surfaces

- `circuit_detecting_regions` now validates supported instructions recursively through repeat blocks instead of rejecting every repeat block.
- Detector and tick counts are computed through repeat blocks with checked arithmetic.
- Reverse traversal snapshots repeat bodies in reverse execution order, preserving global detector, measurement-record, and tick numbering for bounded repeat workloads.
- Detecting-region extraction rejects excessive repeat expansion before unbounded unrolling.
- `circuit_detecting_regions_for_targets` returns detecting regions keyed by `DemTarget` and supports detector and logical-observable target filters while preserving the original detector-id `circuit_detecting_regions` API as a wrapper.
- `all_detecting_region_targets` returns the currently declared detector and logical-observable targets within the dense helper materialization cap, and `all_detecting_region_ticks` returns all tick indices within the documented helper cap.
- The supported validation set now includes `R`/`RX`/`RY`, `M`/`MX`/`MY`, `MXX`/`MYY`/`MZZ`, the full single-qubit Clifford gate set with plain qubit targets, fixed two-qubit tableau-backed Clifford gates with plain qubit-pair targets, `TICK`, `DETECTOR`, and `OBSERVABLE_INCLUDE`.
- `ignore_anticommutation_errors=true` now runs the same reverse traversal with sparse-tracker anticommutations recorded instead of returned as errors, while the default false mode still fails closed on the same incompatible collapses.

## Target-Filter Scope

The target-filter slice promotes a new Rust API that returns regions keyed by `DemTarget` instead of only `DemDetectorId`.
The owned positive subcases are detector targets, logical-observable targets from measurement records or Pauli targets, duplicate target deduplication, default all-detector/all-observable target selection, `M`/`MX`/`MY`, `MXX`/`MYY`/`MZZ`, `H`, `CX`, `TICK`, `DETECTOR`, and `OBSERVABLE_INCLUDE`.
The owned negative subcases are out-of-range detector targets, out-of-range observable targets, separator or numeric DEM targets, dense all-target helper requests beyond the materialization cap or representable logical-observable target range, unsupported gates, feedback or sweep-controlled targets, and excessive repeat expansion.
The comparator class is structural Rust API parity against pinned Stim v1.16.0 Python examples from `circuit_pybind_test.py` and utility failure examples from `circuit_to_detecting_regions_test.py`.
The existing `circuit_detecting_regions` detector-id API remains as a compatibility wrapper and keeps its current output type.

## Clifford Gate Scope

The unsigned Clifford slice promotes the full single-qubit Clifford gate set with plain qubit targets plus fixed two-qubit tableau-backed Clifford gates with plain qubit-pair targets because the sparse reverse tracker now owns those unsigned transformations.
The owned positive subcases are deterministic single-detector circuits whose expected tick-indexed regions were cross-checked against pinned Stim v1.16.0 `detslice-text` output and then encoded as Rust structural tests.
The source-owned reproduction path is to write each circuit from `detecting_regions_clifford_supports_single_qubit_clifford_gate_set`, `detecting_regions_clifford_supports_controlled_pauli_propagation`, `detecting_regions_clifford_supports_swap_gate`, and `detecting_regions_clifford_supports_promoted_controlled_pauli_gate` to a temporary `.stim` file, run `target/oracle/stim-v1.16.0/out/stim diagram --type detslice-text --tick <stim_tick> < file.stim`, and compare Stim diagram tick `n + 1` to Stab detecting-region tick `n` after dropping the diagram sign because this Stab slice intentionally owns unsigned regions.
The full single-qubit Clifford test table covers `I`, `X`, `Y`, `Z`, `H`, `SQRT_Y_DAG`, `H_NXZ`, `SQRT_Y`, `S`, `H_XY`, `H_NXY`, `S_DAG`, `SQRT_X_DAG`, `SQRT_X`, `H_NYZ`, `H_YZ`, `C_XYZ`, `C_XYNZ`, `C_NXYZ`, `C_XNYZ`, `C_ZYX`, `C_ZNYX`, `C_NZYX`, and `C_ZYNX`.
The checked two-qubit unsigned expectations include the earlier `CZ` tick 0 `+ZZ` and tick 1 `+X_`, `CY` tick 0 `+XY` and tick 1 `+X_`, plus exact integration checks for `SWAP` as tick 0 `+_Z` and `XCX` as tick 0 `+ZX`.
The sparse reverse tracker has a tableau-backed all-basis regression for `II`, `XCX`, `XCY`, `XCZ`, `YCX`, `YCY`, `YCZ`, `SWAP`, `ISWAP`, `ISWAP_DAG`, `CXSWAP`, `SWAPCX`, `CZSWAP`, `SQRT_XX`, `SQRT_XX_DAG`, `SQRT_YY`, `SQRT_YY_DAG`, `SQRT_ZZ`, and `SQRT_ZZ_DAG`.
The owned negative subcases keep non-plain controlled-Pauli target shapes, sweep-shaped targets, generated-code multi-detector regions, and gauge-specific behavior fail-closed or partial until those surfaces are explicitly promoted.
The comparator class is structural Rust API parity against pinned Stim detecting-region semantics; the `detslice-text` command is only the pinned-Stim reproduction tool for the expected Pauli regions, and no diagram API parity is claimed.
The benchmark row for this slice is a non-primary report-only Rust utility workload measuring the promoted Clifford gates through `circuit_detecting_regions_for_targets`.
Resource behavior continues to use the existing detecting-region repeat and dense-helper caps.

## Anticommutation Scope

The ignored-anticommutation slice promotes the existing option field instead of adding a new public API.
The owned positive subcases are an in-circuit reset anticommutation and an implicit start-state anticommutation that both return the tick-indexed unsigned region when `ignore_anticommutation_errors=true`, plus empty-output filters under ignored mode.
The owned negative subcases keep the default false mode failing with an anticommutation error for in-circuit conflicts, implicit start-state conflicts, and empty-output filters.
The comparator class is structural Rust API parity against pinned Stim v1.16.0 `Circuit.detecting_regions` failure behavior plus the upstream C++ utility's explicit `ignore_anticommutation_errors` switch.
No separate benchmark row was added because the promoted mode reuses the same sparse reverse traversal and changes only the error policy.

## Tests

Implemented Rust tests:

- `detecting_regions_repeat_supports_bounded_ticks`
- `detecting_regions_repeat_rejects_excessive_expansion`
- `detecting_regions_target_api_matches_mx_python_example`
- `detecting_regions_target_api_supports_mzz_example`
- `detecting_regions_target_api_supports_logical_observable_targets`
- `detecting_regions_target_api_rejects_invalid_targets`
- `detecting_regions_target_api_rejects_dense_helper_expansion`
- `detecting_regions_clifford_supports_promoted_single_qubit_gates`
- `detecting_regions_clifford_supports_single_qubit_clifford_gate_set`
- `detecting_regions_clifford_supports_controlled_pauli_propagation`
- `detecting_regions_clifford_supports_swap_gate`
- `detecting_regions_clifford_supports_promoted_controlled_pauli_gate`
- `detecting_regions_clifford_rejects_feedback_controlled_cx`
- `detecting_regions_clifford_rejects_sweep_controlled_cx`
- `detecting_regions_anticommutation_supports_ignored_mode`
- `detecting_regions_anticommutation_rejects_false_mode`
- `detecting_regions_anticommutation_rejects_implicit_start_state`
- `detecting_regions_anticommutation_rejects_false_mode_with_empty_filters`

These tests cover bounded repeat tick traversal, expected tick-indexed detecting regions, resource rejection for repeat expansion beyond the current cap, pinned `MX` and `MZZ` detecting-region examples, detector and logical-observable target filters, default-like all-target and all-tick helpers, duplicate target deduplication, invalid target rejection, dense helper rejection before large allocation, promoted unsigned full single-qubit Clifford propagation, fixed two-qubit tableau-backed Clifford propagation, non-plain controlled-Pauli target-shape rejection, ignored anticommutation output, and default false-mode anticommutation rejection.

## Oracle Rows

Implemented row:

- `pf5-detecting-regions-repeat-rust`
- `pf5-detecting-regions-targets-rust`
- `pf5-detecting-regions-clifford-rust`
- `pf5-detecting-regions-anticommutation-rust`

Still broad and manifest-only:

- `pf5-detecting-regions-extended`

## Benchmark Rows

Report-only runner coverage:

- `pf5-detecting-regions-repeat`
- `pf5-detecting-regions-targets`
- `pf5-detecting-regions-clifford`

The repeat row measures the bounded repeat-tick detecting-region workload through the Rust public utility API.
The target row uses the default-like helper functions to set up detector, logical-observable, and tick filters, then times detecting-region extraction through the additive `DemTarget` API.
The Clifford row uses the default-like helper functions to set up filters for representative newly promoted single-qubit Clifford fixtures, the existing `CY` controlled-Pauli fixture, and a fixed two-qubit tableau-backed fixture covering `XCX`, `SWAP`, and `SQRT_XX`, then times extraction through the additive `DemTarget` API.
These rows remain `non-primary-report-only` because pinned Stim does not provide a faithful CLI timing ratio for this Rust utility surface.
They are not part of the 1.25x primary threshold file.
The target row is coverage for the promoted helper path, not a claim that all-target/all-tick scaling is representative for large generated-code workloads.

## Verification Evidence

Completed target checks for this slice:

```sh
cargo fmt --all --check
cargo test -p stab-core detecting_regions_repeat_ --quiet
cargo test -p stab-core detecting_regions_target_api --quiet
cargo test -p stab-core detecting_regions_clifford --quiet
cargo test -p stab-core detecting_regions_anticommutation --quiet
cargo test -p stab-core detecting_regions_anticommutation -- --list
cargo test -p stab-bench pf5::detector_utility_benchmark_rows_have_stab_compare_runners --quiet
cargo test -p stab-bench --quiet
cargo test -p stab-oracle fixtures --quiet
cargo clippy -p stab-core -p stab-bench -p stab-oracle --all-targets -- -D warnings
just oracle::run --milestone PF5 --structural
just bench::smoke
just bench::baseline --only pf5-detecting-regions-targets --out target/benchmarks/rpf5-detecting-region-targets-probe
just bench::compare --only pf5-detecting-regions-targets --baseline target/benchmarks/rpf5-detecting-region-targets-probe/baseline.json --report target/benchmarks/rpf5-detecting-region-targets-compare
just bench::baseline --only pf5-detecting-regions-clifford --out target/benchmarks/rpf5-detecting-region-clifford-fixed-two-qubit-probe
just bench::compare --only pf5-detecting-regions-clifford --baseline target/benchmarks/rpf5-detecting-region-clifford-fixed-two-qubit-probe/baseline.json --report target/benchmarks/rpf5-detecting-region-clifford-fixed-two-qubit-compare
# pinned Stim detslice-text reproduction loop for the full single-qubit Clifford table plus representative fixed two-qubit Clifford circuits
```

The pinned-Stim `detslice-text` reproduction passed for all 24 single-qubit Clifford table entries with tick `1` matching the expected prepared basis and tick `2` matching `X` after dropping sign; the same reproduction passed for `CY` as `XY` then `X_`, `CZ` as `ZZ` then `X_`, and the promoted `SWAP` and `XCX` integration expectations.
The target-filter benchmark probe reported `stab_pf5_detecting_regions_target_filters=0.006348216s` and `6.452e5 cases/s`, with output written to `target/benchmarks/rpf5-detecting-region-targets-compare`.
The fixed-two-qubit-inclusive benchmark probe reported `stab_pf5_detecting_regions_clifford_gates=0.041061913s` and `2.993e5 cases/s`, with output written to `target/benchmarks/rpf5-detecting-region-clifford-fixed-two-qubit-compare`.
Both rows remain report-only with the documented note that this Rust utility workload has no faithful pinned Stim CLI timing ratio.

## Audit And Review

Milestone audit status is complete for the target-filter and unsigned Clifford slices and incomplete for broader RPF5.
Full-code-review sidecars found one P1 issue in the dense all-target helper, where huge observable ids or detector counts could cause excessive allocation before failure.
The slice now rejects all-target helper requests beyond the dense materialization cap or representable logical-observable target range before allocation, with `detecting_regions_target_api_rejects_dense_helper_expansion` covering the regression.
The unsigned Clifford audit found a P2 evidence-provenance gap because the initial report did not preserve the pinned-Stim `detslice-text` reproduction path for the promoted-gate expectations; this report now records the exact command shape and source-owned expected region strings.
The full-code-review sidecar found no implementation findings for the earlier unsigned Clifford slice and confirmed the promoted-gate tests and fail-closed regression coverage.
The current Clifford refresh review found P2 documentation and evidence overclaims around future target-shape scope, representative benchmark wording, and repeat-folding coverage; the plan wording now says broader target shapes, the benchmark row is documented as representative, and `unitary_repeat_folding_matches_naive_all_single_qubit_cliffords` covers the full single-qubit Clifford repeat-folding table.
The ignored-anticommutation refresh review found a P2 false-mode compatibility gap where empty target or tick filters returned before anticommutation validation; the early return was removed and `detecting_regions_anticommutation_rejects_false_mode_with_empty_filters` covers the regression.
The same review pass found stale or overly broad evidence wording in the PF5 oracle manifest and historical remaining-partials plan; the manifest now narrows the remaining detecting-region placeholder, the anticommutation row uses the tight `detecting_regions_anticommutation` filter, and the historical plan lists the repeat, target, Clifford, and anticommutation rows.
The remaining review risk is that the report-only benchmark rows exercise promoted helper paths on small fixtures and should not be used as representative scaling evidence for large generated-code workloads.

## Remaining RPF5 Work

- Target-shape support beyond plain qubit pairs and the promoted measurement families, multi-detector generated-code regions, and gauge behavior.
- Missing-detector generated-code suffix analysis beyond the promoted honeycomb and toric cases, plus broader flow-dependent utility behavior.
- Measurement-rich flows beyond the promoted unsigned `has_flow` and `has_all_flows` Rust helper subset, including broader `flow_generators`, diagnostics, signed sampled checks, and transform integration.
