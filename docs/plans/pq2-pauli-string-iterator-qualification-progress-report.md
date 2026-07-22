# PQ2 Pauli-String Iterator Qualification Progress Report

> Historical-inventory note, 2026-07-23: this report remains accepted Linux AArch64 evidence under frozen performance inventory `48eacf03a2ecdca917c05aade52b7e17c9ead1be8b75b203e1d43c2f3b3b7dbf`. Later reviewed Clifford contracts produced subsequent historical digests; the current shared-harness digest after the third DEM parser source refresh is `a98f57cf194f3a021d321266656cf688c9f7780fb39fa337475e8132411eb88a` without relabeling the iterator evidence. The measurements below remain historical.

## Status

The tenth PQ2 executable slice passes its independent `1.25x` timing gates for public borrowed Pauli-string iterator construction and complete traversal on the controlled Linux AArch64 host as of 2026-07-18.

`PERFQ-M6-PAULI-ITER` qualifies X/Z enumeration over weights 2 through 5, while `PERFQ-M6-PAULI-ITER-SINGLETON` separately qualifies X/Y/Z enumeration at weight 1.
Both measurements are named `construct-and-iterate-borrowed`, but their timing, confidence intervals, allocation contracts, rollups, and completion receipts remain independent.

All 12 measurements under the frozen evidence inventory passed on their first attempt without a noise rerun, waiver, report-only outcome, profiler note, or threshold relaxation.
Median Stab-to-Stim elapsed-time ratios range from `0.025664x` to `0.568566x`, corresponding to median speedups of approximately `1.76x` through `38.97x`.
The worst bootstrap confidence-interval upper bound is `0.570628x`, below the exact `1.25x` gate.

This accepted report closes only the two pinned borrowed-iterator workload shapes on Linux AArch64.
It does not qualify restart timing, owned `Iterator::next`, clone, formatting, comparison, arbitrary axis or weight distributions, `CommutingPauliStringIterator`, Tableau iteration, native Linux x86-64, cross-scale memory growth, or the remaining Algebra surface. The separate 271-parent CQ2 checkpoint remains accepted historical evidence at clean hardened-controller revision `3f2f382627c8421de0a668819d467a9f252de20f` and is recorded in `docs/plans/cq2-deterministic-qualification-progress-report.md`.

## Frozen Evidence

- Clean Stab evidence revision: `afaf0bf7f236b9f6ae6f72c19bbbdfea94d26632`, with `local_modifications=false` before and after every final producer and completion controller.
- Stim baseline: v1.16.0 at `e2fc1eca7fd21684d433aa5f10f4504ea4860d07`.
- Performance inventory: `48eacf03a2ecdca917c05aade52b7e17c9ead1be8b75b203e1d43c2f3b3b7dbf`.
- Correctness inventory: `4c940e983df10a7c95cc512939f4a0cce79f1865e141739af9378db581ea5f87`.
- Runtime-group contract: `5d464833b2c16171912d6a22be2d1429d9cd54fd497f33861fd1fd60fc5b9c98`.
- Host profile: verified `linux-aarch64-controlled`, pinned to logical CPU 0 with CPU identity `CPU implementer=0x41, CPU architecture=8, CPU variant=0x0, CPU part=0xd87, CPU revision=1`.
- Rust toolchain: `nightly-2026-06-20`, release profile, target `aarch64-unknown-linux-gnu`.

The later shared-harness correctness checkpoint named in the banner contains 2,886 upstream records, 1,974 default-feature public Rust API items, and 1,744 evidence parents: 580 implemented, 17 evidence-close, and 1,147 planned.
The current shared-harness performance checkpoint named in the banner contains 549 groups and exactly nineteen executable product groups plus one diagnostic protocol group. This report closes only its frozen `48eacf03` inventory and does not imply that the later Clifford, DEM, or shared contracts were executed with the iterator evidence.
The inherited `m6-pauli-iter` row is superseded for timing, while its memory baseline remains guarded until PQ6 supplies equal or stronger evidence.

