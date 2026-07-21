# PQ2 Pauli-String Multiplication Qualification Progress Report

> Historical-inventory note, 2026-07-19: this report remains accepted Linux AArch64 evidence under its frozen correctness and performance inventories. Adding and migrating the split Pauli iterator contracts produced post-migration performance digest `48eacf03a2ecdca917c05aade52b7e17c9ead1be8b75b203e1d43c2f3b3b7dbf`; later reviewed Clifford contracts produced now-historical digest `a76090c996ad404c1cb8bfa85066e286c6f40b32754b3750e984375f7ca90025`; the current shared-harness digest is `4c2407613ee63903c57acb4b177e91799047ed60f7bff780929fe84532f57643`. The results below are not relabeled as simultaneous current-inventory evidence.

## Status

The ninth PQ2 executable slice passes its independent `1.25x` timing gates for public equal-width in-place Pauli-string right multiplication at every full and soak scale on the controlled Linux AArch64 host as of 2026-07-17.

All six promotable measurements passed on their first attempt, without a noise rerun, report-only timing outcome, waiver, profiler note, or performance optimization.
The six median Stab-to-Stim elapsed-time ratios range from `1.001956x` to `1.032352x`, with worst bootstrap confidence-interval upper bound `1.032881x`.
These are small Stab slowdowns of approximately 0.2% through 3.2%, not speedups, but every result is comfortably below the exact `1.25x` acceptance gate.
The faithful scalar public path already passes, so this slice did not add portable SIMD merely to improve an already accepted ratio.

This report closes `PERFQ-M6-PAULI-STRING/right-multiply-in-place` on Linux AArch64 only.
It does not qualify allocating or unequal-width multiplication, the identity fast path, scalar-product or commutation queries, parsing, formatting, randomization, Clifford or Tableau operations, native Linux x86-64, cross-scale memory growth, or the remaining Algebra surface. The separate 271-parent CQ2 checkpoint is source-current at clean hardened-controller revision `3f2f382627c8421de0a668819d467a9f252de20f`.

## Frozen Evidence

- Clean Stab evidence revision: `cd1e33e10f45995ccaca498547ff5aa88bfe51bb`, with `local_modifications=false` in every final correctness and performance producer.
- Stim baseline: v1.16.0 at `e2fc1eca7fd21684d433aa5f10f4504ea4860d07`.
- Performance inventory: `e79edf2e1eaa49a801606245d4a845d47a1d000ed527c9669d95e091c4480237`.
- Correctness inventory: `a739d350eeb3455d4a0b386f8a257d3d4fe01d417d7d11d8a269229d68a6a103`.
- Runtime-group contract: `379feedbb890562ba00d67d71572683420ee3d8e95d30212fc26346afd9df957`.
- Exact correctness corpus: `277f305dc5d6bcbd1b6b7d37d2a436e65cd10b2d92e4764aba27f96980448c81`.
- Host profile: verified `linux-aarch64-controlled`, pinned to logical CPU 0 with CPU identity `CPU implementer=0x41, CPU architecture=8, CPU variant=0x0, CPU part=0xd87, CPU revision=1`.
- Rust toolchain: `nightly-2026-06-20`, release profile, target `aarch64-unknown-linux-gnu`.

At this slice's accepted revision, the correctness inventory contained 2,886 upstream records, 1,972 default-feature public Rust API items, and 1,744 evidence parents: 580 implemented, 17 evidence-close, and 1,147 planned.
At this slice's accepted revision, the performance inventory contained 547 groups and 161 inherited rows classified as 11 retained, 135 reworked, four diagnostic, nine superseded, and two removed, with 21 exact threshold pairs.
This slice does not reinterpret any of the remaining missing comparator, scale, correctness, output, CLI, or heterogeneous-selection dispositions as passes.

## Correctness Preflight

The clean correctness report at `target/qualification/pq2-m6-pauli-full-cd1e33e` selected and passed exactly these two cases:

- `cq-evidence-qualification-3bab0f51237445f6` owns direct in-place multiplication semantics, an independent per-qubit scalar oracle, exhaustive single-qubit left basis, right basis, and sign combinations, returned base-`i` phase, odd and even repetition, left real-sign preservation, right-operand immutability, deterministic word boundaries at 63, 64, 65, 255, 256, and 257 qubits, and runtime scales through 1,000,000 qubits.
- `cq-evidence-qualification-489e6445120743c2` owns Pauli materialization, checked sizing, and the 1,048,576-qubit public resource boundary.

