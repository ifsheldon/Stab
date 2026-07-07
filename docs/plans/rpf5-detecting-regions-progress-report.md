# RPF5 Detecting Regions Progress Report

## Summary

This RPF5 report covers bounded repeat traversal, additive detector or logical-observable target filters, generated repetition-code all-target/all-tick selection with selected exact D0, D6, and L0 regions, selected generated rotated and unrotated surface-code all-target/all-tick helper counts with exact D0, D4, and L0 regions across the first six ticks, promoted unsigned Clifford propagation, selected target shapes including inverted measurement targets, selected measurement-record feedback placements, selected gate-order-valid sweep-controlled Pauli sign-only no-op groups plus selected `CZ` classical-only bit-bit no-op groups, source-owned fail-closed evidence for non-`CZ` classical-bit target groups, `MPAD` measurement pads, `MPP` Pauli-product measurements, `SPP`/`SPP_DAG` unitary Pauli products, and heralded record-producing noise, tagged detecting-region instructions with ordinary non-record-producing noise as a traversal no-op, ignored-anticommutation mode, selected measurement-gauge ignored-mode behavior, and product-measurement gauge-cancellation behavior in the Rust `circuit_detecting_regions` utility for the currently supported gate subset.
The target-filter slice adds a `DemTarget`-based detecting-region API that can query detector and logical-observable targets, default-like helpers for all detector and logical-observable targets and all ticks, and the pinned Stim `MX` and `MZZ` detecting-region examples.
The unsigned Clifford slice now adds the full single-qubit Clifford gate set with plain qubit targets plus fixed two-qubit tableau-backed Clifford gates with plain qubit-pair targets.
It is not an RPF5 completion report because broader detecting-region target shapes outside the promoted positive set and source-owned fail-closed set, broader generated-code cases beyond the promoted repetition-code and selected rotated and unrotated surface-code shapes, broader gauge behavior, missing-detector families, and measurement-rich flow-transform integration remain active work.

## Implemented Surfaces

- `circuit_detecting_regions` now validates supported instructions recursively through repeat blocks instead of rejecting every repeat block.
- Detector and tick counts are computed through repeat blocks with checked arithmetic.
- Reverse traversal snapshots repeat bodies in reverse execution order, preserving global detector, measurement-record, and tick numbering for bounded repeat workloads.
- Detecting-region extraction rejects excessive repeat expansion before unbounded unrolling.
- `circuit_detecting_regions_for_targets` returns detecting regions keyed by `DemTarget` and supports detector and logical-observable target filters while preserving the original detector-id `circuit_detecting_regions` API as a wrapper.
- `all_detecting_region_targets` returns the currently declared detector and logical-observable targets within the dense helper materialization cap, and `all_detecting_region_ticks` returns all tick indices within the documented helper cap.
- The supported validation set now includes `R`/`RX`/`RY`, `M`/`MX`/`MY`, `MXX`/`MYY`/`MZZ`, `MPAD`, `MPP`, `SPP`, `SPP_DAG`, `HERALDED_ERASE`, `HERALDED_PAULI_CHANNEL_1`, ordinary non-record-producing noise gates as no-ops, the full single-qubit Clifford gate set with plain qubit targets, fixed two-qubit tableau-backed Clifford gates with plain qubit-pair targets, selected measurement-record feedback placements for `CX`, `CY`, `CZ`, `XCZ`, and `YCZ`, selected gate-order-valid sweep-controlled Pauli groups with exactly one sweep bit and one plain qubit target, selected `CZ` groups with two classical-bit targets, `TICK`, `DETECTOR`, and `OBSERVABLE_INCLUDE`.
- The selected target-shape slice accepts inverted targets for the promoted measurement and reset-measurement families, selected measurement-record feedback placements for controlled-Pauli gates, selected gate-order-valid sweep-controlled Pauli sign-only no-op groups, selected `CZ` classical-only bit-bit no-op groups, constant-result `MPAD` measurement pads, Pauli-product `MPP` measurement targets, unsigned `SPP`/`SPP_DAG` unitary Pauli-product propagation, and heralded single-qubit record-producing noise while keeping reset, unsupported Clifford target shapes, unsupported feedback placements, source-owned fail-closed non-`CZ` sweep/sweep, record/sweep, and record/record groups, invalid sweep/qubit target orders, and heralded-noise validation fail-closed.
- `MR`/`MRX`/`MRY`, `QUBIT_COORDS`, and `SHIFT_COORDS` are accepted for detecting-region traversal, promoting the generated repetition-code shape from pinned Stim's target-filter example plus selected generated rotated and unrotated surface-code slices.
- `ignore_anticommutation_errors=true` now runs the same reverse traversal with sparse-tracker anticommutations recorded instead of returned as errors, while the default false mode still fails closed on the same incompatible collapses.
- The selected gauge slice covers public detecting-region behavior for single-measurement gauge collapse under ignored mode plus product-measurement cancellation when the anticommuting sensitivities xor to zero.

## Target-Filter Scope

The target-filter slice promotes a new Rust API that returns regions keyed by `DemTarget` instead of only `DemDetectorId`.
The owned positive subcases are detector targets, logical-observable targets from measurement records or Pauli targets, duplicate target deduplication, default all-detector/all-observable target selection, `M`/`MX`/`MY`, `MXX`/`MYY`/`MZZ`, `H`, `CX`, `TICK`, `DETECTOR`, and `OBSERVABLE_INCLUDE`.
The owned negative subcases are out-of-range detector targets, out-of-range observable targets, separator or numeric DEM targets, dense all-target helper requests beyond the materialization cap or representable logical-observable target range, unsupported gates, unsupported feedback placements, source-owned fail-closed non-`CZ` record-record, sweep/sweep, and record/sweep target groups, and excessive repeat expansion.
The comparator class is structural Rust API parity against pinned Stim v1.16.0 Python examples from `circuit_pybind_test.py` and utility failure examples from `circuit_to_detecting_regions_test.py`.
The existing `circuit_detecting_regions` detector-id API remains as a compatibility wrapper and keeps its current output type.

