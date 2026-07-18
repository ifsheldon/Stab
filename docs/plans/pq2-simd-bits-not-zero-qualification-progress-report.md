# PQ2 SIMD-Bits `not_zero` Qualification Progress Report

> Historical-inventory note, 2026-07-19: this report remains accepted passing Linux AArch64 evidence at performance inventory `0161ab09015487ee2a1298be8dafe7c744b426b28a4e9fbdbd688e775c1655a0`. Later product groups, evidence-authorized migrations, and the reviewed Clifford contracts produced source-current digest `0ee3639389860799298164c94c647fcab45b03c9d67b941b1aad12c6e5e06df5`. The measured `not_zero` contracts and outcomes below are not relabeled as current-inventory evidence.

## Status

The sixth PQ2 executable slice passes its independent `1.25x` timing gate for early-hit, all-zero, and late-hit `not_zero` scans at every full and soak scale on the controlled Linux AArch64 host as of 2026-07-16.

All 18 promotable measurements passed on their first attempt, without a noise rerun or profiler note.
Median Stab-to-Stim elapsed-time ratios range from `0.032329x` to `0.663712x`, corresponding to approximately 1.51x to 30.93x the pinned Stim throughput for the exact source-owned workloads.
The worst bootstrap confidence-interval upper bound is `0.071534x` for early-hit, `0.663577x` for all-zero, and `0.664097x` for late-hit.

This report closes these three contracts on Linux AArch64 only:

- `PERFQ-M5-SIMD-BITS-NOT-ZERO-EARLY`
- `PERFQ-M5-SIMD-BITS-NOT-ZERO-ALL-ZERO`
- `PERFQ-M5-SIMD-BITS-NOT-ZERO-LATE`

It does not qualify native Linux x86-64, other bit-kernel phases, cross-scale memory growth, or remaining PQ2 groups.

## Frozen Evidence

- Clean Stab evidence revision: `60b732c77f1828058fbd65ec6c5c4ad582467fd1`, with `local_modifications=false` before and after every correctness, performance, rollup, and completion producer.
- Stim baseline: v1.16.0 at `e2fc1eca7fd21684d433aa5f10f4504ea4860d07`.
- Accepted evidence performance inventory: `0161ab09015487ee2a1298be8dafe7c744b426b28a4e9fbdbd688e775c1655a0`.
- Correctness inventory: `ccb80eb4b660a375b59460c3b7fa03a810abd6f868735b566735378105db22b2`.
- Host profile: verified `linux-aarch64-controlled`, pinned to logical CPU 0 with CPU identity `CPU implementer=0x41, CPU architecture=8, CPU variant=0x0, CPU part=0xd87, CPU revision=1`.
- Rust toolchain: `nightly-2026-06-20`, release profile, target `aarch64-unknown-linux-gnu`.

This evidence was regenerated after the duplicate legacy M12 `not_zero` threshold pair was retired, so report and completion replay bind the same accepted inventory that owns the sixth-slice closure claim.

## Correctness Preflight

The clean correctness report at `target/qualification/pq2-m5-not-zero-full-60b732c` selected and passed exactly these two cases:

- `cq-evidence-qualification-b1530dc4e48e942d` owns `BitVec::not_zero`, zero and nonzero semantics, canonical tails, and the bit-kernel behavior used by the comparator.
- `cq-evidence-qualification-ba252d42660a41ce` owns storage shape, views, and access boundaries.

| Artifact | SHA-256 |
| --- | --- |
| Request | `6ecd34ade356b966415610ad487ab1d186e38910efe62e29ab8ac6a16ab36718` |
| Report | `d57a141e6b77f97478ac139975569e1053db2aa0e97d1a537ae09d73573bad02` |
| Completion | `ebf5d71b6816b6074c2642266981c8506513e87f21cb69c3ba266707a978eea3` |
| Preflight | `6e401d32cd47b04ec5ae8d14054fda016e2a2dfff15639990de47963496186e4` |