| Artifact | SHA-256 |
| --- | --- |
| Request | `e0f1940db9ea7518c4724074350ced96bb919db30d3ffbdbc1a5219dfb6d58f3` |
| Report | `6b2573a8c3374e64f8c0b44079aae52588b79af635298f3ecfe2137491d951e4` |
| Completion | `554ac21a57c540ba550d8ef574415a36d3531c23bd6ec5843e0af277f8f45429` |
| Preflight | `6377ba3310afb3e605487325ec1a3eea2043c5ffa4777da5b0de46d963763116` |

Every performance report independently reconstructed the canonical correctness request, report, completion, both execution receipts, and exact two-case preflight before timing.
The complete 271-parent CQ2 family remains a separate checkpoint and is not claimed by this focused prerequisite run; its source-current PR, full, and soak execution passed at clean revision `3f2f382627c8421de0a668819d467a9f252de20f` under the hardened controller.

## Workload Contract

Both workers generate the same dense non-identity left and right operands from independently frozen SplitMix64 streams.
The canonical input binds width, generator marker, positive left sign, negative right sign, and complete little-endian `left_x`, `left_z`, `right_x`, and `right_z` word planes with canonical tail masking.

| Scale | Qubits | Input bytes | Input digest | Full-tier output digest |
| --- | ---: | ---: | --- | --- |
| Small | 10,000 | 5,056 | `401b897ceb9c02fec1c57b15f76cdc45045fd551354c3dc5ae499e791aef22f4` | `17c9b1c111c7a61a1b61f6afd446f96140bd71b34eb61531c1cbfff4da446cc4` |
| Medium | 100,000 | 50,048 | `51b8460e6069c3590ce2e25ee912a0ef92dfe1000a28aa4a1aa3b644ba0d402f` | `f27d3d62eb6d24c638558b8cc2a536794c6c9f3360f2768897afaf123937dde7` |
| Large | 1,000,000 | 500,032 | `5babb5f0de800c6ed6c644d103b7a0d01ab22fa7696a363e9120c7cac8157fd9` | `48f8d2af9d370d746f230e88fd5b569a3d3a59fbdd5639f7efe5c4166266b672` |

The workers execute two untimed complete multiplications to restore the canonical left state, then time only public Stab `PauliString::right_multiply_in_place_returning_log_i_scalar` against pinned Stim `PauliStringRef::inplace_right_mul_returning_log_i_scalar` behind equivalent optimizer barriers.
Semantic work is the checked product of timed public calls and logical qubits.
The output binds iteration count, semantic work, width, workload marker, accumulated returned phase, final left state, and unchanged right state.
The sealed workers accept every width from 1 through 1,048,576 qubits, while the runtime ledger exposes only the three source-owned timing scales. Preflight also exercises the accepted maximum and rejects zero width, over-cap width, wrong measurement ids, and semantic-work overflow before setup or barrier consumption.

## Worker Identity

`just bench::qualification-worker-reproducibility` rebuilt both sealed workers twice and reproduced these identities:

| Identity | SHA-256 |
| --- | --- |
| Stim source | `976a412ff242de4c5efc4b442c06e512088196664ab30d423bde52191d165230` |
| Stim build fingerprint | `bbedd59f5cea5c6b03152e841ed9a600bcc761fa901999431e976506c9038ea1` |
| Stim binary | `c64f87a22a62af6f3687a10cb8a3cab28090bd254e6d3caa505edb59023bd63c` |
| Stab source | `266d409d6e20ea82a8de6648ae107c01e8846f7a8abab314f1c4c7e15e7eba7b` |
| Stab build fingerprint | `aefc4521c72fd15f12f890c4d167b4a9dfff87559e05d840f77770948018492c` |
| Stab binary | `439300467cde7264933fd5f225530d350bc2464b49f7c6e42ba40d30a3b347d7` |
| Contract preflight | `4dc8024185a9d6ddbe3461eb17d39620410ece83e47e161acfa26c1f5f653202` |

Adapter receipt schema version 9 binds the ordered comparator sources, including `benchmarks/stim_adapter/pauli_string_multiply_contract.h`.
Private Stab build-receipt schema version 2 includes the isolated Pauli worker, contract-preflight schema version 9 binds 104 accepted and rejected probes across the complete adapter surface, and qualification report schema version 27 preserves those identities for offline replay.
The standalone Pauli adapter probe passed, but its tiny timing is diagnostic and is not product ratio evidence.

