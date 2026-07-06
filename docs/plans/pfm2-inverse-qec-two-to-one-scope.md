# PFM2 Inverse-QEC Two-To-One Scope

## Summary

This slice expands the selected `Circuit::inverse_qec` detector-flow coverage to the pinned Stim v1.16.0 `two_to_one` shape: a Z-basis reset pair, one matching `CX`, a matching Z-basis measurement pair, and one detector depending on both measurement records.
It is a narrow exact-output packet, not a general reverse detector-flow algorithm.

## Owned Subcases

- Support a top-level four-instruction circuit shaped as `R`, then `CX`, then `M`, then one `DETECTOR`.
- Require `R` and `M` to have exactly the same two plain unique qubit targets.
- Require `CX` to have exactly one plain qubit-pair target matching the reset and measurement target order.
- Require the detector to contain exactly `rec[-1] rec[-2]`, matching the pinned Stim `two_to_one` case.
- Return the pinned inverse shape `R` on the reversed target pair, the original `CX`, `M` on the reversed target pair, and `DETECTOR rec[-2]`.
- Preserve `CX` and detector tags, preserve detector coordinates, and map reset and measurement tags in the same role-swapping order observed in pinned Stim.
- Preserve existing unitary inverse behavior, selected no-flow reset-measure-detector behavior, and selected measure-reset pass-through behavior.

## Explicit Rejections And Deferrals

- Reject sparse detector subsets, duplicate detector records, empty detectors, detector references outside the selected measurement group, noisy reset or measurement instructions, duplicate reset or measurement targets, nonmatching reset or measurement targets, extra reset or measurement targets, nonmatching `CX` targets, and reversed or multi-pair `CX` shapes.
- Keep non-record `DETECTOR` targets rejected at the circuit construction boundary because `DETECTOR` uses the record-only target rule.
- Keep `RX`/`MX`, `RY`/`MY`, `MR`, `MXX`, `MYY`, `MZZ`, `MPP`, other Clifford interleavings, multiple detectors, `TICK`, coordinates outside detector declarations, observables, feedback, noise beyond the explicit reset or measurement rejection, repeats, `dont_turn_measurements_into_resets`, Python API shape, and flow-returning overloads out of scope.

## Comparator And Evidence

The comparator class is exact structural parity against pinned Stim v1.16.0 `src/stim/util_top/circuit_inverse_qec.test.cc` for the selected untagged `two_to_one` case.
Tag and coordinate preservation is source-owned regression coverage informed by the local pinned-Stim probe below, not an oracle-managed exact-output fixture.
Nearby Stim-supported sparse, noisy, basis-variant, and larger target-list shapes remain explicitly unpromoted.

```text
Input:
R[r] 0 1
CX[c] 0 1
M[m] 0 1
DETECTOR[d](7) rec[-1] rec[-2]

Observed with `uv run --with stim==1.16.0 python` and `stim.Circuit(text).time_reversed_for_flows([])`:
R[m] 1 0
CX[c] 0 1
M[r] 1 0
DETECTOR[d](7) rec[-2]
```

## Oracle And Benchmark Policy

- Oracle row: add an executable structural row for the selected Rust `inverse_qec` two-to-one subset.
- Benchmark rows: no new row is required because this is a narrow transform selector, not a high-volume traversal or throughput path.
- Existing PF2 transform benchmark rows stay unchanged.

## Verification

Targeted verification for this slice:

```sh
cargo test -p stab-core --test circuit_inverse_qec two_to_one --quiet
cargo test -p stab-core --test circuit_inverse_qec --quiet
cargo test -p stab-oracle fixtures --quiet
just oracle::run --milestone PF2 --structural
just bench::smoke
```

Broader pre-commit verification follows `docs/plans/GOAL.md`.