## Target-Shape Scope

The selected target-shape slice promotes inverted qubit targets for supported measurement operations where inversion flips the reported measurement result but does not change Pauli sensitivity, promotes selected measurement-record feedback placements through the sparse reverse tracker classical-feedback path, promotes selected gate-order-valid sweep-controlled Pauli groups as unsigned sign-only no-ops, promotes selected `CZ` classical-only bit-bit groups as no-ops through a detecting-region traversal splitter while keeping the shared sparse reverse tracker strict for other consumers, promotes `MPAD` measurement pads through the sparse reverse tracker record-drop path, promotes `MPP` Pauli-product measurement targets through the sparse reverse tracker product-measurement path, promotes unsigned `SPP`/`SPP_DAG` unitary Pauli-product propagation through the sparse reverse tracker unitary-product path, and promotes heralded single-qubit record-producing noise through the same record-drop path as `MPAD`.
The owned positive subcases are `M !0`, `MX !0`, `MY !0`, `MR !0`, `MRX !0`, `MRY !0`, `MXX !0 1`, `MYY !0 !1`, `MZZ 0 !1`, `CX rec[-1] q`, `CY rec[-1] q`, `CZ rec[-1] q`, `CZ q rec[-1]`, `XCZ q rec[-1]`, `YCZ q rec[-1]`, `CX sweep[k] q`, `CY sweep[k] q`, `CZ sweep[k] q`, `CZ q sweep[k]`, `CZ sweep[i] sweep[j]`, `CZ rec[-k] sweep[j]`, `CZ sweep[j] rec[-k]`, `CZ rec[-a] rec[-b]`, `XCZ q sweep[k]`, `YCZ q sweep[k]`, `MPAD 0 1` record-index preservation with empty pad-only regions, a two-record `MPP !X0*Y1*Z2 Z3` detector plus observable query, `SPP` or `SPP_DAG` multi-product logical-observable propagation compared against the existing decomposed circuit path, `HERALDED_ERASE` and `HERALDED_PAULI_CHANNEL_1` herald-only detector records producing empty regions, and herald-plus-measurement detector records preserving the adjacent measurement sensitivity, each checked through tick-indexed detecting regions.
The owned negative scope keeps single-qubit Clifford gates, unsupported fixed two-qubit Clifford target shapes, `R`/`RX`/`RY`, and heralded record-producing noise plain-qubit-target-only for the current detecting-region subset, rejects unsupported feedback positions, non-`CZ` record-record feedback groups, non-`CZ` sweep/sweep groups, non-`CZ` record/sweep groups, invalid sweep/qubit target orders, and rejects anti-Hermitian Pauli products for both measurement and unitary Pauli-product gates.
The comparator class is structural Rust API parity against pinned Stim v1.16.0 measurement-target and detslice-text semantics plus `SparseUnsignedRevFrameTracker` reverse propagation, which already ignores target inversion when deriving qubit sensitivity, owns classical-feedback propagation, treats selected gate-order-valid sweep-controlled Pauli groups and selected `CZ` classical-only bit-bit groups as unsigned sign-only no-ops, owns measurement-pad and heralded-record dropping, owns Pauli-product measurement undo semantics, and treats `SPP` and `SPP_DAG` as unsigned unitary Pauli-product propagation.
No separate benchmark row was added for the promoted target-shape subcases because this slice has structural Rust API evidence only and no faithful pinned Stim CLI timing ratio for this Rust API.
The existing report-only detecting-region benchmark rows cover repeat traversal, target filtering, Clifford propagation, generated repetition-code extraction, and selected generated rotated surface-code extraction, but they should not be cited as direct performance evidence for the feedback, sweep-control no-op, `MPAD` record-drop, heralded-record-drop, `MPP` product-measurement, `SPP`/`SPP_DAG` unitary-product target-shape branches, or the unrotated surface-code exact-output fixture.

## Ordinary Noise And Tag Scope

The ordinary-noise slice promotes Stim's tagged detecting-region example and the category boundary around noise that does not produce measurement records.
The owned positive subcases are tagged `R`, `X_ERROR`, noisy `M`, and tagged `DETECTOR` instructions matching the pinned `Circuit.detecting_regions` example, plus source-owned no-op traversal for `X_ERROR`, `Y_ERROR`, `Z_ERROR`, `I_ERROR`, `II_ERROR`, `DEPOLARIZE1`, `DEPOLARIZE2`, `PAULI_CHANNEL_1`, `PAULI_CHANNEL_2`, `CORRELATED_ERROR`, and `ELSE_CORRELATED_ERROR`.
`HERALDED_ERASE` and `HERALDED_PAULI_CHANNEL_1` remain outside this ordinary-noise no-op slice because they produce measurement records; their selected detecting-region behavior is covered by the target-shape slice.
The comparator class is structural Rust API parity against pinned Stim v1.16.0 `Circuit.detecting_regions` tagged-noise behavior, plus source-owned category coverage for ordinary noise.
No separate benchmark row was added because ordinary-noise traversal is a validation and sparse-tracker no-op branch inside the existing detecting-region workloads.

