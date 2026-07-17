# Benchmark Contracts

`manifest.csv` is the source-owned benchmark contract manifest for M3 and later performance work.
Each row names the owning implementation milestone, threshold class, runner, upstream Stim source, workload phase, and measurement family.
`just bench::smoke` validates that required M3 contracts remain present, including the primary benchmark matrix, canonical print and convert contracts, result-format convert CLI rows, bit-packed detector conversion contracts, and the M12 performance-hardening target.

The comprehensive follow-up is defined in [../docs/plans/comprehensive-stim-performance-qualification-plan.md](../docs/plans/comprehensive-stim-performance-qualification-plan.md).
PQ0 freezes the schema-versioned feature-disposition overlay in `stim-qualification-suite.json`, including all inherited manifest decisions, correctness dependencies, exact checklist-child and public-API ownership, typed fixture identities, static corpus digests, planned scale families, threshold pairs, waivers, and upstream perf symbols.
Use `just bench::qualification-list`, `just bench::qualification-check`, and `just bench::qualification-regenerate --check` to inspect or validate that checked ledger.
PQ1 is complete at the clean schema-version-13 evidence recorded in [../docs/plans/pq1-performance-harness-progress-report.md](../docs/plans/pq1-performance-harness-progress-report.md). It implements symmetric bounded worker evidence, a faithful pinned-Stim adapter seam, paired confidence intervals, process-memory execution, private committed-source builds, reconstructable build receipts, checked-inventory and current-toolchain replay, and exact report/preflight publication binding. Its protocol-smoke group remains diagnostic. PQ2 extends that harness with source-owned group and scale selection and currently has thirteen correctness-bound product groups. The manifest, M12 primary matrix, threshold files, waiver files, and commands remain authoritative; completed legacy timing pairs are retired only after exact replacement completion evidence, while PQ6 owns memory and scaling graduation.
The required correctness preconditions are planned in [../docs/plans/comprehensive-correctness-qualification-plan.md](../docs/plans/comprehensive-correctness-qualification-plan.md); a timed row may not become comprehensive evidence merely because it runs successfully.
Benchmark operations currently require Unix descriptor-relative filesystem primitives and fail closed on non-Unix hosts; final qualification is already restricted to controlled Linux x86-64 and Linux AArch64 hosts.
Stim CLI benchmark stdin is read at execution time through a bounded `64 MiB` regular nonsymlink repository-file boundary.

Runner meanings:

- `stim-perf`: run pinned C++ Stim's `stim_perf` binary with the row's `stim_perf_filter`.
- `stim-cli`: run pinned C++ Stim's `stim` binary with the row's pipe-delimited `argv` and optional `stdin_path`.
- `contract-only`: reserve a benchmark contract that has no direct pinned C++ executable baseline yet.

Some legacy benchmark ids still contain `contract` after they gain a faithful public CLI baseline.
The `runner` column is authoritative for whether a row is contract-only, not the id suffix.

Generated benchmark run artifacts belong under `target/benchmarks/` and are not source files.
`stim-qualification-suite.json` is the exception: it is a deterministic, checked source-owned contract whose semantic digest is frozen by `ops/bench`, not a machine-specific timing result.
The default baseline command writes `target/benchmarks/baseline/latest/baseline.json` and `target/benchmarks/baseline/latest/report.md`.
Any explicit `--out` value must be a repository-relative path under `target/benchmarks/`.
Use `--only` with exact benchmark ids or milestone names, for example `--only m4-circuit-parse` or `--only M9`.
Pass `--primary` to record only the frozen M12 primary matrix, using the same M4 through M11 row selection as `just bench::compare --primary`.
Post-beta PF rows are planning placeholders for `docs/plans/partial-feature-closure-plan.md`.
They are excluded from `--primary` until a later milestone replaces a placeholder with a real runner, source-owned comparability notes, and explicit threshold policy.