## Correctness Preflight

The clean correctness report at `target/qualification/pq2-pauli-iter-full-afaf0bf` selected and passed exactly these three cases:

- `cq-evidence-qualification-0a4be178ce1c903b` owns independent sparse enumeration, exact order and count, frozen sequence digests, word boundaries, the six runtime scales, complete yielded-state checks throughout every singleton traversal including the accepted maximum, and the accepted iterator constructor boundary.
- `cq-evidence-qualification-489e6445120743c2` owns Pauli materialization and the typed 1,048,577-qubit first rejection after the 1,048,576-qubit accepted maximum.
- `cq-evidence-qualification-5331280b58fd49c7` owns borrowed result reuse, restart, typed construction, and iterator state behavior.

| Artifact | SHA-256 |
| --- | --- |
| Request | `2d04cd1881827fa4e0547649560bbdad291775c78333fad7830694e6cbb67d48` |
| Report | `c61d48b990052436922bc02197b49c40a455f9ee4b4a70f457ebbc603d478287` |
| Completion | `42df1955d4fa59a48620fffcf1ef651050b5c83eaf2b8867f5241c0fa0b1ec70` |
| Preflight | `a06417d24a8f446f4974e4cd7de8b179ae2218e42560d776633ab304000fe5f1` |

Every performance report independently reconstructed the canonical correctness request, report, completion, execution receipts, and exact three-case preflight before timing.
The complete 271-parent CQ2 family remains a separate historical checkpoint and is not claimed by this focused prerequisite run. Its PR, full, and soak execution passed at clean revision `3f2f382627c8421de0a668819d467a9f252de20f` under the hardened controller and is not relabeled as current-revision evidence.

## Correctness Oracle

The exact order owner uses an independent sparse combinatorial enumerator and never uses owned `Iterator::next` as its oracle.
It freezes literal 256-bit sequence digests for all three range scales, all three singleton runtime scales, the singleton accepted maximum, and singleton widths 63, 64, 65, 255, 256, and 257.
It also freezes first and last sparse values, exact output count, total result-width checksum, restart equivalence, borrowed storage reuse, positive sign, and the public resource boundary.

For every singleton output, including all 3,145,728 values at the 1,048,576-qubit accepted maximum, the owner independently advances expected X and Z word planes and compares both complete yielded planes against them.
The owner also checks the positive sign, exact current basis, exact output count, total width checksum, first and last sparse values, and frozen sequence digest at every runtime width, required word boundary, and accepted maximum.
This all-output strategy closes the transient-state loophole without using owned `Iterator::next` or the benchmark worker as an oracle.

## Workload Contracts

Both workers construct one public iterator per timed callback, call borrowed `iter_next` until exhaustion, consume the borrowed result width and output count, and destroy the iterator before returning.
Neither worker times restart, owned result cloning, formatting, or a closed-form combinatorial count.
Semantic work is the checked number of yielded Pauli strings.

| Group | Scale | Width | Weights | Axes | Outputs per traversal | Input digest |
| --- | --- | ---: | --- | --- | ---: | --- |
| Range | Small | 5 | 2 through 5 | X/Z | 232 | `315732711c88257f9f4b2be3dfc3dd01785be01b86bdb7338e663945a90070d5` |
| Range | Medium | 11 | 2 through 5 | X/Z | 21,604 | `d5c711573168f39a388aa386b1fb66b4b9d063f2a070610cd4543442548f4102` |
| Range | Large | 22 | 2 through 5 | X/Z | 972,972 | `85017fcee6d99c399676aac24ff1945f03363f316352eb10d707b51c66f506bc` |
| Singleton | Small | 1,000 | 1 | X/Y/Z | 3,000 | `d8d6b42d21392b7ab593f2b09cb9673e261381aa2d93c8f15b8c4ac52a97235b` |
| Singleton | Medium | 32,000 | 1 | X/Y/Z | 96,000 | `802dc4fd7b6e4d21c2eef73174aa24ee6cb81bc00be978d223a4e4c2242d89f9` |
| Singleton | Large | 1,000,000 | 1 | X/Y/Z | 3,000,000 | `394634d1a0abfaace26d4f3c07b81fe797d60c474314e625fd7f02f64d25fd0d` |

