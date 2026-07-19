# PQ2 SIMD-Bits XOR Qualification Progress Report

> Historical-inventory note, 2026-07-19: this report remains valid passing AArch64 evidence at performance inventory `fb50789c58786219c807c79e87202396b17563ee7cb584bcda4d3379007ed716`. Later product groups, evidence-authorized migrations, and the reviewed Clifford contracts produced now-historical digest `a76090c996ad404c1cb8bfa85066e286c6f40b32754b3750e984375f7ca90025`; the current shared-harness digest is `c238dc4e2500192f310ef3d2378ecaafc9744662b5127784dd4eeb6c60726176`. The measured dense-XOR implementation and outcome below are not relabeled as current-inventory evidence.

## Status

The fifth PQ2 product group, `PERFQ-M5-SIMD-BITS`, passes the exact `1.25x` timing gate at every full and soak scale on the controlled Linux AArch64 host as of 2026-07-16.

All six promotable `xor-complete-vector` measurements passed on their initial attempt, with median Stab-to-Stim elapsed-time ratios from `0.374633x` to `0.559828x` and a worst bootstrap confidence-interval upper bound of `0.561257x`. For this exact complete-vector dense-XOR workload, Stab used about 37.5 percent to 56.0 percent of pinned Stim's measured time, corresponding to roughly 1.79x to 2.67x the throughput.

This report closes dense complete-vector XOR on AArch64. It does not qualify `not_zero`, randomization, masked or ranged mutation, copying, clearing, bit-table operations, sparse XOR, other bit-kernel phases, all remaining PQ2 groups, native Linux x86-64 execution, or PQ6 memory growth.

## Frozen Inputs

- Stab evidence revision: `5d226c94ece70f96d0b771f9c8cde7464ccd261b`, with `local_modifications=false` before and after every correctness, performance, and rollup report.
- Stim baseline: v1.16.0 at `e2fc1eca7fd21684d433aa5f10f4504ea4860d07`.
- Correctness inventory: `ccb80eb4b660a375b59460c3b7fa03a810abd6f868735b566735378105db22b2`.
- Performance inventory: `fb50789c58786219c807c79e87202396b17563ee7cb584bcda4d3379007ed716`.
- Runtime group contract: `c5b2cb952ab49fc1f7021d86cbd02e012daa66b1e360ae8fa6ebfb6009a576c0`.
- Host profile: verified `linux-aarch64-controlled`, pinned to logical CPU 0 with the performance governor at 2,808,000 kHz and no host-policy violation.
- Rust toolchain: `nightly-2026-06-20` targeting `aarch64-unknown-linux-gnu` with the release profile.
- Profiler note: none. Every promotable report passed without a noisy or failed attempt.

## Correctness Preflight

The clean full report at `target/qualification/pq2-m5-simd-bits-full-5d226c9` selected and passed these two exact prerequisites:

- `cq-evidence-qualification-b1530dc4e48e942d` for complete-vector XOR semantics, canonical tails, zero and nonzero behavior, length rejection, and allocation-free in-place mutation.
- `cq-evidence-qualification-ba252d42660a41ce` for storage shape, views, and access boundaries.

The report passed canonical offline regeneration and exact controller-approved preflight with these bindings:

| Artifact | SHA-256 |
| --- | --- |
| Request | `f2d3221349d179ff38069398b606c572950ed144ed445a7e741fdcb48e54f829` |
| Report | `358d0a70043fb959b4b217b6251eab57852cb6a8abcef3b10ccea927adf0a710` |
| Completion | `fe653f5eab2ffb0ab80f654696e3453a384f96835ce6930792bd368a8e42cdf1` |
| Preflight | `4cfe4094a24c45d97633210e3290d2e8f99f6d35f3bdddc9021a9bf859fddb4b` |

Every performance report reopened these artifacts and reconstructed the canonical request, completion, report, and preflight receipts before timing.

## Workload Contract

