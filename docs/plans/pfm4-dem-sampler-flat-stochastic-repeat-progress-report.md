# PFM4 DEM Sampler Flat-Stochastic Repeat Progress Report

## Scope

This PFM4 slice owns one folded DEM sampler subcase: direct detection-event sampling of a zero-shift repeat body whose compiled sampler body is a flat list of `error(p)` operations.
The selected behavior folds each repeated error operation independently by sampling the odd-parity probability for that error over the outer repeat count, then applies each error target set once when the folded parity event occurs.

The selected behavior is semantic and statistical equivalence, not exact random-stream parity.

## Explicit Non-Scope

This slice does not change sampled-error output, sampled-error replay, shifted repeated bodies, nested repeated bodies, repeat bodies containing non-error compiled sampler operations, graphlike search, hypergraph search, SAT/WCNF generation, analyzer traversal, ErrorMatcher traversal, Python APIs, diagrams, or deferred simulator-product surfaces.

Sampled-error output and replay still use flat repeated error-bit order and their existing materialized width caps because Stim-compatible sampled-error records expose one bit per repeated error occurrence.
At the time of this flat-stochastic slice, nested, shifted, and otherwise non-selected stochastic repeated bodies still used the existing sampled-error application work cap.
The later [pfm4-dem-sampler-nested-stochastic-repeat-progress-report.md](pfm4-dem-sampler-nested-stochastic-repeat-progress-report.md) promotes selected nested zero-shift stochastic bodies; shifted and otherwise non-selected stochastic bodies remain capped.

## Comparator And Evidence Plan

Comparator class: statistical and semantic Rust parity for selected flat stochastic DEM repeat bodies, plus resource-boundary evidence that direct detection-event sampling no longer fails the sampled-error application work cap for that selected shape.

## Implemented Surface

`CompiledDemSampler` now recognizes zero-shift repeat bodies whose compiled sampler body contains only error operations.
At compile time, it caches a folded error list whose probabilities are each error's odd-parity probability across the outer repeat count.
When direct detection-event sampling reaches that shape, it samples each cached folded error once and applies the error targets for each folded parity event that occurs.

The direct-sampling work validator uses the same selected-shape recognition, counting only the flat body errors instead of every repeated occurrence.
Sampled-error output and replay still walk the flat error-bit path.

## Tests

Implemented test coverage:

- Add direct detection-event sampling coverage for a huge zero-shift repeated flat body with multiple stochastic and deterministic errors above the previous work cap.
- Check observed detector and observable frequencies against the closed-form independent odd-parity probabilities with fixed seeds and tolerances, including a non-saturated tiny-probability error and a saturated error in the same flat body.
- Keep sampled-error output caps for the same repeated flat stochastic shape.
- Keep nested stochastic repeat bodies capped as the non-selected shape for this flat-stochastic slice.

Concrete test functions:

- `pf4_dem_sampler_flat_stochastic_repeat_folds_independent_error_parities`
- `pf4_dem_sampler_folded_repeat_sampling_and_materialized_error_caps`

## Oracle And Benchmark Evidence

- Update implemented oracle row `pf4-dem-sampler-repeat-resource-rust`.
- Extend non-primary report-only benchmark row `pf4-dem-sampler-folded-repeat` with a flat stochastic parity-repeat submeasurement and measurement work units.
- Keep the row out of the primary 1.25x gate because it remains a Rust API contract workload and not a faithful pinned-Stim ratio.

New submeasurement:

- `stab_pf4_dem_sampler_sample_flat_stochastic_parity_repeat`

## Documentation

- Update the feature checklist, partial-feature inventory, active PFM4 plan, roadmap, RPF4 sampler report, prior single-stochastic report, oracle metadata, and benchmark metadata without claiming full folded stochastic traversal.

## Verification

Focused checks run during implementation:

```sh
cargo fmt --all --check
cargo test -p stab-core --test dem_sampler pf4_dem_sampler_flat_stochastic_repeat_folds_independent_error_parities --quiet
cargo test -p stab-core --test dem_sampler pf4_dem_sampler_ --quiet
cargo test -p stab-core --test dem_sampler --quiet
cargo test -p stab-bench pf4_dem_transform_benchmark_rows_have_stab_compare_runners --quiet
cargo test -p stab-bench --quiet
cargo clippy -p stab-core -p stab-bench --all-targets -- -D warnings
cargo test -p stab-oracle fixtures --quiet
just oracle::run --milestone PF4
just bench::list
just bench::smoke
git diff --check
just bench::baseline --only pf4-dem-sampler-folded-repeat --out target/benchmarks/pfm4-dem-sampler-flat-stochastic-baseline
just bench::compare --only pf4-dem-sampler-folded-repeat --baseline target/benchmarks/pfm4-dem-sampler-flat-stochastic-baseline/baseline.json --report target/benchmarks/pfm4-dem-sampler-flat-stochastic-compare
```

The focused compare report measured `stab_pf4_dem_sampler_sample_flat_stochastic_parity_repeat=0.000002890s`, normalizing to approximately `4.252e15 folded-flat-stochastic-error-occurrences/s`, and kept the row `contract-only` with no pinned-Stim timing ratio claim.

## Audit And Review Closure

Milestone-audit status: complete for this selected PFM4 slice.
The audit found the selected direct detection-event flat stochastic zero-shift sampler subcase implemented with direct statistical tests, distinct per-error parity-probability evidence, sampled-error cap preservation, nested-repeat cap preservation for that slice, oracle metadata, report-only benchmark runner coverage, and synchronized docs.
It did not mark the broader PFM4 folded-traversal milestone complete because sampled-error output and replay, shifted stochastic bodies, graphlike search, hypergraph search, SAT/WCNF generation, analyzer traversal, and ErrorMatcher traversal remain scoped separately.
No milestone under-specification issue needed a new spec-gap log entry for this slice because the non-scope and remaining work are explicit.

Full-code-review status: complete after GPT-5.5/xhigh sidecar review.
The core review found a P2 performance issue because the first implementation recomputed flat-body recognition and odd-parity probabilities per shot; the implementation now caches folded per-error probabilities during sampler compilation.
The core review also found misleading detector-only wording for a path that emits detector and observable records; the documentation and metadata now use direct detection-event scope wording.
The docs and metadata review found that the original flat-stochastic test only proved saturated 0.5 folded probabilities, missing closure evidence in this report, and stale wording in the prior deterministic and single-stochastic reports.
The test now includes a tiny-probability plus saturated-probability flat body, this report records verification and audit/review closure, and prior reports now distinguish their historical scope from the promoted flat-stochastic slice.

Final pre-commit verification: `just maintenance::pre-commit` passed after staging this change set.
