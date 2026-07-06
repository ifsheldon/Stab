# PFM2 Inverse-QEC Reset-Measure-Detector Scope

## Summary

This initial slice promoted one selected `Circuit::inverse_qec` measurement-rich transform case from Stim v1.16.0: a single-target plain reset, a matching single-target plain measurement, and one detector target `rec[-1]`.
It has since been expanded by [pfm2-inverse-qec-multitarget-detector-scope.md](pfm2-inverse-qec-multitarget-detector-scope.md), which owns the current multi-target record-parity behavior for the same plain three-instruction shape.
It remains a narrow PFM2 circuit-transform packet, not a full port of Stim's reverse detector-flow algorithm.

## Owned Subcases

- Support the initial top-level three-instruction circuit shaped as single-target `R`, `RX`, or `RY`; then matching single-target `M`, `MX`, or `MY`; then `DETECTOR rec[-1]`.
- Require the reset and measurement instructions to be noiseless, plain-qubit, and same-basis.
- Require the reset and measurement target lists to match exactly.
- Require the detector to have exactly one target, `rec[-1]`.
- Preserve detector coordinates and instruction tags for the accepted shape by returning the canonical selected circuit unchanged.
- Preserve the existing unitary inverse behavior for all-unitary circuits.

## Explicit Rejections And Deferrals

- Keep `dont_turn_measurements_into_resets` behavior, Python API shape, and flow-returning overloads out of scope.
- Keep `TICK`, `QUBIT_COORDS`, `SHIFT_COORDS`, observables, feedback, noise, `MR`, `MXX`, `MYY`, `MZZ`, `MPP`, multi-instruction detector-flow rewrites, and repeat-contained inverse-QEC behavior rejected unless already accepted by the unitary inverse path.
- Keep duplicate reset or measurement targets rejected through the selected-shape parser.
- Keep broader Stim `circuit_inverse_qec` cases such as `two_to_one`, `pass_through`, broader noisy measure-reset detector-flow beyond the exact `noisy_mr_det` packet, broader `m_det` beyond the selected exact packet, `mzz`, `mpp`, and `obs_include_pauli` active for later PFM2 or PFM5 slices.

## Comparator And Evidence

The comparator class is exact structural parity against pinned Stim v1.16.0 `src/stim/util_top/circuit_inverse_qec.test.cc` for the single-target `r_m_det` no-flow case.
The selected output is the canonical Stab circuit text for the same accepted single-target shape.

## Oracle And Benchmark Policy

- Oracle row: the initial executable structural row covered the selected Rust `inverse_qec` single-target reset-measure-detector subset; the current row is refreshed by the multi-target scope document.
- Benchmark rows: no new row is required because this adds a narrow public transform selector, not a new performance-sensitive traversal or high-volume workload.
- Existing `pf2-time-reverse-flow` benchmark rows stay unchanged because this no-flow `inverse_qec` selector does not affect their measured corpus.

## Verification

Targeted verification for this slice:

```sh
cargo test -p stab-core --test circuit_inverse_qec inverse_qec --quiet
cargo test -p stab-oracle fixtures --quiet
just oracle::run --milestone PF2 --structural
just bench::smoke
```

Broader pre-commit verification follows `docs/plans/GOAL.md`.
