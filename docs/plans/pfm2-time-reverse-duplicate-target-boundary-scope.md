# PFM2 Time-Reversed Flow Duplicate Target Boundary Scope

## Summary

This scope note locks the current PFM2 and PFM5 boundary for duplicate-target `Circuit::time_reversed_for_flows` measurement-rich rewrites.
Stab intentionally rejects duplicate reset-only and duplicate measure-reset target groups until a future compatibility decision chooses bug-compatible Stim v1.16.0 output, corrected semantic output, or permanent rejection.

## Boundary

- Selected reset-to-measurement support remains limited to one noiseless plain `R`, `RX`, or `RY` instruction over one or more unique qubit targets.
- Selected measure-reset flow reversal remains limited to one noiseless `MR`, `MRX`, or `MRY` instruction over one or more unique qubit targets, including inverted result targets.
- Duplicate reset-only targets such as `R 0 0`, `RX 0 0`, and `RY 0 0` fail closed.
- Duplicate measure-reset targets such as `MR 0 0`, `MRX 0 0`, `MRY 0 0`, and repeated targets inside a larger measure-reset group fail closed.
- The fail-closed behavior is a compatibility boundary, not a parser boundary; the circuits can parse, but the selected measurement-rich time-reversal transform does not return malformed or partial inverse flows.

## Evidence

- `time_reversed_for_flows_measurement_rich_subset_rejects_duplicate_reset_targets` covers duplicate reset-only rejection for `R`, `RX`, and `RY`.
- `time_reversed_for_flows_measurement_rich_subset_rejects_duplicate_measure_reset_targets` covers duplicate measure-reset rejection for `MR`, `MRX`, `MRY`, and a repeated target inside a multi-target measure-reset group.
- Oracle row `pf2-time-reverse-flow-measurement-rust` selects the measurement-rich time-reversal test subset and names duplicate reset-only plus duplicate measure-reset semantics as locked fail-closed pending compatibility decisions.
- `docs/plans/rpf5-flow-progress-report.md` records the pinned Stim v1.16.0 probes that returned malformed out-of-range inverse flows for duplicate reset-only and duplicate measure-reset examples.

## Non-Goals

- This note does not change unique-target reset, measurement, measure-reset, pair-measurement, `MZZ` suffix, unitary, feedback, detector, observable-aware, noise, repeat, QEC inverse, Python, CLI, or simulator-product behavior.
- This note does not decide whether a future implementation should be bug-compatible with Stim v1.16.0, semantically corrected, or permanently rejected for duplicate-target measurement-rich time reversal.
- This note does not add a benchmark row because the duplicate-target boundary is negative compatibility behavior, not a throughput path.

## Verification

```sh
cargo test -p stab-core --test circuit_inverse_qec measurement_rich_subset --quiet
cargo test -p stab-oracle fixtures --quiet
just oracle::run --milestone PF2 --structural
just bench::smoke
```
