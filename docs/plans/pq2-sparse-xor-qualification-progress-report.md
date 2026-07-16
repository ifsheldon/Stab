# PQ2 Sparse-XOR Qualification Progress Report

## Status

The seventh PQ2 executable slice passes its independent `1.25x` timing gates for sparse row symmetric difference and sparse item toggling at every full and soak scale on the controlled Linux AArch64 host as of 2026-07-16.

All 12 promotable measurements passed on their first attempt, without a noise rerun, report-only timing outcome, waiver, or profiler note.
Median Stab-to-Stim elapsed-time ratios range from `0.965755x` to `1.026014x`, and the worst bootstrap confidence-interval upper bound is `1.034133x`.
Six measurements are faster than pinned Stim and six are slower by their point estimate; the largest measured speedup is approximately 1.035x and the largest measured slowdown is approximately 1.026x for these exact callbacks.

This report closes these two contracts on Linux AArch64 only:

- `PERFQ-M5-SPARSE-XOR`
- `PERFQ-M5-SPARSE-XOR-ITEM`

It does not qualify native Linux x86-64, broader sparse-density crossover behavior, cross-scale memory growth, or remaining PQ2 groups.

## Frozen Evidence

- Clean Stab evidence revision: `7b43b46d1c08f669264d009b8d3872ce86838f0e`, with `local_modifications=false` before and after every final correctness, performance, rollup, and completion producer.
- Stim baseline: v1.16.0 at `e2fc1eca7fd21684d433aa5f10f4504ea4860d07`.
- Accepted evidence performance inventory: `8cc3ab3eb88faaf539c3c0eabaf3865ad421d8f67b14549cb4c7acc71faf2406`; later transpose ownership work changed the source-current inventory without relabeling these reports.
- Correctness inventory: `ccb80eb4b660a375b59460c3b7fa03a810abd6f868735b566735378105db22b2`.
- Runtime-group contract: `cc46c5c545c73ce462a93b9a6e0e9ae15b2c0ae4abf692717f17f4e2360c4efa`.
- Host profile: verified `linux-aarch64-controlled`, pinned to logical CPU 0 with CPU identity `CPU implementer=0x41, CPU architecture=8, CPU variant=0x0, CPU part=0xd87, CPU revision=1`.
- Rust toolchain: `nightly-2026-06-20`, release profile, target `aarch64-unknown-linux-gnu`.

The accepted evidence performance inventory contains 545 groups: 543 measured, zero covered by a parent, two not performance relevant, and zero without a faithful comparator.
Its 161 inherited rows are classified as 12 retained, 136 reworked, four diagnostic, seven superseded, and two removed; 21 exact threshold pairs remain after the two sparse-XOR legacy timing pairs were retired.
The inventory has ten executable exact-case product groups and preserves all 123 missing scale families, 158 missing correctness preflights, 158 missing output digests, 73 missing comparators, 58 asymmetric CLI rows, and 20 heterogeneous rows as later work.

## Correctness Preflight

The clean correctness report at `target/qualification/pq2-m5-sparse-xor-final-7b43b46` selected and passed exactly `cq-evidence-qualification-bea77c19e9ae0b24`.
That case owns `SparseXorVec::xor_assign`, `SparseXorVec::xor_item`, sorted-unique state, duplicate cancellation, stack-to-heap transitions, and dense-reference equivalence.

| Artifact | SHA-256 |
| --- | --- |
| Request | `5c57fc877d3c629a0cbee71e1c9e29eca4f937db29c5d83c238950cfd20c3215` |
| Report | `b2c622f0e42eefd9201ac81f43f166446ac428ec9581b4115cd1042a925a617e` |
| Completion | `3759e07d57d50ec2ba825c061b36ef6652efa3667070af71b41cac69a9eab040` |
| Preflight | `24b398f158c54bdcb23116b7cdab9ccae294018671913cd24ec52589a3e374f6` |

Every performance report independently reconstructed the canonical correctness request, report, completion, selected case, execution receipt, and preflight before timing.

## Workload Contracts

The row workload reproduces pinned Stim's 1,000-row fixture, with row `k` containing `k`, `k+1`, `k+4`, `k+8`, and `k+15`.
One complete callback performs 999 forward row XORs and 998 reverse row XORs, so qualification counts 1,997 actual row operations instead of Stim's nominal `n * 2` display rate.
The item workload reproduces the exact sequence `2, 5, 9, 5, 3, 6, 10`, so each complete callback performs seven toggles.