Both workers generate identical destination and source vectors outside timing with the `splitmix64-xor-pair-v1` fixture. Destination word `k` uses SplitMix64 index `2*k`, source word `k` uses index `2*k+1`, and the input digest covers the exact little-endian destination bytes followed by the exact source bytes.

The exact aligned scales are 4,096, 262,144, and 16,777,216 bits. Their combined input sizes and digests are:

| Scale | Combined input bytes | Input digest |
| --- | ---: | --- |
| 4,096 bits | 1,024 | `d7fbfcc618ad7e3fd8a616be64f8b41949214afbbca6b58514d40fa5ea7ac498` |
| 262,144 bits | 65,536 | `7f2b0610db451711e538c7bea04e7cdbead09cc41c088ebfeb3da0788d53ca46` |
| 16,777,216 bits | 4,194,304 | `43fe5c79be45a459124be3bd00a45b65dbc886a6915fe19b3a173d37abc088ee` |

The timed Stim body contains one signal fence and `destination ^= source`. The timed Stab body contains one compiler fence and `BitVec::xor_assign`. Allocation, fixture generation, validation, hashing, final-state inspection, and output construction remain outside timing. Both workers count one semantic work item per destination bit visited per iteration.

After timing, both workers hash the complete final destination and unchanged source vectors, construct the same fourteen-field semantic output, and require exact work-count, input-byte, input-digest, and output-digest equality before a ratio can exist. Fixed odd, even, and accepted-maximum output digests are `0a654f5fe059e663b6f2f6ddea1ab61b4fb0b85927dde926da88de95caff58d4`, `b6623d77b32fe22daee0e7c30fcacdf3bc332854e7dcdf7d561a0da0325a3aa3`, and `451ffe13a031a8f9656ff3e3a89c1bd224e0f1cb94193456e32ff2cd854395b8`.

The generated inventory and runtime contract bind the adapter call-site digest `4098eadd7aa23e20cf98c155827ff69133a0a116b989659a747bdee71d677f87`, the isolated popcount comparator digest `0875f68e7e4a725f633c90895906604cc0ff942d7432c0e8c32f25c49e61bdda`, and the isolated dense-XOR comparator digest `542f96a2a88bb6a074a0b63b99f7be9c3dca29a6bb633986979abd0c4e66e00d`. The AArch64 adapter receipt records CMake's resolved `libstim` flags and treats pinned Stim headers as external through `-isystem`, while retaining `-Wextra -Werror` for adapter-owned code.

## Worker Preflight And Reproducibility

Every prepared worker pair executes the shared protocol vector, fixed odd and even popcount vectors, fixed odd and even dense-XOR vectors, both accepted maxima, the first unsupported circuit scale, an 83-item partial gate sweep, and the below-minimum, unaligned, and over-cap rejection classes for both bit workloads. Report schema version 19 stores all 30 actual receipts and binds them to both workers' source, build-fingerprint, and binary identities in contract-preflight digest `b3d87d2e9ea4fa016bf862cac594306c95f31fa3b4c2cc6edba58bdda1530ca8`.

`just bench::qualification-worker-reproducibility` rebuilt both workers twice from the clean revision and reproduced the same identities:

| Worker identity | SHA-256 |
| --- | --- |
| Stim source | `4098eadd7aa23e20cf98c155827ff69133a0a116b989659a747bdee71d677f87` |
| Stim build fingerprint | `3d504053c9ac5f493bb0b4b3e38382a38a01b4e5de20ab5e81bec86819011cdd` |
| Stim binary | `28eb6c52c7cce50ef30cfdd888a4b0c000547ea565a77aa9ecd8f7515841d44f` |
| Stab source | `8dcfecfb5d024ca4bc1c31ba46ebe0d0f7a1b37bfd08010bef86570e632ca774` |
| Stab build fingerprint | `1b6c32d7a3e31a6f8eb6cbb9a8ab1fa32265d0e772c2aad964cdbfd1a7cf50a0` |
| Stab binary | `a4f37ddd25bc141423d95374df8863864b9138e3b7e87f94bbb470211bd173f7` |