`just bench::compare` reads `target/benchmarks/baseline/latest/baseline.json` by default.
Pass `--baseline <path>` to compare against a different generated baseline report.
Use `--only` on compare commands for focused probe evidence against baseline reports recorded with the same row filter.
Pass `--primary` to select the frozen M12 primary matrix, which currently includes M4 through M11 benchmark rows except metadata anchors, explicit `non-primary-report-only` rows, post-beta PF planning rows, and the M12 placeholder row.
Pass `--profile release` to record the intended Cargo profile in compare output; the `just bench::compare` recipe builds the benchmark ops binary with Cargo's release profile before running the subcommand.
Pass `--report target/benchmarks/latest` or another repository-relative directory below `target/benchmarks/` to write `compare.json` and `report.md`.
Compare row ratios use paired measurement ratios when comparable submeasurements are available.
Every compare row has a machine-readable comparability class recorded in `compare.json` as `comparability`.
The class is derived from the source-owned compare-note prefix or from a `contract-only` runner with no note.
Supported classes are `direct-match`, `cli-baseline`, `contract-representative`, `contract-proxy`, `contract-smoke`, `partial-match`, `report-only`, and `contract-only`.
`direct-match` rows match pinned Stim operation shape closely enough to use exact-name submeasurement pairs or positional submeasurement pairs when counts match.
`cli-baseline` rows compare Stab's implementation of the same public CLI command, input, and output contract against pinned Stim's public CLI.
`contract-representative`, `contract-proxy`, `contract-smoke`, `partial-match`, and `report-only` rows are narrower M12 beta evidence classes; their note must explain the missing exact parity or representative scope before the row is treated as reviewable benchmark evidence.
`contract-only` rows do not prove a Stab-vs-Stim timing ratio and require a source-owned beta waiver when they are selected by `--require-beta-gate`.
Pairs are matched by normalized measurement names, or by position for `direct-match` rows whose Stim and Stab measurement counts match.
When paired ratios exist, `direct-match` and `cli-baseline` gates use the worse of the row median ratio and the worst paired ratio; `partial-match` gates use the worst paired ratio so unmatched Stab contract extras remain visible without deciding a Stim-relative gate; rows without paired evidence use the row median ratio.
The JSON report records paired evidence in `measurement_ratios`, and the Markdown report prints the worst pair in the `Ratio Source` column.
Tiny direct-match Stab measurements may use batched timing to reduce clock-noise dominance, but they still report seconds per operation.
Pass `--require-beta-gate` to fail when any selected row does not prove a pass against the 1.25x pinned-Stim beta performance gate.
Pass `--beta-waivers <path>` with `--require-beta-gate` to accept only measured no-ratio rows whose manifest `runner` is `contract-only` and whose lack of a comparable pinned-Stim ratio is explained by a source-owned waiver.
Waivers do not apply to missing baselines, pending runners, invalid baselines, or rows with measured ratios above the beta gate.
Unused waivers fail the gate so the file stays in sync when a row becomes comparable or leaves the selected matrix.
`m12-primary-beta-waivers.json` is the source-owned M12 waiver file for the remaining primary rows that have Stab-side timing evidence but no faithful public Stim baseline.
Run `just bench::primary-beta --baseline <primary-baseline.json>` to check the M12 beta timing gate with source-owned profiler notes and waivers.
Pass `--require-profiler-notes` with `--report` to fail when a row slower than 1.5x pinned Stim lacks a valid note at `<report>/profiler-notes/<benchmark-id>.md`.
Profiler notes must include non-empty `Dominant cost:` and `Next owner action:` lines.
Pass `--profiler-notes-dir benchmarks/profiler-notes/m12` to validate source-owned notes instead of report-local notes.
M12 rows optimized during performance hardening are tracked in `profiler-notes/m12/optimization-log.json`.
Optimization-log rows use schema version 2 and record before and after reports, source-owned before and after ratios, gate status, hot-path status, source profiler-note paths for after rows still above the 1.5x profiler-note threshold, dominant-cost evidence, implementation summary, semantic checks, and follow-up policy.
Pass `--thresholds <path>` to fail when a selected row with a configured regression threshold exceeds its maximum relative ratio or lacks a comparable Stab-vs-Stim ratio.
Threshold files must not contain stale ids: every configured threshold id must be selected by the compare run so row renames and matrix changes cannot silently drop a regression guard.
`m12-primary-thresholds.json` is the source-owned M12 timing-regression threshold file for primary rows that have reached the 1.25x pinned-Stim regression gate with enough local headroom to make an initial stable threshold useful.
Run `just bench::primary-regression --baseline <primary-baseline.json> --report target/benchmarks/<name>` to check those source-owned thresholds and checked timing-regression waivers for the frozen primary matrix after a Stab-side warmup pass, three recorded measurement runs, and source-owned profiler-note validation.
The recipe defaults to the latest generated baseline path when no explicit `--baseline` is passed.
The scheduled `.github/workflows/m12-benchmarks.yml` workflow records a fresh primary pinned-Stim baseline on a GitHub runner, runs this source-owned threshold gate, and uploads the generated baseline and compare reports.
Threshold files are JSON with schema version 1 or 2.
Schema version 1 rows use only row-level thresholds:

