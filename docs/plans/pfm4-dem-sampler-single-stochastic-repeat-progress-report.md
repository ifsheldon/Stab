# PFM4 DEM Sampler Single-Stochastic Repeat Progress Report

## Scope

This PFM4 slice owns one folded DEM sampler subcase: direct detection-event sampling of a zero-shift repeat body containing exactly one compiled stochastic `error(p)` sampler operation and no other compiled sampler operations.

The selected behavior is parity-distribution folding:

- A repeated single stochastic error with probability `p` is sampled once using the odd-parity probability `(1 - (1 - 2p)^repeat_count) / 2`.
- Detector and observable targets are the same targets as the repeated error.
- The behavior is semantic and statistical equivalence, not exact random-stream parity.

## Explicit Non-Scope

This slice does not change sampled-error output, sampled-error replay, shifted repeated bodies, repeat bodies with multiple stochastic error effects, repeat bodies with a mix of stochastic and deterministic sampling effects, graphlike search, hypergraph search, SAT/WCNF generation, analyzer traversal, ErrorMatcher traversal, Python APIs, diagrams, or deferred simulator-product surfaces.

Sampled-error output and replay still use flat repeated error-bit order and their existing materialized width caps because Stim-compatible sampled-error records expose one bit per repeated error occurrence.
Non-selected stochastic repeated bodies still use the existing sampled-error application work cap.

## Comparator And Evidence Plan

Comparator class: statistical and semantic Rust parity for the selected stochastic DEM repeat subcase, plus resource-boundary evidence that direct detection-event sampling no longer fails the sampled-error application work cap for this selected shape.

## Implemented Surface

`CompiledDemSampler` now recognizes zero-shift repeat bodies that contain exactly one compiled stochastic `error(p)` sampler operation.
When direct detection-event sampling reaches that shape, it samples the repeated body once using the odd-parity probability and applies the repeated error targets if the folded parity event occurs.

The direct-sampling work validator uses the same selected-shape recognition, so a huge selected direct detection-event single-stochastic zero-shift repeat no longer fails the sampled-error application work cap.
Sampled-error output and replay still walk the flat error-bit path.
At the time of this single-stochastic slice, repeat bodies with multiple operations, shifted bodies, and mixed stochastic/deterministic bodies still used the existing capped traversal.
The later `docs/plans/pfm4-dem-sampler-flat-stochastic-repeat-progress-report.md` promotes selected direct detection-event flat stochastic zero-shift repeat bodies with multiple error operations, and `docs/plans/pfm4-dem-sampler-nested-stochastic-repeat-progress-report.md` promotes selected nested zero-shift stochastic repeat bodies.

## Tests

Implemented test coverage:

- Add direct detection-event sampling coverage for huge zero-shift repeated single stochastic errors above the previous work cap.
- Check the observed detector and observable parity frequency against the closed-form odd-parity probability with fixed seeds and tolerances, including tiny-probability and near-one-probability regressions.
- Keep sampled-error output caps for the same repeated stochastic error shape.
- At the time of this slice, keep non-selected mixed stochastic repeat bodies capped; later flat and nested stochastic slices promote selected zero-shift shapes while shifted and otherwise non-selected stochastic bodies remain capped.

Concrete test functions:

- `odd_parity_probability_matches_repeated_independent_error_parity`
- `pf4_dem_sampler_single_stochastic_repeat_folds_by_parity_distribution`
- `pf4_dem_sampler_folded_repeat_sampling_and_materialized_error_caps`

## Oracle Rows

Updated implemented row:

- `pf4-dem-sampler-repeat-resource-rust`

The row remains structural and runs `cargo test -p stab-core --test dem_sampler pf4_dem_sampler_`, now including selected direct detection-event single-stochastic zero-shift parity folding and non-selected stochastic cap preservation.

## Benchmarks

