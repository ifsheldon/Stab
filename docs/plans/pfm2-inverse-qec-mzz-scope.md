# PFM2 Inverse-QEC `mzz` Scope

## Summary

This slice expands selected `Circuit::inverse_qec` coverage to the pinned Stim v1.16.0 `mzz` detector-flow packet: one top-level `MRY 0 1`, `M 0`, `TICK`, noisy `MZZ(0.125) 0 1 2 3`, `TICK`, `M 1`, `MRY 0 1`, then one detector referencing `rec[-3] rec[-5] rec[-6]`.
It is a narrow exact packet from `src/stim/util_top/circuit_inverse_qec.test.cc`, not a general pair-measurement reverse detector-flow algorithm.

## Owned Subcases

- Support exactly this top-level instruction shape: noiseless plain `MRY`, noiseless plain `M`, `TICK`, noisy plain `MZZ`, `TICK`, noiseless plain `M`, noiseless plain `MRY`, then one `DETECTOR`.
- Require both `MRY` instructions to target exactly `0 1` with no arguments.
- Require the first `M` to target exactly `0` and the second `M` to target exactly `1`, with no arguments.
- Require `MZZ` to have exactly one probability argument and target exactly the two pairs `0 1` and `2 3`.
- Require both `TICK` instructions to have no targets or coordinate arguments.
- Require the detector to contain exactly `rec[-3] rec[-5] rec[-6]`.
- Preserve detector coordinate arguments and tags when rewriting the selected detector, and preserve the two `TICK` instructions including tags.
- Keep `MRY`, `M`, and `MZZ` instruction tags outside this exact packet.
- Return the pinned inverse shape: `MRY 1 0`, `R 1`, first preserved `TICK`, `MZZ(0.125) 2 3 0 1`, second preserved `TICK`, `M 0`, `DETECTOR rec[-2] rec[-1]`, and `MRY 1 0`.
- Preserve existing selected QEC inverse packets for noisy measurement-only, noisy measure-reset-only, exact noisy measure-reset detector-flow, `m_det`, MPP detector-flow, exact two-to-one, measure-reset pass-through, reset-measure-detector, and unitary shapes.

## Explicit Rejections And Deferrals

- Reject any target list other than exactly the selected `MRY 0 1`, `M 0`, `MZZ 0 1 2 3`, `M 1`, and `MRY 0 1` shape.
- Reject detector targets other than exactly `rec[-3] rec[-5] rec[-6]`.
- Reject noiseless `MZZ`, multi-probability `MZZ`, non-plain targets, instruction tags on `MRY`, `M`, or `MZZ`, missing ticks, extra ticks, extra operations, duplicate or sparse detector records, empty detectors, observables, feedback, repeats, and interleaved Clifford operations.
- Keep broader `mzz` variants, other pair-measurement bases, different target lists, multiple detectors, prior-record detector refs beyond this pinned packet, `dont_turn_measurements_into_resets`, Python API shape, flow-returning overloads, and multi-instruction QEC inverse algorithms active for later PFM2 or PFM5 slices.

## Comparator And Evidence

The comparator class is exact structural parity against pinned Stim v1.16.0 `src/stim/util_top/circuit_inverse_qec.test.cc` for the untagged `mzz` case.
Detector and TICK metadata preservation plus explicit fail-closed nearby shapes are source-owned regression coverage, not oracle-managed exact-output fixtures.

```text
Input:
MRY 0 1
M 0
TICK
MZZ(0.125) 0 1 2 3
TICK
M 1
MRY 0 1
DETECTOR rec[-3] rec[-5] rec[-6]

Expected:
MRY 1 0
R 1
TICK
MZZ(0.125) 2 3 0 1
TICK
M 0
DETECTOR rec[-2] rec[-1]
MRY 1 0
```

## Oracle And Benchmark Policy

- Oracle row: add an executable structural row for the selected Rust `inverse_qec` `mzz` subset.
- Benchmark rows: no new row is required because this is a narrow transform selector, not a high-volume traversal or throughput path.
- Existing PF2 transform benchmark rows stay unchanged.

## Verification

Targeted verification for this slice:

```sh
cargo test -p stab-core --test circuit_inverse_qec_mzz --quiet
cargo test -p stab-core --test circuit_inverse_qec --quiet
cargo test -p stab-oracle fixtures --quiet
just oracle::run --milestone PF2 --structural
just bench::smoke
```

Broader pre-commit verification follows `docs/plans/GOAL.md`.