The exact two-iteration adapter smoke completed with matching semantic identities but is intentionally diagnostic because its tiny retained workload is dominated by process setup. The calibrated source reports below own the performance claim.

## AArch64 Timing Results

Every source report used three warmups, calibrated a common retained batch between 250 milliseconds and 2 seconds, retained raw interleaved paired samples, matched exact semantic output, and completed without a noise rerun.

| Scale | Tier | Pairs | Median ratio | Bootstrap 95% interval | Ratio rMAD | Outcome |
| --- | --- | ---: | ---: | --- | ---: | --- |
| 4,096 bits | Full | 9 | 0.453128 | [0.452692, 0.455814] | 0.000962 | Passed |
| 4,096 bits | Soak | 15 | 0.453819 | [0.452540, 0.454883] | 0.002820 | Passed |
| 262,144 bits | Full | 9 | 0.375074 | [0.374182, 0.377385] | 0.001310 | Passed |
| 262,144 bits | Soak | 15 | 0.374633 | [0.372574, 0.376410] | 0.004742 | Passed |
| 16,777,216 bits | Full | 9 | 0.553342 | [0.546551, 0.557229] | 0.007024 | Passed |
| 16,777,216 bits | Soak | 15 | 0.559828 | [0.557727, 0.561257] | 0.003752 | Passed |

Both family outcomes are `passed`, each with three passing measurements and no failed or noisy measurement. `qualification-regression` accepted every source report because both the median and confidence-interval upper bound remained below `1.25`.

## AArch64 Memory Observations

Setup and peak process RSS remain report-only observations for this slice. They do not establish a bounded-growth class or a Stim-relative memory gate; PQ6 must define and validate explicit cross-scale RSS and allocation slack before memory qualification.

| Scale | Tier | Stim peak RSS | Stab peak RSS | Peak Stab/Stim |
| --- | --- | ---: | ---: | ---: |
| 4,096 bits | Full | 3,403,776 bytes | 4,579,328 bytes | 1.345x |
| 4,096 bits | Soak | 3,407,872 bytes | 4,579,328 bytes | 1.344x |
| 262,144 bits | Full | 3,465,216 bytes | 4,648,960 bytes | 1.342x |
| 262,144 bits | Soak | 3,469,312 bytes | 4,640,768 bytes | 1.338x |
| 16,777,216 bits | Full | 7,610,368 bytes | 8,777,728 bytes | 1.153x |
| 16,777,216 bits | Soak | 7,606,272 bytes | 8,781,824 bytes | 1.155x |

The timed Stab mutation also passed the source-owned allocation-counter test with zero allocations. This is a local timed-phase invariant, not a process-memory parity claim.

## Authoritative Artifacts

| Evidence | Path | Report SHA-256 |
| --- | --- | --- |
| Small full | `target/benchmarks/qualification/perfq-m5-simd-bits-5d226c9-full-small` | `44d9d87a1352951a0665f52451ad9ba6a8aec80e96bc9d4d46e5e1d2294d98c9` |
| Medium full | `target/benchmarks/qualification/perfq-m5-simd-bits-5d226c9-full-medium` | `92bc28bf743db9e4051610a100376b48604562318e589e48119df86fd86010ca` |
| Large full | `target/benchmarks/qualification/perfq-m5-simd-bits-5d226c9-full-large` | `f86e48df45118c6986b7dc6357a312b384c90982395bb2a9e3122b95cbdb722a` |
| Small soak | `target/benchmarks/qualification/perfq-m5-simd-bits-5d226c9-soak-small` | `104166269a3ac0c7b10397dedf13f2c103ea070063e04b69423da4d8b73a1b0b` |
| Medium soak | `target/benchmarks/qualification/perfq-m5-simd-bits-5d226c9-soak-medium` | `ec4e34cae34847f0aeeafae86f9532720908788e984f28b148eb42b2c51234f4` |
| Large soak | `target/benchmarks/qualification/perfq-m5-simd-bits-5d226c9-soak-large` | `98325cd0e03ecb997be272d0e6ed1bfe771a704addc655df54e39249993200c5` |
| AArch64 full rollup | `target/benchmarks/qualification/perfq-m5-simd-bits-5d226c9-aarch64-full-rollup` | `6988b540c312ff76a3ceed854c1ab8b4f00f577f0beab7dc583bf97dc843d8f0` |
| AArch64 soak rollup | `target/benchmarks/qualification/perfq-m5-simd-bits-5d226c9-aarch64-soak-rollup` | `ae6bf256bc8c857cdd3ef231ff79733ce4740dd2d9a759e4afadf455d0369763` |

