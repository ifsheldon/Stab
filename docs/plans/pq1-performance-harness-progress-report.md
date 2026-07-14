# PQ1 Paired Performance Harness Progress Report

> Current-evidence note, 2026-07-15: Algebra ownership and resource-admission regeneration set correctness inventory digest `7e42ddddd662593b56f0bd67885b74babf9a96319de990e4f2cb6218638edea5` and dependent performance inventory digest `67bcbfcf2d991c883b6d889bf48b4d9b8c09bcb52bdbd6dc1e041b6162a30193`. The clean schema-version-13 PR, full, and soak dependent reports from revision `d0ecafd62794daad0ab5eb63d54c481a5e32a30b` bind the previous Generation-refined digests and are historical. Their diagnostic median ratios 1.014015, 1.015160, and 1.015225 remain report-only infrastructure evidence, not product performance; see `docs/plans/cq2-deterministic-qualification-progress-report.md`.

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

## Current Dependency Refresh

The Generation-refined dependency refresh uses schema version 13, `local_modifications=false` before and after execution, correctness inventory digest `d89a5f9eaba428fb72741c66ad74226820660e25e949123871c6c7ab86f82dd6`, performance inventory digest `44276968d035fbd108fd57096dc96aed1d3967ac07d539a8dbfed8f0d5f16fcb`, and commit `d0ecafd62794daad0ab5eb63d54c481a5e32a30b`.

| Tier | Artifact | Pairs | Median diagnostic ratio | Bootstrap 95% interval | Ratio rMAD | Report digest |
| --- | --- | ---: | ---: | --- | ---: | --- |
| PR | `target/benchmarks/qualification/pq1-generation-pr-schema13` | 3 | 1.014015 | [1.013541, 1.014200] | 0.000182 | `ff4d559937167dcf9c495838a22656de183fffdfe7b04fb7a2a74c9f43743a9c` |
| Full | `target/benchmarks/qualification/pq1-generation-full-schema13` | 9 | 1.015160 | [1.013574, 1.016190] | 0.000801 | `eaeeeaf993521997c1aa2061a2c1fbeb4fceac8aba946291d9f5e1b46dd7db94` |
| Soak | `target/benchmarks/qualification/pq1-generation-soak-schema13` | 15 | 1.015225 | [1.014722, 1.015822] | 0.000586 | `9ddfd5154a0d768ae4e3e66308a13483fe46a2617257e2dc29f3d222dab480f5` |

Offline report validation passed for every tier, and regression replay returned `checked=0 report_only=true`. The refresh closes current dependency publication for the harness but does not qualify a product workload or enter any ratio into the 1.25x gate.

## Audit And Review Closure

The milestone audit is complete with no unresolved implementation finding.
The audit maps every PQ1 task, test family, acceptance criterion, command, and evidence artifact in the completion matrix above.

The GPT-5.6/max full-code-review lanes found evidence-trust, publication-race, worker-symmetry, timeout, host-override, toolchain-replay, inventory-binding, and runtime-dispatch issues.
The final remediation is recorded in focused commits `8946e22`, `22898b4`, and `bfef511`.
The post-fix review found no remaining confirmed PQ1 correctness, security, lifecycle, statistics, performance-fidelity, or documentation blocker.

The CQ2 inventory refresh later exposed a post-spawn affinity race under parallel workspace tests: the child leader could be pinned after its test-harness worker already inherited the broad parent mask. The bounded process runner now pins the leader first, enumerates and pins at most 4,096 existing child tasks through `/proc/<pid>/task`, verifies a singleton mask, and fails after eight nonconvergent passes; a focused multithreaded child regression and the full parallel workspace suite pass. The current-digest rerun described above closes the resulting evidence refresh requirement.

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
