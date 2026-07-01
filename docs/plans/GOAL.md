# Goal: Close The 1.25x Primary Beta Gate

## Purpose

This document is the active execution contract for closing the renewed 1.25x primary beta performance gate after the expanded benchmark matrix.
The original blocker was `m10-error-decomp` above the active `1.25x` beta gate, but clean committed-code evidence after the M10 and convert fixes also exposed `m4-circuit-parse` at `1.2973150684931507x` and `m8-sample-primary-unrotated-surface-contract` at `1.2581244226168387x`.
The goal is to make the gate pass with honest paired evidence, no new comparable-row waivers, no hidden slow submeasurements, and final reports regenerated from committed code with `local_modifications=false`.
This work must follow `docs/plans/lessons-learned.md`: use fresh baselines, compare paired submeasurements, treat noisy near-threshold rows carefully, keep waivers machine-checked, avoid dirty-worktree completion claims, and synchronize docs with benchmark sources.

## Scope

Included:

- Preserve the completed `m10-error-decomp` repair and keep all four direct paired measurements visible.
- Fix `m4-circuit-parse` with a production parser improvement, not by hiding or waiving the sparse parser pair.
- Keep `m8-sample-primary-unrotated-surface-contract` in the beta gate and document it as a narrow watch row when it passes without a measured sampler-code win.
- Update benchmark runner tests, profiler notes, optimization logs, active plan documents, and progress reports when benchmark shape, implementation behavior, or evidence changes.
- Run milestone-audit and full-code-review before declaring the goal complete.

Excluded:

- Do not reopen Python, JS/WASM, Crumble, diagrams, `explain_errors`, `repl`, QASM/Quirk, GPU, sweep-conditioned conversion, `m2d --ran_without_feedback`, full ErrorMatcher provenance, or new public graph/vector simulator APIs.
- Do not add beta waivers for comparable rows.
- Do not keep speculative sampler micro-optimizations unless repeated focused evidence shows a real win and semantic tests cover the changed path.
- Do not change public CLI behavior, file formats, DEM semantics, or compatibility scope unless a correctness test proves the current behavior is wrong and the roadmap is updated in the same change set.

## Sources Of Truth

- Active performance plan: `docs/plans/beta-125-performance-plan.md`.
- Planning lessons: `docs/plans/lessons-learned.md`.
- Roadmap and benchmark policy: `docs/plans/rust-stim-drop-in-rewrite.md`.
- Benchmark manifest: `benchmarks/manifest.csv`.
- Parser implementation: `crates/stab-core/src/circuit.rs` and `crates/stab-core/src/gate.rs`.
- M10 implementation: `crates/stab-core/src/dem/analyze/error_decomp.rs` and `crates/stab-core/src/ids.rs`.
- M4, M8, and M10 profiler notes under `benchmarks/profiler-notes/m12/`.
- Optimization log: `benchmarks/profiler-notes/m12/optimization-log.json`.
- Beta waivers: `benchmarks/m12-primary-beta-waivers.json`.
- Timing thresholds: `benchmarks/m12-primary-thresholds.json`.

If these sources disagree on ratios, row counts, waiver counts, threshold ownership, report paths, commit ids, local-modification status, or completion status, fix the stale source before claiming progress.

## Current Evidence

The clean committed-code primary beta run from the post-M10/post-convert state failed because `m4-circuit-parse` measured `1.2973150684931507x` and `m8-sample-primary-unrotated-surface-contract` measured `1.2581244226168387x`.
The focused M4 before report at `target/benchmarks/m4-watch-focused-before/compare.json` repeated the sparse parser failure at about `1.343x`.
The parser fix streams input lines instead of materializing a `Vec<&str>`, keeps top-level capacity from a newline count, and adds exact fast paths for common plain `H`, `M`, `MZ`, `CX`, and `CNOT` instructions.
The focused M4 after report at `target/benchmarks/m4-watch-focused-final-parser/compare.json` measured `m4-circuit-parse` at `1.1081780821917808x`.
The dirty full primary beta report at `target/benchmarks/m12-primary-beta/compare.json` passed all 85 primary rows with `m4-circuit-parse` at `1.110794520547945x`, `m8-sample-primary-unrotated-surface-contract` at `1.2458306026893222x`, and `m10-error-decomp` at `1.25x`.
This dirty report is diagnostic only; final acceptance still requires committed-code reports with `local_modifications=false`.

## Success State

The goal is complete only when:

