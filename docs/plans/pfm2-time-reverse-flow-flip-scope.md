# PFM2 Time-Reversed Flow Flip Scope

## Summary

This slice promotes the exact pinned Stim v1.16.0 `circuit_inverse_qec.flow_flip` subcase for Rust `Circuit::time_reversed_for_flows`.
It is a narrow multi-instruction measurement-rich time-reversal packet, not a general multi-instruction QEC inverse algorithm.

## Owned Subcase

- Input circuit: `MY 0`, `MRX 0`, `MR 1`, `R 0`.
- Input flows, in pinned upstream order:
  - `Y0*Z1 -> rec[-3] xor rec[-1]`.
  - `1 -> Z0*Z1`.
  - `1 -> Z1`.
  - `1 -> Z0`.
- Expected output circuit: `M 0`, `MR 1`, `MRX 0`, `RY 0`.
- Expected output flows, in pinned upstream order:
  - `1 -> Y0*Z1`.
  - `Z0*Z1 -> rec[-3] xor rec[-2]`.
  - `Z1 -> rec[-2]`.
  - `Z0 -> rec[-3]`.
- The implementation must validate the input flows through the existing sparse flow checker before returning the pinned inverse.
- The selector is exact-scope: it accepts only this circuit shape and these four requested flows.

## Explicit Non-Goals

- General multi-instruction measurement-rich `time_reversed_for_flows`.
- Different instruction orderings, tags, probabilities, target lists, bases, inverted reset targets, detectors, observables, feedback, noise, repeats, or additional flows.
- Flow-returning overload parity for C++ internals beyond the Rust API surface.
- Python API shape or exact Python binding behavior.
- New benchmark rows, because the existing report-only `pf2-time-reverse-flow-measurement` row already measures the selected measurement-rich Rust transform corpus.

## Comparator And Evidence

The comparator class is exact structural parity against pinned Stim v1.16.0 `src/stim/util_top/circuit_inverse_qec.test.cc` for `flow_flip`.
The evidence is the focused Rust test `time_reversed_for_flows_measurement_rich_subset_supports_flow_flip`.

## Oracle And Benchmark Policy

- Oracle row `pf2-time-reverse-flow-measurement-rust` selects `cargo test -p stab-core --test circuit_inverse_qec measurement_rich_subset`, which includes the new exact-output test and nearby fail-closed test.
- Benchmark row `pf2-time-reverse-flow-measurement` remains report-only and must include the new `flow_flip` corpus case in its measurement work.
- No primary threshold entry is added because pinned Stim has no faithful Rust direct baseline in this harness for the selected API.

## Verification

```sh
cargo test -p stab-core --test circuit_inverse_qec measurement_rich_subset --quiet
cargo test -p stab-oracle fixtures --quiet
just oracle::run --milestone PF2 --structural
just bench::smoke
cargo test -p stab-bench --quiet
```
