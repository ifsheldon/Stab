# PQ2 Clifford-String Qualification Progress Report

## Status

The eleventh PQ2 executable slice remains active for source-current closure on controlled Linux AArch64.

Clean Stab revision `da7c787d1e9f49110d7054868b146b5fb7d7bda4` published and replayed a complete schema-version-31 chain: one exact three-case focused correctness report, byte-reproducible private workers, both diagnostic adapter probes, all 12 first-attempt full and soak scale reports and regression decisions, four architecture-scoped rollups, and two independently replayed completion receipts. That chain is historical under correctness digest `4dbbb4b2cda3117bdd3d3ddfcd30b55f09e6f401352e3e86130222189d47791f` and performance digest `c238dc4e2500192f310ef3d2378ecaafc9744662b5127784dd4eeb6c60726176` after follow-up review changed correctness source, artifact publication, and generated inventory ownership.

`PERFQ-M6-CLIFFORD-STRING` qualifies only the exact pinned identity-right workload, while `PERFQ-M6-CLIFFORD-STRING-NON-IDENTITY` qualifies a deterministic complete 24-by-23 cycle of every left Clifford against every non-identity right Clifford.
The two contracts have separate measurements, scale reports, thresholds, rollups, completion receipts, and performance conclusions.

All 12 historical schema-version-31 timing reports passed on their first attempt without a noise rerun, waiver, report-only outcome, or threshold relaxation.
Identity median Stab-to-Stim seconds-per-work ratios ranged from `0.000146x` to `0.014535x`, corresponding to approximately `68.80x` through `6841.64x` speedups for the exact identity workload.
Non-identity median elapsed-time ratios ranged from `0.743053x` to `0.765340x`, corresponding to approximately `1.31x` through `1.35x` speedups for complete non-identity multiplication.
The worst historical schema-version-31 bootstrap confidence-interval upper bound was `0.765806x`, below the exact `1.25x` gate.

Successive independent reviews and the first replacement timing attempt after that chain found additional closure defects in short-right-operand complexity, pre-mutation path admission, end-to-end artifact and repository binding, rollback and cleanup safety, production-dispatch coverage, generated checklist ownership, descriptor-root source access, and final correctness-tree revalidation. The current source returns prefix non-identity deltas from the packed kernel, admits every direct output and input path before mutating work, binds exact staged, target, CQ, report, rollup, and completion file sets by descriptor identity, length, and digest through hierarchy durability, owns retained repository descriptors in typed roots, performs repository-relative source, Git metadata, and correctness-tree access from those descriptors without following untrusted path components or reopening procfs descriptor magic links, revalidates every repository-relative correctness ancestor through the exact CQ output, revalidates the recorded repository state through publication, performs descriptor-checked rollback and cleanup, tests the production completion dispatcher, and makes advertised checklist counts a checked inventory contract. These fixes require a fresh clean focused correctness and performance chain before the slice can be source-current again.

The identity result is not used as a proxy for non-identity multiplication.
Its source-owned independent-throughput policy is valid because both implementations perform the same public logical operation and declare the same per-iteration single-qubit work, while Stab's semantically equivalent identity-right metadata fast path is O(1).
Every identity report separately proves exact output at the smaller selected iteration count and normalizes timing by each implementation's exact report-bound work.
The non-identity family retains ordinary common-iteration timing with equal total work.

When the replacement chain passes, this report will close only equal-width public in-place Clifford-string multiplication on Linux AArch64.
It does not qualify allocating multiplication, unequal-width growth, construction, randomization, concatenation, repetition, display, Tableau operations, native Linux x86-64, cross-scale memory growth, or the remaining Algebra surface.

## Current Contract And Historical Evidence

- Latest historical clean Stab revision: `da7c787d1e9f49110d7054868b146b5fb7d7bda4`, with `local_modifications=false` before and after every schema-version-31 correctness and performance producer.
- Historical clean post-migration Stab revision: `91f62d0a78659da2e8e264a6968b3c6cd32456de`, with `local_modifications=false` before and after every historical producer and completion controller.
- Focused migration commit: `91f62d0a78659da2e8e264a6968b3c6cd32456de`.
- Clean pre-migration authorization revision: `127d6661a9e00872fc4aa4c0b0d27171e005afa5`.
- Stim baseline: v1.16.0 at `e2fc1eca7fd21684d433aa5f10f4504ea4860d07`.
- Current performance inventory: `a47866ba5eab70392dd2754391d3d7d8588567a7cbfc1f81a569be813804ce51`; historical `da7c787` evidence inventory: `c238dc4e2500192f310ef3d2378ecaafc9744662b5127784dd4eeb6c60726176`; earlier schema-version-30 evidence inventory: `a76090c996ad404c1cb8bfa85066e286c6f40b32754b3750e984375f7ca90025`.
- Current correctness inventory: `648e7ea5a66997a810498dc871257bd654c7f9af9304651d43a88103eded0289`; historical `da7c787` evidence inventory: `4dbbb4b2cda3117bdd3d3ddfcd30b55f09e6f401352e3e86130222189d47791f`.
- Current runtime-group contract: `e76e8e26a7daad8a5f5504511d8091c5499766e83e065d7b7c785b4bf57ddaf3`; historical runtime-group contract: `4d7b0e4808828217dc0a353ea991321c8483579ed62b84ca42a1cae6f1b4c2ee`.
- Current profiler note: `benchmarks/profiler-notes/qualification/perfq-m6-clifford-string.md` at SHA-256 `4b484afed5b1a20d4e1f9c71eccb592f0e2b8a55dc71cb9b818c0b08b2c52e04`.
- Current frozen vector fixture: `benchmarks/fixtures/pq2-clifford-string-vectors.json` at SHA-256 `e61cd02dd29eb006892444eddd30693031e39746add588a8f538888499a29d85`.
- Checked migration ledger: `benchmarks/qualification-threshold-migrations.json` at SHA-256 `e27cd284ad76c91b213fe5e5fff8c8f5058810c33874965dfe53f49883cec810`.
- Pinned Stim comparator source: `benchmarks/stim_adapter/clifford_string_contract.h` at SHA-256 `95d628eabf8db5795fd3391c97f4f6a0ab118e62e7cce91652458af40f7f6bf8`.
- Host profile: verified `linux-aarch64-controlled`, pinned to logical CPU 0 with CPU identity `CPU implementer=0x41, CPU architecture=8, CPU variant=0x0, CPU part=0xd87, CPU revision=1`.
- Rust toolchain: `nightly-2026-06-20`, release profile, target `aarch64-unknown-linux-gnu`.