Every performance report independently reconstructed the canonical correctness request, report, completion, preflight, selected cases, and execution receipts before timing.

## Workload Contract

Both workers prepare the same logical words outside timing, clear every padded Stim word, set at most one logical bit, bind exact little-endian input bytes, and require matching semantic output digests before a ratio can exist.
The early hit is at `bits * 3 / 50`, the all-zero vector has no hit, and the late hit is at the final logical bit.

| Scale | Logical bits | Input bytes |
| --- | ---: | ---: |
| Small | 10,000 | 1,256 |
| Medium | 640,000 | 80,000 |
| Large | 40,960,000 | 5,120,000 |

The timed Stim body executes a signal fence, obtains an optimizer-opaque immutable reference, calls pinned Stim `simd_bits::not_zero()`, and accumulates the Boolean checksum.
The timed Stab body executes a compiler fence, passes the immutable `BitVec` through `black_box`, calls `BitVec::not_zero()`, and accumulates the same checksum.
Fixture generation, allocation, validation, input hashing, output construction, and semantic hashing stay outside timing.

The workers accept every logical width from 64 through 268,435,456 bits, including unaligned widths, and reject 63 and 268,435,457 before allocation and before consuming the start barrier.
Preflight executes both accepted and rejected boundaries in both sealed workers.

## Worker Identity

`just bench::qualification-worker-reproducibility` rebuilt both sealed workers twice and reproduced these exact identities:

| Identity | SHA-256 |
| --- | --- |
| Stim source | `746ad7c6979b7f3476eba8ead6cf9df84fb329b27b118191d51edd95971e49e6` |
| Stim build fingerprint | `73870b9951e231709996d0d5a48366e10ccb078eaac8a43686d96896327f1066` |
| Stim binary | `e5478cb91d42ff280dc479aba32a78901edf938b7c9e33e2bcc48622b2530f49` |
| Stab source | `3ee87fb7eb34ab72526a5683d1584516d98901f9e4ba64acbd29fedf1e311c38` |
| Stab build fingerprint | `746ddd62327d5761e4d15b849a8cb9ea9aa9dce366e089511f11d39416e4a6e2` |
| Stab binary | `1c06efbc8392e24070317e3dafa910fb0dc31cc07f768587a69d6e300dd1d691` |
| Contract preflight | `8c8334ddaea82eef763c918ec294705638311dbecf30d4d12090b4de32079b07` |

Every performance report contains the same 42 actual contract-preflight receipts and the same worker identities.
The three standalone 10,000-bit adapter probes passed exact input and output identity checks and remain diagnostic rather than ratio evidence.

## Timing Results

Standard mode independently calibrates both sides between 250 milliseconds and 2 seconds, then uses one identical common iteration count.
The early-hit ratios have no valid standard overlap, so all six early reports use source-derived wide-ratio mode.
In wide-ratio mode only the implementation that selected fewer independent iterations may exceed 2 seconds, the common-iteration owner remains at or below 2 seconds, both sides remain at least 250 milliseconds, and neither side may exceed the 20-second hard ceiling under the unchanged 30-second invocation timeout.

