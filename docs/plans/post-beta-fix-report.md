# Post-Beta Fix Report

## Summary

This report records the implemented post-beta hardening work for already implemented Stab surfaces.
The implemented work adds source-owned schema-version-2 threshold support, checked timing-regression waivers, mixed-row timing thresholds with submeasurement guards, streaming `sample_dem`, streaming implemented `detect` and `m2d` conversion paths, and the threshold-completion fixes from `docs/plans/post-beta-threshold-completion-plan.md`.
Historical note: at the time this post-beta hardening report was written, the intentionally deferred Stim parity and ecosystem surfaces included Python, JS/WASM, diagrams, `explain_errors`, `repl`, QASM/Quirk, GPU, sweep-conditioned conversion, `m2d --ran_without_feedback`, full ErrorMatcher provenance, and new public graph/vector simulator APIs.
Later M9 work added scoped `m2d --sweep`, `--sweep_format`, and `--ran_without_feedback` support, so current scope should be checked against `docs/stab-feature-checklist.md`, `docs/plans/m9-sweep-feedback-progress-report.md`, and `docs/plans/non-deferred-partial-feature-milestones.md`.

## Rows Fixed Or Reconciled

- `m4-circuit-parse`: schema-version-2 submeasurement threshold covers the stable direct `circuit_parse` to `stab_circuit_parse` pair at `1.25`.
- `m4-gate-lookup`: schema-version-2 submeasurement threshold covers the faithful pinned Stim `gate_data_hash_all_gate_names` to Stab `stab_gate_data_hash_all_gate_names` hash pair at `1.25`; alias, lowercase, and invalid lookup measurements remain Stab-only contract extras.
- `m5-simd-bits`: schema-version-2 submeasurement thresholds cover the direct `simd_bits_xor_10K` pair and the pinned Stim `simd_bits_not_zero_100K` filter's actual 10K-bit workload, mirrored by `stab_simd_bits_not_zero_10K`, at `1.25`; masked, range, and copy contract extras remain unthresholded.
- `m5-sparse-xor`: schema-version-2 submeasurement thresholds cover `SparseXorTable_SmallRowXor_1000` to `stab_sparse_table_row_xor_1000` and `SparseXorVec_XorItem` to `stab_sparse_xor_item_7` at `1.25`.
- `m8-measure-reader`: supported `01`, `b8`, `r8`, `hits`, and `dets` readers are split into format-specific primary rows with paired packed and sparse Stab submeasurement thresholds; `ptb64` parity remains split into `m8-measure-reader-ptb64-contract` because pinned Stim v1.16.0 has no ptb64 reader perf filter.
- `m10-error-decomp`: schema-version-2 submeasurement threshold covers the stable `disjoint_to_independent_xyz_errors_approx_p10` direct pair at `1.25`; exact and independent-to-disjoint nanosecond filters use enlarged pinned-case batches, exact conversion uses a direct closed-form branch before the generic solver result path, `approx_p10` and `approx_p100` use the zero-component semantic fast reject when the requested disjoint triple cannot decompose exactly, and exact, p100, and independent-to-disjoint remain unthresholded until repeated clean reports prove enough headroom.
- `m7-convert-b8-to-b8-wide`: byte-aligned `b8 -> b8` conversion now validates record byte width and writes the original bytes directly when no observable side output is requested; non-byte-aligned packed records still use the canonical reader/writer path so padding bits remain normalized.
- `m4-circuit-canonical-print`, `m7-convert-stim-canonical`, `m7-convert-01-to-ptb64`, `m8-measure-reader-ptb64-contract`, and `m10-dem-print-contract` remain in `benchmarks/m12-primary-beta-waivers.json` and are also checked by `benchmarks/m12-primary-regression-waivers.json` as explicit no-ratio waivers, not unresolved benchmark failures.

## Remaining Non-Thresholded Subsurfaces

