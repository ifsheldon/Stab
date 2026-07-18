# PQ2 SIMD-Word Popcount Qualification Progress Report

> Historical-inventory note, 2026-07-19: this report remains valid passing AArch64 evidence for its frozen performance inventory `877df12bf1b3d63da92289e22f117097cedbc20860d165c90b41554aa110263b`. Later product groups, migrations, and the reviewed Clifford contracts produced source-current digest `18f2b401f386ec3fb459a2d26497a96b881f03d70ca9089665323b1efb61eff4`. This report is not current-inventory evidence.

## Status

The fourth PQ2 product group, `PERFQ-M5-SIMD-WORD`, passes the exact `1.25x` timing gate at every full and soak scale on the controlled Linux AArch64 host as of 2026-07-16.

All six promotable `toggle-popcount` measurements passed on their initial attempt, with median Stab-to-Stim elapsed-time ratios from `0.488067x` to `0.545545x` and a worst bootstrap confidence-interval upper bound of `0.547441x`. For this exact toggle-plus-complete-vector-popcount workload, Stab used about 48.8 percent to 54.6 percent of pinned Stim's measured time, corresponding to roughly 1.83x to 2.05x the throughput.

This report closes the corrected SIMD-word popcount proving group on AArch64. It does not qualify the remaining bit-kernel operations, all remaining PQ2 groups, PQ2 as a whole, native Linux x86-64 execution, or PQ6 memory growth.

## Frozen Inputs

- Stab evidence revision: `56dfe7569c6da48ffe76bde18f21ff43095f029b`, with `local_modifications=false` before and after every correctness, performance, and rollup report.
- Stim baseline: v1.16.0 at `e2fc1eca7fd21684d433aa5f10f4504ea4860d07`.
- Correctness inventory: `ccb80eb4b660a375b59460c3b7fa03a810abd6f868735b566735378105db22b2`.
- Performance inventory: `877df12bf1b3d63da92289e22f117097cedbc20860d165c90b41554aa110263b`.
- Runtime group contract: `a57ed0baf8f8610913a61b5718074ea245fd8566bed55d60ed1dfce3e3c413ce`.
- Host profile: verified `linux-aarch64-controlled`, pinned to logical CPU 0 with the performance governor at 2,808,000 kHz and no host-policy violation.
- Rust toolchain: `nightly-2026-06-20` targeting `aarch64-unknown-linux-gnu` with the release profile.
- Profiler note: none. Every promotable report passed without a noisy or failed attempt.

## Correctness Preflight

The clean full report at `target/qualification/pq2-m5-simd-word-full-56dfe75` selected and passed these three exact prerequisites:

- `cq-evidence-qualification-5118006702599a45` for scalar-word popcount.
- `cq-evidence-qualification-b1530dc4e48e942d` for logical-vector popcount and tail handling.
- `cq-evidence-qualification-ba252d42660a41ce` for in-range bit access and mutation.

The report passed canonical offline regeneration with these bindings:

| Artifact | SHA-256 |
| --- | --- |
| Request | `9f993bd2e994da6b742cbc6e17db0b8c60d9769e9b5c7b1a109e488e2ed2e05c` |
| Report | `394aad16e388eae936f66ed11ab6a7813729e326eb6f31bf3fe05faeeb9434c1` |
| Completion | `45e67ac7ecfe6545d3f51c7a19fb62a20836fd263c505a577b6dca8502a4f5c5` |
| Preflight | `cf74dba7aa63bd82109e289eeadff582c5cf8c036d6d657fd57360b27dd0432c` |

Every performance report reopened these artifacts and reconstructed the canonical request, completion, report, and preflight receipts before timing.

## Workload Contract

Both workers generate identical little-endian SplitMix64 fixture words outside timing and use exact aligned scales of 4,096, 262,144, and 16,777,216 bits. The Stim adapter inherits CMake's resolved `libstim` flags and reproduces the pinned `simd_compat_popcnt` loop by toggling bit 300 and accumulating `ptr_simd[k].popcount()` for every architecture-dependent SIMD word. Stab toggles the same bit and applies `BitVec::popcount()` to the complete logical vector.

The timed region contains only one symmetric compiler fence, the bit toggle, complete-vector popcount, and wrapping checksum accumulation. Both workers construct the canonical eight-field output and its four-lane digest after timing. Exact input bytes, input digest, semantic work, and output digest must match before a ratio can be accepted.