The source-current performance inventory contains seventeen executable product contracts with one exact `1.25x` rule at each of three scales.
The inherited `m6-clifford-string` timing row is superseded, while its process-memory baseline remains guarded until PQ6 supplies equal or stronger cross-scale evidence.
Current source-owned closure continues to require private Stab build-receipt schema version 5, adapter receipt schema version 11, contract-preflight schema version 12 with 212 probes, and qualification report schema version 31. No report under the two current inventory digests exists yet; the replacement must bind one focused correctness family, worker identities, inventories, runtime-group contract, toolchain, verified host profile, and clean revision.

## Inventory Status

`just bench::qualification-check` validates 549 qualification groups with 547 `measured`, zero `covered-by-parent`, two `not-performance-relevant`, and zero `no-faithful-comparator` dispositions.
The 161 inherited benchmark rows contain nine retained, 135 reworked, four diagnostic, eleven superseded, and two removed rows.
This slice changes only the inherited `m6-clifford-string` row from reworked to superseded after its exact identity replacement completion authorized migration.

The historical schema-version-31 chain contributes twelve raw source measurements: six identity and six non-identity reports across full and soak tiers and three scales.
Full reports contain nine retained pairs apiece and soak reports contain 15, for 144 retained timing pairs in total.
Its exact timing outcome is twelve passed, zero failed, zero noisy, and zero report-only.
There is no slow or noisy row requiring a next action; the source-owned profiler note remains bound because it records the scalar failure, packed portable-SIMD optimization, identity timing policy, and migration provenance.
The schema-version-30 chain has the same 12-report shape and remains historical under its exact producer and inventory. Memory evidence remains report-only, and no scaling or Stim-relative memory claim is made before PQ6.

## Historical Schema-Version-31 Correctness Preflight

The clean focused full-tier report at `target/qualification/pq2-clifford-cq-full-da7c787d` selected and passed exactly three cases with zero failed, planned, or deferred selections under correctness inventory `4dbbb4b2cda3117bdd3d3ddfcd30b55f09e6f401352e3e86130222189d47791f`:

- `cq-evidence-qualification-40e5ad2f2f4c4fd4` owns the public Clifford-string resource contract, accepted 1,048,576-qubit equal-width multiplication, first rejection at 1,048,577 qubits, bounded iterator consumption, checked growth, and pre-RNG rejection.
- `cq-evidence-qualification-510e746ec36e7d1c` owns equal-width public in-place output, right-operand immutability, deterministic identity and complete non-identity cycles, and the representative Clifford-string value contract.
- `cq-evidence-qualification-ae9390dd6a207cb6` owns the independent all-24-by-24 Tableau-backed Clifford multiplication table and all-24-cubed associativity contract.

| Artifact | SHA-256 |
| --- | --- |
| Request | `9d7fd4336871015571970d0d5aaf2cebc1996953a16f32a53ae3ea1c5ad8b83a` |
| Report | `8f8c60ce30c820af18d601942ffc6eee28b6841b78ab206eb96ad3486aa9db09` |
| Completion | `ebe9084416da312a7b1e427f71f99bb48d5c0bcfaa475e76b825e6525f98a7d8` |
| Preflight | `67dcbac6551214f9cde880b1ff91e6135decefc3db3ae5945da1ff037a1a4f74` |

The correctness report and exact preflight replayed successfully before timing. Every `da7c787` performance report independently reconstructed the same canonical correctness request, report, completion, execution receipts, and exact three-case preflight before accepting timing evidence. The next clean revision must generate a replacement under correctness inventory `648e7ea5a66997a810498dc871257bd654c7f9af9304651d43a88103eded0289`.

## Earlier Historical Schema-Version-30 Correctness Preflight

The historical clean correctness report at `target/qualification/pq2-clifford-cq-full-91f62d0a` selected and passed exactly these three cases:

- `cq-evidence-qualification-40e5ad2f2f4c4fd4` owns the public Clifford-string resource contract, accepted 1,048,576-qubit equal-width multiplication, first rejection at 1,048,577 qubits, bounded iterator consumption, checked growth, and pre-RNG rejection.
- `cq-evidence-qualification-510e746ec36e7d1c` owns equal-width public in-place output, right-operand immutability, deterministic identity and complete non-identity cycles, and the representative Clifford-string value contract.
- `cq-evidence-qualification-ae9390dd6a207cb6` owns the independent all-24-by-24 Tableau-backed Clifford multiplication table and all-24-cubed associativity contract.

| Artifact | SHA-256 |
| --- | --- |
| Request | `3fb17eab42bdd501fb89c44712ddcfaff64abc7e42bcd273895a63f9825d937a` |
| Report | `6252dfbdb5544e868535ec6debb013c528bd6165c34b234c8cec351a8fee6f9f` |
| Completion | `dbae7c5b9ab15ac6d6786f8975288c06c2ca324890a03e53e11f52dbea90e001` |
| Preflight | `f00cc3c66d9deec9cc9223c5709edac8753500d5b0a16b4b8b5ad85c49f31a1d` |

Every historical performance report independently reconstructed the canonical correctness request, report, completion, exact execution receipts, and exact three-case preflight before timing.
The historical schema-version-31 chain above replaced this prerequisite for its exact producer while retaining the earlier family for its schema-version-30 producer.
The complete 271-parent CQ2 execution remains a separate historical checkpoint and is not inferred from this focused prerequisite run.

## Workload Contracts

Both workers construct equal-width public Clifford strings before the start barrier, call public in-place multiplication through receipt-owned optimizer barriers, retain the right operand, derive a result-dependent execution witness after every callback, and hash the complete final left and right gate sequences outside timing.
Semantic work is checked `iterations * width` single-qubit products for both implementations.

| Group | Scale | Width | Fixture |
| --- | --- | ---: | --- |
| Identity | Small | 10,000 | Equal-width identity left and right operands |
| Identity | Medium | 100,000 | Equal-width identity left and right operands |
| Identity | Large | 1,000,000 | Equal-width identity left and right operands |
| Non-identity | Small | 10,000 | Repeated complete 24-by-23 composition cycle, tail 64 |
| Non-identity | Medium | 100,000 | Repeated complete 24-by-23 composition cycle, tail 88 |
| Non-identity | Large | 1,000,000 | Repeated complete 24-by-23 composition cycle, tail 328 |

