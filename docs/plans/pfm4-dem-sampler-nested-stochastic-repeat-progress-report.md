# PFM4 DEM Sampler Nested Stochastic Repeat Progress Report

## Summary

This PFM4 slice promotes selected nested zero-shift stochastic DEM repeats in direct detection-event sampling.
`CompiledDemSampler` already used a folded operation tree and folded selected flat zero-shift stochastic repeat bodies by odd-parity probability.
This slice extends that direct-sampling fold through nested zero-shift repeat bodies by recursively summarizing one body pass into independent effective error toggles, then applying the outer repeat's odd-parity probability to those toggles.

Sampled-error output and replay are intentionally unchanged.
They continue to expose Stim-compatible flat repeated error-bit records, so materialized sampled-error records and replay records still use the source-owned width caps locked in [pfm4-dem-sampler-error-bit-cap-evidence-lock.md](pfm4-dem-sampler-error-bit-cap-evidence-lock.md).

## Scope

Included:

- Direct detection-event sampling with `SampledErrorOutput::Discard`.
- Nested repeat bodies whose detector shift is zero at every folded repeat level.
- Mixed deterministic and stochastic error operations inside the selected zero-shift nested body.
- Existing flat sampled-error output and replay caps.
- Existing shifted stochastic direct-sampling cap.

Excluded:

- Shifted stochastic repeat bodies where repeated error occurrences target different detector offsets.
- Sampled-error materialization or replay compression.
- Graphlike, hypergraph, SAT, analyzer, and matcher traversal consumers outside the sampler.
- Python API parity and exact random-stream parity.

## Implementation Evidence

- `crates/stab-core/src/dem_sampler.rs` now computes `folded_zero_shift_errors` for repeat operations whose body can be summarized without detector-shift drift.
- `collect_zero_shift_effect_errors` recursively summarizes nested zero-shift body effects and shifts nested detector ids by the nested repeat start offset.
- `folded_direct_sample_repeat_work_count` counts the folded summary length instead of multiplying by the outer repeat count for selected nested zero-shift stochastic bodies.
- Direct sampling still falls back to the explicit work cap when a body has detector-shift drift or cannot be summarized.

## Tests

Source-owned tests:

- `cargo test -p stab-core --test dem_sampler pf4_dem_sampler_ --quiet`
- `pf4_dem_sampler_nested_stochastic_repeat_folds_independent_error_parities` proves detector and observable parity distributions for a selected nested zero-shift stochastic repeat above the previous sampled-error application work cap.
- `pf4_dem_sampler_folded_repeat_sampling_and_materialized_error_caps` now proves a huge selected nested zero-shift stochastic repeat samples successfully without hitting the sampled-error application work cap.
- The same test still proves shifted stochastic direct sampling rejects before allocating output.
- `pf4_dem_sampler_rejects_excessive_buffered_outputs_before_sampling` continues to prove materialized sampled-error and replay buffer caps.

Oracle metadata:

- `pf4-dem-sampler-repeat-resource-rust` selects `cargo test -p stab-core --test dem_sampler pf4_dem_sampler_` and now names selected nested zero-shift stochastic parity folding.

## Benchmarks

Report-only benchmark metadata:

- `pf4-dem-sampler-folded-repeat` now includes `stab_pf4_dem_sampler_sample_nested_stochastic_parity_repeat`.
- Measurement work is reported as folded nested stochastic error occurrences per second.
- The row remains `non-primary-report-only` and `contract-only` because it is Rust API contract evidence without a faithful direct pinned-Stim baseline.

## Remaining Work

Shifted stochastic repeated direct detection-event sampling remains capped because repeated occurrences target different detector offsets and therefore cannot be collapsed to one parity toggle without changing the dense output work shape.
Broader graphlike, hypergraph, SAT, analyzer, and matcher traversal consumers remain owned by the larger PFM4 and PFM6 folded-traversal work.

## Verification

```sh
cargo test -p stab-core --test dem_sampler pf4_dem_sampler_ --quiet
cargo test -p stab-bench pf4_dem_transform_benchmark_rows_have_stab_compare_runners --quiet
just bench::list
just oracle::list --milestone PF4
```