## Timing Results

Each full report retains nine interleaved pairs and each soak report retains 15.
Every report uses standard equal-work mode, contains one timing attempt, and passes both the median and bootstrap-upper `1.25x` rules independently.

| Scale | Tier | Pairs | Median ratio | 95% upper | Ratio rMAD | Outcome |
| --- | --- | ---: | ---: | ---: | ---: | --- |
| Small | Full | 9 | 1.032352 | 1.032754 | 0.000228 | Passed |
| Medium | Full | 9 | 1.001956 | 1.009451 | 0.000529 | Passed |
| Large | Full | 9 | 1.009823 | 1.010688 | 0.000312 | Passed |
| Small | Soak | 15 | 1.032174 | 1.032881 | 0.000580 | Passed |
| Medium | Soak | 15 | 1.002298 | 1.002472 | 0.000200 | Passed |
| Large | Soak | 15 | 1.010033 | 1.010305 | 0.000551 | Passed |

Slice outcome counts are six passed, zero failed, zero noisy, and zero report-only timing measurements.
All six reports passed on their first attempt, so no result was rerun toward a more favorable sample.

## Resource Evidence

The allocation-counted test proves zero allocation calls and zero allocated bytes for every timed Stab public call at 10,000, 100,000, 1,000,000, and the accepted maximum of 1,048,576 qubits.
Across the six reports, observed Stim setup and peak RSS range from 3,461,120 to 3,960,832 bytes.
Stab setup RSS ranges from 4,730,880 to 6,213,632 bytes, and Stab peak RSS ranges from 4,796,416 to 6,279,168 bytes.
These process observations are report-only and do not establish cross-scale or Stim-relative memory parity.
The retained `m6-pauli-string` entry in `benchmarks/m12-primary-memory-baseline.json` remains explicitly guarded until PQ6 provides equal or stronger memory evidence.

## Source Reports

Every source directory contains canonical `report.json`, `preflight.json`, and derived `report.md`, and every report passed immediate byte-for-byte replay plus source-owned regression.

| Scale | Tier | Directory | Report SHA-256 |
| --- | --- | --- | --- |
| Small | Full | `target/benchmarks/qualification/perfq-m6-pauli-cd1e33e-full-small` | `2dc7fa69d0c35af13baa1e817a01d2cb2d03f790a17486212e3afd022f240333` |
| Medium | Full | `target/benchmarks/qualification/perfq-m6-pauli-cd1e33e-full-medium` | `4a91aa8f919b8091520c3758198158ab0ca7a61b833836273bd3e1d4fcb3c85f` |
| Large | Full | `target/benchmarks/qualification/perfq-m6-pauli-cd1e33e-full-large` | `2b8932d6c7f8447244efd270748965f4eb8352a28ec07616d735e5dcd79d5ce1` |
| Small | Soak | `target/benchmarks/qualification/perfq-m6-pauli-cd1e33e-soak-small` | `c72d7589aa453a0195e82dc03e8f63af3b6184871c8514836742951763f493f0` |
| Medium | Soak | `target/benchmarks/qualification/perfq-m6-pauli-cd1e33e-soak-medium` | `36e225233518017b07cabf05913bd7897fd52fc2fbebae8bfe152dcdccb74ac0` |
| Large | Soak | `target/benchmarks/qualification/perfq-m6-pauli-cd1e33e-soak-large` | `93ee0d8ce6fd8d1abe88451b42473d2aedbe3ed746ed62384ebee9af4fa5e78a` |

## Rollups And Completion Receipt

| Artifact | Directory | Report SHA-256 | Preflight SHA-256 |
| --- | --- | --- | --- |
| Full rollup | `target/benchmarks/qualification/perfq-m6-pauli-cd1e33e-full-rollup` | `de88ac178ac8bf010e2ec466ad3546e12c27afc01c988923acb59e251a08b7ca` | `f51d9218740b9d821a763141b41864366ecea5c36d1be7be08ffa9b37a26d62d` |
| Soak rollup | `target/benchmarks/qualification/perfq-m6-pauli-cd1e33e-soak-rollup` | `27838535e154538c8ec8214c45a837d2e0b058cd620d0c4e6ca783cb1fbf0842` | `77c42ad29b1b60d9592e3424f777149e5739d9f404d90f9437d7201dae6d5ad0` |
| Completion | `target/benchmarks/qualification/perfq-m6-pauli-cd1e33e-completion` | `7a76acb0a09977174c4beb3a83fcfdb5b97d727a525776f00de4b9a88bb624e3` | `063f17d2ba13577fa58e41e32636052a9a499187e5d949b0e028be9a113da6a0` |

