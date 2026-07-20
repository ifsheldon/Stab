# PQ2 BitMatrix Transpose Qualification Progress Report

> Historical-inventory note, 2026-07-18: this report remains accepted passing Linux AArch64 evidence at performance inventory `1d38c155acbaf78234f9b92857cfef8c25ffa059a4a9e9756b272a72272dfd0d` and correctness inventory `5d795e831bc20b3f2780ca72c1eaea7c75387388d38f8e37f4539254a41e821b`. Later Pauli and checklist-refreshed inventories are different, so the transpose outcomes below are not relabeled as simultaneous current-inventory evidence.

## Status

The eighth PQ2 executable slice passes its independent `1.25x` timing gates for public square in-place BitMatrix transpose and public allocating BitMatrix transpose at every full and soak scale on the controlled Linux AArch64 host as of 2026-07-16.

All 12 promotable measurements passed on their first attempt, without a noise rerun, report-only timing outcome, waiver, or profiler note.
The six in-place median Stab-to-Stim elapsed-time ratios range from `0.366307x` to `0.684489x`, with worst bootstrap confidence-interval upper bound `0.686065x`.
The six allocating medians range from `0.397088x` to `0.657658x`, with worst upper bound `0.660876x`.
Every point estimate is faster than pinned Stim for its exact method and scale; the largest measured method-specific speedups are approximately 2.73x for in-place transpose and 2.52x for allocating transpose.
The two methods are never aggregated because their mutation, allocation, and result-lifetime contracts differ.

This report closes these two contracts on Linux AArch64 only:

- `PERFQ-M5-BIT-MATRIX-TRANSPOSE-IN-PLACE`
- `PERFQ-M5-BIT-MATRIX-TRANSPOSE-ALLOCATING`

It does not qualify native Linux x86-64, cross-scale memory growth, row XOR, masked row XOR, row swaps, construction, parsing, serialization, matrix multiplication, inversion, raw random fill, or remaining PQ2 groups.

## Frozen Evidence

- Clean Stab evidence revision: `f912cc3af1f13cc9fab798d69937c155d37d83a0`, with `local_modifications=false` in every final correctness and performance producer.
- Stim baseline: v1.16.0 at `e2fc1eca7fd21684d433aa5f10f4504ea4860d07`.
- Performance inventory: `1d38c155acbaf78234f9b92857cfef8c25ffa059a4a9e9756b272a72272dfd0d`.
- Correctness inventory: `5d795e831bc20b3f2780ca72c1eaea7c75387388d38f8e37f4539254a41e821b`.
- Runtime-group contract: `6cbfc302ba2bbe26cc132a287b5291853f07d3c8bf71b36f0b564776d496d242`.
- Host profile: verified `linux-aarch64-controlled`, pinned to logical CPU 0 with CPU identity `CPU implementer=0x41, CPU architecture=8, CPU variant=0x0, CPU part=0xd87, CPU revision=1`.
- Rust toolchain: `nightly-2026-06-20`, release profile, target `aarch64-unknown-linux-gnu`.

The inventory frozen for this slice contains 547 groups: 545 measured, zero covered by a parent, two not performance relevant, and zero without a faithful comparator.
Its 161 inherited rows are classified as 12 retained, 135 reworked, four diagnostic, eight superseded, and two removed.
The inventory preserves 123 missing scale families, 158 missing correctness preflights, 158 missing output digests, 73 missing comparators, 58 asymmetric CLI rows, and 20 heterogeneous selections as later work instead of counting this slice as broader completion.

## Correctness Preflight

The clean correctness report at `target/qualification/pq2-m5-transpose-review-final-f912cc3` selected and passed exactly these two cases:

- `cq-evidence-qualification-4d0291febfd22b68` owns transpose semantics, exhaustive small matrices, deterministic randomized matrices, 63/64/65 and 255/256/257 boundaries, rectangular shapes, dirty-tail isolation, double transpose, first-result in-place versus allocating equality, source immutability, and rectangular in-place rejection.
- `cq-evidence-qualification-66e29faafe5f2856` owns checked construction, row storage, matrix-size overflow, and materialized resource boundaries.

