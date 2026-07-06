# PFM4 DEM Sampler Deterministic Repeat Progress Report

## Scope

This PFM4 slice owns one folded DEM sampler subcase: detector-only direct sampling of repeated DEM bodies whose nonzero-probability effects are deterministic and whose body has zero net detector shift.

The selected behavior is parity folding for direct detector and observable output:

- Even repeat counts of a deterministic zero-shift body have no net effect.
- Odd repeat counts apply the body once.
- Zero-probability errors inside the body remain no-ops.
- The flat sampled-error count and flat sampled-error order remain unchanged for sampled-error output and replay.

## Explicit Non-Scope

This deterministic-repeat slice did not change sampled-error output, sampled-error replay, stochastic repeated bodies, shifted repeated bodies, graphlike search, hypergraph search, SAT/WCNF generation, analyzer traversal, ErrorMatcher traversal, Python APIs, diagrams, or any deferred simulator-product surface.

The later `docs/plans/pfm4-dem-sampler-single-stochastic-repeat-progress-report.md` promotes one selected detector-only single-stochastic zero-shift repeat shape; broader stochastic repeated bodies still use the existing sampled-error application work cap.
Sampled-error output and replay still use the existing flat sampled-error width caps because Stim-compatible sampled-error records expose one bit per repeated error occurrence.

## Comparator And Evidence Plan

Comparator class: structural Rust parity with Stim semantics for deterministic DEM errors, plus resource-boundary evidence that the previous sampled-error application cap no longer applies to the selected detector-only direct sampling subcase.

## Implemented Surface

`CompiledDemSampler` now tracks whether a compiled folded block contains stochastic direct-sampling errors.
When detector-only direct sampling reaches a repeat body with zero net detector shift and no stochastic errors, it applies parity folding:

- It skips even repeat counts.
- It samples the body once for odd repeat counts.
- At the time of this deterministic slice, it kept the normal folded traversal for stochastic bodies, shifted bodies, sampled-error output, and sampled-error replay; the later single-stochastic report promotes one selected detector-only stochastic shape.

The direct-sampling work validator uses the same folded parity work count, so huge deterministic zero-shift repeats no longer fail the stochastic sampled-error application work cap.

## Tests

Implemented test coverage:

- Extend `cargo test -p stab-core --test dem_sampler pf4_dem_sampler_ --quiet` with deterministic zero-shift odd and even repeat cases above the previous work cap.
- Compare a small folded deterministic repeat followed by a stochastic error against its expanded semantic equivalent, proving the fold does not consume RNG or shift later random draws.
- Keep small sampled-error output coverage for deterministic zero-shift repeats, proving the flat repeated error-bit order remains unchanged.
- Keep sampled-error materialization and replay cap assertions for the same flat repeated error-bit surface.

Concrete test function:

- `pf4_dem_sampler_deterministic_repeat_folding_preserves_rng_and_error_order`
- `pf4_dem_sampler_folded_repeat_sampling_and_materialized_error_caps`

## Oracle Rows

Updated implemented row:

- `pf4-dem-sampler-repeat-resource-rust`

The row remains structural and runs `cargo test -p stab-core --test dem_sampler pf4_dem_sampler_`, now including deterministic zero-shift parity folding, sampled-error flat-order preservation, stochastic repeat cap preservation, and sampled-error materialization cap preservation.

## Benchmarks

- Extend non-primary report-only row `pf4-dem-sampler-folded-repeat` with a deterministic parity-repeat submeasurement and measurement work units.
- Keep the row out of the primary 1.25x gate because it remains a Rust API contract workload and not a faithful pinned-Stim ratio.

New submeasurement:

- `stab_pf4_dem_sampler_sample_deterministic_parity_repeat`

## Documentation

- Update the feature checklist, partial-feature inventory, and prior RPF4 DEM sampler report so the selected deterministic-repeat folded behavior is recorded without claiming full folded traversal.

Updated documents:

- `docs/stab-feature-checklist.md`
- `docs/plans/partial-feature-inventory.md`
- `docs/plans/rpf4-dem-sampler-progress-report.md`
- `docs/plans/rust-stim-drop-in-rewrite.md`

## Verification

Focused checks run during implementation:

```sh
cargo test -p stab-core --test dem_sampler pf4_dem_sampler_ --quiet
cargo test -p stab-core --test dem_sampler --quiet
cargo test -p stab-bench pf4_dem_transform_benchmark_rows_have_stab_compare_runners --quiet
cargo test -p stab-bench --quiet
cargo clippy -p stab-core -p stab-bench --all-targets -- -D warnings
cargo test -p stab-bench runner_smoke --quiet
cargo test -p stab-oracle fixtures --quiet
just oracle::run --milestone PF4
just bench::list
just bench::smoke
just bench::baseline --only pf4-dem-sampler-folded-repeat --out target/benchmarks/pfm4-dem-sampler-deterministic-baseline
just bench::compare --only pf4-dem-sampler-folded-repeat --baseline target/benchmarks/pfm4-dem-sampler-deterministic-baseline/baseline.json --report target/benchmarks/pfm4-dem-sampler-deterministic-compare
just maintenance::pre-commit
```

The focused compare report measured `stab_pf4_dem_sampler_sample_deterministic_parity_repeat=0.000002780s`, normalizing to approximately `1.473e15 folded-deterministic-error-occurrences/s`, and kept the row `contract-only` with no pinned-Stim timing ratio claim.

## Audit And Review Closure

Milestone-audit status: complete for this PFM4 slice.
The audit found the selected deterministic zero-shift sampler subcase implemented with direct tests, oracle metadata, report-only benchmark metadata, benchmark runner coverage, and synchronized docs.
It did not mark the broader PFM4 folded-traversal milestone complete because stochastic repeated bodies, sampled-error output and replay flat-width limits, graphlike and hypergraph search, SAT/WCNF generation, analyzer traversal, and ErrorMatcher traversal remained scoped separately at the time of that slice.

Full-code-review status: complete after GPT-5.5/xhigh sidecar review.
The core review reported no blocking findings and noted residual evidence risks for RNG preservation and sampled-error ordering; those were addressed by `pf4_dem_sampler_deterministic_repeat_folding_preserves_rng_and_error_order`.
The docs and metadata review found that the report needed durable oracle-row and audit/review closure evidence; this report now names `pf4-dem-sampler-repeat-resource-rust` and records the audit and review closure.
