# PQ1 Paired Performance Harness Progress Report

> Current-evidence note, 2026-07-14: the first CQ2 exact-parent mapping refresh changes the correctness inventory digest to `5d1fc9d21e511e13bef5ceb476dbcf9dd20ed067339edd2891013992fb06ced5` and the dependent performance inventory digest to `a7177e298b5e1f05979b871704514fdf2650070a7c48e5d72c6fb48bb80d13bf`. Clean current-digest schema-version-13 PR, full, and soak reports from revision `add37ccb6dc52b0ac96b37397f6b012de0bcd6a4` are host-verified and pass offline validation; see `docs/plans/cq2-deterministic-qualification-progress-report.md`. They remain diagnostic infrastructure evidence, not product performance.

## Status

PQ1 is complete as of 2026-07-14.

The final clean evidence was generated from Stab commit `bfef511ccaa57c61cbe209c41d89d77ba8f52eee` against Stim v1.16.0 commit `e2fc1eca7fd21684d433aa5f10f4504ea4860d07`.
All three reports use performance inventory digest `cc9f6cabfb9a3245d9c52000e82c8f1bec76aed605f3563d1a15244d327c3fbd`, correctness inventory digest `b2909c677a66e2b034c8ab26e8dc1b2ad78e63900b2d83f938a8c4e725852141`, and runtime-group contract digest `4d7d15bf677bd1930cacfde48f06196852857bbff1f29d134058147d7ba1de06`.

PQ1 proves the qualification infrastructure, not Stab product-speed parity.
The sole executable group, `pq1-adapter-protocol-smoke`, is immutable diagnostic infrastructure with `report-only` baseline eligibility, zero regression thresholds, no CQ product cases, and `promotable=false`.

## Completion Matrix

| PQ1 requirement | Status | Evidence |
| --- | --- | --- |
| Schema-versioned performance inventory and validator | Satisfied | `benchmarks/stim-qualification-suite.json`; `just bench::qualification-check`; checked performance digest above |
| Symmetric bounded process execution | Satisfied | `ops/bench/src/qualification/runtime/process.rs`; success, exit, signal, timeout, output-limit, process-group cleanup, start-barrier, affinity, and file-limit tests |
| Pinned-Stim adapter and Stab worker protocol | Satisfied | `benchmarks/stim_adapter/main.cc`; `ops/bench/src/qualification/runtime/adapter.rs`; `stab_build.rs`; `worker.rs`; protocol and build-receipt tests |
| Deterministic calibration and paired statistics | Satisfied | `calibration.rs`; `statistics.rs`; three warmups; 3, 9, and 15 retained pairs; fixed-seed bootstrap interval and paired-ratio MAD tests |
| Exact measurement and output binding | Satisfied | Runtime-group registry, semantic preflight at the common calibrated batch, parent-derived work counts, exact measurement pairing, and hostile report mutation tests |
| CQ correctness preflight seam | Satisfied | Canonical CQ request, report, completion, preflight, and per-case execution receipts are independently reconstructed; a real 410-case CQ soak artifact passed the consumer schema probe after resolved oracle and ops selector digests were handled correctly |
| Host, provenance, toolchain, and memory evidence | Satisfied | Verified AArch64 controlled-host evidence; private committed-source builds; sealed workers; current-toolchain replay; setup and peak RSS kept separate from timing |
| Commands and atomic publication | Satisfied | `qualification-list`, `qualification-check`, `qualification-probe`, `qualification-run`, `qualification-report`, and `qualification-regression`; exact report/preflight binding and compare-and-swap report refresh |

## Clean Evidence

All final reports use schema version 13, `local_modifications=false` before and after execution, `host_verified=true`, `allow_unverified_host=false`, Nightly `2026-06-20`, and target `aarch64-unknown-linux-gnu`.

| Tier | Artifact | Pairs | Median diagnostic ratio | Bootstrap 95% interval | Outcome | Stim peak RSS | Stab peak RSS |
| --- | --- | ---: | ---: | ---: | --- | ---: | ---: |
| PR | `target/benchmarks/qualification/pq1-final-pr-schema13` | 3 | 1.015496 | [1.014793, 1.015539] | Passed | 3,375,104 bytes | 4,091,904 bytes |
| Full | `target/benchmarks/qualification/pq1-final-full-schema13` | 9 | 1.015670 | [1.014387, 1.016352] | Passed | 3,391,488 bytes | 4,104,192 bytes |
| Soak | `target/benchmarks/qualification/pq1-final-soak-schema13` | 15 | 1.015023 | [1.014356, 1.015416] | Passed | 3,395,584 bytes | 4,087,808 bytes |

The ratios above describe only the synthetic protocol-smoke adapter and worker contract.
They must not be cited as a speedup, slowdown, or parity result for any Stab product feature.
Independent regression replay returned `checked=0 report_only=true` for every tier, as required by the source-owned baseline.