Each canonical input is exactly eight little-endian `u64` fields covering width, weight range, axis mask, outputs per traversal, workload marker, private output cap, and public qubit cap.
Each output digest binds iteration count, semantic work, width, workload shape, observed output count, observed width checksum, canonical input digest, and the final yielded result from an untimed validation traversal.

The workers accept the range contract through width 22 and reject width 23 because its 1,233,628 outputs exceed the 1,000,000-output private cap.
The singleton contract accepts width 1,048,576 with 3,145,728 outputs and rejects width 1,048,577 before allocation or barrier consumption.
Both adapters also freeze zero work, malformed shape, wrong measurement, semantic-work overflow, and width-checksum overflow rejection for both implementations.

## Worker Identity

`just bench::qualification-worker-reproducibility` rebuilt both sealed workers twice and reproduced these identities:

| Identity | SHA-256 |
| --- | --- |
| Stim source | `5c8bc8f0b3f76fd688104e9110087a09d1f3c0027b24272d1d30d033e9e6990a` |
| Stim build fingerprint | `d7e6e5997d85f283c5b8c4b4eb515a65503bd537802cec370c34b474f660d30a` |
| Stim binary | `7eac6fff60afb7a93ae9168021b743e286caecc4f164907bb6e4a5a9cad8f75d` |
| Stab source | `8cf7c82e21d8aa42fe8f890eb6e41d8121b60aebf82b08e7bf32b3ca2eff6f45` |
| Stab build fingerprint | `57ccdad4edc45057348ed7be40cfe84fac9f860e8f0449c3e23e858aa57cae85` |
| Stab binary | `e23c6879e10d79c8801068dcb8564c4ec31deb0b29a90580e1d5250a7cbd7865` |
| Contract preflight | `b740d0e536ae71d3be925761f76bf266fd6a6a9d0d8d93542f1b1fd439493190` |

Private Stab build-receipt schema version 3 includes the isolated iterator worker.
Adapter receipt and contract-preflight schema version 10 bind the ordered Rust and C++ sources and 140 accepted or rejected probes across the complete adapter surface.
Qualification report schema version 28 preserves those identities for offline replay.
The two adapter smoke timings are diagnostic and are not product speed evidence.

## Timing Results

Each full report retains nine interleaved pairs and each soak report retains 15.
Every report uses equivalent semantic work, contains one timing attempt, and passes the median and bootstrap-upper `1.25x` rules independently.

| Group | Scale | Tier | Pairs | Median ratio | 95% upper | Ratio rMAD | Outcome |
| --- | --- | --- | ---: | ---: | ---: | ---: | --- |
| Range | Small | Full | 9 | 0.500338 | 0.504913 | 0.008315 | Passed |
| Range | Medium | Full | 9 | 0.492180 | 0.498136 | 0.006547 | Passed |
| Range | Large | Full | 9 | 0.484869 | 0.491952 | 0.012526 | Passed |
| Range | Small | Soak | 15 | 0.498981 | 0.501253 | 0.006747 | Passed |
| Range | Medium | Soak | 15 | 0.491033 | 0.496955 | 0.008412 | Passed |
| Range | Large | Soak | 15 | 0.485768 | 0.488244 | 0.009965 | Passed |
| Singleton | Small | Full | 9 | 0.568566 | 0.570628 | 0.003627 | Passed |
| Singleton | Medium | Full | 9 | 0.317011 | 0.319983 | 0.002025 | Passed |
| Singleton | Large | Full | 9 | 0.025664 | 0.025724 | 0.002450 | Passed |
| Singleton | Small | Soak | 15 | 0.567546 | 0.568185 | 0.002305 | Passed |
| Singleton | Medium | Soak | 15 | 0.317045 | 0.320070 | 0.009542 | Passed |
| Singleton | Large | Soak | 15 | 0.025721 | 0.025752 | 0.001012 | Passed |

