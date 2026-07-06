# PFM2 Inverse-QEC Noisy Measure-Reset Detector Scope

## Summary

This slice expands selected `Circuit::inverse_qec` coverage to the pinned Stim v1.16.0 `noisy_mr_det` shape: three same-basis top-level noisy measure-reset instructions on the same single qubit, a `TICK` between the first and second measure-reset, and one detector referencing the final measure-reset record.
It is a narrow detector-flow packet for the upstream `noisy_mr_det` case, not a general noisy detector-flow algorithm.

## Owned Subcases

- Support exactly this top-level instruction shape: noisy `MR`/`MRX`/`MRY`, `TICK`, matching noisy `MR`/`MRX`/`MRY`, matching noisy `MR`/`MRX`/`MRY`, then `DETECTOR rec[-1]`.
- Require the three measure-reset instructions to use the same basis, the same single plain qubit target, and exactly one probability argument each.
- Reverse the two post-`TICK` measure-resets first, lowering their probabilities to `X_ERROR` for `MR` and `Z_ERROR` for `MRX` or `MRY`.
- Move the detector between the second reversed post-`TICK` measure-reset and the preserved `TICK`, matching pinned Stim `noisy_mr_det` output.
- Preserve instruction tags on measure-reset, emitted Pauli-error, detector, and tick instructions, and preserve detector coordinate arguments.
- Reverse the pre-`TICK` measure-reset after the preserved `TICK`, lowering its probability to the matching Pauli-error instruction.
- Preserve existing selected QEC inverse packets for noisy measure-reset-only, noisy measurement-only, MPP detector-flow, exact two-to-one, measure-reset pass-through, reset-measure-detector, and unitary shapes.

## Explicit Rejections And Deferrals

- Reject detector targets other than exactly `rec[-1]`.
- Reject mixed measure-reset bases, mismatched targets, multi-target measure-resets, inverted noisy measure-reset targets, duplicate detector records, empty detectors, missing ticks, extra ticks, extra operations, observables, feedback, repeats, coordinates or shifts outside the detector declaration, and interleaved Clifford operations.
- Keep broader noisy measure-reset detector-flow rewrites, prior-record detector refs such as `rec[-2]`, multi-target detector-flow behavior, different-target region carry-through, and multi-instruction QEC inverse algorithms active for later PFM2 slices.
- Keep `dont_turn_measurements_into_resets`, Python API shape, and flow-returning overloads out of scope.

## Comparator And Evidence

The comparator class is exact structural parity against pinned Stim v1.16.0 `src/stim/util_top/circuit_inverse_qec.test.cc` for the untagged `noisy_mr_det` case.
Tag preservation, `MRX` and `MRY` basis variants, detector coordinate preservation, and explicit fail-closed nearby shapes are source-owned regression coverage informed by local pinned-Stim probes, not oracle-managed exact-output fixtures.

```text
Input:
MR(0.125) 0
TICK
MR(0.25) 0
MR(0.375) 0
DETECTOR rec[-1]

Expected:
MR 0
X_ERROR(0.375) 0
MR 0
X_ERROR(0.25) 0
DETECTOR rec[-1]
TICK
MR 0
X_ERROR(0.125) 0
```

```text
Additional pinned probes:
MRX(0.125) 0; TICK; MRX(0.25) 0; MRX(0.375) 0; DETECTOR rec[-1] -> Z_ERROR lowering with detector before TICK
MR[m](0.125) 0; TICK; MR[n](0.25) 0; MR[o](0.375) 0; DETECTOR[d](2, 3) rec[-1] -> matching tag and detector-coordinate preservation
MR(0.125) 0; TICK; MRX(0.25) 0; MR(0.375) 0; DETECTOR rec[-1] -> invalid detector-flow anticommutation in pinned Stim
MR(0.125) 0; TICK; MR(0.25) 0; MR(0.375) 0; DETECTOR rec[-3] -> detecting region reaches circuit start in pinned Stim
```

## Oracle And Benchmark Policy

- Oracle row: add an executable structural row for the selected Rust `inverse_qec` noisy measure-reset detector-flow subset.
- Benchmark rows: no new row is required because this is a narrow transform selector, not a high-volume traversal or throughput path.
- Existing PF2 transform benchmark rows stay unchanged.

## Verification

Targeted verification for this slice:

```sh
cargo test -p stab-core --test circuit_inverse_qec_noisy_measure_reset_detector --quiet
cargo test -p stab-core --test circuit_inverse_qec_noisy_measure_reset --quiet
cargo test -p stab-core --test circuit_inverse_qec --quiet
cargo test -p stab-oracle fixtures --quiet
just oracle::run --milestone PF2 --structural
just bench::smoke
```

Broader pre-commit verification follows `docs/plans/GOAL.md`.