## Audit And Review Closure

The milestone audit is complete with no unresolved implementation finding.
The audit maps every PQ1 task, test family, acceptance criterion, command, and evidence artifact in the completion matrix above.

The GPT-5.6/max full-code-review lanes found evidence-trust, publication-race, worker-symmetry, timeout, host-override, toolchain-replay, inventory-binding, and runtime-dispatch issues.
The final remediation is recorded in focused commits `8946e22`, `22898b4`, and `bfef511`.
The post-fix review found no remaining confirmed PQ1 correctness, security, lifecycle, statistics, performance-fidelity, or documentation blocker.

The CQ2 inventory refresh later exposed a post-spawn affinity race under parallel workspace tests: the child leader could be pinned after its test-harness worker already inherited the broad parent mask. The bounded process runner now pins the leader first, enumerates and pins at most 4,096 existing child tasks through `/proc/<pid>/task`, verifies a singleton mask, and fails after eight nonconvergent passes; a focused multithreaded child regression and the full parallel workspace suite pass. Because this changes runner behavior, the older clean PQ1 reports remain historical until the current-digest rerun described above.

The audit also exposed one specification loophole: "existing M12 commands remain backward compatible" did not say whether inherited gate failures blocked PQ1.
The performance plan now defines command compatibility as preserved parsing, execution, report shape, and unchanged source-owned gates; inherited product-row failures remain visible work for PQ2 through PQ6 and do not invalidate the independently scoped PQ1 diagnostic harness.
The corresponding resolved entry is in `docs/plans/milestone-spec-gaps.md`.

## Legacy M12 State

Fresh legacy reports from clean commit `3b8df70dd8045ba73c158976881691e7b9d3f3cb` prove that the 89-row commands still execute with their gates intact.
They also expose product-qualification work that PQ1 deliberately does not hide:

- Beta: 78 comparable passes, five checked no-ratio waivers, and six non-comparable rows whose beta target is not proven.
- Timing regression: 74 configured passes, five checked no-ratio waivers, four not-configured rows, and six non-comparable threshold rows.
- Memory regression: 83 passes, four rows missing a baseline, and two M10 allocation failures.

The generated artifacts are `target/benchmarks/m12-primary-beta`, `target/benchmarks/qualification/m12-schema12-compat-regression-fresh`, and `target/benchmarks/qualification/m12-schema12-compat-memory`.
These failures belong to the product workload migration and graduation milestones; none is a measured PQ1 protocol-harness regression.

The 85-row wording in the benchmark-status row of `docs/stab-feature-checklist.md` remains the frozen PQ0 inventory anchor at the performance digest recorded above, rather than a live qualification-era result.
Revise that row only together with the reviewed PQ2 inventory regeneration and dependent evidence refresh; use this report and `docs/plans/m12-progress-report.md` for the current 89-row operational state.

## Verification

The following checks passed after the final implementation changes:

```sh
cargo fmt --all --check
cargo clippy --workspace --all-targets -- -D warnings
cargo test --workspace --quiet
cargo test -p stab-bench --quiet
just qualification::correctness-check
just bench::smoke
just bench::qualification-check
just bench::qualification-probe --group pq1-process-contract-smoke
just bench::qualification-probe --group pq1-adapter-protocol-smoke
just bench::qualification-run --tier pr --out target/benchmarks/qualification/pq1-final-pr-schema13
just bench::qualification-run --tier full --out target/benchmarks/qualification/pq1-final-full-schema13
just bench::qualification-run --tier soak --out target/benchmarks/qualification/pq1-final-soak-schema13
just bench::qualification-report --input target/benchmarks/qualification/pq1-final-pr-schema13
just bench::qualification-report --input target/benchmarks/qualification/pq1-final-full-schema13
just bench::qualification-report --input target/benchmarks/qualification/pq1-final-soak-schema13
just bench::qualification-regression --input target/benchmarks/qualification/pq1-final-pr-schema13
just bench::qualification-regression --input target/benchmarks/qualification/pq1-final-full-schema13
just bench::qualification-regression --input target/benchmarks/qualification/pq1-final-soak-schema13
just maintenance::pre-commit
```

`cargo test -p stab-bench --quiet` passed 177 tests with one intentionally ignored signal-cancellation subprocess helper test.
The real CQ soak compatibility probe was temporary and target-dependent, so it was removed after passing; its resolved-selector behavior remains protected by the source-owned `resolved_fixture_selector_digest_stays_bound_to_the_approved_request` regression test.

## Next Milestone

CQ2 is active.
PQ2 remains blocked on its exact CQ2 correctness prerequisites and must add real deterministic model, format, gate, kernel, and algebra workloads instead of promoting the PQ1 synthetic ratio.
