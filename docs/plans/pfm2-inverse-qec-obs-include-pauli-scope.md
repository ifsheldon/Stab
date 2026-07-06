# PFM2 Inverse-QEC Observable Pauli Include Scope

## Summary

This slice expands selected `Circuit::inverse_qec` coverage to the pinned Stim v1.16.0 `obs_include_pauli` packet: one top-level `RX 1` followed by `OBSERVABLE_INCLUDE[test](1) X1`.
It is a narrow exact packet from `src/stim/util_top/circuit_inverse_qec.test.cc`, not a general observable-aware QEC inverse algorithm.

## Owned Subcases

- Support exactly this top-level instruction shape: noiseless plain `RX 1`, then `OBSERVABLE_INCLUDE[test](1) X1`.
- Require `RX` to have no probability arguments, no tag, and exactly one plain non-inverted qubit target with id `1`.
- Require `OBSERVABLE_INCLUDE` to have exactly one observable id argument equal to `1`, tag `test`, and exactly one non-inverted Pauli target `X1`.
- Preserve the original tagged Pauli observable declaration before the generated measurement.
- Return the pinned inverse shape: `OBSERVABLE_INCLUDE[test](1) X1`, `MX 1`, then `OBSERVABLE_INCLUDE(1) rec[-1]`.
- Preserve existing selected QEC inverse packets for noisy measurement-only, noisy measure-reset-only, exact noisy measure-reset detector-flow, `m_det`, MPP detector-flow, exact noisy `MZZ`, exact two-to-one, measure-reset pass-through, reset-measure-detector, and unitary shapes.

## Explicit Rejections And Deferrals

- Reject reset bases other than exactly `RX`.
- Reject reset targets other than exactly plain `1`, including inverted targets, duplicate targets, empty targets, and different qubits.
- Reject observable declarations without tag `test`, with ids other than `1`, with targets other than exactly `X1`, with inverted Pauli targets, with extra Pauli targets, with measurement-record targets, or with mixed Pauli and record targets.
- Reject interleaved operations, repeats, extra instructions, feedback, detectors, coordinate shifts, and multi-instruction observable-aware QEC inverse packets.
- Keep broader observable rewrites, observable ids and tags beyond the pinned packet, non-X bases, multi-target observable products, record-only observable includes, Python API shape, flow-returning overloads, and multi-instruction QEC inverse algorithms active for later PFM2 or PFM5 slices.

## Comparator And Evidence

The comparator class is exact structural parity against pinned Stim v1.16.0 `src/stim/util_top/circuit_inverse_qec.test.cc` for the untagged reset plus tagged observable Pauli include case.
Nearby fail-closed shapes are source-owned regression coverage, not oracle-managed exact-output fixtures.

```text
Input:
RX 1
OBSERVABLE_INCLUDE[test](1) X1

Expected:
OBSERVABLE_INCLUDE[test](1) X1
MX 1
OBSERVABLE_INCLUDE(1) rec[-1]
```

## Oracle And Benchmark Policy

- Oracle row: add an executable structural row for the selected Rust `inverse_qec` observable Pauli include subset.
- Benchmark rows: no new row is required because this is a narrow transform selector, not a high-volume traversal or throughput path.
- Existing PF2 transform benchmark rows stay unchanged.

## Verification

Targeted verification for this slice:

```sh
cargo test -p stab-core --test circuit_inverse_qec_obs_include_pauli --quiet
cargo test -p stab-core --test circuit_inverse_qec --quiet
cargo test -p stab-oracle fixtures --quiet
just oracle::run --milestone PF2 --structural
just bench::smoke
```

Broader pre-commit verification follows `docs/plans/GOAL.md`.