```json
{
  "schema_version": 1,
  "rows": [
    {
      "id": "m4-circuit-parse",
      "max_relative_ratio": 1.25
    }
  ]
}
```

Every threshold id must match a selected benchmark row, and every selected benchmark row not present in the threshold file is reported as `not-configured`.
If `--regression-waivers <path>` is also passed, selected measured `contract-only` rows that are not configured in the threshold file, have no comparable ratio, and have source-owned waiver entries are reported as `waived-not-thresholdable` instead of ambiguous `not-configured`.
Timing-regression waivers do not apply to comparable rows, rows with ratios, unselected rows, pending rows, or configured threshold rows, and unused waivers fail the gate so the waiver file cannot drift when a row becomes comparable.
`m12-primary-regression-waivers.json` is the source-owned M12 timing-regression waiver file for primary no-ratio rows that are measured but cannot have a faithful pinned-Stim 1.25x threshold.
Schema version 2 is backward compatible with row-level thresholds and additionally supports exact submeasurement thresholds for rows whose stable direct measurements can be guarded before the whole mixed row is stable:

```json
{
  "schema_version": 2,
  "rows": [
    {
      "id": "m10-error-decomp",
      "measurement_thresholds": [
        {
          "stim_name": "disjoint_to_independent_xyz_errors_approx_p10",
          "stab_name": "stab_disjoint_to_independent_xyz_errors_approx_p10",
          "max_relative_ratio": 1.25
        }
      ]
    }
  ]
}
```

Submeasurement thresholds fail when the selected compare report lacks the named paired evidence or when the paired ratio exceeds the configured ratio.
The timing-regression report is the authoritative completion evidence for configured schema-version-2 threshold pairs because threshold application materializes any configured explicit pair that was not already produced by automatic exact-name or positional pairing.
Threshold ids must be benchmark-id safe because they are matched against report rows and may also be used by generated benchmark tooling.
Beta and timing-regression waiver files are JSON with schema version 1:

```json
{
  "schema_version": 1,
  "rows": [
    {
      "id": "m10-dem-print-contract",
      "reason": "Pinned Stim has no public comparable baseline for this exact workload.",
      "follow_up": "Replace the waiver if a faithful baseline becomes available."
    }
  ]
}
```

