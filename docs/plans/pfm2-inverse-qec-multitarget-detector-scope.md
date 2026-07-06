# PFM2 Inverse-QEC Multi-Target Detector Scope

## Summary

This slice expands the selected `Circuit::inverse_qec` reset-measure-detector packet from the pinned single-target `r_m_det` case to the full plain top-level reset, matching measurement, and detector record-parity shape for one detector.
It is still a narrow PFM2 circuit-transform packet, not a full port of Stim's reverse detector-flow algorithm.

## Owned Subcases

- Support a top-level three-instruction circuit shaped as `R`, `RX`, or `RY`; then matching `M`, `MX`, or `MY`; then one `DETECTOR`.
- Require reset and measurement instructions to be noiseless, plain-qubit, duplicate-free, and same-basis.
- Require reset and measurement target lists to match exactly.
- Accept detector targets that reference records produced by the selected measurement instruction.
- Toggle duplicate detector record targets by parity, dropping the output detector when its record set cancels to empty.
- Accept empty detector target lists and emit the matching target-list reset or measurement reversal with no output detector.
- Preserve detector coordinates and tags when the detector survives parity simplification.
- Preserve the existing unitary inverse behavior for all-unitary circuits.

## Explicit Rejections And Deferrals

- Reject detector record targets outside the selected measurement group.
- Keep `TICK`, `QUBIT_COORDS`, `SHIFT_COORDS`, observables, feedback, noise, `MR`, `MXX`, `MYY`, `MZZ`, `MPP`, multi-detector circuits, interleaved unitary detector-flow rewrites such as `two_to_one`, and repeat-contained inverse-QEC behavior out of scope unless already accepted by the unitary inverse path.
- Keep duplicate reset or measurement targets rejected through the selected-shape parser.
- Keep `dont_turn_measurements_into_resets`, Python API shape, and flow-returning overloads out of scope.

## Comparator And Evidence

The comparator class is exact structural parity against pinned Stim v1.16.0 `src/stim/util_top/circuit_inverse_qec.test.cc` and `Circuit.time_reversed_for_flows([])` probes for selected no-flow reset-measure-detector cases.
The selected output is the canonical Stab circuit text for the same accepted shape.

## Oracle And Benchmark Policy

- Oracle row: refresh the executable structural row for the selected Rust `inverse_qec` reset-measure-detector subset.
- Benchmark rows: no new row is required because this is a narrow public transform selector, not a new performance-sensitive traversal or high-volume workload.
- Existing `pf2-time-reverse-flow` benchmark rows stay unchanged because this no-flow `inverse_qec` selector does not affect their measured corpus.

## Verification

Targeted verification for this slice:

```sh
cargo test -p stab-core --test circuit_inverse_qec inverse_qec --quiet
cargo test -p stab-core --test circuit_inverse_qec --quiet
cargo test -p stab-oracle fixtures --quiet
just oracle::run --milestone PF2 --structural
just bench::smoke
```

Broader pre-commit verification follows `docs/plans/GOAL.md`.
