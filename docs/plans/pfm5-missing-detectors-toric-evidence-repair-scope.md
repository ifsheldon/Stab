# PFM5 Missing-Detectors Toric Evidence Repair Scope

## Summary

This PFM5 slice tightens source-owned evidence for the promoted toric generated-code missing-detector suffix case.
The oracle manifest and progress report already name `pf5-missing-detectors-generated-toric-rust` as implemented, but the manifest used a package-wide Cargo filter that also matched an internal unit test with the same name.
This slice adds the integration-test mirror beside the other PF5 missing-detectors rows and narrows the oracle command to that integration test.

This slice does not expand the `missing_detectors` algorithm, broaden generated-code suffix analysis, change public APIs, or claim full utility parity.

## Owned Surface

- Public Rust API: `stab_core::missing_detectors`.
- Checklist row: detector-analysis utility APIs.
- Active plan row: PFM5 generated-code missing-detector suffix evidence.
- Comparator class: structural Rust API parity against pinned Stim v1.16.0 expected suffix text.
- Oracle row: `pf5-missing-detectors-generated-toric-rust`.
- Benchmark row: no new row; the existing report-only `pf5-missing-detectors-generated-code` row already covers the promoted honeycomb and toric generated-code workload family.

## Selected Positive Scope

Stab should reproduce the pinned Stim v1.16.0 `missing_detectors.toric_code_global_stabilizer_product` suffix under unknown-input semantics.
The selected circuit measures four toric-code X stabilizer products, then four more X stabilizer products, then two rounds of Z stabilizer products, declares eight final detector rows, and expects one missing detector row covering the earlier eight measurement records:

```stim
DETECTOR rec[-16] rec[-15] rec[-14] rec[-13] rec[-12] rec[-11] rec[-10] rec[-9]
```

## Explicit Rejections And Non-Goals

- Broader toric-code generated suffix extraction remains active future work.
- Broader generated-code families beyond the promoted honeycomb and this pinned toric global-stabilizer case remain active future work.
- Folded large-repeat traversal is not expanded by this evidence repair.
- Python binding behavior, diagram behavior, exact random-stream parity, and full utility parity remain out of scope.

## Tests

- Add `missing_detectors_supports_toric_global_stabilizer_product` to `crates/stab-core/tests/missing_detectors.rs`.
- Narrow `pf5-missing-detectors-generated-toric-rust` to `cargo-test|-p|stab-core|--test|missing_detectors|missing_detectors_supports_toric_global_stabilizer_product`.
- Run the focused named test to prove the integration-test mirror has real test coverage.
- Run the PF5 structural oracle to prove the oracle row no longer relies on ambiguous package-wide filtering.

## Done Criteria

- The named test exists and checks the exact expected suffix from pinned Stim v1.16.0.
- The existing oracle row `pf5-missing-detectors-generated-toric-rust` points uniquely to the integration test used by the PF5 progress report.
- Documentation distinguishes this evidence repair from broader generated-code suffix closure.
- Milestone-audit and full-code-review find no remaining P0, P1, or P2 evidence mismatch for this slice.
