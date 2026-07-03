# PF1 Circuit API Progress Report

## Summary

This PF1 slice implements the bounded Rust circuit stats, final-coordinate query, and append-from-text mutation subset from `docs/plans/partial-feature-closure-plan.md`.
It does not claim full Python `stim.Circuit` API parity.
Insert, pop, concatenation operators, file helpers, detector-coordinate maps, instruction-range views, stable typed iterators, reference-sample API closure, and determined-measurement API closure remain active PF1 work.

## Implemented Surfaces

- Added public `Circuit::len`, `Circuit::is_empty`, and `Circuit::clear`.
- Added public `Circuit::append_from_stim_text`, plus `Circuit::append_from_stim_program_text` as a Stim Python compatibility alias.
- Text append parses into a temporary circuit before mutating the receiver, so parse failures leave the original circuit unchanged.
- Text append uses the existing append path, so adjacent compatible instructions fuse and parsed tags or repeat blocks are preserved.
- Added public folded count methods for measurements, detectors, observables, ticks, and sweep bits.
- Added public `Circuit::final_coordinate_shift` and `Circuit::final_qubit_coordinates`.
- Added folded repeat handling for count and coordinate queries so large repeat blocks do not need full unrolling.
- Added measurement-result count coverage for grouped measurement gates including `MPP`, pair measurements, heralded noise, and `MPAD`.
- Added folded-count overflow rejection instead of saturating or silently wrapping.
- Added non-finite folded-coordinate rejection instead of silently returning infinity.

## Oracle Rows

Implemented row:

- `pf1-circuit-stats-coordinates`
- `pf1-circuit-append-text`

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
No separate benchmark row was added for append-from-text because this helper is parser-backed mutation glue; the relevant high-volume path is covered by existing parser benchmarks and the PF1 coordinate-query row covers the PF1 query workload.

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

- Copy, insert, pop, concatenate, repeat, file constructor, and file writer helpers where they are useful Rust APIs.
- Detector-coordinate maps and single-detector coordinate lookup.
- Instruction-range views and stable typed iterators.
- Public reference-sample helpers beyond the existing internal sampler support.
- Public determined-measurement helpers beyond the currently implemented count subset.