| Group | Small | Medium | Large | Input bytes | Protocol input digest |
| --- | ---: | ---: | ---: | ---: | --- |
| Row XOR | 1,997 | 127,808 | 8,179,712 | 28,008 | `9fdcaf10b6a6437d51afade0e21f39acdd1130ff18255e38c0751261f93df2a2` |
| Item toggle | 7 | 448 | 28,672 | 36 | `c2c1749b4bf4c7c355c1d0a8109ea53bba790034d116acea3755b533c1fb1059` |

Both workers build and hash the fixture outside timing, execute exactly two untimed complete callbacks to restore canonical state while retaining capacity, and time only complete callbacks behind matching compiler barriers and optimizer-opaque mutable references.
They encode semantic output from 12 little-endian `u64` fields that bind the iteration count, declared work, workload marker, base callback work, input digest, and final-state digest.
The contract freezes odd, even, and accepted-maximum outputs and rejects zero, partial-callback, and first-over-cap work counts before allocation and before consuming the start barrier.

## Worker Identity

`just bench::qualification-worker-reproducibility` rebuilt both sealed workers twice and reproduced these identities:

| Identity | SHA-256 |
| --- | --- |
| Stim source | `276e1bd8de12064a3234812b4421e47d1e20b9a81c3063f7f9ef53d2e88f5c18` |
| Stim build fingerprint | `71bbff8d0fc4fbba7dca619c0fbb53ba102e3cb797eebabdb7b5a87b8d8cb69a` |
| Stim binary | `47310bc871c4dd5ae4a89864e0fd00caf0650954fbe843999b17896459eaf6ee` |
| Stab source | `65d12b9beb962f5a4fd693ea4368e7a55859f675cf7532602690223db7a51b20` |
| Stab build fingerprint | `3a5716fd8d7bb0d4e20e1e5ab4622f45825c7dde297bb6c9a75b1dda0ab8bc4f` |
| Stab binary | `b586e6c975988e3a522ec8cca71149d51f7e42ab926e2729ffd66c10b31fbbcf` |
| Contract preflight | `eb6de912d219e347469353f3f106d72ef699c575afdfb6e6418cf2260335ecfa` |

Adapter receipt schema version 7 binds `benchmarks/stim_adapter/main.cc` and `benchmarks/stim_adapter/sparse_xor_contract.h` in order.
Private Stab build-receipt schema version 2 includes the sparse-XOR worker module, contract-preflight schema version 6 executes 58 accepted and rejected probes, and qualification report schema version 24 preserves those identities for offline replay.
Both standalone adapter probes passed exact positive complete-callback work through the cap; their quick timings are diagnostic and are not ratio evidence.

## Timing Results

Each full report retains nine interleaved pairs and each soak report retains 15.
All reports use standard equal-work mode, contain one timing attempt, and pass both the median and bootstrap-upper `1.25x` rules independently.

| Workload | Scale | Tier | Pairs | Median ratio | 95% upper | Ratio rMAD | Outcome |
| --- | --- | --- | ---: | ---: | ---: | ---: | --- |
| Row XOR | Small | Full | 9 | 1.001415 | 1.033040 | 0.023335 | Passed |
| Row XOR | Medium | Full | 9 | 1.001974 | 1.022165 | 0.014907 | Passed |
| Row XOR | Large | Full | 9 | 0.995660 | 1.017974 | 0.012175 | Passed |
| Row XOR | Small | Soak | 15 | 1.000890 | 1.012034 | 0.016602 | Passed |
| Row XOR | Medium | Soak | 15 | 1.026014 | 1.034133 | 0.010011 | Passed |
| Row XOR | Large | Soak | 15 | 0.995421 | 1.029083 | 0.019864 | Passed |
| Item toggle | Small | Full | 9 | 1.012329 | 1.026119 | 0.005224 | Passed |
| Item toggle | Medium | Full | 9 | 0.975151 | 0.988440 | 0.006682 | Passed |
| Item toggle | Large | Full | 9 | 0.965755 | 0.983696 | 0.002148 | Passed |
| Item toggle | Small | Soak | 15 | 1.024624 | 1.032124 | 0.010033 | Passed |
| Item toggle | Medium | Soak | 15 | 0.982440 | 0.983656 | 0.004621 | Passed |
| Item toggle | Large | Soak | 15 | 0.977742 | 0.980152 | 0.005350 | Passed |

Slice outcome counts are 12 passed, zero failed, zero noisy, and zero report-only timing measurements.

## Resource Evidence