| Artifact | SHA-256 |
| --- | --- |
| Request | `9e85e897b549b4f90462e9887732bbdd3a1fe2528e37327811c98105df73f9c0` |
| Report | `54d49f890146164fe6c0468e153852426ba457d82da0d55b86ae5af1d99286d1` |
| Completion | `59e573ad2a8c7f432310c1076e31492a4fb707fdafb34aabe59c7fc803a4e74f` |
| Preflight | `15b6d704b2807bc07f55532c2c9abb79b5ae67a3239e65f7bbfa92585a6c0596` |

Every performance report independently reconstructed the canonical correctness request, report, completion, both execution receipts, and two-case preflight before timing.
The complete 271-parent CQ2 family remains a separate historical checkpoint and is not claimed by this focused prerequisite run; its execution passed at clean revision `3f2f382627c8421de0a668819d467a9f252de20f` under the hardened controller and is not relabeled as current-revision evidence.

## Workload Contracts

Both workers generate the same deterministic non-symmetric square matrix from frozen affine and SplitMix64 transforms with eight set attempts per row; duplicate columns collapse under ordinary bit assignment.
The canonical input encodes little-endian `u64` row count, column count, and row-major logical words.

| Scale | Dimension | Logical bits | Input bytes | Input digest |
| --- | ---: | ---: | ---: | --- |
| Small | 256 | 65,536 | 8,208 | `2a2a5f587d3c9fdb6fea43274c06ad453fcc76bbbcf6bcd9563991076cdf79da` |
| Medium | 2,048 | 4,194,304 | 524,304 | `15e610ea94b541a52446f7ff48ff9ca9560f8dbef5f96232806d0bcbff95f054` |
| Large | 16,384 | 268,435,456 | 33,554,448 | `d68c253c0ca01452ce0624f0fdeb67dd92c85b442034b4b0e574286f3c9f636e` |

The in-place workers execute two untimed complete transposes to restore canonical state, then time only `BitMatrix::transpose_square_in_place()` against Stim `simd_bit_table::do_square_transpose()` behind matching compiler fences and optimizer-opaque mutable references.
The allocating workers execute and discard two untimed public transposes, then time `BitMatrix::transpose()` against Stim `simd_bit_table::transposed()` while retaining the final result and dropping the preceding result inside the timed body.
Semantic output binds iteration count, declared work, dimension, workload marker, input digest, final result, and unchanged allocating source where applicable.

Both sealed workers accept only perfect-square work whose dimension is a positive multiple of 256 from 256 through 16,384.
The 90-receipt preflight freezes small odd, small even, and accepted-maximum outputs and rejects below-minimum, non-square, unaligned, over-cap, and valid-shape semantic-work-overflow requests for both methods and both implementations.
The overflow requests use a valid 256-square with `2^48` iterations and an enabled start barrier, proving rejection before fixture allocation and before barrier consumption.

## Worker Identity

`just bench::qualification-worker-reproducibility` rebuilt both sealed workers twice and reproduced these identities:

| Identity | SHA-256 |
| --- | --- |
| Stim source | `be0dc2df9b03021f693682fea805d5d17aee056d73fa0331073ab11b5a9347c3` |
| Stim build fingerprint | `6733fa502af01deacee34e48a87ef61456b0fc9d6557521a7f3c2a5029392264` |
| Stim binary | `12c16df39d9c415302bcbee79dc62f55c6c27142210de4ec83b859ff754a9bd1` |
| Stab source | `3ead29b7a6a1e8dd9cb641fb57e1aa43be94c7544e02918e635b5fd619448ed7` |
| Stab build fingerprint | `bab5ab4670fff498ce2a01526c551285f15b23c84ca8e51f725c1c0e37bfce87` |
| Stab binary | `86ea780b6383720aa7fe56a86ee3e4a4e8a3c30c57199439efa016143bdf7397` |
| Contract preflight | `8a045da751ce3d426218982ad5d8f0358c95f9c6b852cb995aaa4b222e3fadbc` |

