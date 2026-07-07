# PFM2 Inverse-QEC Keep-Measurements Scope

## Summary

This slice promotes the pinned Stim v1.16.0 `circuit_inverse_qec.r_m_det_keep_m` subcase for the already selected reset-measure-detector inverse-QEC packet.
It adds an explicit Rust options API instead of changing the default `Circuit::inverse_qec` behavior.

## Owned Subcase

- Input circuit: `R 0`, `M 0`, `DETECTOR rec[-1]`.
- Options: `InverseQecOptions { keep_measurements: true }`.
- Expected output: `M 0`, `M 0`, `DETECTOR rec[-2] rec[-1]`.
- Default behavior remains the existing selected reset-measure-detector identity output for the same input when `keep_measurements` is false.
- Public surface: additive Rust `stab-core` options API for `circuit_inverse_qec_with_options` and `Circuit::inverse_qec_with_options`.
- Broader selected reset-measure-detector variants reject `keep_measurements` until each variant is explicitly scoped and proven.

## Explicit Non-Goals

- Python API shape or exact Python binding spelling.
- Flow-returning overloads of Stim's C++ helper.
- Applying `keep_measurements` to selected `two_to_one`, `m_det`, `mpp`, `mpad`, `mzz`, noisy measurement, noisy measure-reset, noisy measure-reset detector, observable Pauli include, measure-reset pass-through, feedback, repeat-contained, or broader multi-instruction QEC inverse packets.
- Applying `keep_measurements` to same-basis, tagged, coordinated, multi-target, sparse-detector, duplicate-record, empty-detector, or other broader reset-measure-detector variants.
- Changing `circuit_inverse_qec` or `Circuit::inverse_qec` default output.
- New benchmark coverage, because this is one branch in an existing narrow transform selector and not a distinct throughput workload.

## Comparator And Evidence

The comparator class is exact structural parity against pinned Stim v1.16.0 `src/stim/util_top/circuit_inverse_qec.test.cc` for `r_m_det_keep_m`.
The exact evidence is `circuit_inverse_qec_with_options_keeps_selected_reset_measure_detector_measurements` in `crates/stab-core/tests/circuit_inverse_qec_reset_measure_detector.rs`.

## Oracle And Benchmark Policy

- Oracle row `pf2-inverse-qec-reset-measure-detector-rust` selects `cargo test -p stab-core --test circuit_inverse_qec_reset_measure_detector`, which includes the new options test.
- No benchmark row changes are required; existing PF2 report-only transform rows do not measure this narrow inverse-QEC branch.

## Verification

```sh
cargo test -p stab-core --test circuit_inverse_qec_reset_measure_detector --quiet
cargo test -p stab-core --test circuit_inverse_qec --quiet
cargo test -p stab-oracle fixtures --quiet
just oracle::run --milestone PF2 --structural
just bench::smoke
```
