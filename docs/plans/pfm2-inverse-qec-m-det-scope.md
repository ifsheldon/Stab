# PFM2 Inverse-QEC `m_det` Scope

## Summary

This slice expands selected `Circuit::inverse_qec` coverage to the pinned Stim v1.16.0 `m_det` shape: one top-level plain `R 0 1 2`, `TICK`, `M 0 1 2`, `TICK`, `M 0 1 2`, then the two single-record detectors from the upstream fixture.
It is a narrow detector-flow packet for the upstream `m_det` case, not a general detector-flow rewrite algorithm.

## Owned Subcases

- Support exactly this top-level instruction shape: noiseless plain `R`, `TICK`, matching noiseless plain `M`, `TICK`, matching noiseless plain `M`, then two `DETECTOR` instructions.
- Require the reset and both measurement instructions to use target list `0 1 2` exactly and no arguments.
- Require both `TICK` instructions to have no targets or coordinate arguments.
- Require the first detector to contain exactly `rec[-1]` and the second detector to contain exactly `rec[-2]`, matching pinned Stim `m_det`.
- Preserve detector coordinate arguments and tags when rewriting the selected detectors, and preserve the two `TICK` instructions including tags.
- Keep reset and measurement instruction tags outside this exact packet because the selected output splits the initial reset into both reset and measurement effects.
- Return the pinned inverse shape: `R 2 1`, `M 0`, first preserved `TICK`, `M 2 1 0`, second preserved `TICK`, `M 2 1 0`, `DETECTOR(2) rec[-3]`, and `DETECTOR(1) rec[-2]`.
- Preserve existing selected QEC inverse packets for noisy measurement-only, noisy measure-reset-only, exact noisy measure-reset detector-flow, MPP detector-flow, exact two-to-one, measure-reset pass-through, reset-measure-detector, and unitary shapes.

## Explicit Rejections And Deferrals

- Reject reset or measurement target lists other than exactly `0 1 2`.
- Reject detector targets other than first-detector `rec[-1]` and second-detector `rec[-2]`.
- Reject noisy reset or measurement instructions, reset or measurement instruction tags, non-plain targets, missing ticks, extra ticks, extra operations, duplicate detector records, detector target groups, empty detectors, observables, feedback, repeats, and interleaved Clifford operations.
- Keep broader `m_det` variants, prior-record detector refs beyond this pinned packet, multi-target detector-flow behavior beyond existing selected packets, basis variants, different target lists, more detectors, and multi-instruction QEC inverse algorithms active for later PFM2 slices.
- Keep `dont_turn_measurements_into_resets`, Python API shape, and flow-returning overloads out of scope.

## Comparator And Evidence

The comparator class is exact structural parity against pinned Stim v1.16.0 `src/stim/util_top/circuit_inverse_qec.test.cc` for the untagged `m_det` case.
Detector tag preservation and explicit fail-closed nearby shapes are source-owned regression coverage, not oracle-managed exact-output fixtures.

```text
Input:
R 0 1 2
TICK
M 0 1 2
TICK
M 0 1 2
DETECTOR(2) rec[-1]
DETECTOR(1) rec[-2]

Expected:
R 2 1
M 0
TICK
M 2 1 0
TICK
M 2 1 0
DETECTOR(2) rec[-3]
DETECTOR(1) rec[-2]
```

## Oracle And Benchmark Policy

- Oracle row: add an executable structural row for the selected Rust `inverse_qec` `m_det` subset.
- Benchmark rows: no new row is required because this is a narrow transform selector, not a high-volume traversal or throughput path.
- Existing PF2 transform benchmark rows stay unchanged.

## Verification

Targeted verification for this slice:

```sh
cargo test -p stab-core --test circuit_inverse_qec_m_det --quiet
cargo test -p stab-core --test circuit_inverse_qec --quiet
cargo test -p stab-oracle fixtures --quiet
just oracle::run --milestone PF2 --structural
just bench::smoke
```

Broader pre-commit verification follows `docs/plans/GOAL.md`.
