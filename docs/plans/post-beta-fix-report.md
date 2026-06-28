# Post-Beta Fix Report

## Summary

This report records the implemented post-beta hardening work for already implemented Stab surfaces.
The current worktree adds source-owned schema-version-2 threshold support, tightens mixed-row timing thresholds with submeasurement guards, streams `sample_dem`, streams implemented `detect` and `m2d` conversion paths, and hardens the remaining timing rows from `docs/plans/post-beta-timing-hardening-plan.md`.
The intentionally deferred Stim parity and ecosystem surfaces remain Python, JS/WASM, diagrams, `explain_errors`, `repl`, QASM/Quirk, GPU, sweep-conditioned conversion, `m2d --ran_without_feedback`, full ErrorMatcher provenance, and new public graph/vector simulator APIs.

## Rows Fixed Or Reconciled

- `m4-circuit-parse`: schema-version-2 submeasurement threshold covers the stable direct `circuit_parse` to `stab_circuit_parse` pair at `1.25`.
- `m4-gate-lookup`: canonical, alias, lowercase, and invalid lookup measurements remain split and measured, but the canonical `gate_data_hash_all_gate_names` to `stab_gate_data_hash_all_gate_names` pair is outside strict `1.25x` thresholds because a refreshed pinned Stim baseline landed near 100 ns and exposed timer-dominated instability.
- `m5-simd-bits`: schema-version-2 submeasurement thresholds cover the direct `simd_bits_xor_10K` pair and the pinned Stim `simd_bits_not_zero_100K` filter's actual 10K-bit workload, mirrored by `stab_simd_bits_not_zero_10K`, at `1.25`; masked, range, and copy contract extras remain unthresholded.
- `m5-sparse-xor`: table row-XOR and item-XOR remain split and measured, but both are outside strict `1.25x` thresholds until repeated clean evidence proves stability below the gate.
- `m8-measure-reader`: supported `01`, `b8`, `r8`, `hits`, and `dets` readers are split into format-specific primary rows, while `ptb64` parity is split into `m8-measure-reader-ptb64-contract`.
- `m10-error-decomp`: schema-version-2 submeasurement threshold covers the stable `disjoint_to_independent_xyz_errors_approx_p10` direct pair at `1.25`; exact, p100, and independent-to-disjoint nanosecond filters use case-diverse batches but remain unthresholded.
- `m4-circuit-canonical-print`, `m7-convert-stim-canonical`, `m8-measure-reader-ptb64-contract`, and `m10-dem-print-contract` remain in `benchmarks/m12-primary-beta-waivers.json` as explicit no-ratio waivers, not unresolved benchmark failures.

## Remaining Unthresholded Surfaces

- `m8-measure-reader-01`, `m8-measure-reader-b8`, `m8-measure-reader-r8`, `m8-measure-reader-hits`, and `m8-measure-reader-dets`: the split format rows pass the beta gate but remain outside strict `1.25x` thresholds because each row still compares Stab's public dense reusable-record visitor against pinned Stim dense and sparse internal reader filters.
- `m4-gate-lookup`: the canonical direct pair remains beta-safe but is too small and baseline-sensitive for strict threshold ownership until larger repeated lookup evidence separates lookup cost from timer noise.
- `m8-measure-reader-ptb64-contract`: pinned Stim v1.16.0 has ptb64 reader tests but no ptb64 perf filter, so the row is contract-only with a beta waiver.
- `m5-sparse-xor`: dirty post-beta evidence put table row-XOR on both sides of the `1.25x` line, and the item workload remains a tiny 7-item sequence above `1.25x`; keep both documented until repeated clean evidence, a larger paired workload, or further data-structure optimization supports strict thresholds.
- `m10-error-decomp` exact, p100, and independent-to-disjoint filters: the dedicated `m12-primary-beta` run keeps the row under the `2.0x` beta gate, and the timing-regression report now refreshes row ratio evidence from explicit schema-version-2 pairs before beta/pass-fail fields are recorded. Only `approx_p10` owns a strict threshold because the exact, p100, and independent-to-disjoint filters remain too tiny or unstable for honest `1.25x` ownership.

## Streaming Surfaces

- `CompiledDemSampler` exposes additive visitor APIs for seeded detector-event streaming, detector-event plus sampled-error streaming, and replayed sampled-error conversion.
- `stab sample_dem` writes detector output, observable side output, sampled-error output, and replayed-error copies through streaming writers.
- `sample_dem` `ptb64` output buffers exactly 64 records per stream before writing, while text and byte formats write per record through bounded per-record buffers.
- `sample_dem` replay input validates the requested replay prefix before opening output streams, then replays records through bounded readers without materializing every requested shot.
- `CompiledDetectionConverter` exposes reusable reference-sample conversion with a per-record visitor.
- `stab detect` streams sampled detection events through detection writers, including the existing frame-simulator path for supported Pauli-target observable circuits.
- Implemented `stab m2d` input formats stream text records, `b8`, `r8`, and `ptb64` groups through the compiled converter without materializing all measurement or detection records.
- Existing explicit rejections for sweep-conditioned circuits and `m2d --ran_without_feedback` are preserved.
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
- `target/benchmarks/post-beta-primary-regression/compare.json`: passed the timing regression gate with source-owned profiler-note validation, 65 configured threshold rows passing, 11 rows reported as `not-configured`, and no configured-threshold failures; explicit schema-version-2 measurement pairs are reflected in row ratio and pass/fail fields before the report is written. `m4-gate-lookup` remains a documented unconfigured comparable row with a roughly 1.7x paired ratio.
- `target/benchmarks/m12-primary-memory-regression/compare.json`: passed the memory gate for all 76 primary rows.

## Audit And Review Status

- Milestone-audit was run against the post-beta timing-hardening goal and found the row work substantially reconciled, but it blocks final completion until the intended changes are committed and the primary benchmark reports are regenerated with `local_modifications=false`.
- Full-code-review found no Rust correctness, Stim compatibility, file-format, SIMD isolation, hostile-input, or benchmark-policy blocker in the changed code, but it found stale source-owned profiler evidence in `benchmarks/profiler-notes/m12/optimization-log.json`.
- The optimization log was updated so `m4-gate-lookup`, `m8-measure-reader`, and `m10-error-decomp` match the current manifest, threshold file, beta waivers, and per-row profiler notes.
- Final audit and review closure still requires the clean committed-code benchmark reports named below.

## Clean Evidence Commands

The authoritative clean primary baseline, primary compare, beta, timing-regression, and memory-regression reports use the same report paths as the dirty pre-commit evidence.
They must be regenerated from the final committed tree and must report `local_modifications=false`.
The commands are:

```sh
just bench::baseline --primary --out target/benchmarks/post-beta-primary-baseline
just bench::compare --primary --warmup --measurement-runs 3 --require-profiler-notes --profiler-notes-dir benchmarks/profiler-notes/m12 --baseline target/benchmarks/post-beta-primary-baseline/baseline.json --report target/benchmarks/post-beta-primary-compare
just bench::primary-beta --baseline target/benchmarks/post-beta-primary-baseline/baseline.json
just bench::primary-regression --baseline target/benchmarks/post-beta-primary-baseline/baseline.json --report target/benchmarks/post-beta-primary-regression
just bench::primary-memory-regression --baseline target/benchmarks/post-beta-primary-baseline/baseline.json
```
