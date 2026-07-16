# PQ2 SIMD-Bits `not_zero` Qualification Progress Report

## Status

The first clean sixth-slice evidence chain passed its independent `1.25x` timing gate for early-hit, all-zero, and late-hit `not_zero` scans at every full and soak scale on the controlled Linux AArch64 host as of 2026-07-16. It is historical evidence at inventory `c2362b6fda35626e5f571399512067ab406f92a22692c162ae70002ef7a651f8`, not source-current closure after the legacy-threshold migration changed the inventory digest.

All 18 promotable measurements passed on their first attempt, without a noise rerun or profiler note.
Median Stab-to-Stim elapsed-time ratios range from `0.030337x` to `0.663540x`, corresponding to approximately 1.51x to 32.96x the pinned Stim throughput for the exact source-owned workloads.
The worst bootstrap confidence-interval upper bound is `0.071537x` for early-hit, `0.663810x` for all-zero, and `0.663639x` for late-hit.

Current closure of these three contracts remains pending a complete clean rerun at source-current inventory `0161ab09015487ee2a1298be8dafe7c744b426b28a4e9fbdbd688e775c1655a0`:

- `PERFQ-M5-SIMD-BITS-NOT-ZERO-EARLY`
- `PERFQ-M5-SIMD-BITS-NOT-ZERO-ALL-ZERO`
- `PERFQ-M5-SIMD-BITS-NOT-ZERO-LATE`

It does not qualify native Linux x86-64, other bit-kernel phases, cross-scale memory growth, or remaining PQ2 groups.

## Frozen Evidence

- Clean Stab implementation revision: `817d0fe870fd1b02c8e30f18e534e35df705a1ee`, with `local_modifications=false` before and after every correctness, performance, rollup, and completion producer.
- Stim baseline: v1.16.0 at `e2fc1eca7fd21684d433aa5f10f4504ea4860d07`.
- Evidence performance inventory: `c2362b6fda35626e5f571399512067ab406f92a22692c162ae70002ef7a651f8`.
- Correctness inventory: `ccb80eb4b660a375b59460c3b7fa03a810abd6f868735b566735378105db22b2`.
- Post-evidence source-current performance inventory: `0161ab09015487ee2a1298be8dafe7c744b426b28a4e9fbdbd688e775c1655a0`.
- Host profile: verified `linux-aarch64-controlled`, pinned to logical CPU 0 with CPU identity `CPU implementer=0x41, CPU architecture=8, CPU variant=0x0, CPU part=0xd87, CPU revision=1`.
- Rust toolchain: `nightly-2026-06-20`, release profile, target `aarch64-unknown-linux-gnu`.

The source-current digest differs because the completed early-hit receipt authorized retirement of the duplicate legacy M12 `not_zero` threshold pair after timing finished.
The runtime contracts, workers, workload fixtures, comparators, and timing rules did not change during that migration, but report and completion replay intentionally reject inventory drift, so the historical ratios cannot close the current inventory.

## Correctness Preflight

The clean correctness report at `target/qualification/pq2-m5-not-zero-full-817d0fe` selected and passed exactly these two cases:

- `cq-evidence-qualification-b1530dc4e48e942d` owns `BitVec::not_zero`, zero and nonzero semantics, canonical tails, and the bit-kernel behavior used by the comparator.
- `cq-evidence-qualification-ba252d42660a41ce` owns storage shape, views, and access boundaries.

| Artifact | SHA-256 |
| --- | --- |
| Request | `9222c21fb8533449d58e7c8f1bf18e2db7b1b932dbe4c7347ce7b0e620ce39a5` |
| Report | `373e4ecf08de6c1c72608a6c383f2671c9423df1f07ca732e82c175845993f2c` |
| Completion | `44cd8aeb3fc482b9da1db680b16d092b36459de76a1628e411051d8e48a62874` |
| Preflight | `963a6e4a6c3d2dfd37e3a4a1660aa95cc66ef2d5ebc2e67c178402fdc40ba89a` |

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
| Stab build fingerprint | `d1396cc6a26b000b9a64892ea746c903d59a93f916174cc436ed72fc90ce2a20` |
| Stab binary | `144d2ad6bf94bbc09bb71a4e1da62057eced718e21112633e9bc8681b6d4ed45` |
| Contract preflight | `cd7d488e0c7466623112fc408cd1ab4352748055421ca13dd7bab9ee5b314c1e` |