## Generated Repetition-Code Scope

The generated-code slice promotes the pinned Stim v1.16.0 `Circuit.detecting_regions` repetition-code filter example for the Rust generator surface, not arbitrary generated-code parity.
The owned positive subcases are generated `repetition_code:memory` with distance 3 and rounds 3, default-like all-detector plus all-observable target selection, default-like all-tick selection, selected multi-detector region expectations across the first and final detector rounds, logical-observable sensitivity across selected ticks, `MR` validation, repeat traversal, and `SHIFT_COORDS` traversal.
The source-owned reproduction path is to generate the same circuit with `target/oracle/stim-v1.16.0/out/stim gen --code repetition_code --task memory --distance 3 --rounds 3`, then run `target/oracle/stim-v1.16.0/out/stim diagram --type detslice-text --filter_coords <D#|L#> --tick <stim_tick>` and compare Stim diagram tick `n + 1` to Stab detecting-region tick `n` after dropping diagram formatting.
The exact generated-code expectations encoded from pinned Stim are D0 at Stab ticks 0, 1, and 2 as `+ZZZ__`, `+_ZZ__`, and `+_Z___`; D6 at Stab ticks 6, 7, and 8 as `+_Z___`, `+ZZ___`, and `+ZZZ__`; and L0 at Stab ticks 0, 1, 2, 6, 7, and 8 as `+____Z`.
The owned negative scope is unchanged except for promoted `MPAD` and heralded record-producing noise: broader generated surface-code region tables, coordinate-prefix target filters, non-plain target shapes, and broader gauge-specific behavior remain active work or deferred binding ergonomics.
The benchmark row for this slice is a non-primary report-only Rust utility workload measuring generated repetition-code region extraction through `circuit_detecting_regions_for_targets`.

## Generated Surface-Code Scope

The generated surface-code slices promote narrow source-owned rotated and unrotated surface-code detecting-region samples for the Rust generator surface, not arbitrary generated-code parity.
The rotated owned positive subcases are generated `surface_code:rotated_memory_z` with distance 3 and rounds 3, default-like all-detector plus all-observable target count of 25, default-like all-tick selection from 0 through 20, and exact selected-region extraction for D0, D4, and L0 across Stab ticks 0 through 5.
The source-owned reproduction path is to generate the same circuit with `target/oracle/stim-v1.16.0/out/stim gen --code surface_code --task rotated_memory_z --distance 3 --rounds 3`, then run `target/oracle/stim-v1.16.0/out/stim diagram --type detslice-text --filter_coords <D#|L#> --tick <stim_tick>` and compare Stim diagram tick `n + 1` to Stab detecting-region tick `n` after dropping diagram formatting.
The encoded exact expectations are D0 at ticks 0 through 5 as `+________Z_____ZZ__________`, `+________Z_____ZZ__________`, `+________Z_____Z___________`, `+______________Z___________`, `+______________Z___________`, and `+______________Z___________`; D4 at ticks 0 through 5 as `+__Z_______________________`, `+__X_______________________`, `+__XX______________________`, `+_XXX_____X________________`, `+_XXX_____X________________`, and `+_XXX______________________`; and L0 at ticks 0 through 5 as `+_Z_Z_Z____________________`, `+_Z_Z_Z____________________`, `+_ZZZ_Z____________________`, `+_Z_Z_Z____________________`, `+_Z_Z_Z_____Z______________`, and `+_Z_Z_Z____________________`.
The unrotated owned positive subcases are generated `surface_code:unrotated_memory_z` with distance 3 and rounds 3, default-like all-detector plus all-observable target count of 37, default-like all-tick selection from 0 through 20, and exact selected-region extraction for D0, D4, and L0 across Stab ticks 0 through 5.
The unrotated exact expectations come from pinned Stim v1.16.0 `Circuit.detecting_regions` and are recorded in [pfm5-detecting-regions-unrotated-surface-scope.md](pfm5-detecting-regions-unrotated-surface-scope.md).
The owned negative scope remains broad generated-code parity: full generated surface-code region tables, larger distances, other surface-code tasks, coordinate-prefix target filters, and generated-code gauge-specific behavior remain active work.
The benchmark row for this slice is a non-primary report-only Rust utility workload measuring the same selected generated rotated surface-code target and tick set through `circuit_detecting_regions_for_targets`; no separate benchmark row is added for the unrotated exact-output fixture.

## Clifford Gate Scope