The generated inventory and runtime contract bind the adapter call site digest `8f7bed25d8af3116f705574d2de88745e214c6b1845d8c8d6bb86b72241eacc3` and isolated comparator-loop digest `0875f68e7e4a725f633c90895906604cc0ff942d7432c0e8c32f25c49e61bdda`. The AArch64 CMake receipt recorded `-O3`, `-DNDEBUG`, `-std=gnu++20`, `-Wall`, `-Wpedantic`, `-fPIC`, and `-fno-strict-aliasing`; native x86-64 must independently prove its resolved machine flags before producing a ratio.

## Worker Preflight And Reproducibility

Every prepared worker pair executes the shared frozen protocol vector, odd and even popcount vectors, the actual 268,435,456-bit accepted maximum, the first unsupported circuit scale, an 83-item partial gate sweep, and below-minimum, unaligned, and over-cap popcount rejection probes. Report schema version 18 stores all 18 actual receipts and includes the six exact worker source, build-fingerprint, and binary digests in preflight digest `1ca66df740bbd373a914c1a71c8db68f23b82b6016ef00bdb8f037066ae4171c`.

`just bench::qualification-worker-reproducibility` rebuilt both workers twice from the clean revision and reproduced the same identities:

| Worker identity | SHA-256 |
| --- | --- |
| Stim source | `8f7bed25d8af3116f705574d2de88745e214c6b1845d8c8d6bb86b72241eacc3` |
| Stim build fingerprint | `e48d45cb6b77112bc343f4b6ceec189f8b5865e82c4615eb6064d8fede106794` |
| Stim binary | `bb7d3b89fe376d75687e7677d61fe4efc656b3172e3f5660d880f188eb1ac845` |
| Stab source | `95508f10edf5032794473414f503bee6754bfeb7049e5b5e6dc904294c7e234b` |
| Stab build fingerprint | `271807d17fe4c743664815a51c375883d879cc21d8aadf2d8ed8b6932430447d` |
| Stab binary | `4852eb84867e368bf44fa6db3dc8878a00a36842f3472abf699041323b205427` |

Offline report replay cross-checks the preflight's six typed identities against the report workers and rejects an otherwise valid preflight transplanted from another worker pair. The clean adapter probe processed 524,288 bit visits with exact matching input and output digests and produced a diagnostic ratio of `0.523364x`; the source reports below own the performance claim.

## AArch64 Timing Results

Every source report used three warmups, calibrated a common retained batch between 250 milliseconds and 2 seconds, retained raw interleaved paired samples, matched exact semantic output, and completed without a noise rerun.

| Scale | Tier | Pairs | Median ratio | Bootstrap 95% interval | Ratio rMAD | Outcome |
| --- | --- | ---: | ---: | --- | ---: | --- |
| 4,096 bits | Full | 9 | 0.545294 | [0.544011, 0.547441] | 0.001882 | Passed |
| 4,096 bits | Soak | 15 | 0.545545 | [0.544537, 0.546902] | 0.002486 | Passed |
| 262,144 bits | Full | 9 | 0.488067 | [0.487949, 0.493964] | 0.000276 | Passed |
| 262,144 bits | Soak | 15 | 0.488476 | [0.488000, 0.488709] | 0.000775 | Passed |
| 16,777,216 bits | Full | 9 | 0.509818 | [0.508429, 0.509989] | 0.000340 | Passed |
| 16,777,216 bits | Soak | 15 | 0.509063 | [0.508758, 0.509489] | 0.000837 | Passed |

Both family outcomes are `passed`, each with three passing measurements and no failed or noisy measurement. `qualification-regression` accepted every source report because both the median and confidence-interval upper bound remained below `1.25`.

## AArch64 Memory Observations

Setup and peak process RSS remain report-only observations for this slice. They do not establish a bounded-growth class or a Stim-relative memory gate; PQ6 must define and validate explicit cross-scale RSS and allocation slack before memory qualification.

| Scale | Stim peak RSS | Stab peak RSS | Peak Stab/Stim |
| --- | ---: | ---: | ---: |
| 4,096 bits | 3,407,872 bytes | 4,386,816 bytes | 1.287x |
| 262,144 bits | 3,436,544 bytes | 4,419,584 bytes | 1.286x |
| 16,777,216 bits | 5,505,024 bytes | 6,488,064 bytes | 1.179x |

The largest fixture adds about 2 MiB of input state to both workers. This report preserves the observed resident-memory values without converting them into an unplanned acceptance claim.

## Authoritative Artifacts

