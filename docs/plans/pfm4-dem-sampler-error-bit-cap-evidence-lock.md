# PFM4 DEM Sampler Error-Bit Cap Evidence Lock

## Summary

This PFM4 evidence-lock slice clarifies the current `CompiledDemSampler` boundary for sampled-error output and replay.
Direct detection-event sampling has folded execution for the selected zero-shift repeat shapes already documented in the sampler progress reports, but sampled-error output and replay expose Stim-compatible flat error-bit records where each repeated `error` occurrence owns a distinct public bit.
Because that public record shape is observable, Stab must preserve flat repeated error-bit order instead of folding those bits away.

## Locked Contract

- `sample_detection_events_and_errors_with_seed` preserves flat sampled-error record order, including repeated errors, and rejects materialized or streamed sampled-error records that exceed the documented buffer limits before allocating or walking the oversized record.
- `sample_detection_events_from_error_records` replays flat error-bit records against the folded operation tree while preserving the same public bit order.
- Direct detection-event sampling remains free to fold selected repeated stochastic, deterministic, nested zero-shift, and zero-probability bodies because it does not expose per-error occurrence bits.
- Non-selected shifted repeated stochastic direct-sampling bodies still reject excessive sampled-error application work before walking the folded tree.

## Evidence

Source evidence:

- `crates/stab-core/src/dem_sampler.rs` validates sampled-error materialization by `error_count()` for materialized and streaming sampled-error output.
- `crates/stab-core/src/dem_sampler.rs` uses folded direct detection-event branches only when sampled-error output is discarded.
- `crates/stab-core/tests/dem_sampler.rs` has `pf4_dem_sampler_preserves_flat_error_order_through_nested_repeats`, proving sampled-error order and replay through nested repeats.
- `crates/stab-core/tests/dem_sampler.rs` has `pf4_dem_sampler_deterministic_repeat_folding_preserves_rng_and_error_order`, proving repeated deterministic sampled-error bit order is preserved.
- `crates/stab-core/tests/dem_sampler.rs` has `pf4_dem_sampler_flat_stochastic_repeat_folds_independent_error_parities`, proving the selected direct detection-event flat stochastic fold and the matching sampled-error materialization cap.
- `crates/stab-core/tests/dem_sampler.rs` has `pf4_dem_sampler_nested_stochastic_repeat_folds_independent_error_parities`, proving the selected direct detection-event nested zero-shift stochastic fold.
- `crates/stab-core/tests/dem_sampler.rs` has `pf4_dem_sampler_single_stochastic_repeat_folds_by_parity_distribution`, proving the selected direct detection-event single-stochastic fold and the matching sampled-error materialization cap.
- `crates/stab-core/tests/dem_sampler.rs` has `pf4_dem_sampler_folded_repeat_sampling_and_materialized_error_caps`, proving sampled-error materialization, streamed sampled-error output, selected nested zero-shift stochastic folding, and non-selected shifted stochastic sampled-work caps.
- `crates/stab-core/tests/dem_sampler.rs` has `pf4_dem_sampler_rejects_excessive_buffered_outputs_before_sampling`, proving excessive detector width, observable width, sampled-error materialization, and materialized replay buffers are rejected before sampling.
- Oracle row `pf4-dem-sampler-repeat-resource-rust` selects the PF4 sampler evidence through `cargo test -p stab-core --test dem_sampler pf4_dem_sampler_`.

## Remaining Work

The remaining sampler traversal work is not unclassified sampled-error output or replay behavior.
It is optimizing shifted or otherwise non-selected repeated stochastic direct detection-event sampling where dense detector and observable output can define a safe resource boundary without changing the flat sampled-error record contract.
If a future milestone proposes compressed sampled-error records, it must be a new public-format decision rather than a hidden implementation optimization.

## Verification

Checks for this evidence-lock slice:

```sh
cargo test -p stab-core --test dem_sampler pf4_dem_sampler_ --quiet
cargo test -p stab-oracle fixtures --quiet
just oracle::list --milestone PF4
just bench::list
```
