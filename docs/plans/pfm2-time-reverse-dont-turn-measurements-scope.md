# PFM2 Time-Reversed Flow Keep Measurements Scope

## Summary

This slice promotes the pinned Stim v1.16.0 `Circuit.time_reversed_for_flows(..., dont_turn_measurements_into_resets=True)` behavior for the already-owned single-measurement Rust packet.
It adds a Stab-native Rust options API and does not add Python bindings or general measurement-rich QEC time reversal.

## Owned Subcase

- Input circuit: one noiseless plain single-target `M 0`.
- Input flow: `Z0 -> rec[-1]`.
- Options: `dont_turn_measurements_into_resets=true`.
- Expected output circuit: `M 0`.
- Expected output flow: `1 -> Z0 xor rec[-1]`.
- The existing default API remains unchanged and still converts this selected measurement to `R 0` with output flow `1 -> Z0`.
- Method and free-function forms must agree.

## Explicit Non-Goals

- Python binding parity for the keyword argument.
- General `dont_turn_measurements_into_resets` parity for multi-target measurements, pair measurements, generated circuits, detectors, observables, noisy measurements, feedback, repeats, or broader multi-instruction measurement-rich rewrites.
- Changes to `InverseQecOptions` or `Circuit::inverse_qec`.
- New primary benchmark gates.

## Comparator And Evidence

The comparator class is exact structural parity against the pinned Stim v1.16.0 Python `time_reversed_for_flows` example that keeps `M 0` instead of converting it to `R 0`.
The core evidence is `time_reversed_for_flows_measurement_rich_subset_can_keep_measurements` in `cargo test -p stab-core --test circuit_inverse_qec measurement_rich_subset`.

## Oracle And Benchmark Policy

- Oracle row `pf2-time-reverse-flow-measurement-rust` continues to own the selected measurement-rich Rust test packet.
- Benchmark row `pf2-time-reverse-flow-measurement` remains non-primary report-only and should include this option case in its existing measurement-rich corpus because it exercises the same transform hot path.
- No primary threshold entry is added because the row has no faithful direct pinned-Stim Rust API baseline in this harness.

## Done Criteria

- The exact option example returns the pinned circuit and flow shape.
- The default API and existing default tests remain unchanged.
- Unsupported broader shapes remain fail-closed under the existing measurement-rich time-reversal boundary.
- Scope, roadmap, checklist, oracle metadata, benchmark metadata, and progress reports describe the option without overstating Python or broad transform parity.
- Milestone-audit and full-code-review findings for this packet are fixed or logged as under-specification.