The checked vector fixture freezes the pinned Stim 24-name order, canonical codes, eight accepted descriptors, exact odd and even outputs, all scale tails, both accepted maxima, and all 36 ordered requests per worker.
The canonical preflight binds exactly 72 Clifford receipts inside its 212-probe ordered worker matrix: ten accepted and 26 rejected requests for Stab, followed by the same ordered requests for Stim.
Rejected requests cover first-over-cap, zero width, unknown and opposite valid workload markers, wrong measurement, malformed descriptor fields and descriptor hex, nonzero reserved input, width-to-work mismatch, and semantic-work overflow before allocation and before barrier consumption.

The non-identity oracle uses a scalar 24-by-24 reference independent of the packed production representation and proves that every complete 552-position cycle contains every left Clifford against every non-identity right Clifford exactly once.
The correctness owner independently uses Tableau-backed multiplication across all 24-by-24 pairs, so benchmark-worker agreement alone cannot establish the group law.

## Historical Schema-Version-31 Worker Identity

`just bench::qualification-worker-reproducibility` rebuilt both sealed workers twice from clean revision `da7c787d1e9f49110d7054868b146b5fb7d7bda4` and reproduced these identities:

| Identity | SHA-256 |
| --- | --- |
| Stim source | `248420592bb5c243f86a854d43567fe3ce27e4c273612f6a1809eab7e0308ebf` |
| Stim build fingerprint | `57ca1f53144f10ced1c93860b3c8d9a5cbef7759ef1c55fc87910ed8df0d6d41` |
| Stim binary | `e6bbc3877c52a32174c05318b2c55a7174c2a9ddcf888b6fbc5f40b538cf2856` |
| Stab source | `d8b2f0d59be9e0d2685c2ae243eb203a71abaa17d4edf47af79bb71b0e230bc6` |
| Stab build fingerprint | `1d0aae7b88d1b37e65fe84620706e8881895f3186e153bb16d820084a26cc9d0` |
| Stab binary | `0ea2ab7efae8e86c5bd4e583d039326cafab8b7c03c496d1c7264ece864e59c0` |
| Contract preflight | `43ee44f95984b4134de73cde372f495854372a7ea14fed481dc83b42dd57ad35` |

The private Stab build receipt is schema version 5, the adapter receipt is schema version 11, the contract preflight is schema version 12, and the qualification report is schema version 31. The canonical preflight contains 212 ordered probes, including 72 Clifford receipts, and binds both workers' exact source, build fingerprint, and binary identity. Both source-owned adapter probes passed; their tiny timings remain diagnostic and are not product performance evidence.

## Earlier Historical Schema-Version-30 Worker Identity

`just bench::qualification-worker-reproducibility` rebuilt both historical sealed workers twice and reproduced these identities:

| Identity | SHA-256 |
| --- | --- |
| Stim source | `248420592bb5c243f86a854d43567fe3ce27e4c273612f6a1809eab7e0308ebf` |
| Stim build fingerprint | `57ca1f53144f10ced1c93860b3c8d9a5cbef7759ef1c55fc87910ed8df0d6d41` |
| Stim binary | `e6bbc3877c52a32174c05318b2c55a7174c2a9ddcf888b6fbc5f40b538cf2856` |
| Stab source | `1c3884909f9941e1af257f2ee9021fe557fc85e6b8d4b93c4b47f4e1e55474bd` |
| Stab build fingerprint | `bbb82db20ca156d5729e9a6fb3f84b51f1f6c4b5402c7baa3d1edffbffdc6c30` |
| Stab binary | `3caadaf7f6b4a763fa28502ac6165630a0d0b01f4a7f81569894b2e3c2bd490e` |
| Contract preflight | `805810f4559ad0c678fc744d4b3865b04725721cc341b38207d3e2730585f415` |

The historical private Stab build receipt is schema version 4.
The historical adapter receipt and contract preflight are schema version 11, and the historical qualification report is schema version 30.
The historical identities remain evidence only for revision `91f62d0a78659da2e8e264a6968b3c6cd32456de` and report schema version 30. Adapter smoke timings remain diagnostic and are never product speed evidence.

## Historical Schema-Version-31 Timing Results

Every full report retains nine interleaved pairs and every soak report retains 15. Each report contains one timing attempt and independently passes both the median and bootstrap-upper `1.25x` rules.

| Group | Scale | Tier | Pairs | Stim selected iterations | Stab selected iterations | Median ratio | 95% upper | Ratio rMAD | Outcome |
| --- | --- | --- | ---: | ---: | ---: | ---: | ---: | ---: | --- |
| Identity | Small | Full | 9 | 408,769 | 28,107,035 | 0.014530 | 0.019823 | 0.003091 | Passed |
| Identity | Medium | Full | 9 | 41,103 | 28,515,620 | 0.001459 | 0.001968 | 0.002660 | Passed |
| Identity | Large | Full | 9 | 4,144 | 28,369,686 | 0.000146 | 0.000176 | 0.002548 | Passed |
| Identity | Small | Soak | 15 | 408,722 | 28,099,118 | 0.014535 | 0.019812 | 0.003252 | Passed |
| Identity | Medium | Soak | 15 | 41,101 | 28,314,096 | 0.001455 | 0.001458 | 0.001302 | Passed |
| Identity | Large | Soak | 15 | 4,108 | 28,395,069 | 0.000146 | 0.000148 | 0.002573 | Passed |
| Non-identity | Small | Full | 9 | 401,425 | 524,838 | 0.765340 | 0.765733 | 0.000337 | Passed |
| Non-identity | Medium | Full | 9 | 41,103 | 55,263 | 0.743496 | 0.743752 | 0.000343 | Passed |
| Non-identity | Large | Full | 9 | 4,142 | 5,573 | 0.743414 | 0.743846 | 0.000397 | Passed |
| Non-identity | Small | Soak | 15 | 401,481 | 524,785 | 0.765248 | 0.765806 | 0.000363 | Passed |
| Non-identity | Medium | Soak | 15 | 41,069 | 55,293 | 0.743053 | 0.743763 | 0.000799 | Passed |
| Non-identity | Large | Soak | 15 | 4,144 | 5,574 | 0.743347 | 0.743580 | 0.000160 | Passed |

The selected iteration columns record each implementation's independent calibration result. Identity timing uses those distinct counts and normalized seconds per declared work after exact common semantic validation at the smaller count. Non-identity timing executes both workers at the shared common count recorded in each report, preserving equal total work even when the independent calibration selections differ.

