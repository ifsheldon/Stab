# PQ2 Gate Name Hash Qualification Progress Report

> Historical-evidence note, 2026-07-16: this report remains authoritative for gate-name hashing at performance inventory `1cc0be5c8c0a37c98bd4fb56f331dd6964e6f53e56b328b9564be507cbf88a42`. Later popcount, dense-XOR, `not_zero`, exact replacement-mapping, `not_zero` anti-elision, and bounded wide-ratio policy changes produced source-current performance digest `315c1d985af62f08068cd273a0e06399aaa7dd85ff3009309e284bc46aaaaf3d`. The gate reports below are historical and are not relabeled as simultaneous current-inventory evidence.

## Status

The third PQ2 product group, `PERFQ-M4-GATE-LOOKUP`, passes the exact `1.25x` timing gate at every full and soak scale on the controlled Linux AArch64 host as of 2026-07-16.

All six promotable `hash-all-names` measurements pass on their initial attempt, with median ratios from `0.931886x` to `0.932764x` and a worst bootstrap confidence-interval upper bound of `0.933289x`. Stab takes about 6.7 percent to 6.8 percent less measured time than pinned Stim for the exact complete-table hashing workload.

This report closes only the gate-name-hash proving group on AArch64. It does not qualify alias lookup, lowercase lookup, invalid-name rejection, the broader gate contract, all remaining PQ2 runtime groups, PQ2 on AArch64, native Linux x86-64 evidence, or PQ6 memory growth.

## Frozen Inputs

- Stab evidence revision: `c76b7071fc4d7ac358ef3a2fffc053ea675bd05f`, with `local_modifications=false` before and after every correctness and performance report.
- Stim baseline: v1.16.0 at `e2fc1eca7fd21684d433aa5f10f4504ea4860d07`.
- Correctness inventory: `ccb80eb4b660a375b59460c3b7fa03a810abd6f868735b566735378105db22b2`.
- Performance inventory: `1cc0be5c8c0a37c98bd4fb56f331dd6964e6f53e56b328b9564be507cbf88a42`.
- Runtime group contract: `1eb0c9eb379cb013fc94f20ace84ddd449e4a3313ffafc053a764c077831c4ec`.
- Host profile: verified `linux-aarch64-controlled`, pinned to logical CPU 0 with the performance governor at 2,808,000 kHz and no host-policy violation.
- Profiler note: none. Every promotable report passed without a noisy or failed attempt.

## Correctness Preflight

The clean full report at `target/qualification/pq2-m4-gate-hash-full-c76b707` selected and passed the one exact prerequisite, `cq-evidence-qualification-bd20a013e903a05f`. Its selector freezes the ordered 82-entry Stim v1.16.0 canonical-name table including `NOT_A_GATE` and every per-name hash value. Lowercase, aliases, and invalid-name rejection remain under their separate lookup owner.

The report passed canonical offline regeneration and exact dependent preflight with these bindings:

| Artifact | SHA-256 |
| --- | --- |
| Request | `6bd6f364157be26b617715386150a85dcba3a68afe214cf3172b8244f523425b` |
| Report | `0eba4682881b1a7708de9cbae799aa07b677792204e87634c568e051d4d52822` |
| Completion | `d101dc7fb54c7157171a33551a3285079701c01b962ef8c2147288694c0aa3f8` |
| Preflight | `efe8e20b57b2863fa416fd14ab928fb09e2dbe690c538c83885a508dc62ee500` |

Every performance report reopens these artifacts and reconstructs their canonical receipts before timing.

## Workload Contract

Both workers prepare runtime-owned copies of the 82 names outside timing. Each timed iteration performs only complete table sweeps, places one symmetric compiler fence before each sweep, hashes every name with `gate_name_to_hash` or `Gate::stim_name_hash`, and accumulates a wrapping checksum. The parent compares exact work count, checksum, iteration count, work-item count, and an untimed order-sensitive name-and-hash table fingerprint before accepting a ratio.

