# M9 Detector Utility Closure Completion Report

## Summary

The M9 detector utility closure implemented the bounded follow-up scope from `docs/plans/m9-detector-utility-closure-plan.md`.
Stab now exposes typed Rust APIs for the simple detecting-regions case and the basic single-record missing-detectors subset, and `circuit_with_inlined_feedback` now has source-owned MPP feedback-transform parity evidence for the scoped transform.
Broader detecting-region target-shape support, broader gauge handling, multi-record missing-detector row reduction, repeated MPP stabilizer-product missing-detector analysis, observable-interaction missing-detector analysis, honeycomb suffix analysis, toric suffix analysis, broader repeat-contained feedback, unsupported classical feedback gates, and full transform API parity remain future work. Later PF5 slices added selected detecting-region gauge evidence, and later PF2 feedback slices added selected bounded loop-refolding and nested bounded-repeat detector-parity evidence.

## Implemented Surfaces

- Added `DetectingRegionOptions`, `DetectingRegionMap`, and `circuit_detecting_regions` in `stab-core`.
- Added `MissingDetectorOptions` and `missing_detectors` in `stab-core`.
- Added a sparse reverse tracker region snapshot helper used by detecting-region extraction.
- Added implicit initial Z-basis anticommutation checking for detecting-region extraction and omitted identity snapshots from the returned tick map.
- Added exact MPP feedback-transform coverage for `circuit_with_inlined_feedback`.
- Added fail-closed coverage for unsupported detecting-region target shapes, anti-Hermitian MPP products, multi-record missing-detector row reduction, repeat blocks in feedback inlining at the time of this slice, and unsupported classical controlled feedback gates.
- Split the broad M9 utility oracle rows into implemented subcase rows and explicit future manifest-only rows.
- Added report-only utility benchmark rows for detecting regions, missing detectors, and MPP feedback inlining.

## Oracle Rows

Implemented rows:

- `coverage-util-top-circuit-to-detecting-regions-simple`
- `coverage-util-top-missing-detectors-basic`
- `coverage-util-top-transform-without-feedback`, updated to include MPP feedback parity

Future manifest-only rows:

- `coverage-util-top-circuit-to-detecting-regions-future`
- `coverage-util-top-missing-detectors-row-reduction-future`
- `coverage-util-top-missing-detectors-mpp-future`
- `coverage-util-top-missing-detectors-observable-future`
- `coverage-util-top-missing-detectors-honeycomb-future`
- `coverage-util-top-missing-detectors-toric-future`

## Benchmark Rows

Non-primary report-only rows:

- `m9-detecting-regions-basic-batch`
- `m9-missing-detectors-basic-batch`
- `m9-feedback-inline-mpp-batch`

Probe reports:

- `target/benchmarks/m9-detecting-regions-basic-probe/baseline.json`
- `target/benchmarks/m9-detecting-regions-basic-compare/compare.json`
- `target/benchmarks/m9-missing-detectors-basic-probe/baseline.json`
- `target/benchmarks/m9-missing-detectors-basic-compare/compare.json`
- `target/benchmarks/m9-feedback-inline-mpp-probe/baseline.json`
- `target/benchmarks/m9-feedback-inline-mpp-compare/compare.json`

These rows use the `non-primary-report-only` threshold class, are not selected by `just bench::baseline --primary` or `just bench::compare --primary`, and were not added to `benchmarks/m12-primary-thresholds.json`.

Fresh probe rates from the current manifest:

- `m9-detecting-regions-basic-batch`: `1.473e6 cases/s` and `2.941e6 regions/s`.
- `m9-missing-detectors-basic-batch`: `1.692e7 cases/s` and `6.770e6 suggestions/s`.
- `m9-feedback-inline-mpp-batch`: `8.380e5 transforms/s`.

## Audit And Review

Milestone-audit initially found three blocking or partial-evidence issues, all fixed before completion:

- MPP feedback parity was overclaimed because sparse reverse tracking did not implement generic `MPP`; fixed by adding Hermitian MPP product undo support and anti-Hermitian rejection tests.
- Detecting-regions acceptance lacked a false-mode anticommutation test and accepted unsupported `CX` target shapes; fixed by adding an anticommutation regression plus explicit plain-qubit target-shape validation for the scoped API.
- The completion report still contained a placeholder requiring final verification; fixed by replacing it with this audit and verification summary.

Full-code-review sidecars found three additional issues, all fixed before completion:

- The new utility benchmark rows were marked `report-only` but still selected by `--primary`; fixed with the `non-primary-report-only` threshold class and primary-selection tests.
- `circuit_detecting_regions` accepted feedback-controlled and sweep-controlled `CX` shapes; fixed with explicit target validation and negative tests.
- At the time of this M9 slice, `circuit_with_inlined_feedback` failed open for unsupported classical controlled gates and could expand repeat blocks without a transform-specific budget; fixed in that slice by rejecting unsupported classical controlled feedback gates and repeat blocks in the scoped transform. Later PF2 feedback slices replaced the broad repeat-block rejection with selected bounded repeat-loop refolding, selected nested bounded-repeat detector-parity preservation, and an explicit repeat-work preflight.

A final GPT-5.5/xhigh full-code-review pass found four more compatibility and resource issues, all fixed before this report was finalized:

- Detecting-region extraction did not check anticommutation against the implicit initial Z state; fixed by adding sparse reverse tracker start-state finalization and a regression for `TICK; MXX; DETECTOR`.
- Detecting-region extraction materialized identity regions at requested ticks; fixed by omitting identity snapshots from the output map.
- Basic missing-detectors emitted suggestions for nondeterministic measurements and treated multi-record detector rows as independent coverage; fixed by modeling known-input Z determinism, suppressing nondeterministic suggestions, adding duplicate-target parity coverage, and failing closed on multi-record detector rows until row-reduction is implemented.
- MPP target normalization used a linear remove/insert loop per target; fixed by moving the helper into a submodule and normalizing with a map-backed parity accumulator.

Final milestone status: complete for the scoped plan, with broader detector-analysis and transform parity recorded as future work below.

## Verification Evidence

Passed during implementation:

```sh
cargo test -p stab-core detecting_regions --quiet
cargo test -p stab-core missing_detectors --quiet
cargo test -p stab-core circuit_with_inlined_feedback --quiet
cargo test -p stab-oracle fixtures --quiet
just oracle::run --milestone M9
cargo test -p stab-bench m9 --quiet
just bench::smoke
just bench::baseline --only m9-detecting-regions-basic-batch --out target/benchmarks/m9-detecting-regions-basic-probe
just bench::compare --only m9-detecting-regions-basic-batch --baseline target/benchmarks/m9-detecting-regions-basic-probe/baseline.json --report target/benchmarks/m9-detecting-regions-basic-compare
just bench::baseline --only m9-missing-detectors-basic-batch --out target/benchmarks/m9-missing-detectors-basic-probe
just bench::compare --only m9-missing-detectors-basic-batch --baseline target/benchmarks/m9-missing-detectors-basic-probe/baseline.json --report target/benchmarks/m9-missing-detectors-basic-compare
just bench::baseline --only m9-feedback-inline-mpp-batch --out target/benchmarks/m9-feedback-inline-mpp-probe
just bench::compare --only m9-feedback-inline-mpp-batch --baseline target/benchmarks/m9-feedback-inline-mpp-probe/baseline.json --report target/benchmarks/m9-feedback-inline-mpp-compare
```

Final verification:

```sh
cargo fmt --all --check
cargo clippy -p stab-core -p stab-cli -p stab-oracle -p stab-bench --all-targets -- -D warnings
cargo test -p stab-core detecting_regions --quiet
cargo test -p stab-core missing_detectors --quiet
cargo test -p stab-core circuit_with_inlined_feedback --quiet
cargo test -p stab-cli m2d --quiet
cargo test -p stab-oracle fixtures --quiet
cargo test -p stab-bench m9 --quiet
cargo test -p stab-bench primary --quiet
cargo test --workspace --quiet
just oracle::run --milestone M9
just oracle::run --implemented-only
just bench::smoke
just maintenance::pre-commit
```

The manual pre-commit hook reported no staged changes, which is expected because completion work remained unstaged at report time.

## Remaining Exclusions After Later PF2 Feedback Work

- Full public `Circuit.with_inlined_feedback` parity.
- Broader repeat-contained feedback beyond the selected bounded loop-refolding and nested bounded-repeat detector-parity cases.
- Unsupported classical feedback gates.
- Full MPP stabilizer-product missing-detector analysis.
- Multi-record missing-detector row reduction and deterministic invariant solving.
- Observable-interaction missing-detector analysis.
- Honeycomb-code and toric-code missing-detector suffix analysis.
- Broader detecting-region target-shape support and broader gauge handling.
- Sweep-aware `detect`, Python bindings, JS/WASM, diagrams, `explain_errors`, `repl`, QASM, Quirk, GPU, and public graph/vector simulator APIs.
- Deprecated `--detector_hypergraph` support.