The unsigned Clifford slice promotes the full single-qubit Clifford gate set with plain qubit targets plus fixed two-qubit tableau-backed Clifford gates with plain qubit-pair targets because the sparse reverse tracker now owns those unsigned transformations, and the target-shape slice separately promotes selected measurement-record feedback placements for the controlled-Pauli gates whose sparse reverse feedback semantics are source-owned.
The owned positive subcases are deterministic single-detector circuits whose expected tick-indexed regions were cross-checked against pinned Stim v1.16.0 `detslice-text` output and then encoded as Rust structural tests.
The source-owned reproduction path is to write each circuit from `detecting_regions_clifford_supports_single_qubit_clifford_gate_set`, `detecting_regions_clifford_supports_controlled_pauli_propagation`, `detecting_regions_clifford_supports_swap_gate`, `detecting_regions_clifford_supports_promoted_controlled_pauli_gate`, and `detecting_regions_target_shape_supports_measurement_record_feedback` to a temporary `.stim` file, run `target/oracle/stim-v1.16.0/out/stim diagram --type detslice-text --tick <stim_tick> < file.stim`, and compare Stim diagram tick `n + 1` to Stab detecting-region tick `n` after dropping the diagram sign because this Stab slice intentionally owns unsigned regions.
The full single-qubit Clifford test table covers `I`, `X`, `Y`, `Z`, `H`, `SQRT_Y_DAG`, `H_NXZ`, `SQRT_Y`, `S`, `H_XY`, `H_NXY`, `S_DAG`, `SQRT_X_DAG`, `SQRT_X`, `H_NYZ`, `H_YZ`, `C_XYZ`, `C_XYNZ`, `C_NXYZ`, `C_XNYZ`, `C_ZYX`, `C_ZNYX`, `C_NZYX`, and `C_ZYNX`.
The checked two-qubit unsigned expectations include the earlier `CZ` tick 0 `+ZZ` and tick 1 `+X_`, `CY` tick 0 `+XY` and tick 1 `+X_`, plus exact integration checks for `SWAP` as tick 0 `+_Z` and `XCX` as tick 0 `+ZX`.
The sparse reverse tracker has a tableau-backed all-basis regression for `II`, `XCX`, `XCY`, `XCZ`, `YCX`, `YCY`, `YCZ`, `SWAP`, `ISWAP`, `ISWAP_DAG`, `CXSWAP`, `SWAPCX`, `CZSWAP`, `SQRT_XX`, `SQRT_XX_DAG`, `SQRT_YY`, `SQRT_YY_DAG`, `SQRT_ZZ`, and `SQRT_ZZ_DAG`.
The owned negative subcases keep unsupported feedback positions, source-owned fail-closed non-`CZ` record-record, sweep/sweep, and record/sweep groups, broader generated-code regions beyond the promoted repetition-code and selected rotated and unrotated surface-code cases, and broader gauge-specific behavior fail-closed or partial until those surfaces are explicitly promoted.
The comparator class is structural Rust API parity against pinned Stim detecting-region semantics; the `detslice-text` command is only the pinned-Stim reproduction tool for the expected Pauli regions, and no diagram API parity is claimed.
The benchmark row for this slice is a non-primary report-only Rust utility workload measuring the promoted Clifford gates through `circuit_detecting_regions_for_targets`.
Resource behavior continues to use the existing detecting-region repeat and dense-helper caps.

## Anticommutation Scope

The ignored-anticommutation slice promotes the existing option field instead of adding a new public API.
The owned positive subcases are an in-circuit reset anticommutation and an implicit start-state anticommutation that both return the tick-indexed unsigned region when `ignore_anticommutation_errors=true`, plus empty-output filters under ignored mode.
The owned negative subcases keep the default false mode failing with an anticommutation error for in-circuit conflicts, implicit start-state conflicts, and empty-output filters.
The comparator class is structural Rust API parity against pinned Stim v1.16.0 `Circuit.detecting_regions` failure behavior plus the upstream C++ utility's explicit `ignore_anticommutation_errors` switch.
No separate benchmark row was added because the promoted mode reuses the same sparse reverse traversal and changes only the error policy.

## Selected Gauge Scope

The selected gauge slice promotes public detecting-region behavior that was already implied by the sparse reverse tracker but was not source-owned at the API level.
The owned positive subcase for ignored mode uses `RX 0; TICK; M 0; TICK; MX 0; DETECTOR rec[-1]` and proves the broken detector keeps `+X` sensitivity at both selected ticks when `ignore_anticommutation_errors=true`.
The owned negative subcase proves the same circuit still fails with an anticommutation error when `ignore_anticommutation_errors=false`.
The product-measurement cancellation subcase uses `RX 0 1; TICK; MZZ 0 1; TICK; MX 0 1; DETECTOR rec[-1] rec[-2]` and proves default false mode accepts the detector because the two anticommuting single-qubit sensitivities cancel as a product gauge.
The comparator class is structural Rust API parity against pinned Stim v1.16.0 `SparseUnsignedRevFrameTracker` gauge behavior plus the `Circuit.detecting_regions` ignore switch.
No separate benchmark row was added because the selected gauge behavior changes only sparse-tracker error-policy and gauge-cancellation branches inside the same detecting-region traversal.

## Tests

Implemented Rust tests:

- `detecting_regions_repeat_supports_bounded_ticks`
- `detecting_regions_repeat_rejects_excessive_expansion`
- `detecting_regions_target_api_matches_mx_python_example`
- `detecting_regions_target_api_supports_mzz_example`
- `detecting_regions_target_api_ignores_tags_and_ordinary_noise_like_upstream`
- `detecting_regions_target_shape_ignores_non_record_noise_instructions`
- `detecting_regions_target_shape_supports_inverted_measurement_targets`
- `detecting_regions_target_shape_supports_measurement_pads`
- `detecting_regions_target_shape_supports_pauli_product_measurements`
- `detecting_regions_target_shape_supports_spp_unitary_products`
- `detecting_regions_target_shape_rejects_anti_hermitian_pauli_products`
- `detecting_regions_target_shape_keeps_reset_and_unitaries_plain`
- `detecting_regions_target_shape_supports_heralded_record_noise`
- `detecting_regions_target_shape_keeps_heralded_noise_plain_qubit_scoped`
- `detecting_regions_target_shape_supports_measurement_record_feedback`
- `detecting_regions_target_shape_supports_sweep_controlled_pauli_noops`
- `detecting_regions_target_shape_rejects_unsupported_feedback_shapes`
- `detecting_regions_target_shape_rejects_unpromoted_sweep_shapes`
- `detecting_regions_target_shape_supports_cz_sweep_sweep_noop`
- `detecting_regions_target_shape_keeps_non_cz_sweep_sweep_fail_closed`
- `detecting_regions_target_shape_supports_cz_record_sweep_noop`
- `detecting_regions_target_shape_supports_cz_record_record_noop`
- `detecting_regions_target_shape_cz_classical_noop_skips_record_history`
- `detecting_regions_target_shape_keeps_non_cz_record_sweep_fail_closed`
- `detecting_regions_target_shape_keeps_non_cz_record_record_fail_closed`
- `detecting_regions_target_api_supports_logical_observable_targets`
- `detecting_regions_generated_repetition_code_filters_and_regions`
- `detecting_regions_generated_rotated_surface_code_filters_and_regions`
- `detecting_regions_generated_unrotated_surface_code_filters_and_regions`
- `detecting_regions_target_api_rejects_invalid_targets`
- `detecting_regions_target_api_rejects_dense_helper_expansion`
- `detecting_regions_clifford_supports_promoted_single_qubit_gates`
- `detecting_regions_clifford_supports_single_qubit_clifford_gate_set`
- `detecting_regions_clifford_supports_controlled_pauli_propagation`
- `detecting_regions_clifford_supports_swap_gate`
- `detecting_regions_clifford_supports_promoted_controlled_pauli_gate`
- `detecting_regions_anticommutation_supports_ignored_mode`
- `detecting_regions_anticommutation_rejects_false_mode`
- `detecting_regions_anticommutation_rejects_implicit_start_state`
- `detecting_regions_anticommutation_rejects_false_mode_with_empty_filters`
- `detecting_regions_gauge_ignores_measurement_collapse_when_requested`
- `detecting_regions_gauge_allows_product_measurement_cancellation`

These tests cover bounded repeat tick traversal, expected tick-indexed detecting regions, resource rejection for repeat expansion beyond the current cap, pinned `MX`, `MZZ`, and tagged ordinary-noise detecting-region examples, detector and logical-observable target filters, ordinary non-record-producing noise no-op traversal, inverted measurement target shapes, selected measurement-record feedback placements and unsupported feedback-shape rejection, selected gate-order-valid sweep-controlled Pauli sign-only no-op groups, selected `CZ` classical-only bit-bit no-op groups, source-owned fail-closed non-`CZ` sweep/sweep, record/sweep, and record/record target groups, invalid sweep/qubit target-order rejection, `MPAD` measurement pads with record-index preservation and empty pad-only regions, heralded record-producing noise with empty herald-only regions and preserved adjacent measurement sensitivity, `MPP` Pauli-product target shapes, `SPP` and `SPP_DAG` unitary Pauli-product target shapes compared to decomposed propagation, anti-Hermitian Pauli-product rejection, reset, Clifford, and heralded-noise plain-target validation, generated repetition-code all-target and all-tick selection plus selected exact regions, selected generated rotated and unrotated surface-code all-target and all-tick helper counts plus exact D0, D4, and L0 regions, default-like all-target and all-tick helpers, duplicate target deduplication, invalid target rejection, dense helper rejection before large allocation, promoted unsigned full single-qubit Clifford propagation, fixed two-qubit tableau-backed Clifford propagation, ignored anticommutation output, default false-mode anticommutation rejection, selected measurement-gauge ignored-mode output, and product-measurement gauge cancellation.
Additional focused regressions in `circuit_flows` and `circuit_feedback` prove the selected `CZ` classical-only no-op promotion remains detecting-region-specific and does not silently promote the same shapes for flow checking or feedback inlining.

## Oracle Rows

Implemented row:

- `pf5-detecting-regions-repeat-rust`
- `pf5-detecting-regions-targets-rust`
- `pf5-detecting-regions-target-shapes-rust`
- `pf5-detecting-regions-noise-tags-rust`
- `pf5-detecting-regions-clifford-rust`
- `pf5-detecting-regions-anticommutation-rust`
- `pf5-detecting-regions-gauge-rust`
- `pf5-detecting-regions-generated-repetition-rust`
- `pf5-detecting-regions-generated-surface-rust`
- `pf5-detecting-regions-generated-unrotated-surface-rust`

Still broad and manifest-only:

- `pf5-detecting-regions-extended`

## Benchmark Rows

Report-only runner coverage:

- `pf5-detecting-regions-repeat`
- `pf5-detecting-regions-targets`
- `pf5-detecting-regions-clifford`
- `pf5-detecting-regions-generated-repetition`
- `pf5-detecting-regions-generated-surface`

The repeat row measures the bounded repeat-tick detecting-region workload through the Rust public utility API.
The target row uses the default-like helper functions to set up detector, logical-observable, and tick filters, then times detecting-region extraction through the additive `DemTarget` API.
The Clifford row uses the default-like helper functions to set up filters for representative newly promoted single-qubit Clifford fixtures, the existing `CY` controlled-Pauli fixture, and a fixed two-qubit tableau-backed fixture covering `XCX`, `SWAP`, and `SQRT_XX`, then times extraction through the additive `DemTarget` API.
The generated repetition-code row uses the default-like helper functions to set up all detector and logical-observable targets plus all ticks for the distance-3 rounds-3 generated repetition-code circuit, then times extraction through the additive `DemTarget` API.
The generated surface-code benchmark row uses the selected D0, D4, and L0 targets plus the first six ticks for the distance-3 rounds-3 generated rotated memory-Z surface-code circuit, then times extraction through the additive `DemTarget` API.
These rows remain `non-primary-report-only` because pinned Stim does not provide a faithful CLI timing ratio for this Rust utility surface.
They are not part of the 1.25x primary threshold file.
The target row is coverage for the promoted helper path, not a claim that all-target/all-tick scaling is representative for large generated-code workloads.