- `m4-gate-lookup` alias, lowercase, and invalid-name measurements remain outside strict Stim-relative thresholds because pinned Stim has no matching perf filters for those Stab public lookup contracts.
- `m8-measure-reader-ptb64-contract`: pinned Stim v1.16.0 has ptb64 reader tests but no ptb64 perf filter, so the row is contract-only with checked beta and timing-regression waivers.
- `m5-simd-bits` masked, range, and copy contract extras remain outside Stim-relative timing thresholds because pinned Stim exposes no matching direct filters for those Stab-only contract measurements.
- `m10-error-decomp` exact, p100, and independent-to-disjoint filters: the beta-125 implementation keeps the row under the active `1.25x` beta gate by inlining tiny conversion helpers across crate boundaries, avoiding redundant `Probability` output validation where local formulas prove probability bounds, and fast-rejecting one-zero two-positive disjoint triples that cannot decompose exactly. Only `approx_p10` owns a strict threshold today because exact, p100, and independent-to-disjoint still need repeated clean evidence before they are stable enough for source-owned threshold ownership.

## Streaming Surfaces

- `CompiledDemSampler` exposes additive visitor APIs for seeded detector-event streaming, detector-event plus sampled-error streaming, and replayed sampled-error conversion.
- `stab sample_dem` writes detector output, observable side output, sampled-error output, and replayed-error copies through streaming writers.
- `sample_dem` `ptb64` output buffers exactly 64 records per stream before writing, while text and byte formats write per record through bounded per-record buffers.
- `sample_dem` replay input validates the requested replay prefix before opening output streams, then replays records through bounded readers without materializing every requested shot.
- `CompiledDetectionConverter` exposes reusable reference-sample conversion with a per-record visitor.
- `stab detect` streams sampled detection events through detection writers, including the existing frame-simulator path for supported Pauli-target observable circuits.
- Implemented `stab m2d` input formats stream text records, `b8`, `r8`, and `ptb64` groups through the compiled converter without materializing all measurement or detection records.
- At the time of this report, explicit rejections for sweep-conditioned circuits and `m2d --ran_without_feedback` were preserved; later M9 work replaced those rejections with the scoped support documented in the current checklist and M9 reports.
- Existing materialized Rust APIs remain available and retain their in-memory limits.

## Commands Run

- `cargo fmt --all --check`
- `cargo clippy --workspace --all-targets -- -D warnings`
- `cargo test --workspace --quiet`
- `cargo test -p stab-core result_streaming --quiet`
- `cargo test -p stab-core ptb64_reader_round_trips_writer_output --quiet`
- `cargo test -p stab-core error_decomp --quiet`
- `cargo test -p stab-core --test stim_format gate_lookup --quiet`
- `cargo test -p stab-core --test bits bits_sparse_xor --quiet`
- `cargo test -p stab-core --test bits bits_range_xor --quiet`
- `cargo test -p stab-bench --quiet`
- `cargo test -p stab-bench thresholds --quiet`
- `just bench::baseline --primary --out target/benchmarks/post-beta-primary-baseline`
- `just bench::compare --primary --warmup --measurement-runs 3 --require-profiler-notes --profiler-notes-dir benchmarks/profiler-notes/m12 --baseline target/benchmarks/post-beta-primary-baseline/baseline.json --report target/benchmarks/post-beta-primary-compare`
- `just bench::primary-beta --baseline target/benchmarks/post-beta-primary-baseline/baseline.json`
- `just bench::primary-regression --baseline target/benchmarks/post-beta-primary-baseline/baseline.json --report target/benchmarks/post-beta-primary-regression`
- `just bench::primary-memory-regression --baseline target/benchmarks/post-beta-primary-baseline/baseline.json`
- `just oracle::run --implemented-only`
- `just maintenance::pre-commit`

## Pre-Commit Dirty-Worktree Benchmark Evidence

The pre-commit post-beta benchmark commands were run from Stab commit `bca98ac7fa115183b451a2ff1350587a7c684b99` with `local_modifications=true` because this implementation was not yet committed.
The generated reports were useful verification evidence, but final archival acceptance requires rerunning the same report paths from the committed tree with `local_modifications=false`.

- `target/benchmarks/post-beta-primary-baseline/baseline.json`: generated all 76 primary baseline rows.
- `target/benchmarks/post-beta-primary-compare/compare.json`: measured all 76 primary rows with `command.warmup=true`, `command.measurement_runs=3`, 72 comparable passes, 4 not-comparable contract rows, and no comparable failures.
- `target/benchmarks/m12-primary-beta/compare.json`: passed the beta gate with 72 comparable rows passing and 4 source-owned no-ratio rows waived.
- `target/benchmarks/post-beta-primary-regression/compare.json`: passed the timing regression gate with source-owned profiler-note validation, 65 configured threshold rows passing, 11 rows reported as `not-configured`, and no configured-threshold failures; this report is now superseded by the threshold-completion dirty report below.
- `target/benchmarks/m12-primary-memory-regression/compare.json`: passed the memory gate for all 76 primary rows.

