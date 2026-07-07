# RPF4 DEM Sampler Progress Report

## Summary

This RPF4 slice records source-owned evidence for folded DEM sampler compilation and direct detector sampling across repeat blocks.
It is not an RPF4 completion report because sampled-error records still use Stim-compatible flat error occurrence order and therefore retain materialized width caps, and because graphlike, hypergraph, SAT, matcher-adjacent, and analyzer-adjacent traversal consumers still have active folded-traversal work beyond the implemented graphlike and hypergraph zero-probability repeat skip, weighted SAT/WCNF zero-probability variable elision and repeated-body skip, and selected SAT/WCNF flat and nested zero-shift repeat folding.

## Implemented Surfaces

- `CompiledDemSampler::compile` now stores a folded operation tree instead of unrolling every repeated DEM error into the compiled operation list.
- Direct detection-event sampling walks the folded operation tree directly and no longer allocates a flat sampled-error record internally.
- Direct detection-event sampling skips folded repeat bodies whose errors all have zero probability, so huge no-op repeats no longer consume sampled-error application work.
- Direct detection-event sampling folds deterministic zero-shift repeat bodies by parity, so huge odd counts apply the body once and huge even counts apply no net effect.
- Direct detection-event sampling folds the selected single-stochastic zero-shift repeat body by odd-parity probability, so that shape no longer consumes one sampled-error application per repeated occurrence.
- Direct detection-event sampling folds selected flat stochastic zero-shift repeat bodies by per-error odd-parity probability, so flat multi-error shapes no longer consume one sampled-error application per repeated occurrence.
- Flat sampled-error output and replay preserve the existing public error-bit order, including repeated errors, through the folded operation tree.
- Non-selected direct detection-event stochastic sampling and sampled-error sampling reject per-shot repeated error work above the current sampled-error application limit before walking the folded tree.
- Materialized sampled-error APIs and sampled-error streaming still reject per-shot flat error records that exceed the existing buffer limits before allocating the record.
- Programmatic repeat nesting above the shared DEM nesting limit is still rejected with a domain error.

## Tests

Implemented Rust test:

- `pf4_dem_sampler_compiles_repeats_without_flat_operation_cap`
- `pf4_dem_sampler_preserves_flat_error_order_through_nested_repeats`
- `pf4_dem_sampler_deterministic_repeat_folding_preserves_rng_and_error_order`
- `pf4_dem_sampler_single_stochastic_repeat_folds_by_parity_distribution`
- `pf4_dem_sampler_flat_stochastic_repeat_folds_independent_error_parities`
- `pf4_dem_sampler_folded_repeat_sampling_and_materialized_error_caps`
- `pf4_dem_sampler_rejects_excessive_buffered_outputs_before_sampling`
- `pf4_dem_sampler_rejects_programmatic_deep_repeat_nesting`

These tests cover folded compilation past the previous repeat-count and expanded-iteration caps, shifted repeated detector sampling, observable parity through repeated errors, flat error-bit order and replay through nested repeats, direct detection-event zero-probability repeat skipping, deterministic zero-shift parity folding above the previous work cap, selected single-stochastic zero-shift parity folding above the previous work cap including tiny-probability and near-one-probability regressions, selected flat stochastic zero-shift parity folding above the previous work cap, non-selected nested stochastic sampled-work cap enforcement, sampled-error streaming and materialized buffer cap enforcement, and deep repeat-nesting rejection.

## Oracle Rows

Implemented row:

- `pf4-dem-sampler-repeat-resource-rust`

Still broad and manifest-only:

- `pf4-dem-folded-traversal`

## Benchmark Rows

Report-only runner coverage:

- `pf4-dem-sampler-folded-repeat`

The row measures folded `CompiledDemSampler` compile, stochastic direct sample behavior, zero-probability repeat skipping, deterministic zero-shift repeat parity folding, selected direct detection-event single-stochastic zero-shift repeat parity folding, and selected direct detection-event flat stochastic zero-shift repeat parity folding while sampled-error materialization, replay, and non-selected excessive stochastic repeated-error work remain capped.
It remains `non-primary-report-only` because it is a Rust public API contract workload and because broad PF4 traversal consumers still need folded or explicitly capped treatment.
It is not part of the 1.25x primary threshold file.

## Verification Evidence

Target checks for this slice:

```sh
cargo test -p stab-core --test dem_sampler pf4_dem_sampler_ --quiet
cargo test -p stab-core --test dem_sampler dem_sampler_ --quiet
cargo test -p stab-bench pf4_dem_transform_benchmark_rows_have_stab_compare_runners --quiet
cargo test -p stab-bench --quiet
cargo test -p stab-oracle fixtures --quiet
cargo clippy -p stab-core -p stab-bench --all-targets -- -D warnings
just oracle::run --milestone PF4
just bench::smoke
```

## Remaining RPF4 Work

- Optimize folded DEM sampler execution for nested or shifted repeated stochastic nonzero-probability bodies whose dense detector outputs do not require per-occurrence work, then tighten or remove the current direct-sampling work cap for those selected shapes without changing the flat sampled-error record contract locked in `docs/plans/pfm4-dem-sampler-error-bit-cap-evidence-lock.md`.
- Finish folded traversal or explicit caps for graphlike search, hypergraph search, SAT or WCNF encoding, matcher-adjacent operations, and analyzer-adjacent operations beyond the current graphlike and hypergraph zero-probability repeat skip, weighted SAT/WCNF zero-probability variable elision and repeated-body skip, and selected SAT/WCNF flat and nested zero-shift repeat folding.
- Keep benchmark runners for `pf4-dem-folded-traversal` and `pf4-dem-folded-graphlike-traversal` synchronized when additional implementation or explicit cap behavior becomes source-owned enough to measure honestly.