| Evidence | Path | Report SHA-256 |
| --- | --- | --- |
| Small full | `target/benchmarks/qualification/perfq-m5-simd-word-56dfe75-full-small` | `2dacc0579ddc347aba86c113ad21842f5dd83e70ae47afb496229467e65c35f4` |
| Medium full | `target/benchmarks/qualification/perfq-m5-simd-word-56dfe75-full-medium` | `16ea162fb479275c30da716568f1a3f3ff9f41821c2b7b8c49fb96b90ef562c7` |
| Large full | `target/benchmarks/qualification/perfq-m5-simd-word-56dfe75-full-large` | `26b6d0a3dfb947168097fc93ba2b2b41711ba096dfdd685416f4b1d5c8aa38d3` |
| Small soak | `target/benchmarks/qualification/perfq-m5-simd-word-56dfe75-soak-small` | `e5732df8022e6afc98262742f40c5f7f2c9518248c05f69d6545350e1954e8f0` |
| Medium soak | `target/benchmarks/qualification/perfq-m5-simd-word-56dfe75-soak-medium` | `5967057e5f7d943574612d796f8b6434b2b829cdc2aa1b38deba4a1bd8f360ff` |
| Large soak | `target/benchmarks/qualification/perfq-m5-simd-word-56dfe75-soak-large` | `41bcf36a65e35200b0a940acddeee63df46f73e048bd7d02a30f00867a1296b1` |
| AArch64 full rollup | `target/benchmarks/qualification/perfq-m5-simd-word-56dfe75-aarch64-full-rollup` | `6c68a3813c147e3b2e940b221a7fc9fd6ea7dd40815502d31ea605401481568a` |
| AArch64 soak rollup | `target/benchmarks/qualification/perfq-m5-simd-word-56dfe75-aarch64-soak-rollup` | `0a260e0bcbda4772321fde6713244237f5e4bf89807fba1cb2262a717b1f738f` |

The full and soak rollup preflights are `d84e43405147fbfeed264a5449565fac92017b90a0a8bd8be6f91a606558ba3b` and `0253c4d0d3c1e19b9c7f91040a564b2f1784eaf659a7315b8f3a2e53c4a1dfaa`. Both rollups passed offline replay and bind every required scale, one architecture and tier, the exact correctness and inventory digests, one runtime contract, and one worker-bound canonical preflight identity.

## Audit And Review

Milestone audit rejected the first clean reports from revision `38a2d5eab2fec3211eb9466899c6afd0ba91c4ca` because the Stim adapter used `simd_bits::popcnt()` instead of the exact architecture-dependent SIMD-word loop, the semantic output omitted three fixture-fingerprint lanes, and output construction crossed the timing boundary. Those reports remain diagnostic history only.

The first corrected clean reproducibility run then exposed a wrong frozen protocol output literal. Both independent workers produced the same canonical output, so the literal was corrected and moved into one shared vector contract used by invocation and worker regression tests. A later GPT-5.6/max full-code-review pass found that the 18 receipts were not bound bidirectionally to the exact worker binaries and could be transplanted between worker pairs, and it found that `report.rs` crossed the 1,200-line threshold after the fix. Report schema version 18 now includes all six worker digests in the preflight material, an adversarial regression rejects a refingerprinted cross-worker transplant, and worker-contract validation lives in a focused submodule that leaves `report.rs` at 1,191 lines.

The GPT-5.6/max closure review reported no remaining correctness or acceptance finding. The final milestone audit maps every fourth-slice task and test requirement to the clean correctness report, reproducibility output, adapter probe, six source reports, six regression replays, and two replayed rollups. No threshold, comparator, output obligation, evidence count, or acceptance rule was relaxed.

## Remaining Work

1. Produce the same clean full and soak scale families and rollups on a controlled native Linux x86-64 host, including proof that the adapter inherits CMake's resolved machine flags. No x86-64 timing conclusion is claimed here.
2. Select and implement the next finite dependency-ordered PQ2 runtime group with the same exact evidence discipline.
3. Define and validate explicit cross-scale RSS and allocation-growth slack in PQ6 before making a popcount memory claim.

The separate 271-parent CQ2 checkpoint is source-current at clean hardened-controller revision `3f2f382627c8421de0a668819d467a9f252de20f`.

## Verification

Revision `56dfe7569c6da48ffe76bde18f21ff43095f029b` passed workspace formatting, Clippy with warnings denied, all workspace tests, correctness and performance inventory checks, deterministic regeneration checks, benchmark smoke, staged pre-commit policy, private-worker reproducibility, exact CQ report regeneration, the clean adapter probe, immediate offline replay and regression checks for all six source reports, and replay of both architecture rollups. No required process remains running.