The outcome count is 12 passed, zero failed, zero noisy, and zero report-only timing measurements.
All 12 passed on their first attempt, so no result was rerun toward a more favorable sample.
Both large singleton reports use the strictly derived wide-ratio common-batch mode; the other ten reports use standard common-batch validation.

## Resource Evidence

Allocation-counted tests cover every runtime scale and both accepted maxima.
One complete range callback performs exactly five allocation calls requesting 120 bytes.
One complete singleton callback performs exactly four allocation calls requesting the two result bit planes plus 40 bytes, with requested bytes derived from the checked width.
The call counts and requested-byte formulas stay constant across traversal output count, so a future constructor or traversal allocation cannot silently enter the contract.

Across all 12 reports, observed Stim setup RSS ranges from 3,473,408 to 3,719,168 bytes and Stim peak RSS ranges from 3,473,408 to 3,842,048 bytes.
Stab setup RSS ranges from 4,800,512 to 4,898,816 bytes and Stab peak RSS ranges from 4,866,048 to 4,964,352 bytes.
These process observations are report-only and do not establish cross-scale or Stim-relative memory parity.
The retained `m6-pauli-iter` entry in `benchmarks/m12-primary-memory-baseline.json` remains explicitly guarded until PQ6 provides equal or stronger memory evidence.

## Source Reports

Every source directory contains canonical `report.json`, `preflight.json`, and derived `report.md`.
The completion controllers independently replayed and regression-checked all 12 source reports.

| Group | Scale | Tier | Directory | Report SHA-256 |
| --- | --- | --- | --- | --- |
| Range | Small | Full | `target/benchmarks/qualification/perfq-m6-pauli-iter-afaf0bf-full-small` | `8cc136eaf2cf0228f99967cc8eb0f4d9dd78c7b15d36e5ca287089e9552dfc6b` |
| Range | Medium | Full | `target/benchmarks/qualification/perfq-m6-pauli-iter-afaf0bf-full-medium` | `9b0b91cd18817d6d0bff7d991c7acba2e2a16f01ed127d33f9195d3115991bf6` |
| Range | Large | Full | `target/benchmarks/qualification/perfq-m6-pauli-iter-afaf0bf-full-large` | `192071e26c0e5701d93475b9b225b1e769f9fd3eaeb7be882c6278aec508e227` |
| Range | Small | Soak | `target/benchmarks/qualification/perfq-m6-pauli-iter-afaf0bf-soak-small` | `79009abf42111079e02db562fe62f432c074c3e726f825982a00aad17e1063b8` |
| Range | Medium | Soak | `target/benchmarks/qualification/perfq-m6-pauli-iter-afaf0bf-soak-medium` | `61d9d22ab2f10b7c2469be68540b80da9ecde92eb920aa5fbd77d3c91bab7eb3` |
| Range | Large | Soak | `target/benchmarks/qualification/perfq-m6-pauli-iter-afaf0bf-soak-large` | `d04bd29da682a06bb47a9ffe78c134d3097b9212f8febe0d555d5eb1b6979053` |
| Singleton | Small | Full | `target/benchmarks/qualification/perfq-m6-pauli-iter-singleton-afaf0bf-full-small` | `d99baeb3118042a62e46789436316d6397efdc81ea609733f1c85291fc4b6629` |
| Singleton | Medium | Full | `target/benchmarks/qualification/perfq-m6-pauli-iter-singleton-afaf0bf-full-medium` | `f8046c2ac933ed42c28e904ad95e992d1e984256fb61e45ab082ad5bad15320b` |
| Singleton | Large | Full | `target/benchmarks/qualification/perfq-m6-pauli-iter-singleton-afaf0bf-full-large` | `977de226f70f7ea788ab99ec34984bf921a271206020ec2d6de6682830659775` |
| Singleton | Small | Soak | `target/benchmarks/qualification/perfq-m6-pauli-iter-singleton-afaf0bf-soak-small` | `cb8b47c8a439eaeb593131b54c9c540873d7fe8ee0a0baec99a2457ae7b178a9` |
| Singleton | Medium | Soak | `target/benchmarks/qualification/perfq-m6-pauli-iter-singleton-afaf0bf-soak-medium` | `a23d047011d5ee8bad53c32d8e71c4fa4dd86d07278689041254c33432ec7e38` |
| Singleton | Large | Soak | `target/benchmarks/qualification/perfq-m6-pauli-iter-singleton-afaf0bf-soak-large` | `343c5deed3cf0092090eef4202c80e05903a2de2bc9331a35dd4a4856aeeaba3` |

