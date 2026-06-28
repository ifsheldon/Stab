# Post-Beta Timing Hardening Plan

## Summary

This plan fixes the remaining post-beta timing rows and row families by making each benchmark surface comparable, stable, and source-owned before adding strict `1.25x` regression thresholds.
The goal is not quick threshold coverage.
The goal is correct benchmark evidence, real optimization where the evidence proves a bottleneck, and durable threshold ownership for implemented Stab surfaces.
The narrower follow-up plan in `docs/plans/post-beta-threshold-completion-plan.md` closes the remaining ambiguous timing-regression rows by adding checked no-ratio regression waivers and thresholding `m4-gate-lookup`, `m5-sparse-xor`, and supported `m8-measure-reader-*` pairs.

Do not fix these rows by weakening gates, adding broad waivers, or hiding slow submeasurements behind row medians.
Every row or row family should end in one of two states: guarded by a row-level or schema-version-2 submeasurement threshold, or documented with source-owned evidence proving why the remaining surface is not a meaningful strict threshold target yet.

Recommended work order:

1. `m8-measure-reader`
2. `m5-simd-bits`
3. `m5-sparse-xor`
4. `m10-error-decomp`
5. `m4-gate-lookup`

## General Rules

- Start by regenerating clean post-beta benchmark evidence from committed `HEAD` and require the archived reports to have `local_modifications=false`.
- Split mixed rows before optimizing them.
- Make tiny submeasurements large enough to escape timer noise before treating them as performance evidence.
- Profile stable submeasurements before making implementation changes.
- Optimize the measured hot path behind existing abstractions.
- Add row-level `1.25x` thresholds only when the whole row is comparable and stable.
- Add schema-version-2 submeasurement thresholds when only part of a mixed row is comparable and stable.
- Keep Stab-only contract extras out of Stim-relative timing thresholds, but keep their functional, memory, and resource-boundary tests.
- Update profiler notes, threshold files, benchmark documentation, and plan reports in the same change set as each newly guarded row or submeasurement.

## Row Plans

### `m8-measure-reader`

This row family is the highest priority because result-file reading is user-visible and affects implemented CLI and library workflows.

Tasks:

- Split reader benchmark evidence by result format: `01`, `b8`, `r8`, `hits`, `dets`, and `ptb64`.
- Add missing `ptb64` reader parity to the M8 benchmark family before claiming complete reader threshold coverage.
- Decode `ptb64` one 64-shot group at a time instead of requiring all groups to be materialized for benchmark or CLI streaming paths.
- Reuse record buffers where the reader API permits it.
- Avoid per-record allocation in hot paths where bounded streaming or reusable output can preserve behavior.
- Parse text formats directly from bytes where possible.
- Decode packed formats in bounded chunks.
- Preserve current public validation errors for malformed record widths, invalid bytes, truncated packed records, zero-width packed records, and unsupported detection-output `ptb64` paths.

Tests and acceptance:

- Add parity tests proving `ptb64` reader decoding matches `write_ptb64_records_checked` fixtures.
- Add exact round-trip tests for `01`, `b8`, `r8`, `hits`, `dets`, and `ptb64`.
- Add benchmark-runner tests proving every format submeasurement is present and stale threshold ids are rejected.
- Add schema-version-2 thresholds for stable direct format pairs that remain below `1.25x` in repeated clean evidence; the threshold-completion follow-up pairs Stab packed and sparse reader submeasurements with the pinned Stim dense and sparse filters for `01`, `b8`, `r8`, `hits`, and `dets`, while keeping `ptb64` as a checked no-ratio contract row.

### `m5-simd-bits`

This row currently mixes direct upstream-compatible bit operations with Stab-only contract extras, so it must be split before performance claims become strict.

Tasks:

- Keep `xor_assign`, `not_zero`, `masked_xor_assign`, `xor_range_from`, and `copy_from_bitslice` as separate benchmark submeasurements.
- Report normalized bits-per-second evidence for each submeasurement.
- Increase repeated work for tiny direct operations until measurements are stable across warmup and recorded runs.
- Optimize large aligned word ranges through the existing portable-SIMD path.
- Keep scalar fallbacks for tiny ranges, unaligned ranges, and tail fragments where SIMD setup is not a win.
- Replace the bit-by-bit `xor_range_from` hot path with a word-range implementation that handles unaligned source and target offsets, full middle words, and tail masks.
- Preserve exact unused-tail-bit masking after every mutating operation.

Tests and acceptance:

- Add randomized tests comparing SIMD-backed and scalar reference behavior across varied lengths, offsets, masks, and dirty tail bits.
- Add focused tests for `xor_range_from` when source and target starts are aligned, differently aligned, overlapping, zero length, full length, and tail-only.
- Add benchmark-runner tests proving direct submeasurement threshold pass and fail behavior.
- Add thresholds only for direct comparable submeasurements with stable clean evidence below `1.25x`.

### `m5-sparse-xor`

This row mixes a real table row-XOR workload with a tiny item-XOR workload, so the correct fix is to separate evidence and then optimize the data-structure operations that remain slow.

Tasks:

- Keep table row-XOR and item-XOR as separate benchmark submeasurements.
- Report normalized row-XOR and item-XOR rates.
- Profile allocation, sorted-merge cost, binary-search cost, insertion/removal cost, and small-row behavior separately.
- Optimize row-XOR with reusable merge scratch or tighter in-place symmetric-difference logic without changing the sorted unique invariant.
- Optimize `xor_item` with an adaptive small-row path and binary-search insertion/removal for larger rows.
- Preserve the invariant that `SparseXorVec` items are sorted and unique after every public constructor and mutating operation.

