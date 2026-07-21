# PQ2 Clifford-String Qualification Progress Report

## Status

The eleventh PQ2 executable slice has passing historical schema-version-30 evidence for equal-width public in-place Clifford-string multiplication on controlled Linux AArch64, but source-current closure is pending as of 2026-07-19.

`PERFQ-M6-CLIFFORD-STRING` qualifies only the exact pinned identity-right workload, while `PERFQ-M6-CLIFFORD-STRING-NON-IDENTITY` qualifies a deterministic complete 24-by-23 cycle of every left Clifford against every non-identity right Clifford.
The two contracts have separate measurements, scale reports, thresholds, rollups, completion receipts, and performance conclusions.

All 12 historical post-migration timing reports passed on their first attempt without a noise rerun, waiver, report-only outcome, or threshold relaxation.
Identity median Stab-to-Stim seconds-per-work ratios range from `0.000145x` to `0.014381x`, corresponding to approximately `69.53x` through `6907.11x` speedups for the exact identity workload.
Non-identity median elapsed-time ratios range from `0.742952x` to `0.766078x`, corresponding to approximately `1.31x` through `1.35x` speedups for complete non-identity multiplication.
The worst historical bootstrap confidence-interval upper bound is `0.766206x`, below the exact `1.25x` gate.

Independent review after that chain found four closure defects: formal producers could replace an existing result directory, threshold-retirement authorization existed only in prose, the worker lifecycle was concentrated in a near-limit source file, and the Clifford hostile corpus omitted opposite valid markers, width-to-work mismatch, and malformed descriptor hex. The implementation now rejects producer output replacement, validates a checked threshold-migration ledger, separates prepared workload lifecycle into its own module, expands Clifford coverage to 72 receipts, and bumps the affected source and report contracts. These fixes invalidate source-current promotion of the earlier chain even though its scientific result remains historical evidence.

The identity result is not used as a proxy for non-identity multiplication.
Its source-owned independent-throughput policy is valid because both implementations perform the same public logical operation and declare the same per-iteration single-qubit work, while Stab's semantically equivalent identity-right metadata fast path is O(1).
Every identity report separately proves exact output at the smaller selected iteration count and normalizes timing by each implementation's exact report-bound work.
The non-identity family retains ordinary common-iteration timing with equal total work.

When the replacement chain passes, this report will close only equal-width public in-place Clifford-string multiplication on Linux AArch64.
It does not qualify allocating multiplication, unequal-width growth, construction, randomization, concatenation, repetition, display, Tableau operations, native Linux x86-64, cross-scale memory growth, or the remaining Algebra surface.

## Current Contract And Historical Evidence

- Historical clean post-migration Stab revision: `91f62d0a78659da2e8e264a6968b3c6cd32456de`, with `local_modifications=false` before and after every historical producer and completion controller.
- Focused migration commit: `91f62d0a78659da2e8e264a6968b3c6cd32456de`.
- Clean pre-migration authorization revision: `127d6661a9e00872fc4aa4c0b0d27171e005afa5`.
- Stim baseline: v1.16.0 at `e2fc1eca7fd21684d433aa5f10f4504ea4860d07`.
- Current performance inventory: `4c2407613ee63903c57acb4b177e91799047ed60f7bff780929fe84532f57643`; historical schema-version-30 evidence inventory: `a76090c996ad404c1cb8bfa85066e286c6f40b32754b3750e984375f7ca90025`.
- Correctness inventory: `4dbbb4b2cda3117bdd3d3ddfcd30b55f09e6f401352e3e86130222189d47791f`.
- Historical runtime-group contract: `4d7b0e4808828217dc0a353ea991321c8483579ed62b84ca42a1cae6f1b4c2ee`; the replacement report will record the current contract digest.
- Current profiler note: `benchmarks/profiler-notes/qualification/perfq-m6-clifford-string.md` at SHA-256 `147fa44d09ee656f20f05c92407195880f7a83f417f590018625259fe311e43a`.
- Current frozen vector fixture: `benchmarks/fixtures/pq2-clifford-string-vectors.json` at SHA-256 `e61cd02dd29eb006892444eddd30693031e39746add588a8f538888499a29d85`.
- Checked migration ledger: `benchmarks/qualification-threshold-migrations.json` at SHA-256 `e27cd284ad76c91b213fe5e5fff8c8f5058810c33874965dfe53f49883cec810`.
- Pinned Stim comparator source: `benchmarks/stim_adapter/clifford_string_contract.h` at SHA-256 `95d628eabf8db5795fd3391c97f4f6a0ab118e62e7cce91652458af40f7f6bf8`.
- Host profile: verified `linux-aarch64-controlled`, pinned to logical CPU 0 with CPU identity `CPU implementer=0x41, CPU architecture=8, CPU variant=0x0, CPU part=0xd87, CPU revision=1`.
- Rust toolchain: `nightly-2026-06-20`, release profile, target `aarch64-unknown-linux-gnu`.