Waiver ids must be benchmark-id safe.
Reasons and follow-up text must be non-empty because they are copied into the compare report as durable beta-gate evidence.
Use `just bench::compare-allocations` to build `stab-bench` with the optional `count-allocations` feature and pass `--track-allocations` automatically.
Allocation tracking runs an extra Stab-side measurement pass per reported measurement and records allocation counts, maximum live allocated bytes, sampled resident bytes, and sampled resident-byte deltas in `compare.json`; use plain `just bench::compare` for timing-gate evidence.
Pass `--require-memory-gate --memory-baseline <compare.json>` with `just bench::compare-allocations` to compare selected rows against the first complete Stab memory report.
Schema-version-1 memory baselines keep the historical absolute resident-byte check for compatibility.
Schema-version-2 memory baselines fail rows missing current or baseline allocation bytes, rows missing current or baseline resident-delta bytes, rows whose `stab_allocation_bytes_max` exceeds the baseline by more than 25 percent, and rows whose `stab_resident_delta_bytes_max` exceeds the baseline by more than 25 percent plus a 64 KiB absolute slack for page-granular RSS sampling noise.
`m12-primary-memory-baseline.json` is the source-owned M12 memory-regression baseline for the frozen primary matrix and records `stab_allocation_bytes_max`, `stab_resident_bytes_max`, and `stab_resident_delta_bytes_max`.
Run `just bench::primary-memory-regression --baseline <primary-baseline.json>` to check the source-owned memory baseline with allocation and resident-memory tracking, profiler-note validation, and a report at `target/benchmarks/m12-primary-memory-regression`.
Compare prints Stab-side timings for rows whose implementation milestone has a runner and prints pending rows explicitly for future milestones.
When a comparison runner reports workload-specific rates or comparability notes, treat those notes as part of the benchmark evidence.
For example, M5 labels Stab-only contract-smoke bit-kernel workloads separately from upstream Stim perf rows until M12 introduces optimized parity thresholds.
M8 sample compare rows split Stab core sampler compilation, one-shot latency, and batch throughput in-process; those report-only rows are not a strict CLI-vs-CLI performance gate, and the probability-util row measures the direct Stab biased-random utility API against pinned Stim's probability utility perf filters.
M7 convert compare rows exercise `stab_cli::run_from(["stab", "convert", ...])` for representative result-format conversions over source-owned fixtures covering dense text, dense packed, sparse `dets`, `ptb64` input, circuit layout, DEM layout, raw bit width, and observable side-output overhead.
Pinned Stim v1.16.0 rejects `convert --in_format=01 --out_format=ptb64`, so `m7-convert-01-to-ptb64` is a contract-only Stab timing row with source-owned beta and timing-regression waivers instead of a fake CLI ratio.
When a row is contract-only, compare may report Stab-side timing with `stim=contract-only`; that is not a Stab-vs-Stim performance comparison for the row.
Pass `--strict` to fail when any selected row is still pending, missing from the selected baseline report, backed by an invalid placeholder baseline row, contract-only without a Stab-side measurement, or backed by baseline metadata that does not match pinned Stim v1.16.0.

PQ1 and later executable qualification groups are separately owned by `qualification-runtime-groups.json`. Schema version 4 binds group claim class, baseline eligibility, workload and measurement IDs, immutable named scales, semantic work counts, exact fixture byte counts and digests, exact correctness cases, an owner, any source-owned profiler note, and any exact comparator source paths and SHA-256 digests to the frozen performance inventory. `qualification-run` selects `--group` and `--scale`; callers cannot replace source-owned work counts or input identities. `qualification-baseline.json` must have one exact entry per runtime group: report-only groups have no measurement thresholds, and threshold-eligible groups must have neither missing nor stale measurement rules. `just bench::qualification-check` validates both files together, requires exact set equality between promotable runtime groups and implemented `primary-1.25` inventory groups, and rejects a missing or stale profiler-note or comparator-source digest.

