# PFM5 Missing-Detectors MPP And Observable Evidence Scope

## Summary

This PFM5 slice tightens source-owned oracle evidence for the promoted `missing_detectors` MPP and observable-row subset.
The existing `pf5-missing-detectors-mpp-observable-rust` manifest row uses the broad `missing_detectors_supports_` Cargo filter, which also matches generated-code suffix tests and makes the row's exact ownership unclear.
This slice adds a focused integration test covering only the promoted MPP stabilizer-product and observable-interaction cases, then narrows the oracle row to that test.

This slice does not change `missing_detectors` behavior, broaden generated-code suffix analysis, alter public APIs, or claim full detector-utility parity.

## Owned Surface

- Public Rust API: `stab_core::missing_detectors`.
- Checklist row: detector-analysis utility APIs.
- Active plan row: PFM5 MPP, pair-measurement, observable-row, and row-reduction evidence for `missing_detectors`.
- Comparator class: structural Rust API parity against pinned Stim v1.16.0 expected suffix text.
- Oracle row: `pf5-missing-detectors-mpp-observable-rust`.
- Benchmark row: no new row; the existing report-only `pf5-missing-detectors-mpp` row already covers the promoted MPP and observable-row workload family.

## Selected Positive Scope

Stab should reproduce the pinned Stim v1.16.0 missing-detector suffixes for these promoted cases:

- Repeated `MPP Z0*Z1 X0*X1` stabilizer products with detector rows that leave one missing stabilizer-product detector suggestion.
- The same repeated MPP shape with a combined detector row that changes the reduced suffix.
- Unknown-input semantics where nondeterministic repeated MPP rows are ignored.
- Record-only observable rows that participate as known rows.
- Pauli-target observable rows that mark the observable row ignored and produce the pinned detector suggestion.

## Explicit Rejections And Non-Goals

- Broader generated-code suffix extraction remains active future work.
- Broader folded repeat traversal is not expanded by this evidence slice.
- Full generator-table measurement solving, measurement-rich flow integration, Python binding behavior, and full utility parity remain out of scope.

## Tests

- Add `missing_detectors_supports_mpp_observable_subset` to `crates/stab-core/tests/missing_detectors.rs`.
- Narrow `pf5-missing-detectors-mpp-observable-rust` to `cargo-test|-p|stab-core|--test|missing_detectors|missing_detectors_supports_mpp_observable_subset`.
- Run the focused integration test.
- Run the PF5 structural oracle to prove the row no longer relies on broad prefix filtering.

## Done Criteria

- The integration test checks every selected MPP and observable interaction listed above.
- The existing oracle row `pf5-missing-detectors-mpp-observable-rust` points uniquely to the integration test used by the PF5 progress report.
- Documentation distinguishes this evidence-hardening slice from generated-code suffix closure and broader flow-dependent missing-detector work.
- Milestone-audit and full-code-review find no remaining P0, P1, or P2 evidence mismatch for this slice.