The historical schema-version-31 outcome count is 12 passed, zero failed, zero noisy, and zero report-only timing measurements. All 12 reports passed on their first attempt, so no timing result was rerun toward a more favorable sample. Host verification passed every report with zero violations, load-one readings between `0.79` and `1.79`, maximum observed temperature `48,300` millidegrees Celsius, unchanged swap counters, and `local_modifications=false` before and after execution. Swap was disabled only during each formal timing producer and restored immediately afterward; `/swap.img` was finally verified active with 15 GiB available and zero bytes used.

## Earlier Historical Schema-Version-30 Timing Results

Every historical full report retains nine interleaved pairs and every historical soak report retains 15.
Each historical report contains one timing attempt and independently passes both the median and bootstrap-upper `1.25x` rules.

| Group | Scale | Tier | Pairs | Stim iterations | Stab iterations | Median ratio | 95% upper | Ratio rMAD | Outcome |
| --- | --- | --- | ---: | ---: | ---: | ---: | ---: | ---: | --- |
| Identity | Small | Full | 9 | 408,927 | 28,447,912 | 0.014361 | 0.014461 | 0.003097 | Passed |
| Identity | Medium | Full | 9 | 41,055 | 28,648,028 | 0.001438 | 0.001443 | 0.000920 | Passed |
| Identity | Large | Full | 9 | 4,146 | 28,637,875 | 0.000145 | 0.000146 | 0.001640 | Passed |
| Identity | Small | Soak | 15 | 408,616 | 28,511,511 | 0.014381 | 0.014398 | 0.002214 | Passed |
| Identity | Medium | Soak | 15 | 41,068 | 28,418,249 | 0.001441 | 0.001446 | 0.002101 | Passed |
| Identity | Large | Soak | 15 | 4,140 | 28,706,455 | 0.000145 | 0.000145 | 0.002105 | Passed |
| Non-identity | Small | Full | 9 | 524,063 | 524,063 | 0.766078 | 0.766206 | 0.000118 | Passed |
| Non-identity | Medium | Full | 9 | 55,254 | 55,254 | 0.743310 | 0.743551 | 0.000324 | Passed |
| Non-identity | Large | Full | 9 | 5,575 | 5,575 | 0.743284 | 0.743468 | 0.000222 | Passed |
| Non-identity | Small | Soak | 15 | 524,265 | 524,265 | 0.765940 | 0.766153 | 0.000196 | Passed |
| Non-identity | Medium | Soak | 15 | 55,219 | 55,219 | 0.743073 | 0.743560 | 0.000505 | Passed |
| Non-identity | Large | Soak | 15 | 5,579 | 5,579 | 0.742952 | 0.743043 | 0.000123 | Passed |

The historical outcome count is 12 passed, zero failed, zero noisy, and zero report-only timing measurements.
All 12 accepted reports passed on their first attempt, so no timing result was rerun toward a more favorable sample.
Every identity report uses source-owned `independent-throughput`; every non-identity report uses standard common iterations.

Two pre-migration producer invocations were operator-observed to be rejected by the host controller and published no retained artifact: one observed swap-counter movement, and one observed post-run one-minute load `13.660` above the exact `10.000` limit.
Because no canonical request, execution, report, or completion receipt exists for either attempt, they are execution-history notes rather than qualification evidence. They were not treated as timing failures or replaced by altered host policy.
After the host returned to policy, the complete accepted pre-migration chain was generated with one timing attempt per report.
Every historical post-migration producer passed host verification without a retry.

## Resource Evidence

The allocation-counted test `qualification::runtime::worker::clifford_string::tests::equal_width_callbacks_allocate_nothing_at_every_contract_scale` proves zero timed Stab allocation calls and zero requested bytes for both identity and non-identity callbacks at widths 10,000, 100,000, 1,000,000, and the accepted maximum 1,048,576.
Construction allocations remain outside timing, and no Stim allocation-count claim is made without matching instrumentation.

Across all 12 historical schema-version-31 reports, observed Stim setup RSS ranges from 3,575,808 to 7,036,928 bytes and Stim peak RSS ranges from 3,575,808 to 9,506,816 bytes.
Observed Stab setup RSS ranges from 5,623,808 to 11,370,496 bytes and Stab peak RSS ranges from 5,689,344 to 11,501,568 bytes.
These process observations are report-only and do not establish cross-scale or Stim-relative memory parity.
The retained `m6-clifford-string` entry in `benchmarks/m12-primary-memory-baseline.json` remains explicitly guarded until PQ6 defines and passes equal or stronger growth evidence.

The earlier historical schema-version-30 RSS ranges remain recorded in its source reports and must not be merged with the schema-version-31 observations into one scaling claim.

## Historical Schema-Version-31 Source Reports

Every source directory contains canonical `report.json`, `preflight.json`, and derived `report.md`. Each report passed immediate offline replay and source-owned regression, and both completion controllers independently repeated all six report replays and regressions for their group.

| Group | Scale | Tier | Directory suffix | Report SHA-256 | Preflight SHA-256 |
| --- | --- | --- | --- | --- | --- |
| Identity | Small | Full | `identity-full-small` | `f5245776c4b9f04dcdc95d75b4a8214e1edf136209bfb60eaf339fad6758b29a` | `a745779b7367bafa40f2c0ad34de430a99d6dc5b1dc7134163c5f71832941891` |
| Identity | Medium | Full | `identity-full-medium` | `72881dc11f144cbc72df053599c9d3341c28e2bb553a70370d7d84841c7b728d` | `bfc4b06645a72b297547f44aa38c36bdfa9872e6b2e377b2ba4f9da5c209982d` |
| Identity | Large | Full | `identity-full-large` | `5b7a3fe22418664b2a9d0d89d11f446fa0f535679c5913d850f21f29cb2f71e8` | `d993e3ac82b1da85c9938fd69ee53699cdafba583751d2df1d73a6964fbb69dc` |
| Identity | Small | Soak | `identity-soak-small` | `71d6032c5ecf0347bc6805753ad4f6bfe07b1e5a66c935233f304609fb11fa51` | `5395d430fd3fdd3e02a621c73eae3e7af45cebc29c16c3138b394f7a491326b1` |
| Identity | Medium | Soak | `identity-soak-medium` | `61f3d8663e4e1ef5cd53af02425ae2b99d5f325f0987ee235875e91c9f9d121a` | `27ee89f13b4b4aa8afd6fd54312128224e2ebb6ff1eba199013be06969f0f4de` |
| Identity | Large | Soak | `identity-soak-large` | `0442928c666e4c9d49809848f7fb63ed35e0c6b20665739d7196a3e296757bf2` | `d13fecf7a9dfc2bf829a719a18778f147e41ec4012a4ca613d5239e6fc66def6` |
| Non-identity | Small | Full | `nonidentity-full-small` | `d8ef4916ff5e7737e28f7e2961fb651a75e6fcc3470d91ef2bba5ebb80667264` | `4a32c2573966d755e9f12b06120e631768be9571f7016ea08d12ce4d69bac194` |
| Non-identity | Medium | Full | `nonidentity-full-medium` | `f110c4e0ff4cd28fe3d9609157f382fd391dc1e17af761ec6b332daf6a36f292` | `910b21a04523a5698787c0b89096b167c82fb84e773832c9b44285f51849b70a` |
| Non-identity | Large | Full | `nonidentity-full-large` | `fd370fb46789d7a8cbcbeeea9d01186a82fc93b1812d8c9d015e32eba5137bcc` | `7b85c039e3e9c63863fc2b10e2bae5819ce5a90769f00bef6c583c4d18e1f67d` |
| Non-identity | Small | Soak | `nonidentity-soak-small` | `bb9428626f6800a84c1b7138eb5bf548b784e71799fa04fd74cc91aba9291a07` | `19ad7aee728a24b43126b5cffaaa03a2682c6fb52c0cc65d50ef72c7b1c9da04` |
| Non-identity | Medium | Soak | `nonidentity-soak-medium` | `d7a1f9a6fe73793d8d17e03cda82d747bbbd7feca6930d420aa5751a7f35ce8b` | `535bd6820cf4ee599e55947306c5d1f589b4fcec084bf745bcde0ecfc35f521f` |
| Non-identity | Large | Soak | `nonidentity-soak-large` | `732f22554c9ed7495211af264df15e9c4fc6cf98988f8bcdfb9295d515923c17` | `6514d9393824f736e34c793ea21575d3107c0589ca2d1e457214fcce0ae7aa3d` |

