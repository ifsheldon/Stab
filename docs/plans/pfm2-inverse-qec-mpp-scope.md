# PFM2 Inverse-QEC MPP Scope

## Summary

This slice expands the selected `Circuit::inverse_qec` detector-flow coverage to the pinned Stim v1.16.0 `mpp` shape: one noiseless `MPP` instruction followed by one detector depending on every measurement product record from that instruction, where the combined detector parity reduces to identity.
It is a narrow exact-output packet, not a general measurement-rich reverse detector-flow algorithm.

## Owned Subcases

- Support a top-level two-instruction circuit shaped as `MPP`, then one `DETECTOR`.
- Require the `MPP` instruction to be noiseless and to contain at least one Hermitian Pauli-product target group.
- Require the detector to contain exactly `rec[-1]`, `rec[-2]`, through `rec[-n]`, where `n` is the number of `MPP` product groups.
- Require the combined selected `MPP` product parity, including inverted targets, to reduce to a deterministic identity product.
- Return the pinned inverse shape with the `MPP` product groups in reverse group order and with each product's Pauli factors reversed, followed by a detector containing `rec[-n]` through `rec[-1]`.
- Preserve the `MPP` tag, detector tag, and detector coordinates.
- Preserve existing unitary inverse behavior, selected no-flow reset-measure-detector behavior, selected exact two-to-one detector-flow behavior, and selected measure-reset pass-through behavior.

## Explicit Rejections And Deferrals

- Reject sparse detector subsets, duplicate detector records, detector references outside the selected `MPP` measurement group, non-identity detector parities, empty detectors, empty `MPP` products, noisy `MPP` instructions, and anti-Hermitian `MPP` products.
- Keep multiple detectors, interleaved operations, resets, pair measurements, ordinary measurements, observables, feedback, noise, repeats, `dont_turn_measurements_into_resets`, Python API shape, and flow-returning overloads out of scope for this packet.
- Keep broader `m_det` beyond the selected exact packet, `mzz`, noisy measure-reset detector-flow beyond the exact `noisy_mr_det` packet, `obs_include_pauli`, and multi-instruction QEC inverse behavior active for later PFM2 or PFM5 slices.

## Comparator And Evidence

The comparator class is exact structural parity against pinned Stim v1.16.0 `src/stim/util_top/circuit_inverse_qec.test.cc` for the selected untagged `mpp` case.
Tag and coordinate preservation is source-owned regression coverage informed by the local pinned-Stim probe below, not an oracle-managed exact-output fixture.
Nearby sparse, duplicate-record, non-identity, and anti-Hermitian shapes rejected by Stim remain explicit rejection coverage in Stab.
Noisy `MPP` remains deliberately fail-closed in Stab for this packet even though pinned Stim accepts some deterministic noisy identity products.

```text
Input:
MPP[m] !X0*X1 Y0*Y1 Z0*Z1
DETECTOR[d](7) rec[-1] rec[-2] rec[-3]

Observed with `uv run --with stim==1.16.0 python` and `stim.Circuit(text).time_reversed_for_flows([])`:
MPP[m] Z1*Z0 Y1*Y0 X1*!X0
DETECTOR[d](7) rec[-3] rec[-2] rec[-1]
```

## Oracle And Benchmark Policy

- Oracle row: add an executable structural row for the selected Rust `inverse_qec` MPP subset.
- Benchmark rows: no new row is required because this is a narrow transform selector, not a high-volume traversal or throughput path.
- Existing PF2 transform benchmark rows stay unchanged.

## Verification

Targeted verification for this slice:

```sh
cargo test -p stab-core --test circuit_inverse_qec_mpp --quiet
cargo test -p stab-core --test circuit_inverse_qec --quiet
cargo test -p stab-oracle fixtures --quiet
just oracle::run --milestone PF2 --structural
just bench::smoke
```

Broader pre-commit verification follows `docs/plans/GOAL.md`.
