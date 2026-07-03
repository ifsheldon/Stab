# PF1 Circuit API Progress Report

## Summary

This PF1 slice implements the bounded Rust circuit stats, final-coordinate query, and Rust mutation-helper subset from `docs/plans/partial-feature-closure-plan.md`.
It does not claim full Python `stim.Circuit` API parity.
File helpers, instruction-range views, stable typed iterators, reference-sample API closure, and determined-measurement API closure remain active PF1 work.

## Implemented Surfaces

- Added public `Circuit::len`, `Circuit::is_empty`, and `Circuit::clear`.
- Added public `Circuit::append_from_stim_text`, plus `Circuit::append_from_stim_program_text` as a Stim Python compatibility alias.
- Text append parses into a temporary circuit before mutating the receiver, so parse failures leave the original circuit unchanged.
- Text append uses the existing append path, so adjacent compatible instructions fuse and parsed tags or repeat blocks are preserved.
- Added public `Circuit::append_circuit` and `Circuit::concatenated`, which copy circuit items and fuse compatible boundary instructions through the normal append path.
- Added public `Circuit::repeated` and `Circuit::repeat_in_place`, including Stim-style special cases for zero and one repetitions, single-repeat-block count fusion, and overflow rejection.
- Added public `Circuit::insert_item`, `Circuit::insert_instruction`, `Circuit::insert_repeat_block`, `Circuit::insert_circuit`, `Circuit::pop_item`, and `Circuit::pop_last_item`.
- Insert helpers fuse compatible instruction boundaries around the insertion range, matching Stim's owned insertion semantics for the tested Rust subset, while pop helpers remove top-level items without fusing neighbors after removal.
- Added public folded count methods for measurements, detectors, observables, ticks, and sweep bits.
- Added public `Circuit::final_coordinate_shift` and `Circuit::final_qubit_coordinates`.
- Added public `CircuitDetectorId`, `Circuit::detector_coordinates`, `Circuit::detector_coordinates_for`, and `Circuit::coordinates_of_detector`.
- Detector-coordinate lookup uses folded repeat skipping for requested detectors and preserves Stim semantics for empty detector coordinates.
- Added folded repeat handling for count and coordinate queries so large repeat blocks do not need full unrolling.
- Added measurement-result count coverage for grouped measurement gates including `MPP`, pair measurements, heralded noise, and `MPAD`.
- Added folded-count overflow rejection instead of saturating or silently wrapping.
- Added non-finite folded-coordinate rejection instead of silently returning infinity.

Compatibility note: Stim v1.16.0 C++ coordinate helpers allow finite coordinate inputs to overflow into infinities during folded double arithmetic.
Stab's Rust coordinate query APIs currently reject non-finite folded coordinate results as a deliberate Rust API hardening choice; this must be revisited before claiming exact Python binding or C++ API side-effect parity for coordinate queries.

## Oracle Rows

Implemented row:

- `pf1-circuit-stats-coordinates`
- `pf1-circuit-append-text`
- `pf1-circuit-concat`
- `pf1-circuit-repeat`
- `pf1-circuit-insert-pop`
- `pf1-circuit-detector-coordinates`

Still broad and manifest-only:

- `pf1-circuit-rust-api`

## Benchmark Rows

Non-primary report-only row:

- `pf1-circuit-coordinate-query`

Probe reports:

- `target/benchmarks/pf1-circuit-api-probe/baseline.json`
- `target/benchmarks/pf1-circuit-api-compare/compare.json`

Fresh probe rates from the current worktree:

- `stab_circuit_counts_nested_repeat`: `7.634e6 queries/s`.
- `stab_circuit_final_coordinate_shift_nested_repeat`: `2.564e7 queries/s`.
- `stab_circuit_final_qubit_coordinates_nested_repeat`: `6.098e6 queries/s`.
- `stab_circuit_detector_coordinates_nested_repeat`: `4.310e6 queries/s`.
- `stab_circuit_detector_coordinates_late_nested_repeat`: `6.250e6 queries/s`.

This benchmark remains `non-primary-report-only` because pinned Stim exposes comparable behavior through C++ and Python APIs but not through a faithful Rust direct baseline.
It was not added to `benchmarks/m12-primary-thresholds.json`.
No separate benchmark row was added for append-from-text, concatenation, repetition, insert, or pop helpers because these are structural mutation APIs rather than the PF1 high-volume query workload; parser-backed append is covered by existing parser benchmarks and the PF1 coordinate-query row covers the PF1 query workload.

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

- File constructor and file writer helpers where they are useful Rust APIs.
- Instruction-range views and stable typed iterators.
- Public reference-sample helpers beyond the existing internal sampler support.
- Public determined-measurement helpers beyond the currently implemented count subset.