Adapter receipt schema version 8 binds `benchmarks/stim_adapter/main.cc` and `benchmarks/stim_adapter/bit_matrix_transpose_contract.h` in order.
Private Stab build-receipt schema version 2 includes the isolated transpose worker module, contract-preflight schema version 8 executes 90 accepted and rejected probes, and qualification report schema version 26 preserves those identities for offline replay.
Both standalone adapter probes passed; their tiny timings are diagnostic and are not product ratio evidence.

## Timing Results

Each full report retains nine interleaved pairs and each soak report retains 15.
Every report uses standard equal-work mode, contains one timing attempt, and passes both the median and bootstrap-upper `1.25x` rules independently.

| Method | Scale | Tier | Pairs | Median ratio | 95% upper | Ratio rMAD | Outcome |
| --- | --- | --- | ---: | ---: | ---: | ---: | --- |
| In-place | Small | Full | 9 | 0.684489 | 0.685810 | 0.001371 | Passed |
| In-place | Medium | Full | 9 | 0.677027 | 0.678061 | 0.001043 | Passed |
| In-place | Large | Full | 9 | 0.368149 | 0.368981 | 0.001354 | Passed |
| In-place | Small | Soak | 15 | 0.684370 | 0.686065 | 0.001292 | Passed |
| In-place | Medium | Soak | 15 | 0.675639 | 0.676998 | 0.002012 | Passed |
| In-place | Large | Soak | 15 | 0.366307 | 0.366745 | 0.001346 | Passed |
| Allocating | Small | Full | 9 | 0.657148 | 0.660876 | 0.003114 | Passed |
| Allocating | Medium | Full | 9 | 0.580193 | 0.581551 | 0.005544 | Passed |
| Allocating | Large | Full | 9 | 0.397330 | 0.405427 | 0.008779 | Passed |
| Allocating | Small | Soak | 15 | 0.657658 | 0.660166 | 0.003364 | Passed |
| Allocating | Medium | Soak | 15 | 0.581209 | 0.581541 | 0.007208 | Passed |
| Allocating | Large | Soak | 15 | 0.397088 | 0.398085 | 0.003182 | Passed |

Slice outcome counts are 12 passed, zero failed, zero noisy, and zero report-only timing measurements.

## Resource Evidence

The allocation-counted test covers small, medium, and the accepted-maximum large scale.
It proves zero allocation calls and zero allocated bytes for every timed in-place public call, and exactly one output-data allocation of `dimension * dimension / 8` requested bytes for every allocating public call.
Across the 12 reports, observed Stim setup RSS ranges from 3,440,640 to 37,007,360 bytes and Stim peak RSS ranges from 3,440,640 to 103,903,232 bytes.
Stab setup RSS ranges from 4,726,784 to 38,297,600 bytes and Stab peak RSS ranges from 4,792,320 to 105,213,952 bytes.
These process observations are report-only and do not establish cross-scale or Stim-relative memory parity; PQ6 remains the owner of memory-growth acceptance.

## Source Reports

Every source directory contains canonical `report.json`, `preflight.json`, and derived `report.md`, and every report passed immediate byte-for-byte replay plus source-owned regression.

