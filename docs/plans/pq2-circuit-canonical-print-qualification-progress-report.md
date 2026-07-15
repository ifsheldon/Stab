# PQ2 Circuit Canonical Print Qualification Progress Report

> Current-evidence note, 2026-07-16: this report remains historical passing AArch64 evidence at performance inventory `f3c4009044b0bafcd877f76798c7f4f08c475c0877b85f68d22ae0449e3ddb8f`. Graduating `PERFQ-M4-GATE-LOOKUP` changed the global performance inventory and shared worker, and its exact hash owner later changed the correctness inventory; no claim in this report has been silently promoted to either current inventory.

## Status

The second PQ2 product group, `PERFQ-M4-CIRCUIT-CANONICAL-PRINT`, passes the unchanged `1.25x` timing gate at every full and soak scale on the controlled Linux AArch64 host as of 2026-07-15.

All six promotable `serialize` measurements pass with median ratios from `0.372912x` to `0.376075x` and a worst bootstrap confidence-interval upper bound of `0.398775x`. On this exact flat, argument-free fixture family, Stab takes about 37 percent of pinned Stim's canonical-serialization time.

This report closes the second executable PQ2 slice on AArch64. It does not complete `PERF-CIRCUIT-MODEL`, the remaining PQ2 runtime groups, PQ2 on AArch64, or the separately required native Linux x86-64 evidence.

## Frozen Inputs

- Stab evidence revision: `ba70a52025fdd4122ac97cec263725b2ec56e431`, with `local_modifications=false` in every correctness and performance report.
- Stim baseline: v1.16.0 at `e2fc1eca7fd21684d433aa5f10f4504ea4860d07`.
- Correctness inventory: `b80801fea6eae550feecf40489259de56123f6f3331b747d52c323d576fd0285`.
- Performance inventory: `f3c4009044b0bafcd877f76798c7f4f08c475c0877b85f68d22ae0449e3ddb8f`.
- Runtime group contract shared by the parser and printer reports: `e0e00907862b2fa59700f339f318bb1a15c6f4f0bbb0641caa7ad56f195c86a2`.
- Host profile: verified `linux-aarch64-controlled`, pinned to logical CPU 0 with the performance governor at 2,808,000 kHz and no host-policy violation.

## Correctness Preflight

The clean report at `target/qualification/pq2-m4-parse-print-full-ba70a52` selected and passed all three shared parser and printer prerequisites. Canonical printing depends on these exact owners:

- `cq-evidence-qualification-e660819ae9a223c6`
- `cq-evidence-qualification-ef933925fb901877`

The combined report passed canonical offline regeneration and exact dependent preflight with these bindings:

| Artifact | SHA-256 |
| --- | --- |
| Request | `23e1f36f280a101d39213d403c724fe5d498ffe428bc309796f3bbeae20bd703` |
| Report | `5dcb3134e2fad22de389823f232704bb2b339133a4726f5c59234ed8684a69c0` |
| Completion | `05037f849b9b317c9c2cc61653f3b979e742346a91b12237d4e1958d961cad8a` |
| Preflight | `68c05127c48674aae13d3908355e335a86c9ec89c4f9755d38308e93c81e72dd` |

Every performance report reopens the correctness artifacts, reconstructs their canonical receipts, and proves that its source-owned case set passed before timing.

## Reproducible Workers

`just bench::qualification-worker-reproducibility` rebuilt both private workers twice from the clean evidence revision, verified their live protocol identities, and proved exact pre-barrier rejection of the first unsupported circuit size.