Allocation-counted tests prove zero calls and zero bytes allocated by the timed Stab body for both workloads at small, medium, large, and the accepted maximum after the required two-callback capacity priming.
Across the 12 reports, observed Stim setup and peak RSS range from 3,403,776 to 3,567,616 bytes, Stab setup RSS ranges from 4,325,376 to 4,485,120 bytes, and Stab peak RSS ranges from 4,456,448 to 4,616,192 bytes.
These are report-only process observations, not a cross-scale or Stim-relative memory conclusion; PQ6 remains the owner of memory-growth acceptance.

## Source Reports

All paths contain canonical `report.json`, `preflight.json`, and derived `report.md` artifacts, and every report passed immediate byte-for-byte replay plus source-owned regression.

| Workload | Scale | Tier | Directory | Report SHA-256 |
| --- | --- | --- | --- | --- |
| Row XOR | Small | Full | `target/benchmarks/qualification/sparse-row-final-7b43b46-full-small` | `aa3110661ae274a7874385a3e0f31bccc37c9269fa41a810f406087bda026182` |
| Row XOR | Medium | Full | `target/benchmarks/qualification/sparse-row-final-7b43b46-full-medium` | `efa8afa31ee7adcb49c9a195e63ff49de66bf0e18055219ad70c2299f687b2ee` |
| Row XOR | Large | Full | `target/benchmarks/qualification/sparse-row-final-7b43b46-full-large` | `dea116b3fba3432e1c7084a32a2af33bd39e8fcdc44332935cd11b0b7f072c70` |
| Row XOR | Small | Soak | `target/benchmarks/qualification/sparse-row-final-7b43b46-soak-small` | `7fa3fd8f1e291ff0a86bd95e0503c5b466ee45491ae3e126fbdb3d9028f5e489` |
| Row XOR | Medium | Soak | `target/benchmarks/qualification/sparse-row-final-7b43b46-soak-medium` | `f7945f7c924042ac14717fd3c35a0a6ef494080576be58197f7b3bf0fa620f8e` |
| Row XOR | Large | Soak | `target/benchmarks/qualification/sparse-row-final-7b43b46-soak-large` | `cc8adf11cf0b48d3d23af5f0c76cc695ffc65618f2d555910f62bcc951d70cf8` |
| Item toggle | Small | Full | `target/benchmarks/qualification/sparse-item-final-7b43b46-full-small` | `236a55b2dae0dda39fc713bc848d1dd5dac635a63927216ef0cd2d3a878dc981` |
| Item toggle | Medium | Full | `target/benchmarks/qualification/sparse-item-final-7b43b46-full-medium` | `9a17873ad665257d12a247523cde0ac90c3e56232d650773a502820f6986f7bd` |
| Item toggle | Large | Full | `target/benchmarks/qualification/sparse-item-final-7b43b46-full-large` | `cdeb0081cbbc72079cf0f65dccaf1a2e60ca059c5ab818fe90697924e3934abe` |
| Item toggle | Small | Soak | `target/benchmarks/qualification/sparse-item-final-7b43b46-soak-small` | `29a50d2d45c5e3b076028eba795bc8dde8a7cea7feb14a49cb3f06b47f0400b0` |
| Item toggle | Medium | Soak | `target/benchmarks/qualification/sparse-item-final-7b43b46-soak-medium` | `60fde8f615cfff8217fa17c6b3f7243e9b6c0db9b853ade89100a5518b3b2afd` |
| Item toggle | Large | Soak | `target/benchmarks/qualification/sparse-item-final-7b43b46-soak-large` | `b9b7e3092139a352d19fca462902e8ac52b2ea57a121d6f0f4572723158f85b0` |

## Rollups And Completion Receipts

| Group | Full rollup SHA-256 | Soak rollup SHA-256 | Completion report SHA-256 | Completion preflight SHA-256 |
| --- | --- | --- | --- | --- |
| Row XOR | `99ddc3f8879c38767a14af0942d1c1b4c9f81487ff84c438715fb105f7b4b5db` | `9b74a0e3303daf4785ed6fd7b0b411e1a0912084737face8ff6ea88ca221a37c` | `51895550f65f31e4341851c65b69c3030712b74ac6fd8e30774485e3e5c1c861` | `4b1eb5b9cdb17ea4421b41da131532af230b6780299652dc1a8016e99b74837c` |
| Item toggle | `9cfd2585e64d8d5d35bf76ef242db499542f465ed5314d644d0f5dc36b83367e` | `d5167b83110a97c2a49404eaa04ac0959f7b1190d98911bd49c62e4aa85a37c2` | `5f99bc908f2644c1daacc5e275d24b4f0859c5d1a7b61202da48c1ee4b3244c0` | `134338edad93e2f760fb2c603c1aa3e61af4a6a3e03bac4bddbc46f4a8431655` |

