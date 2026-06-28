# Post-Beta Threshold Completion Plan

## Summary

This plan finishes the two remaining post-beta timing items for implemented Stab surfaces.
The first item is to remove ambiguous `not-configured` timing-regression rows by making no-ratio rows explicitly waived and checked.
The second item is to complete the real timing work for comparable implemented rows: `m4-gate-lookup`, `m5-sparse-xor`, and the `m8-measure-reader-*` family.

This plan is deliberately narrower than `docs/plans/post-beta-timing-hardening-plan.md`.
It does not reopen `m5-simd-bits` or `m10-error-decomp`, which already have source-owned schema-version-2 thresholds for their stable direct submeasurements and documented exclusions for tiny unstable surfaces.
It also does not reopen intentionally deferred Stim parity or ecosystem surfaces such as Python, JS/WASM, Crumble, diagrams, `explain_errors`, `repl`, QASM/Quirk, GPU, sweep-conditioned conversion, `m2d --ran_without_feedback`, full ErrorMatcher provenance, or new public graph/vector simulator APIs.

## Lessons Applied

Use `docs/plans/lessons-learned.md` as a guardrail while executing this plan.
The key lessons for this work are:

- Do not treat a benchmark row as complete until its exact submeasurements, comparability class, and threshold or waiver decision are machine-checkable.
- Do not use non-strict compare reports, dirty-worktree reports, or stale local paths as completion evidence.
- Do not hide slow submeasurements behind a passing row median.
- Do not add a waiver for a row that can be made comparable with a better benchmark shape.
- Do not add a strict `1.25x` threshold to timer-noisy tiny evidence just to remove a `not-configured` label.
- Keep waiver reasons, threshold files, profiler notes, reports, and plan docs synchronized in the same change set.

## Starting State

The clean post-beta timing-regression report from commit `871900eda4a6880bafe7830d2ad264febfeb9f00` had 76 primary rows, 65 `pass` rows, and 11 `not-configured` rows.

The 11 rows are:

| Row | Current class | Current finish problem |
| --- | --- | --- |
| `m4-circuit-canonical-print` | `contract-only` | No faithful pinned-Stim v1.16.0 timing ratio exists. |
| `m4-gate-lookup` | `partial-match` | Canonical lookup is beta-safe but too tiny and baseline-sensitive for strict threshold ownership. |
| `m5-sparse-xor` | `direct-match` | Table row-XOR is close to threshold, while tiny item-XOR remains above `1.25x`. |
| `m7-convert-stim-canonical` | `contract-only` | Pinned Stim has no matching canonical `.stim` conversion timing surface. |
| `m8-measure-reader-01` | `partial-match` | Public Stab dense visitor is compared against mixed pinned-Stim dense and sparse internal reader filters. |
| `m8-measure-reader-b8` | `partial-match` | Public Stab dense visitor is compared against mixed pinned-Stim dense and sparse internal reader filters. |
| `m8-measure-reader-r8` | `partial-match` | Public Stab dense visitor is compared against mixed pinned-Stim dense and sparse internal reader filters. |
| `m8-measure-reader-hits` | `partial-match` | Public Stab dense visitor is compared against mixed pinned-Stim dense and sparse internal reader filters. |
| `m8-measure-reader-dets` | `partial-match` | Public Stab dense visitor is compared against mixed pinned-Stim dense and sparse internal reader filters. |
| `m8-measure-reader-ptb64-contract` | `contract-only` | Pinned Stim v1.16.0 has `ptb64` reader tests but no `ptb64` reader perf filter. |
| `m10-dem-print-contract` | `contract-only` | Pinned Stim has no matching DEM canonical-print CLI or `stim_perf` row. |

## Finish Target

The final timing-regression report should have no ambiguous `not-configured` rows.
Comparable implemented rows must have strict `1.25x` row-level or schema-version-2 submeasurement thresholds.
True no-ratio contract-only rows must have source-owned regression waivers and should report a distinct status such as `waived-not-thresholdable`.