The source-owned scales are 82, 5,248, and 335,872 hashes, corresponding to 1, 64, and 4,096 complete sweeps. All bind zero input bytes and the exact empty-input digest. The worker reproducibility command also invokes both sealed workers with 83 work items, enables the start barrier, supplies no barrier input, and requires the exact partial-sweep error before either process can reach the barrier.

## Reproducible Workers

`just bench::qualification-worker-reproducibility` rebuilt both private workers twice from the clean evidence revision, verified their live protocol identities, proved exact circuit-cap and partial-gate-sweep rejection before the start barrier, and produced identical identities across both isolated builds.

| Worker identity | SHA-256 |
| --- | --- |
| Stim source | `50fa28efa00f061088dae8ecda0ff90ec810e3a1dc96ef13dfb98653f44fac99` |
| Stim build fingerprint | `5c711adfce8ebb5dd3364901c4363e7b16bb21882d6364a1511f82529463263c` |
| Stim binary | `77568768d1d2f3babf932681380d5cf84bbcc1e9344f7858f7c5398979570db8` |
| Stab source | `1ca94294369e5ddde71f3a643002af550d45ac0c1291038ccdbcd058a4ae77bb` |
| Stab build fingerprint | `7844e17562cc90f346d761557440705132c908e1565b2fdbdc4d602704e8a5ec` |
| Stab binary | `d2c20251fd637edaaacebe30a5a2b8b4346f4547b17782c50a6d1b8aa671f6e0` |

The clean adapter probe processed 20,992 hashes with an exact matching output digest and a diagnostic ratio of `0.938626x`. The probe is supporting smoke evidence only; the results below own the performance claim.

## AArch64 Timing Results

Every source report used three warmups, calibrated a common batch between 250 milliseconds and 2 seconds, retained raw interleaved paired samples, matched exact semantic output, and completed without a noise rerun.

| Scale | Tier | Pairs | Median ratio | Bootstrap 95% interval | Ratio rMAD | Outcome |
| --- | --- | ---: | ---: | --- | ---: | --- |
| 82 hashes | Full | 9 | 0.932013 | [0.931460, 0.932919] | 0.000593 | Passed |
| 82 hashes | Soak | 15 | 0.931886 | [0.931542, 0.932582] | 0.000496 | Passed |
| 5,248 hashes | Full | 9 | 0.932764 | [0.932305, 0.933289] | 0.000193 | Passed |
| 5,248 hashes | Soak | 15 | 0.932764 | [0.932695, 0.932849] | 0.000091 | Passed |
| 335,872 hashes | Full | 9 | 0.932162 | [0.931810, 0.932464] | 0.000294 | Passed |
| 335,872 hashes | Soak | 15 | 0.932222 | [0.932067, 0.932433] | 0.000224 | Passed |

Both family outcomes are `passed`, with three passing measurements and no failed or noisy measurement. `qualification-regression` accepts every source report because both the median and confidence-interval upper bound remain below `1.25`.

## AArch64 Memory Observations

Setup and peak process RSS are report-only observations for this slice. They do not establish a bounded-growth class or a Stim-relative memory gate; PQ6 must define and validate explicit cross-scale RSS and allocation slack before memory qualification.

| Scale | Stim setup and peak RSS | Stab setup RSS | Stab peak RSS | Peak Stab/Stim |
| --- | ---: | ---: | ---: | ---: |
| 82 hashes | 3,411,968 bytes | 4,259,840 bytes | 4,390,912 bytes | 1.287x |
| 5,248 hashes | 3,411,968 bytes | 4,247,552 bytes | 4,378,624 bytes | 1.283x |
| 335,872 hashes | 3,407,872 bytes | 4,251,648 bytes | 4,382,720 bytes | 1.286x |

These soak observations are nearly flat over the three semantic scales, but this report does not turn that observation into a machine-checked acceptance claim.

## Authoritative Artifacts