The source-current performance inventory contains seventeen executable product contracts with one exact `1.25x` rule at each of three scales.
The inherited `m6-clifford-string` timing row is superseded, while its process-memory baseline remains guarded until PQ6 supplies equal or stronger cross-scale evidence.
Current source-owned closure uses private Stab build-receipt schema version 5, adapter receipt schema version 11, contract-preflight schema version 12 with 212 probes, and qualification report schema version 31. Fresh clean worker identities, focused correctness artifacts, report hashes, rollups, and completion receipts remain pending until the review-fix source state is committed.

## Inventory Status

`just bench::qualification-check` validates 549 qualification groups with 547 `measured`, zero `covered-by-parent`, two `not-performance-relevant`, and zero `no-faithful-comparator` dispositions.
The 161 inherited benchmark rows contain nine retained, 135 reworked, four diagnostic, eleven superseded, and two removed rows.
This slice changes only the inherited `m6-clifford-string` row from reworked to superseded after its exact identity replacement completion authorized migration.

The historical chain contributes twelve raw source measurements: six identity and six non-identity reports across full and soak tiers and three scales.
Full reports contain nine retained pairs apiece and soak reports contain 15, for 144 retained timing pairs in total.
Its exact timing outcome is twelve passed, zero failed, zero noisy, and zero report-only.
There is no slow or noisy row requiring a next action; the source-owned profiler note remains bound because it records the scalar failure, packed portable-SIMD optimization, identity timing policy, and migration provenance.
Those counts are not source-current schema-version-31 outcomes. Memory evidence remains report-only, and no scaling or Stim-relative memory claim is made before PQ6.

## Historical Correctness Preflight

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
The replacement chain must publish and replay a fresh focused report from the same clean revision as its schema-version-31 workers.
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

## Historical Worker Identity

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
The source-current contract requires private Stab build-receipt schema version 5, unchanged adapter receipt schema version 11, contract-preflight schema version 12, and qualification report schema version 31. The replacement reproducibility and probe identities will replace this section only after they are generated from the clean review-fix revision. Adapter smoke timings remain diagnostic and are never product speed evidence.

## Historical Timing Results

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

Across all 12 historical post-migration reports, observed Stim setup RSS ranges from 3,575,808 to 7,036,928 bytes and Stim peak RSS ranges from 3,575,808 to 9,449,472 bytes.
Observed Stab setup RSS ranges from 5,496,832 to 11,374,592 bytes and Stab peak RSS ranges from 5,562,368 to 11,505,664 bytes.
These process observations are report-only and do not establish cross-scale or Stim-relative memory parity.
The retained `m6-clifford-string` entry in `benchmarks/m12-primary-memory-baseline.json` remains explicitly guarded until PQ6 defines and passes equal or stronger growth evidence.

## Historical Source Reports

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

## Historical Rollups And Completion Receipts

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

The pre-review milestone audit found no remaining finding under the schema-version-30 contract. The later independent review exposed closure defects that the audit did not catch, so that audit is historical and cannot close the source-current contract.