The expected final classification is:

- `pass`: all comparable rows and submeasurements with stable strict thresholds.
- `waived-not-thresholdable`: true no-ratio contract-only rows with checked source-owned waiver entries.
- `not-configured`: zero rows.

## Milestone 0: Establish Fresh Baseline Truth

Regenerate a starting evidence set before changing benchmark logic or performance code.
The first run may be dirty during implementation, but final acceptance evidence must be regenerated from committed code with `local_modifications=false`.

Commands:

```sh
just bench::baseline --primary --out target/benchmarks/timing-finish-baseline
just bench::compare --primary --warmup --measurement-runs 3 --require-profiler-notes --profiler-notes-dir benchmarks/profiler-notes/m12 --baseline target/benchmarks/timing-finish-baseline/baseline.json --report target/benchmarks/timing-finish-compare
just bench::primary-regression --baseline target/benchmarks/timing-finish-baseline/baseline.json --report target/benchmarks/timing-finish-regression
```

Acceptance:

- The report records the Stab commit id, pinned Stim tag and commit, warmup status, measurement-run count, and local-modification status.
- The initial row ledger identifies every remaining unconfigured row and preserves the reason it is not yet threshold-owned.
- No implementation work starts from stale or missing benchmark evidence.

## Milestone 1: Make No-Ratio Rows Explicitly Finished

The contract-only rows should not remain ambiguous `not-configured` rows, but they also must not receive fake strict thresholds.
Add source-owned regression-threshold waiver support for rows that cannot produce a faithful pinned-Stim ratio.

Rows:

- `m4-circuit-canonical-print`
- `m7-convert-stim-canonical`
- `m8-measure-reader-ptb64-contract`
- `m10-dem-print-contract`

Tasks:

- Add `benchmarks/m12-primary-regression-waivers.json` or an equivalent source-owned waiver file dedicated to timing-regression threshold waivers.
- Require every waiver entry to include the row id, reason, follow-up owner action, and evidence that the row is measured but not thresholdable.
- Teach the timing-regression gate to report a distinct waiver status for checked no-ratio rows instead of `not-configured`.
- Reject stale waiver ids, waivers for selected comparable rows, waivers for rows with a faithful ratio, duplicate waiver ids, missing reasons, and missing follow-up text.
- Keep `benchmarks/m12-primary-beta-waivers.json` separate unless the implementation deliberately unifies beta and regression waiver schemas with explicit status fields.

Tests:

```sh
cargo test -p stab-bench thresholds --quiet
```

Add focused tests for:

- valid no-ratio contract-only waiver handling;
- stale waiver ids;
- waiver applied to a comparable row;
- waiver applied to an unselected row;
- missing waiver reason or follow-up;
- duplicate waiver ids;
- compatibility with existing schema-version-1 and schema-version-2 threshold parsing.

Acceptance:

- The four no-ratio rows no longer appear as ambiguous `not-configured` rows in timing-regression output.
- The report and Markdown table identify them as source-owned threshold waivers, not unresolved benchmark failures.
- Waivers are machine-checked and cannot silently drift.

## Milestone 2: Finish `m8-measure-reader-*`

The reader rows are user-visible and currently beta-safe, but strict threshold ownership requires faithful paired evidence.
Do benchmark-shape work before optimization.

Rows:

- `m8-measure-reader-01`
- `m8-measure-reader-b8`
- `m8-measure-reader-r8`
- `m8-measure-reader-hits`
- `m8-measure-reader-dets`

Tasks:

- Split each reader row into explicit dense and sparse Stab submeasurements that can be paired with the pinned Stim reader filters.
- Match pinned Stim filter names where possible: `read_01_dense_per10`, `read_01_sparse_per10`, `read_b8_dense_per10`, `read_b8_sparse_per10`, `read_r8_dense_per10`, `read_r8_dense_per100`, `read_r8_sparse_per10`, `read_r8_sparse_per100`, `read_hits_dense_per10`, `read_hits_dense_per100`, `read_hits_sparse_per10`, `read_hits_sparse_per100`, `read_dets_dense_per10`, `read_dets_dense_per100`, `read_dets_sparse_per10`, and `read_dets_sparse_per100`.
- Use Stab public readers where they represent the public contract, but shape benchmark fixtures so dense and sparse record patterns are explicit and paired.
- Use reusable record buffers and bounded streaming readers in the benchmark path.
- Preserve malformed-input rejection for invalid bytes, malformed text records, truncated packed records, record-width mismatches, zero-width packed records, and unsupported detection-output `ptb64` routes.
- Keep `m8-measure-reader-ptb64-contract` under Milestone 1 because pinned Stim has no matching perf filter.

Tests:

```sh
cargo test -p stab-core result_formats --quiet
cargo test -p stab-bench measure_reader --quiet
cargo test -p stab-bench thresholds --quiet
```

If exact test names differ, add or run the closest focused tests that prove:

- exact round trips for `01`, `b8`, `r8`, `hits`, and `dets`;
- dense and sparse reader fixtures both decode correctly;
- benchmark runner emits every expected dense and sparse submeasurement id;
- stale schema-version-2 submeasurement thresholds fail;
- missing submeasurement evidence fails.

Acceptance:

- Every supported reader row has paired dense and sparse evidence where pinned Stim exposes dense and sparse filters.
- Every stable direct pair below `1.25x` is guarded in `benchmarks/m12-primary-thresholds.json`.
- Any reader pair still left unthresholded has a source-owned profiler note explaining why the evidence is not stable or not faithful enough.
- The timing-regression report has no `not-configured` reader rows except no-ratio `ptb64`, which is handled by Milestone 1.

## Milestone 3: Finish `m5-sparse-xor`

The row is a direct-match row, so the right finish state is strict submeasurement threshold ownership after real evidence and any needed data-structure optimization.
Current clean evidence shows table row-XOR near `1.05x` and item-XOR near `1.47x`.

Tasks:

- Keep table row-XOR and item-XOR as separate benchmark submeasurements.
- Increase item-XOR work enough that the measurement is not just a tiny 7-item timer-noise surface.
- Report normalized row-XOR and item-XOR rates.
- Profile allocation cost, sorted-merge cost, binary-search cost, insertion/removal cost, and small-row behavior separately before changing the data structure.
- Optimize `SparseXorVec::xor_item` with an adaptive small-row path and binary-search insertion/removal for larger rows if profiling confirms the bottleneck.
- Optimize table row-XOR only if repeated evidence shows it cannot own a stable strict threshold as-is.
- Preserve the invariant that sparse XOR rows are sorted and unique after every public constructor and mutating operation.

Tests:

```sh
cargo test -p stab-core --test bits bits_sparse_xor --quiet
cargo test -p stab-bench m5_sparse_xor --quiet
cargo test -p stab-bench thresholds --quiet
```

If exact test names differ, add or run focused tests that prove:

- empty rows, identical rows, disjoint rows, duplicate inputs, repeated `xor_item`, and mixed row-XOR plus item-XOR sequences;
- property-style equivalence against a reference sorted set or symmetric-difference implementation;
- sorted-unique invariants after every mutation;
- schema-version-2 threshold pass and fail behavior for row-XOR and item-XOR pairs.

Acceptance:

- `SparseXorTable_SmallRowXor_1000` to `stab_sparse_table_row_xor_1000` has stable clean evidence below `1.25x` and owns a threshold.
- `SparseXorVec_XorItem` to the Stab item-XOR submeasurement has stable clean evidence below `1.25x` and owns a threshold.
- If the benchmark shape changes to a larger direct pair, the profiler note must explain why the new work unit is faithful to the pinned Stim filter instead of hiding a slow operation.

## Milestone 4: Finish `m4-gate-lookup`