Every directory above is prefixed by `target/benchmarks/qualification/pq2-clifford-schema31-da7c787d-`.

## Earlier Historical Schema-Version-30 Source Reports

Every historical source directory contains canonical `report.json`, `preflight.json`, and derived `report.md`.
The completion controllers independently replayed and regression-checked all 12 source reports.

| Group | Scale | Tier | Directory | Report SHA-256 |
| --- | --- | --- | --- | --- |
| Identity | Small | Full | `target/benchmarks/qualification/pq2-clifford-post-migration-91f62d0a-identity-full-small` | `dc80e848454ed90a8d562656c6fb7a0cb3cb4ab092141a2f1a37a6a25bee83ff` |
| Identity | Medium | Full | `target/benchmarks/qualification/pq2-clifford-post-migration-91f62d0a-identity-full-medium` | `d64e8a6933c280934aa1b9ef2a6eb31296293df268f18ded72bc1dc7c83478e0` |
| Identity | Large | Full | `target/benchmarks/qualification/pq2-clifford-post-migration-91f62d0a-identity-full-large` | `bbb9cd69cb7f3386819613d1650baa350c6172ae3b87ece313140fd812bea956` |
| Identity | Small | Soak | `target/benchmarks/qualification/pq2-clifford-post-migration-91f62d0a-identity-soak-small` | `6d0df21eb34e46d353227f9809183113301fd11262c333ae5dc138df5490f6a8` |
| Identity | Medium | Soak | `target/benchmarks/qualification/pq2-clifford-post-migration-91f62d0a-identity-soak-medium` | `137f9a3875ecf1c345d25904bc507d7c07c980e931bbb79d14cb71d5a02cf1ad` |
| Identity | Large | Soak | `target/benchmarks/qualification/pq2-clifford-post-migration-91f62d0a-identity-soak-large` | `207ba4e5b08c7a8662d91f551687709bb927066d718b6cd06f87d33665f92aa8` |
| Non-identity | Small | Full | `target/benchmarks/qualification/pq2-clifford-post-migration-91f62d0a-nonidentity-full-small` | `8b5c38060da5c4165550b8090962d2169ba8d721357b6c9099479f7c3b5650c0` |
| Non-identity | Medium | Full | `target/benchmarks/qualification/pq2-clifford-post-migration-91f62d0a-nonidentity-full-medium` | `97adee26f7c64c838f9c3b2b2cf237f76ff70348a6564d455160296ee77dd591` |
| Non-identity | Large | Full | `target/benchmarks/qualification/pq2-clifford-post-migration-91f62d0a-nonidentity-full-large` | `6b7cb40be04611063b9f8f6986f4b19fc4f8409ec12feabd6c6f189014b586cf` |
| Non-identity | Small | Soak | `target/benchmarks/qualification/pq2-clifford-post-migration-91f62d0a-nonidentity-soak-small` | `6ed235ad8932998b138c94397c13fc2675974415a1bf011ac611524adeed9f6c` |
| Non-identity | Medium | Soak | `target/benchmarks/qualification/pq2-clifford-post-migration-91f62d0a-nonidentity-soak-medium` | `eeaf07ee6d976b1f07239d1c869248c6ed8ba21a3b54769040238affbed2dd07` |
| Non-identity | Large | Soak | `target/benchmarks/qualification/pq2-clifford-post-migration-91f62d0a-nonidentity-soak-large` | `76690e279ac20ab96fd35dfaf2bcef8875d550bae996859dfb4d9b3a1cb15dec` |

## Historical Schema-Version-31 Rollups And Completion Receipts

| Group | Artifact | Directory suffix | Report SHA-256 | Preflight SHA-256 |
| --- | --- | --- | --- | --- |
| Identity | Full rollup | `identity-full-rollup` | `b18d065f3cb8cca18210218974eb1e61e582bf7130da3f8b8445d644a5d8ef01` | `cce42294f75076ea368740a4c36e3496d5dcb861f6cc68426a542c8d5f547fbe` |
| Identity | Soak rollup | `identity-soak-rollup` | `49be6474c1d0ed1c8a9dc60b75f0e2211a17900d52c65e8f160070ebd700662a` | `f99b60b3ae5c69dc6318655aa8e50d9e06868404d1c847800b6a22d15795e3fb` |
| Identity | Completion | `identity-completion` | `f9eca9d1d7a41badf6ccc61cd82acf9aaac5e57b01390c923e7c018295d00a26` | `b30ee72a3dc2fa1109a87b3e0d6a863806d89e735cda8a1d2be6338455ca5115` |
| Non-identity | Full rollup | `nonidentity-full-rollup` | `9288964c47c2f926944c7acbb59d635a374a6b10bd7ef82dee86b30ebc4af045` | `54d7d22e2ef1e9962baaffe126532ccbc067e01c05c35f5b17d2c850191b7366` |
| Non-identity | Soak rollup | `nonidentity-soak-rollup` | `18db424b0a1b04eff3cdf36c707d1e3bbff92318b2092dc72ac8cafc9655486b` | `030f6f0980d389c5776a7a6d4d4e8dc3701c040a4bac11ea6488e0905ac97382` |
| Non-identity | Completion | `nonidentity-completion` | `3a1c80c97a53634b30e270086512ae6b92fff658ae415ba8dda8bf5ef997c301` | `20407e3f4f1a6f83789dea1793fa4c59adc4d66933c183014802a31d2d64b08c` |