| Requirement | Status | Evidence |
| --- | --- | --- |
| Independent identity and non-identity contracts | Implemented; fresh evidence pending | Distinct checked runtime groups, measurements, thresholds, scale reports, rollups, and completion requirements |
| Exact correctness and independent oracle | Implemented; fresh focused CQ pending | Three exact CQ parents, complete 24-by-23 cycle, independent scalar worker oracle, and all-24-by-24 Tableau-backed group owner |
| Public lifecycle and optimizer resistance | Implemented | Symmetric public in-place calls, prepared-workload lifecycle, receipt-owned barriers, callback counts, result-derived witnesses, final-state digests, and source-shape tests |
| Hostile and resource boundaries | Implemented; fresh worker receipts pending | Frozen 72-receipt Clifford matrix, accepted maximum, first rejection, opposite valid markers, malformed descriptor hex, width/work mismatch, work overflow, pre-allocation rejection, and pre-barrier rejection |
| Performance and resource claims | Historical only | Twelve historical first-attempt reports, independent `1.25x` rules, replayed regressions, zero timed Stab allocations, and report-only RSS boundaries; schema-version-31 replacement pending |
| Migration and closure | Authorization checked; closure pending | Machine-bound pre-migration authorization, focused timing-only retirement, preserved memory baseline, and append-only formal publication; replacement rollups and completions pending |

The follow-up audit must rerun after the clean replacement chain exists. The previously resolved independent-throughput, selected-calibration-floor, and Clifford-vector amendments in `docs/plans/milestone-spec-gaps.md` specify the current behavior; the review findings were implementation and evidence defects, not new under-specification.

## Independent Review

Four independent GPT-5.6/max review lanes inspected qualification lifecycle and publication, migration and evidence provenance, Clifford implementation and hostile inputs, and documentation. The core and portable-SIMD lane found no product correctness or performance-kernel defect.

Confirmed findings and resolutions:

- Formal `qualification-run`, rollup, and completion producers could replace an existing output directory and therefore erase a failed, noisy, host-rejected, or malformed artifact. Producers now use append-only publication and fail with `OutputAlreadyExists`; replay commands retain compare-and-swap refresh for the exact existing artifact, with a regression proving producer refusal.
- The legacy timing retirement was justified only by prose. `benchmarks/qualification-threshold-migrations.json` and its validator now bind the exact legacy pair, replacement target, authorization and migration revisions and inventories, completion report and preflight hashes, and retained memory row; adversarial tests reject refingerprinting and reopening.
- `worker.rs` mixed preparation, barrier, execution, and output lifecycle near the project size limit. `worker/prepared.rs` now owns the typed prepared-workload lifecycle, and both files remain below 1,200 lines while their ordered source identities are receipt-bound.
- The hostile Clifford matrix omitted opposite valid markers, declared width/work mismatch, and malformed descriptor hex. The checked vector expands from 31 to 36 requests per worker and from 62 to 72 Clifford receipts, with exact implementation-specific rejection classes and unconsumed barriers.
- Progress and adapter documentation retained stale schema, contract, inventory, fixture, evidence, and contract-count claims, and the profiler note still described completed optimization as future work. The synchronized documents now distinguish historical evidence from source-current closure, describe the checked migration and append-only producer contracts, and use current counts and digests.
- Two host rejections had no retained artifacts. They are now labeled operator-observed history rather than promotable or machine-replayable evidence.

Acceptance remains pending until fresh schema-version-31 evidence exists and follow-up GPT-5.6/max review confirms that these resolutions introduce no remaining P0 through P3 finding.

## Verification Record

The historical clean evidence revision passed the exact three-case correctness run, report replay, and preflight; worker reproducibility; both adapter probes; all full and soak report producers; all report replays and regressions; all four rollup producers and replays; both completion producers and replays; and the allocation-counted Clifford contract.

The review-fix source state currently passes the targeted `stab-bench` test suite, Clifford vector and invocation tests, artifact publication tests, migration-ledger tests, Clippy for `stab-bench`, qualification inventory validation and regeneration, and frozen vector validation. Full repository verification and all clean replacement evidence commands remain pending until this source state is committed.

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