Both rollups passed publication and offline replay.
The completion receipt binds all six source reports, both rollups, 16 successful closure steps, exact correctness artifacts, one worker identity set, one clean revision, one CPU identity, adapter probe, every report replay and regression, and both rollup replays.
The completion receipt passed independent byte-for-byte replay.

## Legacy M12 Migration

Clean pre-migration revision `3a0fcd814f8d1a9441420ab85edf3d757572ba93` passed and replayed the first complete Pauli chain at correctness digest `3db44922e3310cb3a573fcff3b28d5eea5d28e0d6975e0856965c601ecc23c72` and performance digest `84d5ab682acda2a847972a74c5d58443fde8d2c820e62e46b634562e7c918e46`.
Its six median ratios ranged from `1.002254x` to `1.031540x`, with worst upper bound `1.031846x`, and authorized retirement of only the identity-only `m6-pauli-string` timing threshold, its three exact pair thresholds, and temporary scale mappings.
Migration commit `42c132f2c49538364649cd90962166223c72b4c6` made that focused change while preserving the memory baseline.
Strengthening the exact correctness owner then changed both source inventories, so the historical first chain was not relabeled as current.
The complete frozen post-migration chain recorded here was regenerated and replayed from reviewed clean revision `cd1e33e10f45995ccaca498547ff5aa88bfe51bb`.

## Milestone Audit

The milestone audit mapped every ninth-slice task to direct implementation, exact correctness, hostile-boundary, allocation, comparator, report, regression, rollup, completion, migration, and documentation evidence.
It found that the exact CQ owner lacked exhaustive basis and sign coverage and omitted 255, 256, and 257-qubit word boundaries.
It also exposed a broken allocation-feature test and a stale timing-threshold row-count assertion.
Commit `ec4f783` closed those gaps with an independently computed direct scalar oracle inside the fingerprinted exact corpus, repaired allocation instrumentation, and asserted explicit absence of the retired legacy timing row.
No milestone under-specification was revealed, so `docs/plans/milestone-spec-gaps.md` gains no entry.

## Independent Review

The independent GPT-5.6/max review found no P0, P1, or P2 issues.
It reported two P3 evidence-maintenance findings: the retained legacy memory baseline lacked an explicit presence guard, and a scale test merely echoed a locally constructed field instead of protecting behavior.
Commit `cd1e33e10f45995ccaca498547ff5aa88bfe51bb` added the memory-baseline guard and removed the low-value constructor echo test.
The reviewer independently confirmed exact CQ ownership, allocation repair, timing-threshold migration, direct scalar-kernel equivalence, comparator fidelity, and narrow legacy migration.
The GPT-5.6/max follow-up reported no remaining P0, P1, P2, or P3 findings at `cd1e33e10f45995ccaca498547ff5aa88bfe51bb`.
It passed all five targeted benchmark tests, replayed the focused correctness report and exact preflight, verified all six performance reports and both rollups, checked the completion bindings, and matched all 34 copied evidence artifacts byte-for-byte to their originals.
Its additional detached end-to-end completion replay was interrupted during CMake worker reconstruction after the shared checkout correctly rejected unrelated uncommitted documentation; the already accepted completion replay and immutable artifact checks remain the evidence for closure.

## Verification Record

The evidence revision passed the exact two-case correctness run, report, and preflight; worker reproducibility; adapter probe; all full and soak report producers; immediate report replay and regression; both rollup producers and replays; and completion producer and replay.
The implementation and qualification checks also passed:

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

1. Run the same clean full and soak families, rollups, and completion receipt on a controlled native Linux x86-64 host before making an x86-64 conclusion.
2. Define and validate explicit cross-scale RSS and allocation-growth rules in PQ6 before making a memory qualification claim or retiring the legacy memory baseline.
3. Qualify allocating multiplication, unequal-width growth, identity-fast-path timing, commutation, randomization, Clifford operations, Tableau operations, and the remaining Algebra surfaces only through their own exact public API groups.
4. Select the next finite dependency-ordered PQ2 runtime group without reopening this completed AArch64 slice.