Every directory above is prefixed by `target/benchmarks/qualification/pq2-clifford-schema31-da7c787d-`. All four rollups passed publication and byte-for-byte replay with three passed, zero failed, and zero noisy scales. Each completion receipt binds six source reports, two rollups, 16 machine-checked closure steps, exact focused correctness artifacts, one worker identity set, one clean revision, one CPU identity, one adapter probe, every report replay and regression, and both rollup replays. Both completion receipts passed independent byte-for-byte reconstruction.

## Earlier Historical Schema-Version-30 Rollups And Completion Receipts

| Group | Artifact | Directory | Report SHA-256 | Preflight SHA-256 |
| --- | --- | --- | --- | --- |
| Identity | Full rollup | `target/benchmarks/qualification/pq2-clifford-post-migration-91f62d0a-identity-full-rollup` | `57f852d518e6762c3f80533d7572b679ac367eb2b0fed5acd8dab1dda475081c` | `9e3ebc0271179ff7cb1cfabbeb779a8041d48d2f0c366735a3aeff5c093ecc0e` |
| Identity | Soak rollup | `target/benchmarks/qualification/pq2-clifford-post-migration-91f62d0a-identity-soak-rollup` | `b34b0bd3148e13168be5177aac56961d74ae3d0205ab443cd510d138ee216115` | `07f50000bd7135e8383901039807faa90747b515f80374dbbd04342b57a50436` |
| Identity | Completion | `target/benchmarks/qualification/pq2-clifford-post-migration-91f62d0a-identity-completion` | `ed44626965b4a3e5b650e586e01159e69fa615e49a598d9b34a8ea0b0267b505` | `816f1e0f8a9cd123647bb70c950c971c279e10c21f16485ea2e466adb2d5ec56` |
| Non-identity | Full rollup | `target/benchmarks/qualification/pq2-clifford-post-migration-91f62d0a-nonidentity-full-rollup` | `6335ab23f1a71d6680887fc3c16fbdd37416956bbd64e5ba907e0292f76873a5` | `89c964c9771bb43d5f668a4251db62358b5adcf0fa5f834ee7fdab5a5c73afe5` |
| Non-identity | Soak rollup | `target/benchmarks/qualification/pq2-clifford-post-migration-91f62d0a-nonidentity-soak-rollup` | `00ea76a833f6c1d163e1fe448de5b2babbc0dd074fbcd776e0884e6c8201c6b9` | `a79e394185a4d757a655054f2326e53f8add1b395a313fa597f6191c7268d3a2` |
| Non-identity | Completion | `target/benchmarks/qualification/pq2-clifford-post-migration-91f62d0a-nonidentity-completion` | `a0d3a57b64f663c5257e4be2526f1a72093a3d4247afd0a523168451d9c64c26` | `635875898bf9f16817a12540fda7cf309095b045bab1f708a5ec856aab108447` |

Every historical rollup passed publication and byte-for-byte replay.
Each completion receipt binds six source reports, two rollups, all closure steps, exact correctness artifacts, one worker identity set, one clean revision, one CPU identity, adapter preflight, every report replay and regression, and both rollup replays.
Both historical completion receipts passed independent byte-for-byte replay with zero failed steps.

## Legacy M12 Migration

Clean pre-migration revision `127d6661a9e00872fc4aa4c0b0d27171e005afa5` completed and replayed both Clifford chains under performance inventory `0ee3639389860799298164c94c647fcab45b03c9d67b941b1aad12c6e5e06df5`.
Its 12 accepted first-attempt median ratios range from `0.000146x` to `0.764673x`, with worst upper bound `0.764792x`.

Identity completion report SHA-256 `78fc10ca29e432641f3d978ed871c4b96d1ba344d714c20bf726f574239d2126` authorized retirement of only the inherited `m6-clifford-string` timing threshold and its exact identity/small replacement mapping.
Non-identity completion report SHA-256 `f5842ddcf86f024a78293b203196e9490396ffb0762196a6f2cc169b1f8489c6` independently closed the companion contract but did not authorize a legacy mapping because the inherited row was identity-only.

Migration commit `91f62d0a78659da2e8e264a6968b3c6cd32456de` made that focused change, marked the inherited timing row superseded, and preserved the M12 memory baseline.
`benchmarks/qualification-threshold-migrations.json` now machine-binds the exact legacy pair, replacement group, measurement, and scale, authorization revision and inventory, authorization completion report and preflight hashes, migration revision and inventory, and retained memory baseline. Inventory validation rejects a refingerprinted authorization, missing replacement evidence, stale scale, or reopened legacy timing row. The complete schema-version-30 post-migration chain recorded here was regenerated and replayed from the clean migration revision under its historical inventory.

## Milestone Audit

The audit that followed revision `da7c787d1e9f49110d7054868b146b5fb7d7bda4` closed the preceding schema-version-30 findings, but a subsequent independent review found additional source, lifecycle, regression, and documentation defects. The current source fixes all confirmed findings and keeps the `da7c787` evidence intact as historical evidence under its exact inventories. Follow-up milestone audit remains required after targeted verification and before the replacement evidence commit.

| Requirement | Status | Evidence |
| --- | --- | --- |
| Independent identity and non-identity contracts | Implemented; replacement evidence pending | Distinct checked runtime groups, measurements, thresholds, historical 12-report chains, four rollups, and two completion receipts |
| Exact correctness and independent oracle | Implemented; replacement evidence pending | Complete 24-by-23 cycle, independent scalar worker oracle, all-24-by-24 Tableau-backed group owner, and prefix-delta metadata repair for short right operands |
| Public lifecycle and optimizer resistance | Implemented; replacement evidence pending | Symmetric public in-place calls, typed prepared-workload lifecycle, receipt-owned barriers, callback counts, result-derived witnesses, final-state digests, and source-shape tests |
| Hostile and resource boundaries | Implemented; replacement evidence pending | Frozen 72-receipt Clifford matrix, accepted maximum, first rejection, opposite valid markers, malformed descriptor hex, width/work mismatch, work overflow, pre-allocation rejection, and pre-barrier rejection |
| Performance and resource claims | Historical timing accepted; current timing pending | Twelve first-attempt `da7c787` reports under historical inventories, independent `1.25x` rules, replayed regressions, zero timed Stab allocations, and explicitly report-only RSS observations |
| Migration and artifact lifecycle | Implemented; follow-up audit pending | Checked pre-migration authorization, focused timing-only retirement, preserved memory baseline, all-path pre-mutation admission, exact descriptor-and-digest file-set bindings, repository-state binding through publication, descriptor-checked rollback and cleanup, and production-dispatch regressions |
| Documentation and inventory ownership | Implemented; follow-up audit pending | Current and historical digests are separated, generated public-API counts are checked against the checklist marker, swap restoration is recorded, and commands remain `just` plus Rust ops |