| Method | Scale | Tier | Directory | Report SHA-256 |
| --- | --- | --- | --- | --- |
| In-place | Small | Full | `target/benchmarks/qualification/perfq-m5-transpose-in-place-review-f912cc3-full-small` | `137b4dceaca18539361fd2628e3292faba6634ec3ae9af042a9e4f2d49129562` |
| In-place | Medium | Full | `target/benchmarks/qualification/perfq-m5-transpose-in-place-review-f912cc3-full-medium` | `0fb82cf195dc28e7c3cc4112fd553df485a40bedc017ce0407400c444ca8c329` |
| In-place | Large | Full | `target/benchmarks/qualification/perfq-m5-transpose-in-place-review-f912cc3-full-large` | `60585c1e5bd7bc19e41218fa77634ee85c2fb2c731fb0b0942839c6ece46c246` |
| In-place | Small | Soak | `target/benchmarks/qualification/perfq-m5-transpose-in-place-review-f912cc3-soak-small` | `c5f1a576e7e96fd20c721dfa1b7cebc95b65ef9b374b7c7e7679d564110c31f4` |
| In-place | Medium | Soak | `target/benchmarks/qualification/perfq-m5-transpose-in-place-review-f912cc3-soak-medium` | `a23055de9d68f4b0835b069a53d664e2a4ad69d4352be71776c238fbb1187cee` |
| In-place | Large | Soak | `target/benchmarks/qualification/perfq-m5-transpose-in-place-review-f912cc3-soak-large` | `b8deb82cdf0fffc764bcb8c110723d62df21bfa99313143693c29c749ab68d0d` |
| Allocating | Small | Full | `target/benchmarks/qualification/perfq-m5-transpose-allocating-review-f912cc3-full-small` | `17f56ac44fa9f325d4fba9ea2137efd48f0c9f4f14bc62cf41f4e9c5a457ebc9` |
| Allocating | Medium | Full | `target/benchmarks/qualification/perfq-m5-transpose-allocating-review-f912cc3-full-medium` | `1b4b202605098297ad340111023c3067634cfa762b71c48cc99ee496a03898eb` |
| Allocating | Large | Full | `target/benchmarks/qualification/perfq-m5-transpose-allocating-review-f912cc3-full-large` | `a3351e0f10ff095ce979788201770143b3161cf44ed00a459ac41d92fa8f4a2d` |
| Allocating | Small | Soak | `target/benchmarks/qualification/perfq-m5-transpose-allocating-review-f912cc3-soak-small` | `1dd4deda0142bc15b7a07ad12fdc44daf7fff2776041f51ed0c35f723687ca19` |
| Allocating | Medium | Soak | `target/benchmarks/qualification/perfq-m5-transpose-allocating-review-f912cc3-soak-medium` | `2404eb0fd9cb80686df6629494e3296db0ccf445deb0bb5064a04b47814b9fc9` |
| Allocating | Large | Soak | `target/benchmarks/qualification/perfq-m5-transpose-allocating-review-f912cc3-soak-large` | `c796270c7e05f9a82869929b7e51d6c8da20286db284a77884f150c693c4a036` |

## Rollups And Completion Receipts

| Group | Full rollup SHA-256 | Soak rollup SHA-256 | Completion report SHA-256 | Completion preflight SHA-256 |
| --- | --- | --- | --- | --- |
| In-place | `ec586fc24cde86727a0341c57b0b2b44ce6e34e1932efb7401a30a849bc5ddd7` | `7480e6448caa975f3c25cf22e14c2aea9af368efebb47f673926dabd958fada3` | `f0265381c4bfb90f0fe93a298c7722e90c3f438065de44d5bb5d4f47180a03b0` | `00cb26f21366addd20da17ecfc020398c132861bc3fb75e411f5f631df09e42b` |
| Allocating | `28368770ae8898bc8ffb128fd1bee6f7af4bda2740166fcabaa3c2d85a5b6340` | `4f589a50b8243f81f55eab5b605ed4207a7144c29d9ec1c9b4046da1461c1376` | `0381cb798961a09a910030f4ad059192e6a686d62a28ba56943fe01717da8861` | `4d014aa523cda1e467ff64a01a16c574355eab46d1fa119c994894b17cb910d9` |

All four rollups passed publication and offline replay.
Each completion receipt binds six source reports, two rollups, 16 successful closure steps, exact correctness artifacts, one worker identity set, one clean revision, one CPU identity, the method-specific adapter probe, every report replay and regression, and both rollup replays.
Both completion receipts passed independent byte-for-byte replay.

## Legacy M12 Migration