- Extend non-primary report-only row `pf4-dem-sampler-folded-repeat` with a selected direct detection-event single-stochastic parity-repeat submeasurement and measurement work units.
- Keep the row out of the primary 1.25x gate because it remains a Rust API contract workload and not a faithful pinned-Stim ratio.

New submeasurement:

- `stab_pf4_dem_sampler_sample_single_stochastic_parity_repeat`

## Documentation

- Update the feature checklist, partial-feature inventory, active PFM4 plan, roadmap, prior PFM4/RPF4 sampler reports, oracle metadata, and benchmark metadata without claiming full folded stochastic traversal.

Updated documents:

- `docs/stab-feature-checklist.md`
- `docs/plans/non-deferred-partial-feature-milestones.md`
- `docs/plans/partial-feature-inventory.md`
- `docs/plans/rust-stim-drop-in-rewrite.md`
- `docs/plans/rpf4-dem-sampler-progress-report.md`
- `docs/plans/pfm4-dem-sampler-deterministic-repeat-progress-report.md`

## Verification

Focused checks run during implementation:

```sh
cargo test -p stab-core --test dem_sampler pf4_dem_sampler_ --quiet
cargo test -p stab-core --test dem_sampler --quiet
cargo test -p stab-core dem_sampler::tests::odd_parity_probability_matches_repeated_independent_error_parity --quiet
cargo fmt --all --check
cargo test -p stab-bench pf4_dem_transform_benchmark_rows_have_stab_compare_runners --quiet
cargo test -p stab-bench runner_smoke --quiet
cargo test -p stab-bench --quiet
cargo clippy -p stab-core -p stab-bench --all-targets -- -D warnings
cargo test -p stab-oracle fixtures --quiet
just oracle::run --milestone PF4
just bench::list
just bench::smoke
git diff --check
just bench::baseline --only pf4-dem-sampler-folded-repeat --out target/benchmarks/pfm4-dem-sampler-single-stochastic-baseline
just bench::compare --only pf4-dem-sampler-folded-repeat --baseline target/benchmarks/pfm4-dem-sampler-single-stochastic-baseline/baseline.json --report target/benchmarks/pfm4-dem-sampler-single-stochastic-compare
just maintenance::pre-commit
```

The focused compare report measured `stab_pf4_dem_sampler_sample_single_stochastic_parity_repeat=0.000003010s`, normalizing to approximately `1.361e15 folded-stochastic-error-occurrences/s`, and kept the row `contract-only` with no pinned-Stim timing ratio claim.

## Audit And Review Closure

Milestone-audit status: complete for this selected PFM4 slice.
The audit found the selected direct detection-event single-stochastic zero-shift sampler subcase implemented with direct statistical tests, tiny-probability and near-one-probability numerical regressions, oracle metadata, report-only benchmark runner coverage, and synchronized docs.
At the time of this slice, it did not mark the broader PFM4 folded-traversal milestone complete because sampled-error output and replay, shifted stochastic bodies, multi-stochastic bodies, mixed bodies, graphlike search, hypergraph search, SAT/WCNF generation, analyzer traversal, and ErrorMatcher traversal remained scoped separately; the later flat-stochastic report promotes the selected direct detection-event flat multi-error shape.
No milestone under-specification issue needed a new spec-gap log entry for this slice because the non-scope and remaining work are now explicit.

Full-code-review status: complete after GPT-5.5/xhigh sidecar review.
The core review found a P1 numerical stability issue in the first parity formula for tiny probabilities; the implementation now uses `ln_1p` and `exp_m1`, with tiny-probability and near-one-probability regressions.
The docs and metadata review found missing closure evidence, overbroad metadata wording, and a benchmark submeasurement that consumed only record counts; the report now records audit/review closure, metadata names the selected direct detection-event scope, and PF4 sampler benchmark submeasurements consume sampled detector and observable bits.

Final pre-commit verification: `just maintenance::pre-commit` passed after staging this change set.