Milestone status is **Active** for source-current controlled Linux AArch64 closure. The previously resolved independent-throughput, selected-calibration-floor, and Clifford-vector amendments in `docs/plans/milestone-spec-gaps.md` specify the current behavior. The follow-up review did not reveal a new product-scope under-specification; it revealed concrete implementation and evidence-lifecycle defects that are fixed in source and must now pass audit, review, and clean replacement evidence. Native Linux x86-64 execution and PQ6 cross-scale memory rules remain explicit later-plan work rather than loopholes in this architecture-scoped acceptance.

## Independent Review

Independent GPT-5.6/max review lanes inspected qualification lifecycle and publication, Clifford implementation and complexity, scientific evidence, and documentation. They preserved the scientific interpretation of the equal-width `da7c787` timing results but found the following defects that prevent source-current promotion.

Confirmed findings and resolutions:

- Formal `qualification-run`, rollup, and completion producers could replace an existing output directory and therefore erase a failed, noisy, host-rejected, or malformed artifact. Producers now use append-only publication and fail with `OutputAlreadyExists`; replay commands retain compare-and-swap refresh for the exact existing artifact, with a regression proving producer refusal.
- The legacy timing retirement was justified only by prose. `benchmarks/qualification-threshold-migrations.json` and its validator now bind the exact legacy pair, replacement target, authorization and migration revisions and inventories, completion report and preflight hashes, and retained memory row; adversarial tests reject refingerprinting and reopening.
- `worker.rs` mixed preparation, barrier, execution, and output lifecycle near the project size limit. `worker/prepared.rs` now owns the typed prepared-workload lifecycle, and both files remain below 1,200 lines while their ordered source identities are receipt-bound.
- The hostile Clifford matrix omitted opposite valid markers, declared width/work mismatch, and malformed descriptor hex. The checked vector expands from 31 to 36 requests per worker and from 62 to 72 Clifford receipts, with exact implementation-specific rejection classes and unconsumed barriers.
- Progress and adapter documentation retained stale schema, contract, inventory, fixture, evidence, and contract-count claims, and the profiler note still described completed optimization as future work. The synchronized documents now distinguish historical evidence from source-current closure, describe the checked migration and append-only producer contracts, and use current counts and digests.
- Two host rejections had no retained artifacts. They are now labeled operator-observed history rather than promotable or machine-replayable evidence.
- Public `CliffordString::right_multiply_in_place` updated packed data only over a short right operand but then rescanned the untouched left tail to reconstruct non-identity metadata. The bit kernel now returns old and new prefix counts, and the public method repairs metadata by checked subtraction and addition without work proportional to the left tail. A 65,537-qubit correctness case protects partial-word metadata, while the existing source-owned `m6-clifford-string` benchmark row now reports a fixed one-qubit non-identity RHS over 10,000-, 100,000-, and 1,000,000-qubit left operands to expose any return to left-width scaling without a timing assertion in unit tests. These asymmetric measurements remain report-only and do not extend the equal-width Stim qualification claim.
- A nondirect `qualification-run --out` could create `.publication.lock` inside an existing formal artifact before direct-path rejection. Direct-child validation now precedes every absence check, lock, and artifact read, with a regression proving that rejected output admission leaves the filesystem unchanged.
- Directory-inode checks did not detect in-place mutation of a bound `report.json`, `preflight.json`, or `report.md`. Publication now retains each file descriptor, device and inode identity, length, and SHA-256; it revalidates before and after exchange and after parent durability, and restores the displaced target if a post-exchange check fails. Regressions cover target and sibling-source mutation before and during exchange.
- Performance run and report publication did not retain every consumed correctness artifact through publication, rollup omitted source Markdown, and completion replay omitted target Markdown. The current boundary retains all CQ request, report, completion, preflight, Markdown, and execution-receipt files; all performance report, preflight, and Markdown files; and every completion source and replay target until final publication validation.
- Completion loaded earlier source reports before admitting later report and rollup paths, and rollup created its private Git view before path admission. Completion and rollup now parse every direct path and reject collisions before any artifact read, lock creation, or private Git work.
- Bound source directories rechecked only named files, failed staged-child cleanup unlinked by name, and displaced replay trees were not retained through cleanup. Publication now requires each live child-name set to equal its bound set, retains every staged and displaced child descriptor, refuses to unlink a substituted entry, propagates cleanup failure, fsyncs the hierarchy after successful cleanup, and revalidates all retained sources before reporting success.
- Clean and commit repository state was checked immediately before publication but not tied to the descriptor-owned publication root through source execution, exchange, and hierarchy durability. Performance run, report replay, rollup, and completion now retain one nofollow-opened repository descriptor before their first artifact or source access, resolve Git, build, worker, probe, regression, and nested replay work through its Linux descriptor-root view, require the admitted path to keep naming that descriptor, repeat both bindings through final cleanup and synchronization, and roll back a derived artifact when drift occurs before displaced-tree removal.
- The interrupted descriptor-root implementation represented the retained repository only as `/proc/<controller-pid>/fd/<n>`, while repository and Git metadata readers reopened every absolute path component with `O_NOFOLLOW`. The procfs file-descriptor component is a magic symlink, so the first descriptor-root Git audit failed with `ELOOP` before Git started and all later repository-relative source reads were unreachable. `RepoRoot` now owns the retained descriptor behind its process path, repository readers start from a duplicate of that handle, Git metadata directories and linked-worktree `gitdir` and `commondir` references are opened and retained descriptor-relatively, and private Git `objects` and `refs` links target retained directory handles. Focused regressions prove source reads and completion Git audits keep using the retained repository after path swap and that linked worktrees remain supported.
- Correctness preflight opened its artifact tree through the retained descriptor but stored only the synthetic `/proc/<controller-pid>/fd/<n>/target/qualification/...` path for final publication validation. The final check fed that path back through the generic absolute `O_NOFOLLOW` walker, which rejected the procfs magic-link component as an apparent artifact mutation after timing completed. Correctness bindings now duplicate the retained repository descriptor, retain every repository-relative ancestor through the exact CQ output, and revalidate the chain with `openat` before checking bound files and case receipts. Regressions reproduce a retained descriptor root directly and reject replacement of `target`, `target/qualification`, the output directory, `cases`, or a case directory.
- The completion tests exercised helper publication but not the real producer dispatch branch. A production dispatcher now has a regression proving that producer mode preserves an existing output and publishes a previously absent output.
- The checklist advertised 1,972 public APIs and 654 Algebra APIs while generated ownership contained 1,974 and 656. A checked metadata marker now binds both counts to discovery, and malformed, missing, duplicate, unknown-field, and mismatched markers fail closed.