All four rollups passed publication and offline replay.
Each completion receipt binds six source reports, two rollups, 16 successful closure steps, the exact correctness artifacts, one worker identity set, one clean revision, one CPU identity, the adapter probe, every report replay and regression, and both rollup replays.
Both completion receipts passed independent byte-for-byte replay.

## Legacy M12 Migration

The first-stage completion receipts at clean revision `e2f6292f473b034d8886fc100039c7a78c4a3989` and pre-migration inventory `2d9cb3e3e2dc36a29c31964480f9b735e2411b26f4ba2b3ac66ed6791b617dc0` authorized retirement of exactly two legacy timing pairs:

- `SparseXorTable_SmallRowXor_1000` / `stab_sparse_table_row_xor_1000`
- `SparseXorVec_XorItem` / `stab_sparse_xor_item_7`

Commit `7b43b46d1c08f669264d009b8d3872ce86838f0e` superseded the heterogeneous `m5-sparse-xor` timing row, removed only those two M12 timing thresholds and temporary replacement markers, and preserved the existing M12 memory baseline.
The complete correctness, reproducibility, report, regression, rollup, and completion chain was then regenerated and replayed at the post-migration inventory cited by this report.

## Milestone Audit

The milestone audit verified every seventh-slice task and done criterion against the exact CQ report, both source contracts, odd and even frozen vectors, accepted and rejected work boundaries, zero-allocation tests, 58-probe preflight, reproducible workers, all 12 first-attempt reports and replays, all 12 regressions, four rollup replays, both completion replays, and the narrow threshold migration.
The audit found and closed one implementation omission before final evidence: the two public adapter probes were initially absent and were added in commit `e2f6292f473b034d8886fc100039c7a78c4a3989` with positive, partial-callback, and over-cap validation.
Post-evidence closure checks found stale cross-document state and a frozen threshold-row-count assertion that still expected 80 rows after `m5-sparse-xor` was superseded; the documentation was synchronized and the assertion now validates all 79 remaining threshold rows.
No new under-specification was revealed, so `docs/plans/milestone-spec-gaps.md` gains no entry.

## Independent Review

Independent GPT-5.6/max full code review confirmed pinned-Stim loop fidelity, the 1,997-operation denominator, the exact seven-item sequence, canonical encodings, two-callback restoration, optimizer barriers, allocation claims, pre-allocation caps, probe and receipt contracts, schema and replay integrity, narrow threshold migration, module ownership, and the absence of a material Rust-modernization opportunity.
It found one P2 documentation defect: `benchmarks/stim_adapter/README.md` still described adapter schema version 6, 42 preflight receipts, and no sparse-XOR probes.
The README now documents schema version 7, all 58 receipts, both probe commands, and the seventh product adapter surface.
Final review status has no remaining confirmed finding; native x86-64 and PQ6 memory-growth evidence remain explicit residual work rather than defects.

## Verification Record

The evidence revision passed the exact correctness run, report, and preflight; worker reproducibility; both adapter probes; all full and soak report producers; immediate report replay and regression; all rollup producers and replays; and both completion producers and replays.
The focused implementation and migration checks also passed:

```sh
cargo test -p stab-bench sparse_xor --quiet
cargo test -p stab-bench --features count-allocations sparse_xor_timed_workloads_allocate_nothing_after_capacity_priming --quiet
cargo test -p stab-bench qualification::runtime --quiet
cargo test -p stab-bench qualification::validation --quiet
just bench::qualification-worker-reproducibility
just bench::qualification-probe --group pq2-sparse-xor-row-adapter-smoke --iterations 2 --work-items 1997
just bench::qualification-probe --group pq2-sparse-xor-item-adapter-smoke --iterations 2 --work-items 7
just bench::qualification-regenerate --check
just bench::qualification-check
just bench::smoke
```

Before committing the closure documentation, rerun workspace formatting, Clippy with warnings denied, all workspace tests, qualification regeneration checks, benchmark smoke, and staged pre-commit policy.

## Remaining Work

1. Run the same clean full and soak families, rollups, and completion receipts on a controlled native Linux x86-64 host before making an x86-64 conclusion.
2. Define and validate explicit cross-scale RSS and allocation-growth slack in PQ6 before making a memory qualification claim.
3. Add broader active-cardinality and density-crossover groups only when the selected public sparse API or engine path owns that exact work.
4. Select the next finite dependency-ordered PQ2 runtime group without reopening this completed slice.