| Worker identity | SHA-256 |
| --- | --- |
| Stim source | `0efaa28c8b44616df925b6731b04821b536c83c692e3ae25dd7623d58c2be187` |
| Stim build fingerprint | `7c8d9c6bd06efb54845543e1df0ef03aa3ce3fc2aaaba55ec327efc7ef9fb3b4` |
| Stim binary | `2403e3ea9d1cdd56ccab2b19ed9483b4d860426117d9ab6b64bb7031822b6999` |
| Stab source | `9b92a8ac92a014fd1ebae619f386eb1664b5ca90fc2feb0d3eab7318c08f8b99` |
| Stab build fingerprint | `dd3ed1465aebcfc153070e86172b7fa0badb57c0197f180fc819e929e935f880` |
| Stab binary | `3772ceaaff4d2e5e802dc014767386feb7146ddfb203ab96d08e439bf2bf8de2` |

## AArch64 Timing Results

Both workers parse the same exact fixture once before the start barrier and time only repeated canonical serialization, including output allocation and destruction. Every produced string is consumed, the final output is retained, and the exact output digest is compared outside timing after normalizing only Stab's one terminal newline.

| Scale | Tier | Pairs | Median ratio | Bootstrap 95% interval | Ratio rMAD | Outcome |
| --- | --- | ---: | ---: | --- | ---: | --- |
| 64 instructions | Full | 9 | 0.375252 | [0.368819, 0.398775] | 0.008239 | Passed |
| 64 instructions | Soak | 15 | 0.373080 | [0.369450, 0.373375] | 0.009647 | Passed |
| 4,096 instructions | Full | 9 | 0.372912 | [0.371740, 0.380652] | 0.003593 | Passed |
| 4,096 instructions | Soak | 15 | 0.376075 | [0.372705, 0.383190] | 0.009808 | Passed |
| 65,536 instructions | Full | 9 | 0.373970 | [0.372570, 0.385050] | 0.003336 | Passed |
| 65,536 instructions | Soak | 15 | 0.375580 | [0.373864, 0.380583] | 0.008461 | Passed |

Both family outcomes are `passed`, with three passing measurements and no failed or noisy measurement. `qualification-regression` accepts every source report because both the median and confidence-interval upper bound remain below `1.25`.

## AArch64 Memory Results

Memory is separate observational evidence and does not supply timing evidence. Stab has higher process peak RSS for canonical serialization at all three scales on this host. This is visible work for later resource and memory qualification, not a reason to weaken or reinterpret the timing result.

| Scale | Tier | Stim peak RSS | Stab peak RSS | Stab/Stim |
| --- | --- | ---: | ---: | ---: |
| 64 instructions | Soak | 3,407,872 bytes | 4,509,696 bytes | 1.323x |
| 4,096 instructions | Soak | 3,809,280 bytes | 5,029,888 bytes | 1.320x |
| 65,536 instructions | Soak | 9,400,320 bytes | 12,668,928 bytes | 1.348x |

These process-RSS results do not contradict the focused Stab allocation regressions: `Circuit::to_stim_string` uses one output allocation for the qualification cycle and one exactly sized output allocation for a 4,096-argument float-heavy circuit, while `write_stim_file` allocation counts do not grow between one and 4,096 targets.

## Authoritative Artifacts

| Evidence | Path | Report SHA-256 |
| --- | --- | --- |
| Small full | `target/benchmarks/qualification/pq2-m4-print-ba70a52-small-full` | `911b26280f239dfc367f58c1e6f671f7332af8e8c1812339cb637cf142b2ed88` |
| Medium full | `target/benchmarks/qualification/pq2-m4-print-ba70a52-medium-full` | `cbc09b854612114d8efc07f28eecdac4f869412f26d6fd76c8bc7d84760891c1` |
| Large full | `target/benchmarks/qualification/pq2-m4-print-ba70a52-large-full` | `d5942a806055c2b1490b19def0207e026cfa57498d8f5ee7433a696553fc23a4` |
| Small soak | `target/benchmarks/qualification/pq2-m4-print-ba70a52-small-soak` | `66b7c4892ac43f1e65cb59a481097ba9f7959205712599b142f248f22e16e7ec` |
| Medium soak | `target/benchmarks/qualification/pq2-m4-print-ba70a52-medium-soak` | `b256a4720c04663b63b2b77394757195d2d5ef3dec4ccac3d0b953e70583caea` |
| Large soak | `target/benchmarks/qualification/pq2-m4-print-ba70a52-large-soak` | `41f4e51fc3f3f5106e088b463b0c8ebae5d0e4c1179b11911f7d80bdf2152dce` |
| AArch64 full rollup | `target/benchmarks/qualification/pq2-m4-print-ba70a52-aarch64-full-rollup` | `470eb4e75b61f131ada6618733d57c2979eb94e7910ad6b0947181f89c5fbaff` |
| AArch64 soak rollup | `target/benchmarks/qualification/pq2-m4-print-ba70a52-aarch64-soak-rollup` | `906925ed30eb30f421708b2e633e7942a6e60c2a555e8469083d55d990bc945e` |