| Evidence | Path | Report SHA-256 |
| --- | --- | --- |
| Small full | `target/benchmarks/qualification/perfq-m4-gate-lookup-c76b707-full-small` | `7a7eafccd59d085fcb216371e75a7707732d5c50793651429afccccc0d48c992` |
| Medium full | `target/benchmarks/qualification/perfq-m4-gate-lookup-c76b707-full-medium` | `6434aee8054afbd3889abd9489dba40ab226721ac7b93ec39eccb6651f4069aa` |
| Large full | `target/benchmarks/qualification/perfq-m4-gate-lookup-c76b707-full-large` | `1bbd191958efc50c03a3c02aff1a1a22d8b73227b723c2243455134318e061f4` |
| Small soak | `target/benchmarks/qualification/perfq-m4-gate-lookup-c76b707-soak-small` | `e74e493f9fee819a8589c10a9153a6377c2855ecbd88e1f9ffa0c607a7dff9a9` |
| Medium soak | `target/benchmarks/qualification/perfq-m4-gate-lookup-c76b707-soak-medium` | `73540a52a83dfd03c7cd5364b9636eb45b871bf276ddebf5ea3657f921daded4` |
| Large soak | `target/benchmarks/qualification/perfq-m4-gate-lookup-c76b707-soak-large` | `cc43ca7e0d22a43230e0854bb5dd90e139cbb9f6755a88cd5825a73b78663f08` |
| AArch64 full rollup | `target/benchmarks/qualification/perfq-m4-gate-lookup-c76b707-full-aarch64-rollup` | `4a304e459a9fb30c9b454c5e01cde3ac0aad3da375a37f6c9520f741ce0a4ab6` |
| AArch64 soak rollup | `target/benchmarks/qualification/perfq-m4-gate-lookup-c76b707-soak-aarch64-rollup` | `4ba532bb34027e73be1d0218bcd7f7bb77273aabb2776c684c668c2fccd5f35f` |

The full and soak rollup preflights are `d135bb322bd78337a26a8c81052dcca9b9ace941a303f76a8770caad64ae6513` and `50f115a8f1ba90cbce194a93a31bbeab70c796e977ef96536058dc5db0396817`. Both rollups passed offline replay and bind every required scale, one architecture and tier, the exact correctness and inventory digests, one runtime contract, and one six-digest worker identity.

## Audit And Review

Milestone audit found three issues before evidence: the exact CQ owner also asserted lowercase behavior owned elsewhere, the C++ partial-sweep rejection lacked sealed-worker execution evidence, and the milestone implied bounded memory without a machine-checked growth rule. The fixes narrowed the CQ owner, added exact pre-barrier rejection for both sealed workers, and deferred cross-scale memory acceptance to PQ6. Audit closure then found and fixed one stale root operational instruction.

The GPT-5.6/max full-code-review pass found one stale 59-parent gate count in the CQ2 progress narrative. That narrative now records the exact new parent and 60-parent total. The closure review reported no remaining P0 through P2 finding. No threshold, comparator, output obligation, evidence count, or acceptance rule was relaxed.

A final evidence audit then found that the PQ2 section-level status still described gate-hash execution as pending even though the same plan recorded this completed evidence. The status now names the clean `c76b7071fc4d7ac358ef3a2fffc053ea675bd05f` AArch64 evidence and rollups. This documentation-only P2 finding is closed; no executable contract or result changed.

## Remaining Work

1. Produce the same clean full and soak scale families and rollups on a controlled native Linux x86-64 host. No x86-64 timing conclusion is claimed.
2. Preserve the completed clean `PERFQ-M5-SIMD-WORD` evidence and select the next finite PQ2 runtime group with the same exact evidence discipline.
3. Rerun the complete 271-parent CQ2 family before claiming current-digest all-domain correctness execution.
4. Define and validate explicit cross-scale RSS and allocation-growth slack in PQ6 before making a gate-hash memory claim.

## Verification

Revision `c76b7071fc4d7ac358ef3a2fffc053ea675bd05f` passed workspace formatting, Clippy with warnings denied, all workspace tests, correctness and performance inventory checks, deterministic regeneration checks, selector validation, benchmark smoke, staged pre-commit policy, private-worker reproducibility, exact CQ report regeneration and preflight, the clean adapter probe, immediate offline replay and regression checks for all six source reports, and replay of both architecture rollups. No required process remains running.