The current surface is sub-microsecond and baseline-sensitive, so stabilize the benchmark before optimizing.
The goal is strict threshold ownership for canonical lookup if the evidence becomes meaningful.

Tasks:

- Replace the tiny all-gates lookup timing with larger repeated in-process lookup sets.
- Keep canonical uppercase names, aliases, lowercase normalization, and invalid names as separate Stab submeasurements.
- Pair only faithful canonical lookup evidence with pinned Stim `gate_data_hash_all_gate_names` unless a faithful pinned-Stim filter exists for the other lookup classes.
- Black-box benchmark inputs and outputs so lookup work cannot be optimized away.
- If stabilized evidence still shows real overhead, add an allocation-free lookup path derived from the canonical gate metadata source of truth.
- Do not introduce a hand-maintained duplicate gate table.
- Preserve alias behavior, case-normalization behavior, canonical-name round trips, metadata single-sourcing, and `UnknownGate` errors.

Tests:

```sh
cargo test -p stab-core --test stim_format gate_lookup --quiet
cargo test -p stab-bench m4_gate_lookup --quiet
cargo test -p stab-bench thresholds --quiet
```

If exact test names differ, add or run focused tests that prove:

- every canonical gate name is accepted;
- known aliases are accepted and resolve correctly;
- lowercase variants behave as required by the current public contract;
- invalid names fail with the expected domain error;
- generated or table-driven lookup data remains derived from canonical metadata.

Acceptance:

- The canonical lookup pair has stable repeated evidence below `1.25x` and owns a schema-version-2 threshold.
- Alias, lowercase, and invalid lookup contract extras are either thresholded only when faithful Stim pairs exist or documented as Stab-only contract extras.
- No sub-100ns unstable surface is forced into the threshold file.

## Milestone 5: Synchronize Documentation And Evidence

Update documentation in the same change set as code, thresholds, and waivers.
Do not leave stale row counts, stale report paths, stale status labels, or stale "must regenerate" wording.

Files to check and update:

- `benchmarks/m12-primary-thresholds.json`
- `benchmarks/m12-primary-regression-waivers.json` or the chosen waiver source
- `benchmarks/m12-primary-beta-waivers.json` if shared waiver semantics change
- `benchmarks/README.md`
- `benchmarks/profiler-notes/m12/m4-gate-lookup.md`
- `benchmarks/profiler-notes/m12/m5-sparse-xor.md`
- `benchmarks/profiler-notes/m12/m8-measure-reader.md`
- `docs/plans/post-beta-fix-report.md`
- `docs/plans/post-beta-timing-hardening-plan.md`
- `docs/plans/m12-progress-report.md`
- `docs/plans/milestone-spec-gaps.md` only for true new under-specification issues

Acceptance:

- Docs and machine-readable sources agree on the final number of thresholded rows, waived rows, and remaining unconfigured rows.
- `docs/plans/post-beta-fix-report.md` no longer implies that already regenerated clean evidence is still missing unless it is intentionally referring to the next final run.
- Every unthresholded contract-only row has a source-owned waiver and a clear follow-up owner action.

## Milestone 6: Audit, Review, And Final Gate

Run milestone-audit and full-code-review after the row work and documentation are synchronized.
Fix implementation, test, benchmark, compatibility, security, and documentation findings.
Log only true under-specification findings in `docs/plans/milestone-spec-gaps.md`.

Suggested milestone-audit names:

- `post-beta-threshold-completion: regression-waivers`
- `post-beta-threshold-completion: m8-measure-reader`
- `post-beta-threshold-completion: m5-sparse-xor`
- `post-beta-threshold-completion: m4-gate-lookup`
- `post-beta-threshold-completion: final-evidence`

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

Final acceptance:

- Final benchmark evidence is generated from committed code with `local_modifications=false`.
- The timing-regression report has zero ambiguous `not-configured` rows.
- Comparable rows are strict-threshold `pass`.
- True no-ratio rows are checked regression waivers.
- The worktree is clean or the user has explicitly accepted uncommitted follow-up work.