The full and soak rollup preflights are `612390bcc8ccd2981d0d570bb557dbf24a5ffef70fafc22aa8403b999e915820` and `616fc0db05efeb9116d91f2a0d65164fd337104ea7b1fcaaae8c7dc385b6f15d`. Both rollups passed offline replay and bind every required scale, one architecture and tier, the exact correctness and performance inventory digests, one runtime contract, and one worker-bound canonical preflight identity.

## Closure Command Record

The following exact commands produced or independently replayed the source-owned closure evidence from clean revision `5d226c94ece70f96d0b771f9c8cde7464ccd261b`:

```sh
just qualification::correctness-run --tier full --case cq-evidence-qualification-b1530dc4e48e942d --case cq-evidence-qualification-ba252d42660a41ce --out target/qualification/pq2-m5-simd-bits-full-5d226c9
just qualification::correctness-report --out target/qualification/pq2-m5-simd-bits-full-5d226c9
just qualification::correctness-preflight --out target/qualification/pq2-m5-simd-bits-full-5d226c9 --case cq-evidence-qualification-b1530dc4e48e942d --case cq-evidence-qualification-ba252d42660a41ce --request-sha256 f2d3221349d179ff38069398b606c572950ed144ed445a7e741fdcb48e54f829 --completion-sha256 fe653f5eab2ffb0ab80f654696e3453a384f96835ce6930792bd368a8e42cdf1
just bench::qualification-worker-reproducibility
just bench::qualification-probe --group pq2-simd-bits-xor-adapter-smoke --iterations 2 --work-items 262144
just bench::qualification-run --group PERFQ-M5-SIMD-BITS --scale small --tier full --correctness-out target/qualification/pq2-m5-simd-bits-full-5d226c9 --correctness-request-sha256 f2d3221349d179ff38069398b606c572950ed144ed445a7e741fdcb48e54f829 --correctness-completion-sha256 fe653f5eab2ffb0ab80f654696e3453a384f96835ce6930792bd368a8e42cdf1 --out target/benchmarks/qualification/perfq-m5-simd-bits-5d226c9-full-small
just bench::qualification-run --group PERFQ-M5-SIMD-BITS --scale medium --tier full --correctness-out target/qualification/pq2-m5-simd-bits-full-5d226c9 --correctness-request-sha256 f2d3221349d179ff38069398b606c572950ed144ed445a7e741fdcb48e54f829 --correctness-completion-sha256 fe653f5eab2ffb0ab80f654696e3453a384f96835ce6930792bd368a8e42cdf1 --out target/benchmarks/qualification/perfq-m5-simd-bits-5d226c9-full-medium
just bench::qualification-run --group PERFQ-M5-SIMD-BITS --scale large --tier full --correctness-out target/qualification/pq2-m5-simd-bits-full-5d226c9 --correctness-request-sha256 f2d3221349d179ff38069398b606c572950ed144ed445a7e741fdcb48e54f829 --correctness-completion-sha256 fe653f5eab2ffb0ab80f654696e3453a384f96835ce6930792bd368a8e42cdf1 --out target/benchmarks/qualification/perfq-m5-simd-bits-5d226c9-full-large
just bench::qualification-run --group PERFQ-M5-SIMD-BITS --scale small --tier soak --correctness-out target/qualification/pq2-m5-simd-bits-full-5d226c9 --correctness-request-sha256 f2d3221349d179ff38069398b606c572950ed144ed445a7e741fdcb48e54f829 --correctness-completion-sha256 fe653f5eab2ffb0ab80f654696e3453a384f96835ce6930792bd368a8e42cdf1 --out target/benchmarks/qualification/perfq-m5-simd-bits-5d226c9-soak-small
just bench::qualification-run --group PERFQ-M5-SIMD-BITS --scale medium --tier soak --correctness-out target/qualification/pq2-m5-simd-bits-full-5d226c9 --correctness-request-sha256 f2d3221349d179ff38069398b606c572950ed144ed445a7e741fdcb48e54f829 --correctness-completion-sha256 fe653f5eab2ffb0ab80f654696e3453a384f96835ce6930792bd368a8e42cdf1 --out target/benchmarks/qualification/perfq-m5-simd-bits-5d226c9-soak-medium
just bench::qualification-run --group PERFQ-M5-SIMD-BITS --scale large --tier soak --correctness-out target/qualification/pq2-m5-simd-bits-full-5d226c9 --correctness-request-sha256 f2d3221349d179ff38069398b606c572950ed144ed445a7e741fdcb48e54f829 --correctness-completion-sha256 fe653f5eab2ffb0ab80f654696e3453a384f96835ce6930792bd368a8e42cdf1 --out target/benchmarks/qualification/perfq-m5-simd-bits-5d226c9-soak-large
just bench::qualification-report --input target/benchmarks/qualification/perfq-m5-simd-bits-5d226c9-full-small
just bench::qualification-regression --input target/benchmarks/qualification/perfq-m5-simd-bits-5d226c9-full-small
just bench::qualification-report --input target/benchmarks/qualification/perfq-m5-simd-bits-5d226c9-full-medium
just bench::qualification-regression --input target/benchmarks/qualification/perfq-m5-simd-bits-5d226c9-full-medium
just bench::qualification-report --input target/benchmarks/qualification/perfq-m5-simd-bits-5d226c9-full-large
just bench::qualification-regression --input target/benchmarks/qualification/perfq-m5-simd-bits-5d226c9-full-large
just bench::qualification-report --input target/benchmarks/qualification/perfq-m5-simd-bits-5d226c9-soak-small
just bench::qualification-regression --input target/benchmarks/qualification/perfq-m5-simd-bits-5d226c9-soak-small
just bench::qualification-report --input target/benchmarks/qualification/perfq-m5-simd-bits-5d226c9-soak-medium
just bench::qualification-regression --input target/benchmarks/qualification/perfq-m5-simd-bits-5d226c9-soak-medium
just bench::qualification-report --input target/benchmarks/qualification/perfq-m5-simd-bits-5d226c9-soak-large
just bench::qualification-regression --input target/benchmarks/qualification/perfq-m5-simd-bits-5d226c9-soak-large
just bench::qualification-rollup --group PERFQ-M5-SIMD-BITS --tier full --input target/benchmarks/qualification/perfq-m5-simd-bits-5d226c9-full-small --input target/benchmarks/qualification/perfq-m5-simd-bits-5d226c9-full-medium --input target/benchmarks/qualification/perfq-m5-simd-bits-5d226c9-full-large --out target/benchmarks/qualification/perfq-m5-simd-bits-5d226c9-aarch64-full-rollup
just bench::qualification-rollup-report --input target/benchmarks/qualification/perfq-m5-simd-bits-5d226c9-aarch64-full-rollup
just bench::qualification-rollup --group PERFQ-M5-SIMD-BITS --tier soak --input target/benchmarks/qualification/perfq-m5-simd-bits-5d226c9-soak-small --input target/benchmarks/qualification/perfq-m5-simd-bits-5d226c9-soak-medium --input target/benchmarks/qualification/perfq-m5-simd-bits-5d226c9-soak-large --out target/benchmarks/qualification/perfq-m5-simd-bits-5d226c9-aarch64-soak-rollup
just bench::qualification-rollup-report --input target/benchmarks/qualification/perfq-m5-simd-bits-5d226c9-aarch64-soak-rollup
```