Every performance report contains the same 42 actual contract-preflight receipts and the same worker identities.
The three standalone 10,000-bit adapter probes passed exact input and output identity checks and remain diagnostic rather than ratio evidence.

## Timing Results

Standard mode independently calibrates both sides between 250 milliseconds and 2 seconds, then uses one identical common iteration count.
The early-hit ratios have no valid standard overlap, so all six early reports use source-derived wide-ratio mode.
In wide-ratio mode only the implementation that selected fewer independent iterations may exceed 2 seconds, the common-iteration owner remains at or below 2 seconds, both sides remain at least 250 milliseconds, and neither side may exceed the 20-second hard ceiling under the unchanged 30-second invocation timeout.

| Pattern | Scale | Tier | Pairs | Mode | Median ratio | 95% upper | Outcome |
| --- | --- | --- | ---: | --- | ---: | ---: | --- |
| Early | Small | Full | 9 | Wide ratio | 0.071509 | 0.071537 | Passed |
| Early | Small | Soak | 15 | Wide ratio | 0.071500 | 0.071515 | Passed |
| Early | Medium | Full | 9 | Wide ratio | 0.034421 | 0.034451 | Passed |
| Early | Medium | Soak | 15 | Wide ratio | 0.034419 | 0.034431 | Passed |
| Early | Large | Full | 9 | Wide ratio | 0.030337 | 0.030909 | Passed |
| Early | Large | Soak | 15 | Wide ratio | 0.032677 | 0.033282 | Passed |
| All-zero | Small | Full | 9 | Standard | 0.663540 | 0.663810 | Passed |
| All-zero | Small | Soak | 15 | Standard | 0.663212 | 0.663595 | Passed |
| All-zero | Medium | Full | 9 | Standard | 0.509575 | 0.510001 | Passed |
| All-zero | Medium | Soak | 15 | Standard | 0.509579 | 0.509853 | Passed |
| All-zero | Large | Full | 9 | Standard | 0.552156 | 0.565710 | Passed |
| All-zero | Large | Soak | 15 | Standard | 0.558518 | 0.562429 | Passed |
| Late | Small | Full | 9 | Standard | 0.663045 | 0.663639 | Passed |
| Late | Small | Soak | 15 | Standard | 0.663175 | 0.663498 | Passed |
| Late | Medium | Full | 9 | Standard | 0.509786 | 0.510035 | Passed |
| Late | Medium | Soak | 15 | Standard | 0.509610 | 0.509878 | Passed |
| Late | Large | Full | 9 | Standard | 0.558561 | 0.565682 | Passed |
| Late | Large | Soak | 15 | Standard | 0.558516 | 0.559970 | Passed |

All 18 regressions checked one exact `not-zero` measurement and passed both the median and bootstrap-upper `1.25x` rules.
Every report has exactly one timing attempt, so no favorable rerun occurred.

## Resource Evidence

The initial allocation-counted worker test proved zero timed Stab allocations for every pattern at small, medium, and large widths but accidentally omitted the accepted maximum. Commit `fd016d5dc4afbe539c9a5a44242694693edfc622` adds the accepted maximum for every pattern, and the corrected allocation-enabled test passes before the source-current evidence rerun.
Each report records setup and peak process RSS, but those values remain observations rather than a cross-scale or Stim-relative memory claim.
Observed peak RSS ranges from 3,395,584 to 8,536,064 bytes for Stim and from 4,657,152 to 9,797,632 bytes for Stab.
PQ6 remains responsible for a source-owned memory-growth rule and accepted slack.

## Source Reports

All paths below are relative to the repository root and contain canonical `report.json`, `preflight.json`, and derived `report.md` artifacts.

