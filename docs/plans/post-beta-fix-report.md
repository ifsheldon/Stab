# Post-Beta Fix Report

## Summary

This report records the implemented post-beta hardening work for already implemented Stab surfaces.
The change adds source-owned schema-version-2 threshold support, partially tightens mixed-row timing thresholds with submeasurement guards, streams `sample_dem` CLI output and replay paths, and streams implemented `detect` and `m2d` CLI conversion paths.
The intentionally deferred Stim parity and ecosystem surfaces remain Python, JS/WASM, diagrams, `explain_errors`, `repl`, QASM/Quirk, GPU, sweep-conditioned conversion, `m2d --ran_without_feedback`, full ErrorMatcher provenance, and new public graph/vector simulator APIs.

## Rows Fixed

- `m4-circuit-parse`: schema-version-2 submeasurement threshold added for the stable direct `circuit_parse` to `stab_circuit_parse` pair at `1.25`.
- `m10-error-decomp`: schema-version-2 submeasurement thresholds added for the stable `disjoint_to_independent_xyz_errors_approx_p10` and `disjoint_to_independent_xyz_errors_approx_p100` direct pairs at `1.25`.
- `m4-circuit-canonical-print`, `m7-convert-stim-canonical`, and `m10-dem-print-contract`: remain in `benchmarks/m12-primary-beta-waivers.json` as explicit no-ratio waivers, not unresolved benchmark failures.

## Rows Still Needing Timing Work

- `m4-gate-lookup`: still needs a larger repeated direct benchmark or implementation work before a strict `1.25` threshold is meaningful.
- `m5-simd-bits`: still needs split direct bit-operation evidence or larger repeated work before source-owned threshold coverage can be added.
- `m5-sparse-xor`: still needs row-XOR and item-XOR optimization or separate stable evidence because current paired ratios exceed `1.25`.
- `m8-measure-reader`: still needs format-split reader evidence, including `ptb64` reader parity, before strict threshold coverage can be added.
- `m10-error-decomp`: exact and independent-to-disjoint filters remain outside the strict threshold gate until tiny-filter timing noise is separated from arithmetic cost.

## Streaming Surfaces

- `CompiledDemSampler` now exposes additive visitor APIs for seeded detector-event streaming, detector-event plus sampled-error streaming, and replayed sampled-error conversion.
- `stab sample_dem` now writes detector output, observable side output, sampled-error output, and replayed-error copies through streaming writers.
- `sample_dem` `ptb64` output buffers exactly 64 records per stream before writing, while text and byte formats write per record through bounded per-record buffers.
- `sample_dem` replay input validates the requested replay prefix before opening output streams, then replays records through bounded readers without materializing every requested shot.
- `CompiledDetectionConverter` now exposes reusable reference-sample conversion with a per-record visitor.
- `stab detect` streams sampled detection events through detection writers, including the existing frame-simulator path for supported Pauli-target observable circuits.
- Implemented `stab m2d` input formats now stream text records, `b8`, `r8`, and `ptb64` groups through the compiled converter without materializing all measurement or detection records.
- Existing explicit rejections for sweep-conditioned circuits and `m2d --ran_without_feedback` are preserved.
- Existing materialized Rust APIs remain available and retain their in-memory limits.

## Commands Run

- `just bench::baseline --primary --out target/benchmarks/post-beta-primary-baseline`
- `just bench::compare --primary --warmup --measurement-runs 3 --require-profiler-notes --profiler-notes-dir benchmarks/profiler-notes/m12 --baseline target/benchmarks/post-beta-primary-baseline/baseline.json --report target/benchmarks/post-beta-primary-compare`
- `just bench::primary-beta --baseline target/benchmarks/post-beta-primary-baseline/baseline.json`
- `just bench::primary-regression --baseline target/benchmarks/post-beta-primary-baseline/baseline.json --report target/benchmarks/post-beta-primary-regression`
- `just bench::primary-memory-regression --baseline target/benchmarks/post-beta-primary-baseline/baseline.json`
- `cargo test -p stab-bench thresholds --quiet`
- `cargo test -p stab-cli sample_dem --quiet`
- `cargo test -p stab-cli detect --quiet`
- `cargo test -p stab-cli m2d --quiet`
- `cargo test -p stab-cli detect_streams_huge_output_until_writer_failure --quiet`
- `cargo test -p stab-cli m2d_streams_large_ptb64_input_until_writer_failure --quiet`
- `cargo test -p stab-core dem_streaming_samples_match_materialized_seeded_samples --quiet`
- `cargo test -p stab-core compiled_detection_converter_streams_like_materialized_conversion --quiet`
- `cargo test -p stab-core sampled_detection_streams_like_materialized_sampling --quiet`
- `cargo fmt --all --check`
- `cargo clippy --workspace --all-targets -- -D warnings`
- `cargo test --workspace --quiet`
- `just oracle::run --implemented-only`
- `just maintenance::pre-commit`

## Dirty-Worktree Benchmark Evidence

The post-beta benchmark commands were run from Stab commit `a933375a3154cb18d27033432a38038ae49231d9` with `local_modifications=true` because this implementation was not committed.
The generated reports are useful verification evidence but are not the final clean evidence required for archival M12 acceptance.

- `target/benchmarks/post-beta-primary-baseline/baseline.json`: generated all 71 primary baseline rows.
- `target/benchmarks/post-beta-primary-compare/compare.json`: measured all 71 primary rows with `command.warmup=true` and `command.measurement_runs=3`.
- `target/benchmarks/m12-primary-beta/compare.json`: passed the beta gate with 68 comparable rows passing and 3 waived no-ratio rows.
- `target/benchmarks/post-beta-primary-regression/compare.json`: passed the timing regression gate with 64 configured threshold rows passing and 7 rows reported as `not-configured`.
- `target/benchmarks/m12-primary-memory-regression/compare.json`: passed the memory gate with 71 rows passing.

## Clean Evidence To Regenerate

The authoritative clean primary baseline, primary compare, beta, timing-regression, and memory-regression reports should be regenerated from the eventual clean commit with `local_modifications=false`.
The commands are:

```sh
just bench::baseline --primary --out target/benchmarks/post-beta-primary-baseline
just bench::compare --primary --warmup --measurement-runs 3 --require-profiler-notes --profiler-notes-dir benchmarks/profiler-notes/m12 --baseline target/benchmarks/post-beta-primary-baseline/baseline.json --report target/benchmarks/post-beta-primary-compare
just bench::primary-beta --baseline target/benchmarks/post-beta-primary-baseline/baseline.json
just bench::primary-regression --baseline target/benchmarks/post-beta-primary-baseline/baseline.json --report target/benchmarks/post-beta-primary-regression
just bench::primary-memory-regression --baseline target/benchmarks/post-beta-primary-baseline/baseline.json
```
