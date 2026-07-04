# RPF4 DEM Sampler Progress Report

## Summary

This RPF4 slice records source-owned evidence for the current capped DEM sampler repeat behavior.
It is not an RPF4 completion report because true folded sampler traversal and the graphlike, hypergraph, SAT, matcher-adjacent, and analyzer-adjacent traversal consumers remain active work.

## Implemented Surfaces

- `CompiledDemSampler::compile` validates DEM repeat expansion before counting detectors or compiling operations.
- Repeat counts above 100,000 are rejected with a domain error.
- Nested repeat expansion above 1,000,000 repeat iterations is rejected with a domain error.
- Allowed shifted repeat blocks compile and sample correctly with detector shifts and observable parity.

## Tests

Implemented Rust test:

- `pf4_dem_sampler_repeat_caps_and_allowed_shifted_repeat_sampling`

This test covers allowed shifted repeat sampling, excessive repeat-count rejection, and nested repeat-expansion rejection.

## Oracle Rows

Implemented row:

- `pf4-dem-sampler-repeat-resource-rust`

Still broad and manifest-only:

- `pf4-dem-folded-traversal`

## Benchmark Rows

Report-only runner coverage:

- `pf4-dem-sampler-folded-repeat`

The row measures current capped-repeat `CompiledDemSampler` compile and sample behavior.
It remains `non-primary-report-only` because it is a Rust public API contract workload and because true folded sampler traversal is not implemented.
It is not part of the 1.25x primary threshold file.

## Verification Evidence

Target checks for this slice:

```sh
cargo test -p stab-core --test dem_sampler pf4_dem_sampler_ --quiet
cargo test -p stab-bench pf4_dem_transform_benchmark_rows_have_stab_compare_runners --quiet
cargo test -p stab-bench --quiet
cargo test -p stab-oracle fixtures --quiet
cargo clippy -p stab-core -p stab-bench --all-targets -- -D warnings
just oracle::run --milestone PF4
just bench::smoke
```

## Remaining RPF4 Work

- Decide whether `CompiledDemSampler` should gain true folded repeat traversal instead of bounded unrolling.
- Finish folded traversal or explicit caps for graphlike search, hypergraph search, SAT or WCNF encoding, matcher-adjacent operations, and analyzer-adjacent operations.
- Add benchmark runners for the remaining `pf4-dem-folded-traversal` and `pf4-dem-folded-graphlike-traversal` rows only when their implementation or explicit cap behavior is source-owned enough to measure honestly.
