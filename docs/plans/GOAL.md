# Goal: Fix `m10-error-decomp` For The 1.25x Beta Gate

## Purpose

This document is the active execution contract for fixing the renewed `m10-error-decomp` performance blocker.
The current observed blocker is `m10-error-decomp` at `1.3559322033898304x` in the expanded primary beta report generated from `just bench::primary-beta --baseline target/benchmarks/convert-primary-baseline/baseline.json`, which is above the active `1.25x` beta gate.
The goal is to make the row pass the gate with honest paired evidence, not to hide or waive the slow submeasurement.
This work must follow `docs/plans/lessons-learned.md`: use fresh baselines, compare paired submeasurements, treat tiny benchmarks carefully, keep waivers machine-checked, avoid dirty-worktree completion claims, and synchronize docs with benchmark sources.

## Scope

Included:

- Fix `m10-error-decomp` until it passes `just bench::primary-beta` at `<=1.25x` under the same paired-ratio rule used by the gate.
- Preserve direct-match evidence for the four pinned Stim error-decomposition filters: `independent_to_disjoint_xyz_errors`, `disjoint_to_independent_xyz_errors_approx_exact`, `disjoint_to_independent_xyz_errors_approx_p10`, and `disjoint_to_independent_xyz_errors_approx_p100`.
- Determine whether the failing pair reflects real arithmetic cost, benchmark timer overhead, or an unfaithful benchmark shape before changing implementation code.
- Update benchmark runner tests, profiler notes, threshold files, optimization logs, and plan documents when benchmark shape, implementation behavior, or evidence changes.
- Run milestone-audit and full-code-review before declaring the goal complete.

Excluded:

- Do not reopen Python, JS/WASM, Crumble, diagrams, `explain_errors`, `repl`, QASM/Quirk, GPU, sweep-conditioned conversion, `m2d --ran_without_feedback`, full ErrorMatcher provenance, or new public graph/vector simulator APIs.
- Do not optimize unrelated rows unless a fresh full primary beta run proves another comparable row is blocking the same gate after the M10 fix.
- Do not change public CLI behavior, file formats, DEM semantics, or compatibility scope unless a correctness test proves the current behavior is wrong and the roadmap is updated in the same change set.

## Sources Of Truth

- Active performance plan: `docs/plans/beta-125-performance-plan.md`, especially Milestone B4.
- Planning lessons: `docs/plans/lessons-learned.md`.
- Roadmap and benchmark policy: `docs/plans/rust-stim-drop-in-rewrite.md`.
- Benchmark manifest: `benchmarks/manifest.csv`.
- M10 benchmark runner: `ops/bench/src/baseline/m10.rs`.
- M10 profiler note: `benchmarks/profiler-notes/m12/m10-error-decomp.md`.
- Optimization log: `benchmarks/profiler-notes/m12/optimization-log.json`.
- Beta waivers: `benchmarks/m12-primary-beta-waivers.json`.
- Timing thresholds: `benchmarks/m12-primary-thresholds.json`.
- Error-decomposition implementation: `crates/stab-core/src/dem/analyze/error_decomp.rs`.
- Probability wrapper implementation: `crates/stab-core/src/ids.rs`.

If these sources disagree on ratios, row counts, waiver counts, threshold ownership, report paths, commit ids, local-modification status, or completion status, fix the stale source before claiming progress.

## Success State

The goal is complete only when:

- `m10-error-decomp` passes `just bench::primary-beta` at `<=1.25x`.
- All four M10 paired direct measurements remain visible in the compare report.
- No M10 measurement is waived, renamed casually, hidden behind a row median, downgraded to report-only, or compared against unmatched work.
- Any benchmark-shape change keeps Stim and Stab workloads faithful and normalizes batched timings back to seconds per operation.
- Timing regression still protects every stable thresholded submeasurement and has no stale submeasurement ids.
- Memory regression still passes.
- The profiler note, optimization log, beta-125 plan, M12 progress report, post-beta report, roadmap, and benchmark source files agree with the final evidence.
- Final benchmark evidence is regenerated from committed code with `local_modifications=false`.
- Milestone-audit and full-code-review findings are fixed or logged as accepted future specification gaps.

## Non-Negotiable Rules

- Do not raise the beta gate above `1.25x`.
- Do not add a beta waiver for `m10-error-decomp`.
- Do not remove or merge the failing paired submeasurement to make the row pass.
- Do not treat a dirty-worktree benchmark report as completion evidence.
- Do not optimize from one noisy nanosecond measurement and call it final evidence.
- Do not weaken public `Probability` validation or error-decomposition semantics for benchmark speed.
- Do not use unchecked constructors unless the local formula proves probability bounds and tests cover the proof boundary.
- Do not add new strict thresholds for exact, p100, or independent-to-disjoint pairs until repeated clean reports show enough headroom below `1.25x`.

## Work Loop

### 1. Establish Fresh Evidence

Regenerate a primary baseline and beta report from the current code before changing behavior:

```sh
just bench::baseline --primary --out target/benchmarks/m10-error-decomp-primary-baseline
just bench::primary-beta --baseline target/benchmarks/m10-error-decomp-primary-baseline/baseline.json
```

Inspect whether `m10-error-decomp` is still the blocker.
If the row passes on the fresh run, do not immediately declare success; run focused warm evidence to determine whether the `1.3559322033898304x` result was noise:

```sh
just bench::compare --only m10-error-decomp --warmup --measurement-runs 3 --baseline target/benchmarks/m10-error-decomp-primary-baseline/baseline.json --report target/benchmarks/m10-error-decomp-focused-before
```

If a different comparable row fails after M10 is repaired, update this document or the active performance plan before changing that row.

### 2. Identify The Failing Pair

Inspect paired ratios instead of row medians:

```sh
jq -r '.rows[] | select(.id == "m10-error-decomp") | .measurement_ratios[] | [.stim_name, .stab_name, .relative_ratio, .stim_seconds, .stab_seconds] | @tsv' target/benchmarks/m12-primary-beta/compare.json
```

Record which pair is above `1.25x`, the exact ratio, absolute Stim timing, absolute Stab timing, and whether the failure repeats across at least two warm focused runs.
The renewed blocker must inspect all four pairs, because previous evidence has shown exact disjoint-to-independent and Newton fallback pairs can fail independently.

### 3. Fix Benchmark Shape Before Arithmetic

Decide whether the failing pair measures useful arithmetic or mostly timer overhead.
If the operation is too small for honest strict-gate evidence, fix the benchmark shape before optimizing code.

Acceptable benchmark-shape fixes must:

- keep the `m10-error-decomp` row id unless a split is explicitly documented;
- keep all four paired measurements visible;
- use workloads that faithfully correspond to pinned Stim perf filters;
- keep Stim and Stab pair names stable enough for threshold and profiler-note tooling;
- normalize batched or case-array timings back to seconds per operation;
- add benchmark runner tests that prove expected measurements, comparability class, compare notes, and normalized work are emitted.

Do not change benchmark shape merely to hide a slow pair.
If a faithful larger shape still fails, optimize the implementation.

### 4. Optimize The Real Hot Path

Optimize only after the evidence shape is honest.
Focus first on `crates/stab-core/src/dem/analyze/error_decomp.rs` and the tiny probability helpers in `crates/stab-core/src/ids.rs`.

Investigate in this order:

- missed inlining on tiny probability conversion helpers;
- avoidable `Probability` construction or validation on locally proven intermediate values;
- repeated `Probability::get()` extraction;
- exact zero, identity, and symmetric-case branches;
- Newton solver iteration count and termination checks;
- algebraic fast rejects for disjoint triples that cannot decompose exactly;
- repeated floating-point transformations for fixed benchmark probabilities.

For every fast path, add or update tests that cover exact solved cases, approximate cases, zero-probability edges, invalid probability rejection, and round-trip consistency where relevant.
Preserve analyzer behavior and public error semantics, not just benchmark inputs.

### 5. Keep Gates And Waivers Honest

Keep `m10-error-decomp` as a comparable direct-match row.
Do not edit `benchmarks/m12-primary-beta-waivers.json` for this row.
Keep existing schema-version-2 threshold coverage only for submeasurements that have repeated stable evidence below `1.25x`.
Add or remove submeasurement thresholds only with matching profiler-note updates, threshold-source tests, and focused benchmark evidence in the same change set.

### 6. Synchronize Documentation

Update every affected source in the same change set:

- `docs/plans/GOAL.md`;
- `docs/plans/beta-125-performance-plan.md`;
- `docs/plans/rust-stim-drop-in-rewrite.md`;
- `docs/plans/m12-progress-report.md`;
- `docs/plans/post-beta-fix-report.md`;
- `docs/plans/milestone-spec-gaps.md`, only for true under-specified acceptance criteria;
- `benchmarks/profiler-notes/m12/m10-error-decomp.md`;
- `benchmarks/profiler-notes/m12/optimization-log.json`;
- `benchmarks/m12-primary-thresholds.json`, only if threshold ownership changes.

Do not leave contradictory ratios, row counts, waiver counts, report paths, commit ids, or `local_modifications` claims for the next agent.

### 7. Audit And Review

After implementation and evidence updates, run milestone-audit against this goal and Milestone B4 in `docs/plans/beta-125-performance-plan.md`.
Then run full-code-review.
Fix all correctness, compatibility, benchmark-policy, and documentation findings.
Only log a finding in `docs/plans/milestone-spec-gaps.md` when it is genuinely future-scope under-specification rather than an implementation bug or stale documentation.

## Required Targeted Tests

Use targeted checks during iteration:

```sh
cargo test -p stab-core error_decomp --quiet
cargo test -p stab-core dem_analyzer_pauli_channel_clifford --quiet
cargo test -p stab-bench m10_dem_benchmark_rows_have_stab_compare_runners --quiet
```

Add or update targeted tests for any changed arithmetic branch, benchmark batching rule, measurement name, threshold parser behavior, or compare-note expectation.
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
just bench::primary-beta --baseline target/benchmarks/m10-error-decomp-primary-baseline/baseline.json
just bench::primary-regression --baseline target/benchmarks/m10-error-decomp-primary-baseline/baseline.json --report target/benchmarks/m10-error-decomp-primary-regression
just bench::primary-memory-regression --baseline target/benchmarks/m10-error-decomp-primary-baseline/baseline.json
just maintenance::pre-commit
```

Final completion requires committed-code evidence with `local_modifications=false`.
If the user has not explicitly requested commits, stop after successful uncommitted verification and ask before committing.