The correctness run selected two cases and passed two, with no failure or deferral. Worker reproducibility produced identical sealed worker identities across both private builds. The adapter smoke matched exact input and output identities and remained diagnostic. All six source reports passed on their first attempt, all six offline report replays and regressions passed, and both three-scale rollups passed publication and offline replay.

## Audit And Review

The independent GPT-5.6/max milestone audit found no implementation, comparator-fidelity, schema, resource-boundary, M12-threshold, or performance-gate defect. It found that the command-level closure chain had not yet been recorded and that the performance plan, active goal, and `AGENTS.md` still described only four implemented groups or omitted the dense-XOR probe. The closure command record above and the synchronized documentation resolve those findings.

The audit also exposed two genuine under-specifications: the plan did not require a machine-readable receipt for the standalone closure-command sequence, and it did not define whether allocation instrumentation must cover both implementations and every scale. Both are now resolved in `docs/plans/milestone-spec-gaps.md`. Stab allocation instrumentation covers every source-owned dense-XOR scale and the accepted maximum, while completion receipt schema version 1 binds future machine-checkable closure sequences. Neither later resolution changes this historical AArch64 timing contract, retroactively broadens its allocation claim, or relabels it as completion-receipt-backed evidence.

The independent GPT-5.6/max full code review found no implementation, Stim-comparator, SIMD, hostile-input, resource-boundary, receipt, report or rollup, M12, or timing-gate defect. It found three documentation defects: older progress reports still called the preceding inventory and popcount evidence current, this report mislabeled the custom four-lane fixture digest as SHA-256, and it assigned allocation-free XOR evidence to the storage-and-access CQ owner instead of the complete-vector XOR owner. The PQ0, PQ1, CQ2, gate-hash, and popcount evidence notes now distinguish current dense-XOR evidence from historical inventories; the workload table now says `Input digest`; and the CQ bullets name the correct owners.