The full and soak rollup preflights are `87bead397ef0ef13c4f8bbcd262c9b4d3770c496629ab929ebe1b3dd920319c0` and `1fd05a3b999d87437cfa82838d4fbc32e1d99ab013029060a21b9e7f5ef3a6e9`. Both rollups passed offline replay and bind every required scale, one architecture and tier, the exact correctness and inventory digests, one runtime contract, and one six-digest worker identity.

## Implementation And Migration Closure

The first diagnostic probe exposed an `8.483215x` implementation slowdown. The printer now computes its exact output capacity, formats floats and numeric targets through bounded stack buffers, writes file targets without per-target `String` allocation, and preserves Stim's empty, tagged-empty, and nested-empty `REPEAT` framing in both string and writer paths.

The focused resource regressions cover the qualification cycle, 4,096 float arguments, and public file writing across one and 4,096 targets. Exact CQ evidence remains semantic rather than allocation-coupled.

The legacy `m4-circuit-canonical-print` row is now `non-primary-report-only` and `superseded`. It has been removed from the M12 beta and timing-regression waiver files and from the legacy memory baseline. The symmetric source-owned group in this report is the sole Stim-relative canonical-print timing gate.

## Audit And Review

The milestone audit found the second executable slice complete on AArch64: tasks 1 through 10 have source-owned runtime, correctness, timing, memory, regression, migration, reproducibility, and rollup evidence. No threshold, comparator, output obligation, or acceptance rule was relaxed.

The GPT-5.6/max review findings concerning empty-repeat bytes, float-heavy capacity, per-target writer allocations, and allocation-policy ownership were fixed before the final evidence run. Source files remain below the project's 1,200-line threshold. Programmatically constructed circuits deeper than the parser's 256-level admission limit can still recurse during printing and destruction; this is now an explicit CQ6/PQ6 public resource contract, not evidence claimed by this flat timing group. The review also suggested deleting a one-assertion Clap registration smoke as low-value; it remains a narrow internal CLI-boundary guard and is not treated as product or qualification evidence because stronger worker invocation and source-report execution cover the behavioral contract.

## Remaining Work

1. Produce the same clean full and soak scale families and rollups on a controlled native Linux x86-64 host. No x86-64 timing conclusion is claimed.
2. Qualify the remaining PQ2 runtime groups for circuit and DEM models, result I/O, gates, bit kernels, and stabilizer algebra. These two M4 groups do not close broader `PERF-CIRCUIT-MODEL` ownership.
3. Qualify recursive programmatic-circuit resource behavior and the shared one-million-instruction accepted boundary in CQ6/PQ6.
4. Treat float-heavy, repeat-heavy, and public file-output performance as unclaimed until separate equivalent-work runtime groups select those workload shapes.

## Verification

The evidence revision passed workspace format, Clippy, tests, correctness and performance inventory checks, deterministic regeneration checks, benchmark smoke, private-worker reproducibility, exact CQ report regeneration and preflight, immediate offline replay for all twelve parser and printer reports, all twelve regression checks, and replay of all four architecture rollups. No required process remains running.