| Pattern | Scale | Tier | Pairs | Mode | Median ratio | 95% upper | Outcome |
| --- | --- | --- | ---: | --- | ---: | ---: | --- |
| Early | Small | Full | 9 | Wide ratio | 0.071500 | 0.071534 | Passed |
| Early | Small | Soak | 15 | Wide ratio | 0.071514 | 0.071522 | Passed |
| Early | Medium | Full | 9 | Wide ratio | 0.034484 | 0.034531 | Passed |
| Early | Medium | Soak | 15 | Wide ratio | 0.034484 | 0.034522 | Passed |
| Early | Large | Full | 9 | Wide ratio | 0.032329 | 0.032833 | Passed |
| Early | Large | Soak | 15 | Wide ratio | 0.032499 | 0.032874 | Passed |
| All-zero | Small | Full | 9 | Standard | 0.662948 | 0.663428 | Passed |
| All-zero | Small | Soak | 15 | Standard | 0.663097 | 0.663577 | Passed |
| All-zero | Medium | Full | 9 | Standard | 0.508492 | 0.509267 | Passed |
| All-zero | Medium | Soak | 15 | Standard | 0.508635 | 0.509275 | Passed |
| All-zero | Large | Full | 9 | Standard | 0.558005 | 0.593449 | Passed |
| All-zero | Large | Soak | 15 | Standard | 0.543488 | 0.552667 | Passed |
| Late | Small | Full | 9 | Standard | 0.663712 | 0.664097 | Passed |
| Late | Small | Soak | 15 | Standard | 0.663407 | 0.663593 | Passed |
| Late | Medium | Full | 9 | Standard | 0.508932 | 0.509633 | Passed |
| Late | Medium | Soak | 15 | Standard | 0.508856 | 0.509360 | Passed |
| Late | Large | Full | 9 | Standard | 0.560871 | 0.577030 | Passed |
| Late | Large | Soak | 15 | Standard | 0.562381 | 0.563299 | Passed |

All 18 regressions checked one exact `not-zero` measurement and passed both the median and bootstrap-upper `1.25x` rules.
Every report has exactly one timing attempt, so no favorable rerun occurred.

## Resource Evidence

The allocation-counted worker test proves zero timed Stab allocations for every pattern at small, medium, large, and accepted-maximum widths. Commit `fd016d5dc4afbe539c9a5a44242694693edfc622` added the accepted maximum for every pattern after independent review exposed the original omission.
Each report records setup and peak process RSS, but those values remain observations rather than a cross-scale or Stim-relative memory claim.
Observed peak RSS ranges from 3,395,584 to 8,531,968 bytes for Stim and from 4,526,080 to 9,662,464 bytes for Stab.
PQ6 remains responsible for a source-owned memory-growth rule and accepted slack.

## Source Reports

All paths below are relative to the repository root and contain canonical `report.json`, `preflight.json`, and derived `report.md` artifacts.

