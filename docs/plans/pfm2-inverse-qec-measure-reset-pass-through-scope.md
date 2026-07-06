# PFM2 Inverse-QEC Measure-Reset Pass-Through Scope

## Summary

This slice expands the selected `Circuit::inverse_qec` detector-flow coverage from the no-flow reset-measure-detector packet to the pinned Stim v1.16.0 `pass_through` shape: a reset group, a matching measurement group, a matching measure-reset group, and one detector whose targets reference only the measure-reset records.
It is still a narrow PFM2 circuit-transform packet, not a full port of Stim's reverse detector-flow algorithm.

## Owned Subcases

- Support a top-level four-instruction circuit shaped as `R`, `RX`, or `RY`; then matching `M`, `MX`, or `MY`; then matching `MR`, `MRX`, or `MRY`; then one `DETECTOR`.
- Require reset, measurement, and measure-reset instructions to be noiseless, plain-qubit, duplicate-free, non-empty, and same-basis.
- Require reset, measurement, and measure-reset target lists to match exactly.
- Accept detector targets that reference records produced by the selected measure-reset instruction.
- Toggle duplicate detector record targets by parity, dropping the output detector when its record set cancels to empty.
- Preserve reset, measurement, measure-reset, and detector tags in the reversed output, and preserve detector coordinates when the detector survives parity simplification.
- Preserve the existing unitary inverse behavior and the selected no-flow reset-measure-detector packet behavior.

## Explicit Rejections And Deferrals

- Reject detector record targets outside the selected measure-reset group, including detector targets that reference the earlier matching measurement group.
- Reject noisy measure-reset instructions, inverted measure-reset targets, duplicate reset, measurement, or measure-reset targets, empty target lists, nonmatching bases, and nonmatching target lists.
- Keep `TICK`, `QUBIT_COORDS`, `SHIFT_COORDS`, observables, feedback, noise beyond this explicit rejection, `MXX`, `MYY`, `MZZ`, `MPP`, multi-detector circuits, interleaved unitary detector-flow rewrites such as `two_to_one`, and repeat-contained inverse-QEC behavior out of scope unless already accepted by the unitary inverse path.
- Keep `dont_turn_measurements_into_resets`, Python API shape, and flow-returning overloads out of scope.

## Comparator And Evidence

The comparator class is exact structural parity against pinned Stim v1.16.0 `src/stim/util_top/circuit_inverse_qec.test.cc` and direct `Circuit.time_reversed_for_flows([])` probes for selected pass-through cases.
The selected output is canonical Stab circuit text for the same accepted shape.

## Oracle And Benchmark Policy

- Oracle row: add an executable structural row for the selected Rust `inverse_qec` measure-reset pass-through subset.
- Benchmark rows: no new row is required because this is a narrow public transform selector, not a new high-volume traversal or throughput path.
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