| Pattern | Scale | Tier | Directory | Report SHA-256 |
| --- | --- | --- | --- | --- |
| Early | Small | Full | `target/benchmarks/qualification/pq2-not-zero-early-small-full-817d0fe` | `52f2cea384999ec9736681bebca2d1627fa9f6524741d36313c20f14d74b68ec` |
| Early | Small | Soak | `target/benchmarks/qualification/pq2-not-zero-early-small-soak-817d0fe` | `6efad5e3784d47216649839a30448dda050e41c83cdaeb1e0fc052a9a53d3af4` |
| Early | Medium | Full | `target/benchmarks/qualification/pq2-not-zero-early-medium-full-817d0fe` | `584ff5a829be2f804eb87f821ce1c9665071161701c6bd96451677137fb172b0` |
| Early | Medium | Soak | `target/benchmarks/qualification/pq2-not-zero-early-medium-soak-817d0fe` | `dd5a9afd9e2b8cbcb6cacc49488869eab3858fdb8b267332677bdec61c9d54b3` |
| Early | Large | Full | `target/benchmarks/qualification/pq2-not-zero-early-large-full-817d0fe` | `192e4d49b507949ae9525f4e4f54bb01a1c62a63289149dc7b3265177e4ff361` |
| Early | Large | Soak | `target/benchmarks/qualification/pq2-not-zero-early-large-soak-817d0fe` | `f525fcfdb3898d4090adca8cf943c9789dcb2873026b8996badbe6cce597ab5a` |
| All-zero | Small | Full | `target/benchmarks/qualification/pq2-not-zero-all-zero-small-full-817d0fe` | `0b69d63c685927d8bc549c188724626b9410738f10ae07c35f13e470e9b4f748` |
| All-zero | Small | Soak | `target/benchmarks/qualification/pq2-not-zero-all-zero-small-soak-817d0fe` | `b699eff26c309aabacd9c82221afe9e9838bc82bfe131decd71f9b32ec5b9637` |
| All-zero | Medium | Full | `target/benchmarks/qualification/pq2-not-zero-all-zero-medium-full-817d0fe` | `e1730478bcf185fd403a7d11473bc71733a311160ce9c505e0b560bafcf13370` |
| All-zero | Medium | Soak | `target/benchmarks/qualification/pq2-not-zero-all-zero-medium-soak-817d0fe` | `f7edcb0a34b291c983626da26c8846b7f6f069f82d2a08ec5c29b542cd7cc5ac` |
| All-zero | Large | Full | `target/benchmarks/qualification/pq2-not-zero-all-zero-large-full-817d0fe` | `8ee3e020217f374e4a9df6499852349fda8be6594d492b1ad56e8b93786b4982` |
| All-zero | Large | Soak | `target/benchmarks/qualification/pq2-not-zero-all-zero-large-soak-817d0fe` | `e0d6479ec09f5cdfbddae84876d08b10c5f02d0371db656dab84918c88683299` |
| Late | Small | Full | `target/benchmarks/qualification/pq2-not-zero-late-small-full-817d0fe` | `924b33cc9b7dcbf3ba1ae73f35a0af28f2ac243d5888d304d7fefa9bfe454f1d` |
| Late | Small | Soak | `target/benchmarks/qualification/pq2-not-zero-late-small-soak-817d0fe` | `df8dd105b248e285c7ca6298e09c8ce49e75d790476c3766c28c58e059277cdf` |
| Late | Medium | Full | `target/benchmarks/qualification/pq2-not-zero-late-medium-full-817d0fe` | `9182e9649f7a4a46e8f359a17cfb7ec54f48fbac6815d0eaa1ac18b8c24a2147` |
| Late | Medium | Soak | `target/benchmarks/qualification/pq2-not-zero-late-medium-soak-817d0fe` | `07e71519df12b4530a16fb4c6bb2986601648e9f0c8e99b4af4661b3af42a459` |
| Late | Large | Full | `target/benchmarks/qualification/pq2-not-zero-late-large-full-817d0fe` | `ecbfc924d1df6ee5fa59f415fdacad0bee38799213b157a1d1ec847108d77ed9` |
| Late | Large | Soak | `target/benchmarks/qualification/pq2-not-zero-late-large-soak-817d0fe` | `2af6433691f7e78dbac0d1f9025c1c2f0ee54b660178395596fdac5598553b38` |

## Rollups And Completion Receipts