| Pattern | Scale | Tier | Directory | Report SHA-256 |
| --- | --- | --- | --- | --- |
| Early | Small | Full | `target/benchmarks/qualification/pq2-not-zero-early-small-full-60b732c` | `f3bc4bf4f9a1b951c46e198eb2ffb65eb1ab37408cf226f869efb9d71a098ec2` |
| Early | Small | Soak | `target/benchmarks/qualification/pq2-not-zero-early-small-soak-60b732c` | `26e59cd3b1309e3e4557417d80556c7da8d2030aa730725984bb06ed049b2048` |
| Early | Medium | Full | `target/benchmarks/qualification/pq2-not-zero-early-medium-full-60b732c` | `29c05b5eeb5a7fec612b303468581a2dd7ae2a7796ca0cbb16fab93aa08513cc` |
| Early | Medium | Soak | `target/benchmarks/qualification/pq2-not-zero-early-medium-soak-60b732c` | `0582a3a92dae626d2c2456259490185f264ca50edcada621bd2fb38222699cc9` |
| Early | Large | Full | `target/benchmarks/qualification/pq2-not-zero-early-large-full-60b732c` | `bd358e4cc851fe8c4a7f695d5f000a0724adf1b516c2aa6a83353388653b11db` |
| Early | Large | Soak | `target/benchmarks/qualification/pq2-not-zero-early-large-soak-60b732c` | `b4d5aadbca31db850ef3cb4da5086053bf1be099383e4a78b7b75daa1b0be132` |
| All-zero | Small | Full | `target/benchmarks/qualification/pq2-not-zero-all-zero-small-full-60b732c` | `5f44b1c1d66003e3cc9973884f3cbf68bf097664859e4be0ea73598ee67d4c2c` |
| All-zero | Small | Soak | `target/benchmarks/qualification/pq2-not-zero-all-zero-small-soak-60b732c` | `91ee9ec01fb09eec5a8040063ed588044d99eb9aa4b7c10559c68621065a0398` |
| All-zero | Medium | Full | `target/benchmarks/qualification/pq2-not-zero-all-zero-medium-full-60b732c` | `adfab6994c415c0f73500e47ea8c6cbf1685e89410a2f5aacef2e9c5ee5bc686` |
| All-zero | Medium | Soak | `target/benchmarks/qualification/pq2-not-zero-all-zero-medium-soak-60b732c` | `ede509af77b5c22a21b47c6376aab713228a05fc35792aeffdaa2e20cd04932a` |
| All-zero | Large | Full | `target/benchmarks/qualification/pq2-not-zero-all-zero-large-full-60b732c` | `6ba45ab4b5abadf9c1ac0371349f296d11221f26b03f705b6f8a01ed41fb4a69` |
| All-zero | Large | Soak | `target/benchmarks/qualification/pq2-not-zero-all-zero-large-soak-60b732c` | `e9ef17a352fef17ff515eb470c0fbfbab9cf94ae9dd00f4afe322cd82aaa2f76` |
| Late | Small | Full | `target/benchmarks/qualification/pq2-not-zero-late-small-full-60b732c` | `d1bfefc340af9874d4f27b17394a9c82981c0d69f217db76183e698b3d422039` |
| Late | Small | Soak | `target/benchmarks/qualification/pq2-not-zero-late-small-soak-60b732c` | `4c68c45ac094d77bf212bb4959bd301614e616fd6d9357640330023e45ad98ba` |
| Late | Medium | Full | `target/benchmarks/qualification/pq2-not-zero-late-medium-full-60b732c` | `342f9d4bbc0f4f04179891fecae2bb177653a59a39eb2352fc71007f9c710a96` |
| Late | Medium | Soak | `target/benchmarks/qualification/pq2-not-zero-late-medium-soak-60b732c` | `97c61a9a5f8798677711e494992fe65f6ae00b042a902e773280d82cf4454066` |
| Late | Large | Full | `target/benchmarks/qualification/pq2-not-zero-late-large-full-60b732c` | `b1754c5a03ea928e3f26ff8b0cb8feec1fde9c2706e9e1b4c7b1052fa91f0fc1` |
| Late | Large | Soak | `target/benchmarks/qualification/pq2-not-zero-late-large-soak-60b732c` | `6a74ae54c1e9a3f9fead5b8f7d000b920ccfcf6dfdd7b91166500741e55a296e` |

## Rollups And Completion Receipts

| Group | Full rollup SHA-256 | Soak rollup SHA-256 | Completion report SHA-256 | Completion preflight SHA-256 |
| --- | --- | --- | --- | --- |
| Early | `a174db3ced85bf03b90811c882b9b7f910665041c499a1daba29d6d4060207d0` | `fe7c0504422f478c9c5dedfb192c060586c7b732b943a4c1e4f52d5a43c167a5` | `121c3a85e7c5c8a781772ea19ca9ecba3ee4741e3b1ae20838ff1d4391202ffb` | `15a55190c268aa613dfab7f0e84eff8c0a5a3978ac64d75b83bcf84ca896b2c6` |
| All-zero | `61872631f07cbefa7d7fe19bac8742fa3664096756594eb75bc87dfb1895e8e0` | `16df946bd9bb2e9ece6661c10fdb5715ab87a9fe6347b332f5830056d3095156` | `58d58dc2c84714fd47c21241ae8e18f63e831988a7c5f7f5014f2db6b4cd583a` | `7137bda02301be208377aa9ea1db8c223e265a50a32c6f1f30ca37650ae53384` |
| Late | `ba7ba43e5dd3fe7c10218e461ac767ddbe39103954aec663809124b1ccb05e03` | `c228c762a3f1bbcdf7ea79140f4df54ef7ea11990bd318a58a6405c79eb0a255` | `2f58f5bd508bf7cdc85bc0dda240495dc60f54de3534f1420bb90940a70b1df2` | `6865dc5bb1d7446a232f45c0268da1b9d61330564eb86bfb3dca5af1d16f8d58` |

