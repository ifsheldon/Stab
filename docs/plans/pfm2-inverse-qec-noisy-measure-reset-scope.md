# PFM2 Inverse-QEC Noisy Measure-Reset Scope

## Summary

This slice expands selected `Circuit::inverse_qec` coverage to the pinned Stim v1.16.0 `noisy_mr` shape: a top-level measure-reset-only circuit made from `MR`, `MRX`, and `MRY` instructions, with optional probability arguments lowered into Pauli error instructions after each reversed measure-reset chunk.
It is a narrow measure-reset-only packet, not a general noisy detector-flow algorithm.

## Owned Subcases

- Support top-level circuits whose items are all `MR`, `MRX`, or `MRY` instructions.
- Reverse the instruction order.
- Reverse the target order within each measure-reset instruction.
- Preserve duplicate targets by splitting noisy reversed targets into duplicate-free chunks before emitting the corresponding Pauli error instruction.
- Strip probability arguments from the reversed `MR`, `MRX`, or `MRY` instruction and emit `X_ERROR` after noisy `MR`, or `Z_ERROR` after noisy `MRX` and `MRY`, with the original probability argument.
- Preserve instruction tags on both the reversed measure-reset instruction and emitted Pauli error instruction.
- Preserve inverted result targets only for noiseless measure-reset instructions because noisy inverted measure-reset would require invalid inverted targets on Pauli error gates.
- Drop empty measure-reset instructions by reusing the existing append helper that omits empty target lists.
- Preserve existing unitary inverse behavior and existing selected QEC inverse packets for reset-measure-detector, exact two-to-one, measure-reset pass-through, MPP detector-flow, and noisy measurement-only shapes.

## Explicit Rejections And Deferrals

- Reject noisy inverted `MR`, `MRX`, and `MRY` targets instead of emitting invalid `X_ERROR` or `Z_ERROR` targets.
- Keep broader noisy measure-reset plus detector rewrites, pair measurements, `MPP`, observables, feedback, ordinary noise instructions, repeat blocks beyond the exact `noisy_mr_det` packet, `TICK` placements beyond the exact packet, coordinates outside detector declarations, `SHIFT_COORDS`, and interleaved Clifford or detector-flow rewrites out of scope for this packet. The selected exact `noisy_mr_det` packet is owned by `docs/plans/pfm2-inverse-qec-noisy-measure-reset-detector-scope.md`.
- Keep `dont_turn_measurements_into_resets`, Python API shape, and flow-returning overloads out of scope.

## Comparator And Evidence

The comparator class is exact structural parity against pinned Stim v1.16.0 `src/stim/util_top/circuit_inverse_qec.test.cc` for the untagged `noisy_mr` case.
Tag preservation, duplicate-target chunking, noiseless measure-reset reversal, empty measure-reset omission, and noisy inverted-target rejection are source-owned regression coverage informed by local pinned-Stim probes, not oracle-managed exact-output fixtures.

```text
Input:
MR[m](0.125) 0 1

Observed with `uv run --with stim==1.16.0 python` and `stim.Circuit(text).time_reversed_for_flows([])`:
MR[m] 1 0
X_ERROR[m](0.125) 1 0
```

```text
Additional pinned probes:
MR(0.125) 0 0 -> MR 0; X_ERROR(0.125) 0; MR 0; X_ERROR(0.125) 0
MR 0 0 -> MR 0 0
MR(0.125) !0 -> invalid target modifier for X_ERROR
MRX(0.25) !0 and MRY(0.375) !0 -> invalid target modifier for Z_ERROR
```

## Oracle And Benchmark Policy

- Oracle row: add an executable structural row for the selected Rust `inverse_qec` noisy measure-reset-only subset.
- Benchmark rows: no new row is required because this is a narrow transform selector, not a high-volume traversal or throughput path.
- Existing PF2 transform benchmark rows stay unchanged.

## Verification

Targeted verification for this slice:

```sh
cargo test -p stab-core --test circuit_inverse_qec_noisy_measure_reset --quiet
cargo test -p stab-core --test circuit_inverse_qec_noisy_measurement --quiet
cargo test -p stab-core --test circuit_inverse_qec --quiet
cargo test -p stab-oracle fixtures --quiet
just oracle::run --milestone PF2 --structural
just bench::smoke
```

Broader pre-commit verification follows `docs/plans/GOAL.md`.
