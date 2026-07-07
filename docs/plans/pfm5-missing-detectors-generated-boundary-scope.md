# PFM5 Missing-Detectors Generated Boundary Scope

## Summary

This scope note locks the current PFM5 generated-code `missing_detectors` suffix boundary.
The selected implementation covers the pinned Stim v1.16.0 honeycomb-code suffix case plus the pinned toric global-stabilizer suffix case.
It does not claim broader generated-code missing-detector suffix parity.

## Selected Surface

- Public Rust API: `stab_core::missing_detectors`.
- Checklist row: detector-analysis utility APIs.
- Active plan row: PFM5 generated-code missing-detector suffix evidence.
- Input family: selected generated-code circuits from pinned Stim v1.16.0 `src/stim/util_top/missing_detectors.test.cc`.
- Comparator: exact canonical `.stim` suffix text under unknown-input semantics.

## Selected Positive Cases

- Pinned upstream `missing_detectors.big_case_honeycomb_code`, promoted by `pf5-missing-detectors-generated-honeycomb-rust`.
- Pinned upstream `missing_detectors.toric_code_global_stabilizer_product`, promoted by `pf5-missing-detectors-generated-toric-rust`.

## Selected Negative And Resource Cases

- No additional generated-code family is selected by this boundary note.
- Broader generated-code suffix analysis must wait for a future exact-subcase plan naming circuits, expected suffix behavior, known-input mode, comparator class, resource behavior, oracle metadata, and benchmark policy.
- Broader folded large-repeat traversal remains governed by the selected folded final-repeat scope notes and the existing caps for unselected repeat bodies.

## Evidence

- `missing_detectors_supports_honeycomb_generated_code_suffix` covers the pinned honeycomb suffix.
- `missing_detectors_supports_toric_global_stabilizer_product` covers the pinned toric suffix.
- Oracle row `pf5-missing-detectors-generated-honeycomb-rust` selects the honeycomb integration test.
- Oracle row `pf5-missing-detectors-generated-toric-rust` selects the toric integration test.
- Benchmark row `pf5-missing-detectors-generated-code` is report-only and measures only the promoted honeycomb and toric generated-code workloads.

## Explicit Non-Goals

- This slice does not select additional honeycomb variants, additional toric variants, other generated circuit families, generated-code suffix mining, Python API behavior, diagram behavior, or full detector-utility parity.
- This slice does not promote the broad `pf5-missing-detectors-extended` manifest-only row.
- This slice does not add a primary benchmark gate or a new benchmark row.

## Verification Commands

- `cargo test -p stab-core --test missing_detectors missing_detectors_supports_honeycomb_generated_code_suffix --quiet`
- `cargo test -p stab-core --test missing_detectors missing_detectors_supports_toric_global_stabilizer_product --quiet`
- `cargo test -p stab-oracle fixtures --quiet`
- `just oracle::run --milestone PF5 --structural`
- `just bench::smoke`