Thirteen PQ2 product groups are executable: parser, canonical printer, gate-name hashing, SIMD-word popcount, dense XOR, separate early-hit, all-zero, and late-hit `not_zero` scans, sparse row XOR, sparse item toggle, allocating BitMatrix transpose, square in-place BitMatrix transpose, and non-identity Pauli-string right multiplication.
The parser and printer scales contain 64, 4,096, and 65,536 deterministic instructions; gate lookup contains 82, 5,248, and 335,872 hashes; popcount and dense XOR contain 4,096, 262,144, and 16,777,216 deterministic bits; each `not_zero` group contains 10,000, 640,000, and 40,960,000 logical bits; sparse row XOR contains 1,997, 127,808, and 8,179,712 row operations; sparse item toggle contains 7, 448, and 28,672 item operations; each transpose group contains 65,536, 4,194,304, and 268,435,456 logical matrix bits at dimensions 256, 2,048, and 16,384; and Pauli multiplication contains 10,000, 100,000, and 1,000,000 logical qubits.
Each executable scale binds exact semantic work and generated input identity.
The three `not_zero` groups deliberately remain independent because early termination and full scans execute different work, the two sparse-XOR groups remain independent because row symmetric difference and sorted item insertion or removal are different algorithms, and allocating and in-place transpose remain independent because their allocation and result-lifetime contracts differ. Pauli multiplication uses dense non-identity operands because the inherited identity-only callbacks exercise Stab's intentional identity-right fast path and cannot qualify full-width mutation.
The generated inventory and runtime group bind adapter call sites and isolated comparator implementations by path and digest, and the materialized adapter receipt must match them before invocation or report replay.
Product runs require exact source-owned CQ2 cases plus controller-approved correctness request and completion digests.
Correctness evidence may use any producer-valid normal run directory below `target/qualification/`; the performance consumer applies the same path boundary before reopening the artifacts.
Private Stab build-receipt schema version 2 hashes the ordered framed collection of `worker.rs`, `worker/bits.rs`, `worker/not_zero.rs`, `worker/sparse_xor.rs`, `worker/transpose.rs`, `worker/pauli.rs`, and `worker/error.rs` from the materialized commit.
Adapter receipt schema version 9, contract-preflight schema version 9, and qualification report schema version 27 retain 104 accepted or rejected worker receipts and exact worker identities for offline replay.
Each implementation still calibrates independently under the 2-second ceiling.
Standard common batches keep both sides between 250 milliseconds and 2 seconds; derived wide-ratio mode preserves identical work while allowing only the implementation that selected fewer iterations to exceed 2 seconds, with a hard 20-second common ceiling below the unchanged 30-second per-invocation timeout.
A fabricated mode or incorrectly refingerprinted preflight transplanted from another worker pair is rejected.
After committing changes to worker sources, build inputs, or receipt policy, run `just bench::qualification-worker-reproducibility` from the clean unchanged commit to repeat that contract across two isolated builds and require exact source, build, binary, and preflight identities.
Probe the transpose adapters with `just bench::qualification-probe --group pq2-bit-matrix-transpose-in-place-adapter-smoke` and `just bench::qualification-probe --group pq2-bit-matrix-transpose-allocating-adapter-smoke`; each probe validates exact deterministic output and rejects below-minimum, non-square, unaligned, over-cap, and semantic-work-overflow requests before invoking either worker.
Probe Pauli multiplication with `just bench::qualification-probe --group pq2-pauli-string-multiply-adapter-smoke`; the probe validates dense non-identity output, returned phase, operand preservation, the accepted maximum, and every source-owned pre-setup rejection.
The reproducibility command fails before private builds when the checkout is dirty.
Reports retain setup and peak RSS separately.
Memory observations remain report-only until PQ6 defines explicit cross-scale growth slack.
Any failed or noisy promotable group must retain its source-owned owner and profiler-note contract during offline replay.

The sparse-XOR completion receipts at clean pre-migration revision `e2f6292f473b034d8886fc100039c7a78c4a3989` authorized retirement of exactly the two duplicate `m5-sparse-xor` M12 timing pairs. The current inventory preserves that migration and keeps its M12 memory baseline until PQ6 provides equal or stronger memory evidence. Clean post-migration revision `7b43b46d1c08f669264d009b8d3872ce86838f0e` regenerated and replayed the complete sparse row and item evidence chain at historical performance inventory `8cc3ab3eb88faaf539c3c0eabaf3865ad421d8f67b14549cb4c7acc71faf2406`.