## Verification Evidence

Completed target checks for this slice:

```sh
cargo fmt --all --check
cargo test -p stab-core detecting_regions_repeat_ --quiet
cargo test -p stab-core detecting_regions_target_api --quiet
cargo test -p stab-core detecting_regions_target_api_ignores_tags --quiet
cargo test -p stab-core detecting_regions_target --quiet
cargo test -p stab-core detecting_regions_target_shape --quiet
cargo test -p stab-core detecting_regions_target_shape_ignores_non_record_noise_instructions --quiet
cargo test -p stab-core detecting_regions_target_shape_supports_heralded_record_noise --quiet
cargo test -p stab-core detecting_regions_target_shape_keeps_heralded_noise_plain_qubit_scoped --quiet
cargo test -p stab-core detecting_regions_target_shape_supports_measurement_record_feedback --quiet
cargo test -p stab-core detecting_regions_target_shape_rejects_unsupported_feedback_shapes --quiet
cargo test -p stab-core --test detecting_regions_cz_sweep_sweep --quiet
cargo test -p stab-core detecting_regions_generated_repetition --quiet
cargo test -p stab-core detecting_regions_generated_rotated_surface_code --quiet
cargo test -p stab-core detecting_regions_clifford --quiet
cargo test -p stab-core detecting_regions_anticommutation --quiet
cargo test -p stab-core detecting_regions_anticommutation -- --list
cargo test -p stab-core detecting_regions_gauge --quiet
cargo test -p stab-bench pf5::detector_utility_benchmark_rows_have_stab_compare_runners --quiet
cargo test -p stab-bench --quiet
cargo test -p stab-oracle fixtures --quiet
cargo clippy -p stab-core -p stab-oracle --all-targets -- -D warnings
cargo clippy -p stab-core -p stab-bench -p stab-oracle --all-targets -- -D warnings
cargo clippy --workspace --all-targets -- -D warnings
cargo test --workspace --quiet
just oracle::version
just oracle::run --milestone PF5 --structural
just bench::smoke
just bench::baseline --only pf5-detecting-regions-targets --out target/benchmarks/rpf5-detecting-region-targets-probe
just bench::compare --only pf5-detecting-regions-targets --baseline target/benchmarks/rpf5-detecting-region-targets-probe/baseline.json --report target/benchmarks/rpf5-detecting-region-targets-compare
just bench::baseline --only pf5-detecting-regions-clifford --out target/benchmarks/rpf5-detecting-region-clifford-fixed-two-qubit-probe
just bench::compare --only pf5-detecting-regions-clifford --baseline target/benchmarks/rpf5-detecting-region-clifford-fixed-two-qubit-probe/baseline.json --report target/benchmarks/rpf5-detecting-region-clifford-fixed-two-qubit-compare
just bench::baseline --only pf5-detecting-regions-generated-repetition --out target/benchmarks/rpf5-detecting-region-generated-repetition-probe
just bench::compare --only pf5-detecting-regions-generated-repetition --baseline target/benchmarks/rpf5-detecting-region-generated-repetition-probe/baseline.json --report target/benchmarks/rpf5-detecting-region-generated-repetition-compare
just bench::baseline --only pf5-detecting-regions-generated-surface --out target/benchmarks/rpf5-detecting-region-generated-surface-probe
just bench::compare --only pf5-detecting-regions-generated-surface --baseline target/benchmarks/rpf5-detecting-region-generated-surface-probe/baseline.json --report target/benchmarks/rpf5-detecting-region-generated-surface-compare
# pinned Stim detslice-text reproduction loop for the full single-qubit Clifford table plus representative fixed two-qubit Clifford circuits
# pinned Stim detslice-text reproduction loop for generated rotated surface-code D0 D4 and L0 across Stim ticks 1 through 6
cargo test -p stab-core detecting_regions_generated_unrotated_surface_code_filters_and_regions --quiet
```

The pinned-Stim `detslice-text` reproduction passed for all 24 single-qubit Clifford table entries with tick `1` matching the expected prepared basis and tick `2` matching `X` after dropping sign; the same reproduction passed for `CY` as `XY` then `X_`, `CZ` as `ZZ` then `X_`, and the promoted `SWAP` and `XCX` integration expectations.
The pinned-Stim `detslice-text` reproduction also passed for the generated repetition-code selected-region expectations recorded above, the generated rotated surface-code reproduction passed for the selected D0, D4, and L0 targets across Stim ticks 1 through 6, and the unrotated memory-Z exact regions were reproduced from pinned Stim v1.16.0 `Circuit.detecting_regions`.
The target-filter benchmark probe reported `stab_pf5_detecting_regions_target_filters=0.006348216s` and `6.452e5 cases/s`, with output written to `target/benchmarks/rpf5-detecting-region-targets-compare`.
The fixed-two-qubit-inclusive benchmark probe reported `stab_pf5_detecting_regions_clifford_gates=0.041061913s` and `2.993e5 cases/s`, with output written to `target/benchmarks/rpf5-detecting-region-clifford-fixed-two-qubit-compare`.
The generated repetition-code benchmark probe reported `stab_pf5_detecting_regions_generated_repetition=0.037847554s` and `1.082e5 cases/s`, with output written to `target/benchmarks/rpf5-detecting-region-generated-repetition-compare`.
The generated rotated surface-code benchmark probe reported `stab_pf5_detecting_regions_generated_surface=0.074829076s` and `5.474e4 cases/s`, with output written to `target/benchmarks/rpf5-detecting-region-generated-surface-compare`.
These rows remain report-only with the documented note that this Rust utility workload has no faithful pinned Stim CLI timing ratio.