## Rollups And Completion Receipts

| Group | Artifact | Directory | Report SHA-256 | Preflight SHA-256 |
| --- | --- | --- | --- | --- |
| Range | Full rollup | `target/benchmarks/qualification/perfq-m6-pauli-iter-afaf0bf-full-rollup` | `65c282647e3c51f0ea63c0a4e98453ea61739d17bce0a5eba47229db47fc393c` | `ffc97ea5204d553f0b7238e2154fadc1ba0d178544b685e214cfdc4513e0c7d4` |
| Range | Soak rollup | `target/benchmarks/qualification/perfq-m6-pauli-iter-afaf0bf-soak-rollup` | `08cfdc344c098f923c29e6d3858e8e846efa5549a4f6ec6927a52a2dcb299d34` | `4042fd366c5c80f4620bf11aa3458676de11e30f4e08c066a0a9531fbcd93c6b` |
| Range | Completion | `target/benchmarks/qualification/perfq-m6-pauli-iter-afaf0bf-completion` | `55042e37a07653fdfccae3035605db2f31ff78bdf655fd8e8faa973f9d77a445` | `0f9a1a2952a2a9915225a28273c15b245a41d2d0518ec6efcab5d97f6d43e64f` |
| Singleton | Full rollup | `target/benchmarks/qualification/perfq-m6-pauli-iter-singleton-afaf0bf-full-rollup` | `3b4492d4cb80663388dadeb905e2cac0dae4d728acc8bf213ef097a7cfc654af` | `04a9588ec78a762ba503178b8711d96b053b0b377e830f9e630b0a03cbd08733` |
| Singleton | Soak rollup | `target/benchmarks/qualification/perfq-m6-pauli-iter-singleton-afaf0bf-soak-rollup` | `56259ec621c826e061780301d7f9276047dd58671782cd75e6925848e68dab07` | `e557f573b7e234f6680fc49a9ab08fd3e4f3f7a1f1bb8c500767d39baa1d125b` |
| Singleton | Completion | `target/benchmarks/qualification/perfq-m6-pauli-iter-singleton-afaf0bf-completion` | `12b1ba3ec6bcb93c73ed405f01ece6bac99323b95b4c4710de52861d250d5057` | `8ec39b578c758299959129996f37edb409e29c77f59d979287edd607755ade70` |

Every rollup passed publication and byte-for-byte replay.
Each completion receipt binds six source reports, two rollups, 16 successful closure steps, exact correctness artifacts, one worker identity set, one clean revision, one CPU identity, adapter preflight, every report replay and regression, and both rollup replays.
Both completion receipts passed independent byte-for-byte replay with zero failed steps.

## Legacy M12 Migration

Clean pre-migration revision `f2388dccc01abb7ef89e5f56d9062c6656837470` completed and replayed both iterator chains under performance inventory `ad3b6640e04855ac76d4851f856bb405e7c80c55dbcd67b204d67ea41d40c1eb`.
Its 12 first-attempt median ratios ranged from `0.025433x` to `0.563411x`, with worst upper bound `0.564226x`.
Those receipts authorized retirement of only the bundled `m6-pauli-iter` timing threshold and its two exact small-scale mappings.