Acceptance remains pending until targeted verification, follow-up milestone audit, GPT-5.6/max review, one clean source commit, and fresh same-revision schema-version-31 evidence confirm that these resolutions introduce no remaining P0 through P3 finding.

## Verification Record

Historical clean revision `da7c787d1e9f49110d7054868b146b5fb7d7bda4` passed the exact three-case correctness run, report replay, and preflight; worker reproducibility; both adapter probes; all full and soak report producers; all report replays and regressions; all four rollup producers and replays; both completion producers and replays; and the allocation-counted Clifford contract under its recorded inventories.

The current review-fix source state passes all 374 non-ignored `stab-bench` tests with two explicitly ignored subprocess or reproducibility tests, including repository-reader path-swap, linked-worktree Git, unsafe Git-marker, bounded Git-directory-scan, artifact-publication, completion, replay-CAS, early-admission root-swap, and swap-and-restore descriptor-root regressions; all 386 non-ignored allocation-enabled `stab-bench` tests with two ignored; strict ordinary, allocation-enabled, and complete-workspace Clippy; complete workspace tests; correctness inventory validation at `648e7ea5a66997a810498dc871257bd654c7f9af9304651d43a88103eded0289`; performance inventory validation at `a47866ba5eab70392dd2754391d3d7d8588567a7cbfc1f81a569be813804ce51`; frozen Clifford-vector validation; and a dirty diagnostic identity adapter probe that exercised retained source reads, the pinned Stim submodule Git view, private source materialization, native adapter build, and both workers. Clean same-revision probes, follow-up audit and review, and all replacement evidence commands remain pending until this source state is committed.

The first source-current correctness invocation at revision `cf44e57b0d2cd6fdb78cd62c4c8c5dfffcf1f451` passed the three source contract labels directly to `--case` and was rejected before execution because the runner accepts generated `cq-evidence-qualification-*` IDs. It created no output directory and therefore has no request, execution, report, completion, or preflight artifact; it is operator-observed command history, not qualification evidence. `GOAL.md` now requires the checked source-label-to-case-ID mapping so later agents do not repeat the mismatch.

Clean revision `476be59a68d4cd9706a2354f6fbf1565ef3a901c` then generated and replayed the exact three-case focused correctness report at `target/qualification/pq2-clifford-cq-full-476be59`, with request `dcbe5be767d94a30796d1ddacfb8aa4a64e61e4a85ba3659c01169d1ae8a47b4`, report `4cb1b63d3c21497a1051675489843534aae55de1a26cca387939fa387993a9a8`, completion `d8b37b85bd8084c82816f026c79fe63bc9bb1b6abf860f14209b0543d84ab57e`, preflight `346e2b8fe00b5dc5999a60841ecf4d66767fde8d5d3cce6feaeb88f9f5f36373`, and Markdown `1a661d1d6bc00180447acce3032bf788c97694e060c8aabe026ab6e6ffc3c7f1`. Worker reproducibility and both diagnostic Clifford adapter probes also passed at that revision. These artifacts are now historical diagnostics because the subsequent correctness-binding source fix changes the producer revision.

The first formal timing invocation at revision `476be59a68d4cd9706a2354f6fbf1565ef3a901c` targeted the identity full-tier small scale at `target/benchmarks/qualification/pq2-clifford-schema31-476be59-identity-full-small`. Timing execution completed, but final publication rejected the unchanged correctness source as `correctness evidence artifact changed before performance publication` when it reopened the retained procfs descriptor path through the generic absolute nofollow walker. The producer created no output directory and therefore retained no canonical request, execution, report, preflight, or timing samples; this is operator-observed command history, not qualification evidence or a timing outcome. The trap-protected timing window restored `/swap.img` immediately, and `swapon --show --bytes` reported the 16 GiB swap file active with zero bytes used.

The dirty-tree report-only compare at `target/benchmarks/clifford-short-rhs-review-compare-20260720` measured the fixed one-qubit non-identity RHS at `31 ns` per public call for each 10,000-, 100,000-, and 1,000,000-qubit left width. The flat diagnostic is regression guidance only: it was not produced by the qualification controller, does not satisfy a Stim comparator, and cannot be promoted as source-current timing evidence.

The final closure revision must also pass:

```sh
cargo fmt --all --check
cargo clippy --workspace --all-targets -- -D warnings
cargo test --workspace --quiet
cargo clippy -p stab-bench --all-targets --features count-allocations -- -D warnings
cargo test -p stab-bench --features count-allocations --quiet
just qualification::correctness-regenerate --check
just qualification::correctness-check
just bench::qualification-regenerate --check
just bench::qualification-check
just maintenance::pre-commit
```

## Remaining Work

1. Commit the review-fix source state, generate and replay a fresh same-revision focused CQ report, reproduce both workers, and pass both current adapter probes.
2. Run all twelve unique full and soak reports, report replays and regressions, four rollups and replays, and two completion receipts and replays on the controlled Linux AArch64 host under schema version 31, with swap restored immediately after the timing window.
3. Run follow-up milestone audit and independent GPT-5.6/max full review, fix every confirmed finding, and regenerate any evidence invalidated by a source or contract change.
4. Run both clean full and soak families, rollups, and completion receipts on a controlled native Linux x86-64 host before making an x86-64 conclusion.
5. Define and validate explicit cross-scale RSS and allocation-growth rules in PQ6 before making a memory qualification claim or retiring the legacy memory baseline.
6. Qualify allocating multiplication, unequal-width growth, construction, randomization, concatenation, repetition, display, Tableau operations, and the remaining Algebra surfaces only through their own exact public API groups.