## Threshold-Completion Dirty Evidence

The threshold-completion implementation was checked before commit with `just bench::primary-regression --baseline target/benchmarks/timing-finish-baseline/baseline.json --report target/benchmarks/timing-finish-regression-thresholded-2`.
That dirty report recorded `local_modifications=true`, passed all 72 configured threshold rows, marked the 4 checked no-ratio rows as `waived-not-thresholdable`, and left zero ambiguous `not-configured` rows.
Final archival acceptance is proven by rerunning the final primary benchmark commands from the committed tree with `local_modifications=false`.

## Audit And Review Status

- Milestone-audit for `docs/plans/post-beta-threshold-completion-plan.md` found no source-shape blocker after clarifying that waiver entries carry row id, reason, and follow-up while the timing-regression gate supplies measured no-ratio evidence at report time.
- Full-code-review of the threshold-completion commits found no confirmed Rust correctness, Stim compatibility, file-format, SIMD isolation, hostile-input, benchmark-policy, or documentation blocker.
- Final audit and review closure uses the clean committed-code benchmark reports named below.

## Clean Evidence

The post-beta clean primary compare and timing-regression reports were regenerated at `target/benchmarks/post-beta-primary-compare/compare.json` and `target/benchmarks/post-beta-primary-regression/compare.json` from committed code with `local_modifications=false`.
The active beta-125 completion evidence supersedes those paths for the stricter gate: `target/benchmarks/beta-125-primary-compare/compare.json`, `target/benchmarks/m12-primary-beta/compare.json`, `target/benchmarks/beta-125-primary-regression/compare.json`, and `target/benchmarks/m12-primary-memory-regression/compare.json` were regenerated from committed Stab commit `c9c96f80844dc2b4c952ec137d191ce369b2f233` with `local_modifications=false`.
The beta report for that post-beta cycle passed 72 comparable rows and 4 no-ratio rows, the timing-regression report passed 72 configured threshold rows with 4 no-ratio waivers and zero ambiguous `not-configured` rows, and the memory-regression report passed all 76 primary rows.
After the M7 convert benchmark expansion and the current M7/M10/M4 fixes, the final clean primary beta report at `target/benchmarks/m12-primary-beta/compare.json` was regenerated from Stab commit `c5ccd7967130e764d3319d699ed0a9fe680de81a` with `local_modifications=false`, measures 85 primary rows, passes beta with 80 comparable rows and 5 checked `waived-not-comparable` no-ratio rows, reports `m7-convert-b8-to-b8-wide` at `0.00337415937762406x`, reports `m4-circuit-parse` at `1.1185x`, reports `m8-sample-primary-unrotated-surface-contract` at `1.0918461483384692x`, and reports `m10-error-decomp` at `1.25x`.
The final clean timing-regression report at `target/benchmarks/m10-error-decomp-primary-regression/compare.json` passes with 80 configured threshold rows and 5 checked `waived-not-thresholdable` rows, including `m7-convert-b8-to-b8-wide` at `0.002401335057205131x`.
The final clean memory-regression report at `target/benchmarks/m12-primary-memory-regression/compare.json` passes all 85 rows using the schema-version-2 resident-delta memory baseline.
The reproducible post-beta evidence commands are:

```sh
just bench::baseline --primary --out target/benchmarks/post-beta-primary-baseline
just bench::compare --primary --warmup --measurement-runs 3 --require-profiler-notes --profiler-notes-dir benchmarks/profiler-notes/m12 --baseline target/benchmarks/post-beta-primary-baseline/baseline.json --report target/benchmarks/post-beta-primary-compare
just bench::primary-beta --baseline target/benchmarks/post-beta-primary-baseline/baseline.json
just bench::primary-regression --baseline target/benchmarks/post-beta-primary-baseline/baseline.json --report target/benchmarks/post-beta-primary-regression
just bench::primary-memory-regression --baseline target/benchmarks/post-beta-primary-baseline/baseline.json
```