Migration commit `d706634eeaa536b2ce48d3dc9431b4feb513317f` made that focused change, marked the inherited timing row superseded, and preserved the memory baseline.
Audit and review strengthening then changed the exact correctness source, so intermediate post-migration evidence at `d706634e` and `72d34f3f` remains historical rather than being relabeled as final.
The complete frozen post-migration chain recorded here was regenerated and replayed from clean revision `afaf0bf7f236b9f6ae6f72c19bbbdfea94d26632`.

## Milestone Audit

The milestone audit mapped every tenth-slice task to implementation, exact correctness, hostile-boundary, allocation, comparator, report, regression, rollup, completion, migration, and documentation evidence.
It found that the exact order owner initially computed sequence summaries without frozen literal digests and first or last sparse values.
Commit `72d34f3fad80c5922959f77936489658f91e585d` added independently reproduced literal sequence contracts for all runtime scales and every required word boundary.
Follow-up review exposed one specification loophole: the milestone's accepted-constructor wording did not require complete traversal at the 1,048,576-qubit public maximum.
Commit `afaf0bf7f236b9f6ae6f72c19bbbdfea94d26632` resolves that loophole in the plan and exact owner, and `docs/plans/milestone-spec-gaps.md` records the resolved amendment.
The final audit found no remaining implementation, evidence, resource, benchmark, documentation, or specification gap.

## Independent Review

The initial GPT-5.6/max review found no product implementation defect and confirmed API ownership, comparator fidelity, migration scope, allocation contracts, and receipt integrity.
It reported one P2 evidence gap: the wide singleton owner recorded expected sparse tuples for most outputs, so a transient unexpected term outside the current and predecessor positions could evade the sequence digest.
Commit `40a01814bc80ec039a3444dc03b79c6b53e55140` added actual sparse-state checkpoints, and follow-up review correctly found that transient states between checkpoints could still evade the oracle.
Commit `8c7966e458beb68a00412f1e8b7a35cb5c325aaf` replaced checkpoint sampling with complete X and Z plane comparison for every singleton output at all then-selected widths, while commit `15067ba8721cda03300301fffe80ee6b51332756` aligned the documented calibration contract with strict derived wide-ratio mode.
The next GPT-5.6/max review confirmed those two fixes but found that the 1,048,576-qubit accepted maximum was only constructed, leaving its final 48,576 positions outside full traversal evidence; it also rejected the provisional `40a01814` documentation as obsolete.
Commit `afaf0bf7f236b9f6ae6f72c19bbbdfea94d26632` adds all-output accepted-maximum validation and the resolved specification amendment, and the complete correctness and performance chains were regenerated from that clean revision.
The final two GPT-5.6/max review lanes found an operational-documentation gap, provisional-status inconsistencies, and one inaccurate probe-order sentence. The documentation was synchronized to the fifteen executable groups, current receipt and report schemas, 140-receipt preflight, both iterator probes, accurate rejection timing, and a single provisional state. Both final follow-up lanes then reported no remaining P0 through P3 finding and accepted this progress report.

## Verification Record

The clean evidence revision passed the exact three-case correctness run, report, and preflight; worker reproducibility; both adapter probes; all full and soak report producers; report replay and regression; all four rollup producers and replays; both completion producers and replays; and the allocation-counted iterator contract.

The final documentation revision must also pass:

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

1. Run both clean full and soak families, rollups, and completion receipts on a controlled native Linux x86-64 host before making an x86-64 conclusion.
2. Define and validate explicit cross-scale RSS and allocation-growth rules in PQ6 before making a memory qualification claim or retiring the legacy memory baseline.
3. Qualify restart, owned iteration, clone, formatting, comparison, arbitrary weight and axis shapes, commuting iteration, Tableau iteration, and the remaining Algebra surfaces only through their own exact public API groups.
4. Select the next finite dependency-ordered PQ2 runtime group without reopening this accepted AArch64 slice merely to produce a newer aggregate digest.