The BitMatrix transpose completion receipts at clean pre-migration revision `e660c91cff142b611f52a0a28a36e0a3d15670ed` authorized retirement of only the heterogeneous `m5-simd-bit-table` M12 timing threshold, and clean post-migration revision `1264d885087761b19b37beded47811cc0c117e4d` completed the first replacement chain. Independent review then strengthened the exact edge oracle and pre-setup semantic-work overflow contract. Clean revision `f912cc3af1f13cc9fab798d69937c155d37d83a0` regenerated and replayed the reviewed two-case correctness preflight, all 12 first-attempt full and soak reports, all regressions, four method-specific rollups, and both method-specific completion receipts at the current inventory. The exact allocating and in-place groups retain both pinned Stim transpose symbols as provenance, while the unrelated legacy row-XOR proxy is not paired to either group. The current inventory preserves `m5-simd-bit-table` in `m12-primary-memory-baseline.json` until PQ6 provides equal or stronger memory evidence; exact results and hashes are in `docs/plans/pq2-bit-matrix-transpose-qualification-progress-report.md`.

The Pauli right-multiplication completion receipt at clean pre-migration revision `3a0fcd814f8d1a9441420ab85edf3d757572ba93` authorized retirement of only the identity-only `m6-pauli-string` M12 timing threshold, its three explicit timing pairs, and their temporary scale mappings. Post-migration inventory `7eedf59cb65d2bd244accc56973d7831001191cd62511c56b05a5cd7ed612ac6` preserves the legacy memory baseline for PQ6 and requires a fresh complete evidence chain before source-current closure.

The historical `m4-circuit-canonical-print` microbenchmark remains available as a non-primary Stab-only diagnostic, but it no longer owns a beta, timing-regression, or memory waiver. `PERFQ-M4-CIRCUIT-CANONICAL-PRINT` is the sole source-owned Stim-relative canonical-print gate.

Use `just bench::qualification-rollup --group <group> --tier <full-or-soak> --input <scale-report> ... --out <rollup-directory>` to bind one complete architecture-scoped scale family. Every input and the output must be a distinct, conservatively named direct child of `target/benchmarks/qualification/`. The command must run from the same clean committed revision as the source reports; it records that producer state, reopens each bounded canonical report and preflight, requires exactly one promotable report for every source-owned scale, requires one commit, inventory pair, correctness preflight, group contract, host profile, CPU identity, architecture, target triple, toolchain, exact Stim and Stab worker source, build, binary, canonical-contract-preflight identity, and tier across the family, preserves failed and noisy outcomes, and atomically verifies that no source artifact changed while the rollup is published. A family is failed when any measurement failed, otherwise noisy when any measurement is noisy, and passed only when every measurement passed. Run `just bench::qualification-rollup-report --input <rollup-directory>` from that clean revision to reopen the checked contract and every exact source artifact, reconstruct the canonical rollup and preflight bytes, reject output-path, source-digest, outcome, count, or derived-field tampering, and atomically refresh the Markdown only after compare-and-swap checks. Full and soak families require separate rollups; AArch64 and x86-64 evidence must never share a rollup.

Use `just bench::qualification-completion --group <group> --full-input <report> ... --soak-input <report> ... --full-rollup <rollup> --soak-rollup <rollup> --out <completion-directory>` to close the machine-checkable evidence sequence for one product group. Every source report, rollup, and output must be a distinct direct child of `target/benchmarks/qualification/`. The command runs the same typed handlers as the named standalone CLI operations and records canonical argument vectors with exit status zero only after each handler returns success; any handler error aborts publication. It requires reproducible private workers that exactly match all source reports, a deterministic source-owned adapter probe with matching Stim binary identity and Stab source identity, idempotent report and rollup replays, passing source-owned regression at every full and soak scale, passing full and soak rollups with one exact CPU, host policy, host profile, architecture, target, toolchain, correctness preflight, and worker identity, and unchanged source artifact digests through atomic publication. The canonical receipt is stored as `report.json`, its digest and complete step and evidence directory bindings are repeated in `preflight.json`, and `report.md` is derived. Run `just bench::qualification-completion-report --input <completion-directory>` to rerun the complete machine-checkable sequence and require byte-identical report and preflight reconstruction. Human milestone audit and independent code review are intentionally excluded from the receipt instead of being represented as self-certified operations.
