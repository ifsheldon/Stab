# PF1 Circuit API Progress Report

## Summary

This PF1 slice implements the bounded Rust circuit stats and final-coordinate query subset from `docs/plans/partial-feature-closure-plan.md`.
It does not claim full Python `stim.Circuit` API parity.
Append-from-text helpers, insert, pop, concatenation operators, file helpers, detector-coordinate maps, instruction-range views, stable typed iterators, reference-sample API closure, and determined-measurement API closure remain active PF1 work.

## Implemented Surfaces

- Added public `Circuit::len`, `Circuit::is_empty`, and `Circuit::clear`.
- Added public folded count methods for measurements, detectors, observables, ticks, and sweep bits.
- Added public `Circuit::final_coordinate_shift` and `Circuit::final_qubit_coordinates`.
- Added folded repeat handling for count and coordinate queries so large repeat blocks do not need full unrolling.
- Added measurement-result count coverage for grouped measurement gates including `MPP`, pair measurements, heralded noise, and `MPAD`.
- Added folded-count overflow rejection instead of saturating or silently wrapping.
- Added non-finite folded-coordinate rejection instead of silently returning infinity.

## Oracle Rows

Implemented row:

- `pf1-circuit-stats-coordinates`

Still broad and manifest-only:

- `pf1-circuit-rust-api`

## Benchmark Rows

Non-primary report-only row:

- `pf1-circuit-coordinate-query`

Probe reports:

- `target/benchmarks/pf1-circuit-api-probe/baseline.json`
- `target/benchmarks/pf1-circuit-api-compare/compare.json`

Fresh probe rates from the current worktree:

- `stab_circuit_counts_nested_repeat`: `7.692e6 queries/s`.
- `stab_circuit_final_coordinate_shift_nested_repeat`: `2.500e7 queries/s`.
- `stab_circuit_final_qubit_coordinates_nested_repeat`: `6.211e6 queries/s`.

This benchmark remains `non-primary-report-only` because pinned Stim exposes comparable behavior through C++ and Python APIs but not through a faithful Rust direct baseline.
It was not added to `benchmarks/m12-primary-thresholds.json`.

## Verification Evidence

Passed during implementation:

```sh
cargo test -p stab-core --test circuit_api --quiet
cargo test -p stab-core count_determined --quiet
cargo test -p stab-bench pf1_circuit_coordinate --quiet
cargo test -p stab-bench manifest --quiet
cargo test -p stab-oracle fixtures --quiet
just oracle::run --milestone PF1
just bench::baseline --only pf1-circuit-coordinate-query --out target/benchmarks/pf1-circuit-api-probe
just bench::compare --only pf1-circuit-coordinate-query --baseline target/benchmarks/pf1-circuit-api-probe/baseline.json --report target/benchmarks/pf1-circuit-api-compare
```

## Remaining PF1 Circuit API Work

- Append text through the parser.
- Copy, insert, pop, concatenate, repeat, file constructor, and file writer helpers where they are useful Rust APIs.
- Detector-coordinate maps and single-detector coordinate lookup.
- Instruction-range views and stable typed iterators.
- Public reference-sample helpers beyond the existing internal sampler support.
- Public determined-measurement helpers beyond the currently implemented count subset.
