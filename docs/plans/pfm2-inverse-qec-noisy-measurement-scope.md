# PFM2 Inverse-QEC Noisy Measurement Scope

## Summary

This slice expands selected `Circuit::inverse_qec` coverage to the pinned Stim v1.16.0 `noisy_m` shape: a top-level measurement-only circuit made from `M`, `MX`, and `MY` instructions, with probability arguments preserved and instruction order reversed.
It is a narrow measurement-only packet, not a general noisy reverse detector-flow algorithm.

## Owned Subcases

- Support top-level circuits whose items are all `M`, `MX`, or `MY` instructions.
- Preserve each measurement instruction's canonical gate, probability arguments, and tag.
- Reverse the instruction order.
- Reverse the target order within each measurement instruction.
- Preserve duplicate measurement targets.
- Preserve inverted measurement targets because pinned Stim accepts `M !0` and returns it unchanged for the single-target case.
- Drop empty measurement instructions by reusing the existing append helper that omits empty target lists.
- Preserve existing unitary inverse behavior and existing selected QEC inverse packets for reset-measure-detector, exact two-to-one, measure-reset pass-through, and MPP detector-flow shapes.

## Explicit Rejections And Deferrals

- Keep `MR`, `MRX`, `MRY`, pair measurements, `MPP`, detectors, observables, feedback, ordinary noise instructions, repeat blocks, `TICK`, coordinates, `SHIFT_COORDS`, and interleaved Clifford or detector-flow rewrites out of scope for this packet.
- Keep broader noisy measure-reset plus detector rewrites active for later PFM2 slices; selected top-level noisy `MR`/`MRX`/`MRY` measure-reset-only reversal is owned by `docs/plans/pfm2-inverse-qec-noisy-measure-reset-scope.md`, and the selected exact `noisy_mr_det` packet is owned by `docs/plans/pfm2-inverse-qec-noisy-measure-reset-detector-scope.md`.
- Keep `dont_turn_measurements_into_resets`, Python API shape, and flow-returning overloads out of scope.

## Comparator And Evidence

The comparator class is exact structural parity against pinned Stim v1.16.0 `src/stim/util_top/circuit_inverse_qec.test.cc` for the untagged `noisy_m` case.
Tag preservation, inverted measurement target preservation, and empty measurement omission are source-owned regression coverage informed by the local pinned-Stim probe below, not oracle-managed exact-output fixtures.

```text
Input:
M[m](0.125) 0 1
MX[x](0.25) 2
MY[y](0.375) 3

Observed with `uv run --with stim==1.16.0 python` and `stim.Circuit(text).time_reversed_for_flows([])`:
MY[y](0.375) 3
MX[x](0.25) 2
M[m](0.125) 1 0
```

```text
Additional pinned probes:
M !0 -> M !0
M -> empty circuit
MR(0.125) 0 -> MR 0 followed by X_ERROR(0.125) 0
M 0 followed by DETECTOR rec[-1] -> detecting region reached the start of the circuit
```

## Oracle And Benchmark Policy

- Oracle row: add an executable structural row for the selected Rust `inverse_qec` noisy measurement-only subset.
- Benchmark rows: no new row is required because this is a narrow transform selector, not a high-volume traversal or throughput path.
- Existing PF2 transform benchmark rows stay unchanged.

## Verification

Targeted verification for this slice:

```sh
cargo test -p stab-core --test circuit_inverse_qec_noisy_measurement --quiet
cargo test -p stab-core --test circuit_inverse_qec --quiet
cargo test -p stab-oracle fixtures --quiet
just oracle::run --milestone PF2 --structural
just bench::smoke
```

Broader pre-commit verification follows `docs/plans/GOAL.md`.