## Audit And Review

Milestone audit status is complete for the target-filter, generated repetition-code, generated rotated surface-code, generated unrotated surface-code, unsigned Clifford, and ignored-anticommutation slices and incomplete for broader RPF5.
Full-code-review sidecars found one P1 issue in the dense all-target helper, where huge observable ids or detector counts could cause excessive allocation before failure.
The slice now rejects all-target helper requests beyond the dense materialization cap or representable logical-observable target range before allocation, with `detecting_regions_target_api_rejects_dense_helper_expansion` covering the regression.
The unsigned Clifford audit found a P2 evidence-provenance gap because the initial report did not preserve the pinned-Stim `detslice-text` reproduction path for the promoted-gate expectations; this report now records the exact command shape and source-owned expected region strings.
The full-code-review sidecar found no implementation findings for the earlier unsigned Clifford slice and confirmed the promoted-gate tests and fail-closed regression coverage.
The current Clifford refresh review found P2 documentation and evidence overclaims around future target-shape scope, representative benchmark wording, and repeat-folding coverage; the plan wording now says broader target shapes, the benchmark row is documented as representative, and `unitary_repeat_folding_matches_naive_all_single_qubit_cliffords` covers the full single-qubit Clifford repeat-folding table.
The ignored-anticommutation refresh review found a P2 false-mode compatibility gap where empty target or tick filters returned before anticommutation validation; the early return was removed and `detecting_regions_anticommutation_rejects_false_mode_with_empty_filters` covers the regression.
The same review pass found stale or overly broad evidence wording in the PF5 oracle manifest and historical remaining-partials plan; the manifest now narrows the remaining detecting-region placeholder, the anticommutation row uses the tight `detecting_regions_anticommutation` filter, and the historical plan lists the repeat, target, Clifford, and anticommutation rows.
The selected gauge refresh added public API evidence for measurement-gauge ignored mode and product-measurement gauge cancellation, plus oracle metadata that names the pinned Stim sparse-tracker source of the behavior.
The inverted-target-shape refresh split the near-limit test module out of the implementation file, then added source-owned public API evidence for inverted measurement targets and explicit negative coverage that reset and Clifford target-shape validation remains plain-qubit-only.
The Pauli-product target-shape refresh promoted `MPP` detecting-region validation through the existing sparse reverse tracker product-measurement undo path, with detector and observable output evidence plus anti-Hermitian rejection coverage.
The Pauli-product target-shape audit and GPT-5.5/xhigh full-code-review sidecars found no implementation, evidence, or documentation findings; the residual risk is that the `MPP` detecting-region evidence is structural Rust API evidence, not an exact-output CLI oracle row, which matches the declared comparator class.
The unitary Pauli-product target-shape refresh promoted `SPP` and `SPP_DAG` detecting-region validation through the existing sparse reverse tracker unitary-product undo path, with decomposed-propagation equivalence, fixed unsigned-region expectations, and anti-Hermitian rejection coverage.
The unitary Pauli-product target-shape audit and GPT-5.5/xhigh full-code-review sidecars found no implementation blockers; the documentation stale-wording finding was fixed in this report and in the oracle manifest, and the residual risk is that the `SPP`/`SPP_DAG` detecting-region evidence remains structural Rust API evidence instead of an exact-output CLI oracle row, matching the declared comparator class.
The heralded-noise target-shape refresh promoted `HERALDED_ERASE` and `HERALDED_PAULI_CHANNEL_1` for plain qubit targets, preserving Stim-compatible empty herald-only regions and adjacent measurement sensitivity while keeping them outside the ordinary-noise no-op slice.
The measurement-record feedback target-shape refresh promoted `CX rec[-1] q`, `CY rec[-1] q`, `CZ rec[-1] q`, `CZ q rec[-1]`, `XCZ q rec[-1]`, and `YCZ q rec[-1]` detecting-region validation through the existing sparse reverse tracker classical-feedback path, with pinned detslice-text expectations for the selected unsigned regions and explicit rejection coverage for unsupported feedback positions and then-unpromoted record-record groups.
The measurement-record feedback target-shape audit and GPT-5.5/xhigh full-code-review sidecar found no Rust implementation finding; the sidecar found one P3 stale remaining-work wording issue around selected feedback placements, and this report now includes selected feedback placements in the promoted and remaining-scope lists.
The selected sweep-controlled Pauli target-shape refresh promoted gate-order-valid `CX`, `CY`, `CZ`, `XCZ`, and `YCZ` groups with exactly one sweep bit and one plain qubit target as unsigned sign-only no-ops, using pinned Stim v1.16.0 `Circuit.detecting_regions` probes for representative outputs and explicit rejection coverage for then-unpromoted sweep/sweep groups, record/sweep groups, and invalid sweep/qubit target orders.
The selected `CZ` sweep/sweep follow-up promotes `CZ sweep[i] sweep[j]` bit-bit groups as detecting-region no-ops through the existing sparse-tracker sweep-bit branch, adds focused positive and negative target-shape tests, and keeps non-`CZ` sweep/sweep plus record/sweep shapes fail-closed.
The selected `CZ` sweep/sweep milestone-audit pass found the slice complete against its scope note after the oracle manifest and future-work wording were updated to keep the promoted `CZ` bit-bit case out of remaining-work lists.
The GPT-5.5/xhigh full-code-review sidecars found no Rust implementation or test findings; the docs sidecar found a P2 stale underclaim where the promoted `CZ` bit-bit case still appeared in future-work wording, and this report, the active plan, checklist, roadmap, inventory, spec-gap log, and oracle manifest distinguish implemented selected `CZ` sweep/sweep from then-unpromoted record/sweep shapes.
The selected `CZ` classical-only no-op follow-up promotes `CZ rec[-k] sweep[j]`, `CZ sweep[j] rec[-k]`, and `CZ rec[-a] rec[-b]` bit-bit groups as detecting-region no-ops, adds focused positive evidence including the no-record-history detector-slice probe accepted by pinned Stim, and keeps non-`CZ` record/sweep plus record/record shapes fail-closed.
The selected `CZ` classical-only no-op audit found the implementation complete against `pfm5-detecting-regions-cz-classical-noop-scope.md` after tightening the implementation to skip these groups only in detecting-region traversal instead of the shared sparse reverse tracker.
The PFM5 target-shape evidence lock [pfm5-detecting-regions-target-shape-evidence-lock.md](pfm5-detecting-regions-target-shape-evidence-lock.md) reconciles stale remaining-work wording: non-`CZ` sweep/sweep, record/sweep, and record/record groups are source-owned fail-closed evidence under `pf5-detecting-regions-target-shapes-rust`, not active unclassified target-shape work.
The GPT-5.5/xhigh full-code-review sidecars found no blocking Rust or documentation findings; the Rust sidecar called out the shared-tracker inheritance as residual risk, and the final implementation now keeps flow checking and feedback inlining fail-closed for these `CZ` classical-only groups with targeted regressions.
The ordinary-noise and tag milestone-audit pass found the slice complete against current PF5 text, with broader detecting-region target shapes, generated-code cases, and gauge behavior still active.
The GPT-5.5/xhigh full-code-review sidecar found no implementation, evidence, benchmark, or documentation findings for the ordinary-noise and tag slice; the residual risk is that this remains structural Rust API evidence for a core utility, not an exact-output CLI oracle row, matching the declared comparator class.
The generated repetition-code audit found an evidence-provenance gap where the oracle row claimed `MPAD` rejection but only ran the positive generated-region test; the row was narrowed to generated-region evidence before this slice later promoted `MPAD` under the target-shape row, and this report records the exact D0, D6, and L0 detslice translations.
The generated rotated surface-code refresh adds exact D0, D4, and L0 selected-region evidence plus report-only benchmark metadata while leaving broad surface-code region-table parity active.
The generated rotated surface-code full-code-review sidecars found no Rust implementation or benchmark-runner defects and one P2 documentation provenance gap where the PF5 source inventories omitted `src/stim/gen/gen_surface_code.test.cc`; the source inventories now list that path beside the detecting-region utility sources.
The generated unrotated surface-code refresh adds exact D0, D4, and L0 selected-region evidence for the distance-3 rounds-3 `surface_code:unrotated_memory_z` Rust generator surface while leaving broad surface-code region-table parity active and without adding a separate performance row.
The generated unrotated surface-code milestone-audit and GPT-5.5/xhigh full-code-review sidecars found no Rust implementation, oracle-metadata, or benchmark-alignment blockers; their P2 documentation and maintainability findings were fixed by recording the pinned Stim reproduction table, synchronizing the active plan oracle and benchmark wording, and splitting generated detecting-region tests out of the over-threshold parent test module.
The generated repetition-code full-code-review sidecars found no implementation or benchmark-runner defects after the validation set was narrowed to `QUBIT_COORDS` and `SHIFT_COORDS` instead of every annotation gate.
The `MPAD` target-shape refresh promoted measurement pads through the existing sparse reverse tracker record-drop path, then fixed the GPT-5.5/xhigh full-code-review finding that `circuit_detecting_regions` must use Stim's stats-style qubit count instead of public `Circuit::count_qubits` so `MPAD 1` does not widen reported detecting-region Pauli strings.
The same review pass found stale benchmark and provenance wording around target-shape evidence; this report and the oracle manifest now state that the feedback, `MPAD`, heralded-record-drop, `MPP`, and `SPP`/`SPP_DAG` target-shape branches have structural Rust API evidence only, with no direct report-only benchmark row for those branches.
The remaining review risk is that the report-only benchmark rows exercise promoted helper paths on small fixtures and should not be used as representative scaling evidence for large generated-code workloads.

## Remaining RPF5 Work

- Broader detecting-region target shapes outside the promoted positive set and source-owned fail-closed set remain active, especially unsupported feedback placements and any shapes not covered by measurement inversions, selected feedback placements, selected gate-order-valid sweep-controlled Pauli sign-only no-op groups, selected `CZ` classical-only bit-bit groups, explicit non-`CZ` classical-bit rejection, `MPAD`, heralded record-producing noise, `MPP`, and `SPP`/`SPP_DAG` Pauli products; broader generated-code regions beyond the promoted repetition-code and selected rotated and unrotated surface-code cases and broader gauge behavior also remain active.
- Missing-detector generated-code suffix analysis beyond the promoted honeycomb and toric cases, plus broader flow-dependent utility behavior.
- Measurement-rich flows beyond the promoted unsigned `has_flow`, `has_all_flows`, unsigned diagnostic Rust helper, and scoped signed sampled Rust checker subset, including broader `flow_generators`, solver or generator diagnostics, signed sampled diagnostics or Python binding shape, and transform integration.