Tests and acceptance:

- Add invariant tests for empty rows, identical rows, disjoint rows, duplicate inputs, repeated `xor_item`, and mixed row-XOR plus item-XOR sequences.
- Add property-style tests comparing `SparseXorVec` behavior against a reference sorted set or symmetric-difference implementation.
- Add a row-XOR submeasurement threshold when repeated clean evidence is stable below `1.25x`.
- Add an item-XOR submeasurement threshold only after the repeated item benchmark is stable enough to be meaningful.
- The threshold-completion follow-up guards both row-XOR and item-XOR with schema-version-2 thresholds after tightening the sorted-unique small-row `xor_item` path.

### `m10-error-decomp`

The `approx_p10` submeasurement is guarded.
The `approx_p100`, exact, and independent-to-disjoint filters are too small or too close to the `1.25x` line for honest strict thresholding, so they need arithmetic-normalized batched evidence before any future threshold expansion.

Tasks:

- Keep the existing `approx_p10` schema-version-2 threshold.
- Replace nanosecond-scale single-case evidence for exact and independent-to-disjoint paths with batched conversion arrays.
- Report normalized conversions-per-second evidence for exact, approximate, and independent-to-disjoint conversion families.
- Profile only after batched evidence separates arithmetic cost from timer overhead.
- If profiling proves real arithmetic overhead, reduce temporary wrappers in internal loops, avoid repeated validation where public `Probability` boundaries have already validated inputs, and keep public validation at external API boundaries.
- Preserve numerical stability and Stim v1.16.0 parity over speed when the two are in tension.

Tests and acceptance:

- Add numerical tests for zero, one, near-zero, near-boundary, symmetric, exact, approximate, invalid, and round-trip cases.
- Compare direct conversion helpers against Stim-derived upstream examples and high-precision reference calculations where practical.
- Add schema-version-2 thresholds for exact and independent-to-disjoint submeasurements only after repeated clean evidence is stable below `1.25x`.
- Keep any remaining tiny unstable submeasurement documented in the profiler note instead of forcing it into the strict threshold file.

### `m4-gate-lookup`

This row is low priority because the current measured surface is sub-microsecond and can be dominated by timer noise.
It should be fixed by making the lookup benchmark larger and more representative before generating lookup-table code.

Tasks:

- Replace the tiny all-gates lookup timing with larger repeated in-process lookup sets.
- Split canonical uppercase names, aliases, lowercase normalization, and invalid names into separate submeasurements.
- Ensure benchmark inputs and outputs are black-boxed so the compiler cannot erase lookup work.
- If profiling shows real lookup cost after benchmark stabilization, generate an allocation-free static lookup path from the existing gate metadata source of truth.
- Keep canonical gate definitions single-sourced.
- Preserve alias behavior, case-normalization behavior, canonical-name round trips, and `UnknownGate` errors.

Tests and acceptance:

- Add tests covering every canonical name, known alias, lowercase variant, and invalid lookup.
- Add tests proving generated or table-driven lookup data is derived from the canonical gate metadata instead of duplicating hand-maintained definitions.
- Add thresholds only for stable lookup submeasurements that remain below `1.25x` in repeated clean evidence.
- The threshold-completion follow-up guards the faithful pinned Stim `gate_data_hash_all_gate_names` pair and keeps alias, lowercase, and invalid lookup contracts outside Stim-relative thresholds.

## Verification Plan

Run targeted tests while implementing each row, then run the full verification suite before considering the plan complete.

Required final checks:

```sh
cargo fmt --all --check
cargo clippy --workspace --all-targets -- -D warnings
cargo test --workspace --quiet
just oracle::run --implemented-only
just bench::baseline --primary --out target/benchmarks/post-beta-primary-baseline
just bench::compare --primary --warmup --measurement-runs 3 --require-profiler-notes --profiler-notes-dir benchmarks/profiler-notes/m12 --baseline target/benchmarks/post-beta-primary-baseline/baseline.json --report target/benchmarks/post-beta-primary-compare
just bench::primary-beta --baseline target/benchmarks/post-beta-primary-baseline/baseline.json
just bench::primary-regression --baseline target/benchmarks/post-beta-primary-baseline/baseline.json --report target/benchmarks/post-beta-primary-regression
just bench::primary-memory-regression --baseline target/benchmarks/post-beta-primary-baseline/baseline.json
just maintenance::pre-commit
```

The final benchmark reports must be generated from the committed implementation with `local_modifications=false`.
If a benchmark command cannot be run, the plan is not complete until the blocker and missing evidence are recorded and accepted as a follow-up.

## Completion Criteria

This plan is complete when:

- Clean benchmark evidence exists for every row in this plan.
- Every mixed row has explicit paired submeasurement evidence.
- Every stable comparable row or submeasurement below `1.25x` is guarded in `benchmarks/m12-primary-thresholds.json`.
- Every row or submeasurement left outside the threshold file has a source-owned profiler note explaining the dominant cost, stability issue, and next owner action.
- Functional tests protect every behavior changed while optimizing.
- Benchmark-runner tests protect the new submeasurement ids and threshold behavior.
- `docs/plans/post-beta-fix-report.md`, `docs/plans/m12-progress-report.md`, profiler notes, threshold docs, and this plan are updated to match the final evidence.
- Milestone-audit and full-code-review have been run against this follow-up plan, and all findings are fixed or logged as accepted specification follow-ups.