The full and soak rollups for every group passed publication and offline replay.
Each schema-version-1 completion receipt binds exactly six source reports, two rollups, 16 successful closure steps, one clean revision, one CPU identity, one worker identity set, the exact adapter probe, all report replays and regressions, and both rollup replays.
All three completion receipts passed independent replay.

## Legacy M12 Migration

The early group's clean completion receipt is the exact migration evidence for legacy pair `simd_bits_not_zero_100K` / `stab_simd_bits_not_zero_10K`.
At the accepted sixth-slice checkpoint, the M12 threshold file therefore retired only that pair and retained `simd_bits_xor_10K` / `stab_simd_bits_xor_10K` at `1.25x`.
The generated inventory removes the stale `not_zero` replacement marker and keeps the XOR replacement marker.
All-zero and late-hit evidence remain independent additional guards and were not used as substitutes for the legacy pair.

## Milestone Audit

The initial milestone audit missed the accepted-maximum allocation omission and post-migration evidence mismatch that independent review later exposed. After fixing both, the final re-audit verified 18 promotable first-attempt passes, 18 report replays, 18 regression passes, six rollup replays, three completion receipt replays, 42 preflight probes per report, exact clean revision and worker identity agreement, accepted and rejected width boundaries, zero timed Stab allocations at every runtime scale and accepted maximum, and the narrow legacy-threshold migration. No milestone under-specification was found, so `docs/plans/milestone-spec-gaps.md` gains no new entry.

## Independent Review

Independent GPT-5.6/max full code review confirmed the portable-SIMD implementation, scalar and tail coverage, pinned-Stim comparator fidelity, optimizer barriers, hostile width bounds, wide-ratio policy, receipt identities, narrow threshold migration, generated digest, and module boundaries. It initially found two P1 closure defects, missing accepted-maximum allocation instrumentation and lack of post-migration evidence at the current inventory, plus stale PQ0 inventory counts. After the allocation test, complete current-inventory evidence chain, and all corrected generated counts landed, the same reviewer inspected the new artifacts and source and reported no confirmed findings. Final review status is complete.

## Verification Record

The clean evidence revision produced and replayed the exact correctness preflight, all 18 reports, all six rollups, and all three completion receipts.
The closure also passed these focused checks:

```sh
cargo test -p stab-core not_zero --quiet
cargo test -p stab-bench not_zero --quiet
cargo test -p stab-bench wide_ratio --quiet
cargo test -p stab-bench --features count-allocations not_zero_timed_scans_allocate_nothing --quiet
cargo test -p stab-bench completion --quiet
just bench::qualification-worker-reproducibility
just bench::qualification-probe --group pq2-simd-bits-not-zero-early-adapter-smoke --iterations 2 --work-items 10000
just bench::qualification-probe --group pq2-simd-bits-not-zero-all-zero-adapter-smoke --iterations 2 --work-items 10000
just bench::qualification-probe --group pq2-simd-bits-not-zero-late-adapter-smoke --iterations 2 --work-items 10000
```

The accepted sixth-slice migration and evidence state passed:

```sh
just bench::qualification-regenerate --check
just bench::qualification-check
cargo test -p stab-bench reworked_heterogeneous --quiet
```

Before committing this closure report, rerun workspace formatting, Clippy with warnings denied, all workspace tests, qualification regeneration, correctness regeneration, benchmark smoke, and staged pre-commit policy.

## Remaining Work

1. Run the same clean full and soak families, rollups, and completion receipts on a controlled native Linux x86-64 host before making an x86-64 conclusion.
2. Define and validate explicit cross-scale RSS and allocation-growth slack in PQ6 before making a memory qualification claim.
3. Select the next finite dependency-ordered PQ2 runtime group without reopening this completed slice.

The separate 271-parent CQ2 checkpoint is source-current at clean hardened-controller revision `3f2f382627c8421de0a668819d467a9f252de20f` without relabeling earlier performance reports.