| Group | Full rollup SHA-256 | Soak rollup SHA-256 | Completion report SHA-256 | Completion preflight SHA-256 |
| --- | --- | --- | --- | --- |
| Early | `7a4a9719a68abd6e43c11b67b9a2c6310ce74c46241bbdadf9c73e92185fbb42` | `a5cbe71c7e62fa8276d5020688b16ebaff26645f7e0645fa61a9cfcc76209d79` | `17544d5dde520f6dc9b7988d988593f837cf14e2812da49f8faee9880ae2e936` | `b0d5c8613797d32fad3a26f58b1f3306b1d79f235b5b0179172aee7e5132d071` |
| All-zero | `c5554a83f6afdc83c5b72a3b934bdd1c6e0fe9619ff634fa8d9ab4b3f4c190f1` | `e1f84bf35cbbb62db006cdaf1c09c255799a8e266d66c40a69a19a54b1b259ff` | `d526b28d3b6770185ac8e68c036165f0802b293713ac6049389eadc1f9d9e28e` | `bcd0170d396b3f810f9d0641926bff5be7ed5ae84b2a9287966051f9289d04a9` |
| Late | `221d27ae29bc4b1eded1d6002dd1093182cf3f92f752729fdda0b60f3426b620` | `080d9a0c714ecf0f6a7b85a6f95f46971abf9c392ac4fae219ed3293fb9d22df` | `50e82f4d298810a6a5149171e1cd61aad5bcce2d606f96bcff0eed933ca64744` | `ff8d339b5faea6262db9c1fd4f8ef5f64809f3c67632c3f7c3613c5e25b52139` |

The full and soak rollups for every group passed publication and offline replay.
Each schema-version-1 completion receipt binds exactly six source reports, two rollups, 16 successful closure steps, one clean revision, one CPU identity, one worker identity set, the exact adapter probe, all report replays and regressions, and both rollup replays.
All three completion receipts passed independent replay.

## Legacy M12 Migration

The early group's clean completion receipt is the exact migration evidence for legacy pair `simd_bits_not_zero_100K` / `stab_simd_bits_not_zero_10K`.
The source-current M12 threshold file therefore retires only that pair and retains `simd_bits_xor_10K` / `stab_simd_bits_xor_10K` at `1.25x`.
The generated inventory removes the stale `not_zero` replacement marker and keeps the XOR replacement marker.
All-zero and late-hit evidence remain independent additional guards and were not used as substitutes for the legacy pair.

## Milestone Audit

The initial milestone audit found no defect, but independent review later revealed that the accepted maximum was missing from allocation-counted testing and that post-migration closure requires a complete source-current evidence chain. The allocation defect is fixed in `fd016d5dc4afbe539c9a5a44242694693edfc622`; the current-inventory rerun and final milestone re-audit remain pending. No milestone under-specification was found, so `docs/plans/milestone-spec-gaps.md` gains no new entry.

## Independent Review

Independent GPT-5.6/max full code review confirmed the portable-SIMD implementation, scalar and tail coverage, pinned-Stim comparator fidelity, optimizer barriers, hostile width bounds, wide-ratio policy, receipt identities, narrow threshold migration, generated digest, and module boundaries. It found two P1 closure defects: missing accepted-maximum allocation instrumentation and lack of post-migration evidence at the current inventory, plus one P3 stale-count defect in the PQ0 table. The allocation and table defects are fixed. Current-inventory correctness, worker, adapter, full, soak, regression, rollup, and completion evidence must be regenerated and replayed before final review closure.

## Verification Record

The clean implementation revision produced and replayed the exact correctness preflight, all 18 reports, all six rollups, and all three completion receipts.
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

The post-evidence migration passed:

```sh
just bench::qualification-regenerate --check
just bench::qualification-check
cargo test -p stab-bench reworked_heterogeneous --quiet
```

Before committing the documentation and migration change, rerun workspace formatting, Clippy with warnings denied, all workspace tests, qualification regeneration, benchmark smoke, and staged pre-commit policy. After that clean commit exists, regenerate the entire evidence chain at the current inventory and replace historical-only status with exact current report hashes and final review closure.

## Remaining Work

1. Regenerate and replay the complete Linux AArch64 correctness, worker, adapter, full, soak, regression, rollup, and completion chain at source-current inventory `0161ab09015487ee2a1298be8dafe7c744b426b28a4e9fbdbd688e775c1655a0`, then rerun milestone audit and independent review.
2. Run the same clean full and soak families, rollups, and completion receipts on a controlled native Linux x86-64 host before making an x86-64 conclusion.
3. Rerun the complete 271-parent CQ2 family at the next simultaneous-current program checkpoint instead of relabeling historical all-domain reports.
4. Define and validate explicit cross-scale RSS and allocation-growth slack in PQ6 before making a memory qualification claim.
5. Select the next finite dependency-ordered PQ2 runtime group without reopening this slice until current closure is complete.