Clean pre-migration revision `e660c91cff142b611f52a0a28a36e0a3d15670ed` passed and replayed both first-stage completion receipts and authorized retirement of only the heterogeneous `m5-simd-bit-table` timing threshold.
Commit `1264d885087761b19b37beded47811cc0c117e4d` superseded that timing row while retaining its exact upstream provenance and `benchmarks/m12-primary-memory-baseline.json` entry.
Independent review then strengthened the first-result in-place oracle and pre-setup semantic-work overflow contract in `f912cc3af1f13cc9fab798d69937c155d37d83a0`, invalidating earlier worker fingerprints.
The complete correctness, reproducibility, report, regression, rollup, and completion chain recorded here was regenerated and replayed from that reviewed post-migration commit.

## Milestone Audit

The final milestone audit maps every eighth-slice task to direct implementation, correctness, hostile-boundary, allocation, comparator, report, regression, rollup, completion, migration, and documentation evidence.
The initial audit and independent review found two implementation gaps: the exact CQ case verified only double in-place transpose at critical edge sizes, and semantic-work overflow was rejected after setup instead of before fixture allocation and barrier consumption.
They also found documentation that aggregated method ranges or described exact eight-bit rows and stale schema counts.
Commit `f912cc3af1f13cc9fab798d69937c155d37d83a0` added the independent first-result oracle, valid-shape pre-barrier overflow receipts for both methods and workers, schema-version-8 90-probe preflight, schema-version-26 reports, refreshed source fingerprints, and corrected method-specific documentation.
The final machine chain and allocation-counted test close those findings.
No milestone under-specification was revealed, so `docs/plans/milestone-spec-gaps.md` gains no entry.

## Independent Review

The final independent GPT-5.6/max follow-up review reported no P0, P1, P2, or P3 findings and no actionable defect at `f912cc3af1f13cc9fab798d69937c155d37d83a0`.
It independently confirmed the critical-edge first-result oracle, pre-setup and pre-barrier semantic-work overflow rejection, 90-receipt worker preflight, schema-version-26 replay bindings, equal public lifecycles and work counts, canonical input and output digests, exact allocation contracts at dimensions 256, 2,048, and 16,384, ordered source fingerprints, and the narrow legacy timing migration.
The reviewer also verified all 12 clean first-attempt reports, report replays, regressions, four rollup bindings, and two completion receipts against the immutable evidence revision without rerunning timing.
Its residual observations match this report's unclaimed scope: native x86-64 performance, PQ6 cross-scale memory growth, Stim allocator-count instrumentation, and broader BitMatrix operations remain future work rather than defects in this slice. The separate full 271-parent CQ2 family remains accepted historical evidence at clean hardened-controller revision `3f2f382627c8421de0a668819d467a9f252de20f`.

## Verification Record

The evidence revision passed the exact two-case correctness run, report, and preflight; worker reproducibility; both adapter probes; all full and soak report producers; immediate report replay and regression; all rollup producers and replays; and both completion producers and replays.
The implementation and allocation checks also passed:

```sh
cargo fmt --all --check
cargo clippy --workspace --all-targets -- -D warnings
cargo test --workspace --quiet
cargo test -p stab-bench --features count-allocations transpose_timed_public_calls_preserve_exact_allocation_contracts --quiet
just bench::qualification-regenerate --check
just bench::qualification-check
just maintenance::pre-commit
```

Before committing closure documentation, rerun formatting, Clippy, workspace tests, qualification regeneration and validation, benchmark smoke, and staged pre-commit policy.

## Remaining Work

1. Run the same clean full and soak families, rollups, and completion receipts on a controlled native Linux x86-64 host before making an x86-64 conclusion.
2. Define and validate explicit cross-scale RSS and allocation-growth slack in PQ6 before making a memory qualification claim or retiring the legacy memory baseline.
3. Qualify the remaining BitMatrix methods only through their own exact public API groups without folding them into transpose evidence.
4. Select the next finite dependency-ordered PQ2 runtime group without reopening this completed AArch64 slice.