- `just bench::primary-beta` passes from committed code with `local_modifications=false`.
- `m4-circuit-parse`, `m8-sample-primary-unrotated-surface-contract`, and `m10-error-decomp` all pass the `1.25x` beta gate.
- All four M10 paired direct measurements remain visible in the compare report.
- No comparable row is waived, renamed casually, hidden behind a row median, downgraded to report-only, or compared against unmatched work to make the gate pass.
- Timing regression still protects every stable thresholded submeasurement and has no stale submeasurement ids.
- Memory regression still passes.
- The profiler notes, optimization log, beta-125 plan, M12 progress report, post-beta report, roadmap, and benchmark source files agree with the final evidence.
- Milestone-audit and full-code-review findings are fixed or logged as accepted future specification gaps.

## Non-Negotiable Rules

- Do not raise the beta gate above `1.25x`.
- Do not add beta waivers for `m4-circuit-parse`, `m8-sample-primary-unrotated-surface-contract`, or `m10-error-decomp`.
- Do not remove or merge failing paired submeasurements to make a row pass.
- Do not treat a dirty-worktree benchmark report as completion evidence.
- Do not optimize from one noisy near-threshold measurement and call it final evidence.
- Do not weaken parser validation, target bounds, public sampler semantics, probability validation, or error-decomposition semantics for benchmark speed.

## Work Loop

1. Keep the M10 fix intact and recheck its paired submeasurements whenever primary beta is rerun.
2. Fix M4 through parser implementation work, then prove the sparse parser pair with focused and primary beta evidence.
3. Recheck M8 in focused and primary beta reports; if it fails repeatedly from clean committed code, stop and write a dedicated sampler plan before changing sampler internals.
4. Remove failed speculative code experiments instead of committing them as incidental churn.
5. Synchronize `docs/plans/beta-125-performance-plan.md`, profiler notes, optimization log, and progress reports with the evidence that remains.
6. Run milestone-audit against this goal and the beta-125 plan, then run full-code-review.

## Required Targeted Tests

Use targeted checks during iteration:

```sh
cargo test -p stab-core --test stim_format --quiet
cargo test -p stab-core gates --quiet
cargo test -p stab-core target --quiet
cargo test -p stab-bench m10_dem_benchmark_rows_have_stab_compare_runners --quiet
just bench::compare --only m4-circuit-parse --warmup --measurement-runs 3 --baseline target/benchmarks/m10-error-decomp-primary-baseline/baseline.json --report target/benchmarks/m4-watch-focused-final-parser
just bench::compare --only m8-sample-primary-unrotated-surface-contract --warmup --measurement-runs 3 --baseline target/benchmarks/m10-error-decomp-primary-baseline/baseline.json --report target/benchmarks/m8-unrotated-focused-final
```

Add or update targeted tests for any changed parser branch, arithmetic branch, benchmark batching rule, measurement name, threshold parser behavior, or compare-note expectation.
Avoid tests that only restate constants.

## Required Final Verification

Run the full verification before marking the goal complete:

```sh
cargo fmt --all --check
cargo clippy --workspace --all-targets -- -D warnings
cargo test --workspace --quiet
just oracle::run --implemented-only
just bench::baseline --primary --out target/benchmarks/m10-error-decomp-primary-baseline
just bench::compare --only m10-error-decomp --warmup --measurement-runs 3 --baseline target/benchmarks/m10-error-decomp-primary-baseline/baseline.json --report target/benchmarks/m10-error-decomp-focused-final
just bench::compare --only m4-circuit-parse --warmup --measurement-runs 3 --baseline target/benchmarks/m10-error-decomp-primary-baseline/baseline.json --report target/benchmarks/m4-watch-focused-final-parser
just bench::compare --only m8-sample-primary-unrotated-surface-contract --warmup --measurement-runs 3 --baseline target/benchmarks/m10-error-decomp-primary-baseline/baseline.json --report target/benchmarks/m8-unrotated-focused-final
just bench::primary-beta --baseline target/benchmarks/m10-error-decomp-primary-baseline/baseline.json
just bench::primary-regression --baseline target/benchmarks/m10-error-decomp-primary-baseline/baseline.json --report target/benchmarks/m10-error-decomp-primary-regression
just bench::primary-memory-regression --baseline target/benchmarks/m10-error-decomp-primary-baseline/baseline.json
just maintenance::pre-commit
```

Final completion requires committed-code evidence with `local_modifications=false`.
The user has explicitly allowed commits for this goal, so commit focused implementation and documentation changes before final evidence collection.
