# PFM3 Analyzer Sweep Boundary Scope

## Summary

This scope note locks the current PFM3 analyzer sweep-shape boundary.
The selected implementation covers the pinned Stim v1.16.0 sweep-control no-op case plus the source-owned selected `CY`, `CZ`, `XCZ`, and `YCZ` analyzer matrix, including selected `CZ` classical-only groups.
It does not claim full analyzer sweep-shape parity.

## Selected Surface

- Core Rust API: `stab_core::circuit_to_detector_error_model`.
- Public CLI dependency: `stab analyze_errors`.
- Input family: selected sweep-controlled and measurement-record-controlled controlled-Pauli target groups that the analyzer treats as semantic no-ops.
- Comparator: exact or structural DEM output for accepted analyzer no-op cases, plus explicit domain-error assertions for invalid controlled-Pauli target positions.

## Selected Positive Cases

- Pinned upstream case: `CNOT sweep[0] 0` from Stim v1.16.0 `ErrorAnalyzer, ignores_sweep_controls`.
- Source-owned selected no-op gates: `CX`, `CY`, `CZ`, `XCZ`, and `YCZ` with gate-order-valid sweep-controlled Pauli target groups.
- Source-owned selected `CZ` classical-only no-op groups: sweep/sweep, record/sweep, sweep/record, and record/record.
- Public CLI selected matrix: `stab analyze_errors` emits the selected no-op DEM, including `CZ rec[-a] rec[-b]`, with empty stderr and successful exit.

## Selected Negative Cases

- Invalid controlled-Pauli target positions reject with explicit analyzer errors.
- Neighboring non-`CZ` record/record groups remain fail-closed for the selected rejection set.
- Unsupported analyzer sweep shapes outside the selected matrix must fail closed or wait for a future exact-subcase plan.

## Evidence

- `dem_analyzer_ignores_sweep_controls_like_upstream` covers the pinned upstream no-op case and selected source-owned analyzer matrix.
- `dem_analyzer_rejects_invalid_sweep_target_positions` covers invalid target-position rejection.
- `analyze_errors_sweep_controls_match_pf3_oracle` covers public CLI success behavior for the selected matrix.
- `analyze_errors_sweep_controls_reject_invalid_target_positions` covers public CLI failure behavior for invalid target positions.
- Oracle row `pf3-analyze-errors-sweep-core` selects `cargo test -p stab-core --test dem_analyzer_classical sweep`.
- Oracle row `pf3-analyze-errors-sweep-cli` selects `cargo test -p stab-cli analyze_errors_sweep_controls`.
- Benchmark row `pf3-analyze-errors-sweep` is report-only and measures only the selected analyzer sweep-control and `CZ` classical-only no-op matrix.

## Explicit Non-Goals

- This slice does not select additional analyzer sweep target placements, additional gate families, Python detector-sampler sweep APIs, public `detect --sweep`, or full legal-gate execution parity.
- This slice does not promote broader analyzer sweep-shape parity beyond the selected matrix.
- Broader analyzer sweep-shape parity is under-specified until a future plan names exact gate-target shapes, expected no-op or rejection behavior, CLI and Rust surfaces, comparator class, positive and negative tests, oracle metadata, resource behavior, and benchmark or no-benchmark rationale.

## Verification Commands

- `cargo test -p stab-core --test dem_analyzer_classical sweep --quiet`
- `cargo test -p stab-cli analyze_errors_sweep_controls --quiet`
- `cargo test -p stab-oracle fixtures --quiet`
- `just oracle::list --milestone PF3`
- `just bench::smoke`