The review retained four residual risks without converting them into findings: the two logged under-specifications, the broad `Reworked` validation exemption for any future heterogeneous replacement, three qualification modules within 13 lines of the 1,200-line source threshold, and the explicitly unclaimed x86-64 and simultaneous-current CQ2 evidence. Subsequent source work resolved the first three risks by defining the allocation and completion-receipt contracts, adding structured exact replacement mappings, and splitting the large modules by invariant ownership. Clean hardened-controller revision `3f2f382627c8421de0a668819d467a9f252de20f` also supplies source-current CQ2 evidence; native x86-64 remains unclaimed.

A narrow follow-up milestone re-audit verified the command record, synchronized current and historical evidence notes, full-review resolutions, explicit claim boundary, and retained deferrals. It reported no remaining blocker or finding and marked the AArch64 fifth slice closure-complete.

## Remaining Work

1. Produce the same clean full and soak scale families and rollups on a controlled native Linux x86-64 host. No x86-64 timing conclusion is claimed here.
2. Select and implement the next finite dependency-ordered PQ2 runtime group with the same exact evidence discipline.
3. Define and validate explicit cross-scale RSS and allocation-growth slack in PQ6 before making a dense-XOR memory claim.
4. Qualify the remaining excluded bit-kernel phases separately before retiring their historical M12 guards; `not_zero` now has its own accepted PQ2 slice.

## Verification

Revision `5d226c94ece70f96d0b771f9c8cde7464ccd261b` passed workspace formatting, Clippy with warnings denied, all workspace tests, allocation-counter tests, correctness and performance inventory checks, deterministic regeneration checks, benchmark smoke, staged pre-commit policy, private-worker reproducibility, exact CQ report regeneration and preflight, the clean adapter probe, immediate offline replay and regression checks for all six source reports, and replay of both architecture rollups. No required process remains running.
