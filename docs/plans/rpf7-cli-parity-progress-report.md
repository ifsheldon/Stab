# RPF7 CLI Parity Progress Report

## Implemented Slice: Public `m2d --sweep` Packed Benchmark Evidence

Stab now has source-owned report-only benchmark coverage for the PF7 public CLI `m2d --sweep` packed `b8` path.

This slice reuses the source-owned M9 packed sweep fixture and routes through `stab_cli::run_from`, so it measures public CLI behavior instead of a lower-level conversion helper.

## Evidence

Benchmark row:

- `pf7-cli-m2d-sweep-b8` now has a non-primary report-only runner named `stab_pf7_cli_m2d_sweep_b8`, normalized as shots per second.
- Local probe command `just bench::compare --only pf7-cli-m2d-sweep-b8 --baseline target/benchmarks/pf7-cli-sweep-probe-baseline/baseline.json --report target/benchmarks/pf7-cli-sweep-probe-compare` measured `stab_pf7_cli_m2d_sweep_b8=0.000063680s`, or approximately `7.852e4 shots/s`, as report-only evidence on the local machine.

## Still Open In RPF7

- `pf7-cli-m2d-feedback-inline` still needs public CLI benchmark extraction for `m2d --ran_without_feedback`.
- `pf7-cli-analyze-errors-generated` and `pf7-cli-analyze-errors-decompose` remain open for public `analyze_errors` CLI parity evidence.
- `pf7-cli-legacy-dispatch-startup` remains open for accepted legacy dispatch startup evidence.
- The broad `pf7-m2d-cli-parity`, `pf7-analyze-errors-cli-parity`, and `pf7-legacy-dispatch-parity` oracle rows remain manifest-only until their selected CLI subcases are exhausted.
