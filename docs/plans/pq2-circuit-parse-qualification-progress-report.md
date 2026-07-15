# PQ2 Circuit Parse Qualification Progress Report

> Current-evidence note, 2026-07-16: this report remains historical passing AArch64 evidence at performance inventory `f3c4009044b0bafcd877f76798c7f4f08c475c0877b85f68d22ae0449e3ddb8f`. Graduating `PERFQ-M4-GATE-LOOKUP` changed the global performance inventory and shared worker, and its exact hash owner later changed the correctness inventory; no claim in this report has been silently promoted to either current inventory.

## Status

`PERFQ-M4-CIRCUIT-PARSE` passes the unchanged `1.25x` timing gate at every full and soak scale on the controlled Linux AArch64 host as of 2026-07-15.

All six source-current `parse` measurements pass with median ratios from `0.897744x` to `0.970298x` and a worst bootstrap confidence-interval upper bound of `0.974833x`. Stab takes between about 10 percent and 3 percent less measured parse time than pinned Stim across these rows.

The earlier reports from revision `d2df9766c5e3543c8df016db31f48f552354d79f` remain valid historical evidence. This report replaces them as source-current evidence after the shared worker gained canonical printing and the qualification inventories changed.

This report closes only the parser proving group on AArch64. It does not complete `PERF-CIRCUIT-MODEL`, the remaining PQ2 runtime groups, PQ2 on AArch64, or native Linux x86-64 evidence.

## Frozen Inputs

- Stab evidence revision: `ba70a52025fdd4122ac97cec263725b2ec56e431`, with `local_modifications=false` in every correctness and performance report.
- Stim baseline: v1.16.0 at `e2fc1eca7fd21684d433aa5f10f4504ea4860d07`.
- Correctness inventory: `b80801fea6eae550feecf40489259de56123f6f3331b747d52c323d576fd0285`.
- Performance inventory: `f3c4009044b0bafcd877f76798c7f4f08c475c0877b85f68d22ae0449e3ddb8f`.
- Runtime group contract shared by parser and canonical printing: `e0e00907862b2fa59700f339f318bb1a15c6f4f0bbb0641caa7ad56f195c86a2`.
- Profiler note: `benchmarks/profiler-notes/qualification/perfq-m4-circuit-parse.md` at `da819f8a2faeebfe7873db2b84cd090cae9853772b8ac756bb63eb86bd8d7a58`.
- Host profile: verified `linux-aarch64-controlled`, pinned to logical CPU 0 with the performance governor at 2,808,000 kHz and no host-policy violation.

## Correctness Preflight

The clean report at `target/qualification/pq2-m4-parse-print-full-ba70a52` selected and passed all three shared parser and printer prerequisites. Parsing depends on these exact owners:

- `cq-evidence-qualification-633fa529edf5f549`
- `cq-evidence-qualification-e660819ae9a223c6`

The combined report passed canonical offline regeneration and exact dependent preflight with these bindings:

| Artifact | SHA-256 |
| --- | --- |
| Request | `23e1f36f280a101d39213d403c724fe5d498ffe428bc309796f3bbeae20bd703` |
| Report | `5dcb3134e2fad22de389823f232704bb2b339133a4726f5c59234ed8684a69c0` |
| Completion | `05037f849b9b317c9c2cc61653f3b979e742346a91b12237d4e1958d961cad8a` |
| Preflight | `68c05127c48674aae13d3908355e335a86c9ec89c4f9755d38308e93c81e72dd` |

Every performance report reopens these artifacts and reconstructs their canonical receipts before timing.

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

All reports used the verified host, matched exact input bytes and digests, matched the final canonical circuit digest, retained raw interleaved paired samples, and completed without a noise rerun.

| Scale | Tier | Pairs | Median ratio | Bootstrap 95% interval | Ratio rMAD | Outcome |
| --- | --- | ---: | ---: | --- | ---: | --- |
| 64 instructions | Full | 9 | 0.920317 | [0.908506, 0.924204] | 0.004224 | Passed |
| 64 instructions | Soak | 15 | 0.920661 | [0.917691, 0.929440] | 0.003872 | Passed |
| 4,096 instructions | Full | 9 | 0.897744 | [0.887921, 0.922286] | 0.009427 | Passed |
| 4,096 instructions | Soak | 15 | 0.900131 | [0.886904, 0.904183] | 0.006687 | Passed |
| 65,536 instructions | Full | 9 | 0.963578 | [0.949537, 0.970843] | 0.006921 | Passed |
| 65,536 instructions | Soak | 15 | 0.970298 | [0.966151, 0.974833] | 0.004275 | Passed |

Both family outcomes are `passed`, with three passing measurements and no failed or noisy measurement. `qualification-regression` accepts every source report against the exact `1.25x` median and confidence-bound rules.

## AArch64 Memory Results

Memory remains separate observational evidence. Stab has higher peak RSS at small and medium scales but lower peak RSS at the large scale.

| Scale | Tier | Stim peak RSS | Stab peak RSS | Stab/Stim |
| --- | --- | ---: | ---: | ---: |
| 64 instructions | Soak | 3,411,968 bytes | 4,444,160 bytes | 1.303x |
| 4,096 instructions | Soak | 4,038,656 bytes | 5,328,896 bytes | 1.319x |
| 65,536 instructions | Soak | 19,451,904 bytes | 18,518,016 bytes | 0.952x |

## Authoritative Artifacts

| Evidence | Path | Report SHA-256 |
| --- | --- | --- |
| Small full | `target/benchmarks/qualification/pq2-m4-parse-ba70a52-small-full` | `ce514a3f13705b734a9b3567184c2b5ef95cf3672125efef20d25617e3d7fa99` |
| Medium full | `target/benchmarks/qualification/pq2-m4-parse-ba70a52-medium-full` | `dd7982999414b0bd561aa32c4a41d25891f025c1d2c026f8f3206bb28c58cbdd` |
| Large full | `target/benchmarks/qualification/pq2-m4-parse-ba70a52-large-full` | `a4ba58e1a78fadfa124308d89800cab1826037b54cf56ef4633ebcea07eec38c` |
| Small soak | `target/benchmarks/qualification/pq2-m4-parse-ba70a52-small-soak` | `9af73fd2cf31be87a32c5b4912b727feeee0e7d1bffdf09af56e3e195fe4e747` |
| Medium soak | `target/benchmarks/qualification/pq2-m4-parse-ba70a52-medium-soak` | `2d0c6d145c951bed45494f264c4603322c0b2eb90ed42e0839fd46ddbf5463af` |
| Large soak | `target/benchmarks/qualification/pq2-m4-parse-ba70a52-large-soak` | `f6fdf949c89511689f7cf3e414eec23cae2d95f74bd3484280d68cd2c3f9289f` |
| AArch64 full rollup | `target/benchmarks/qualification/pq2-m4-parse-ba70a52-aarch64-full-rollup` | `55bf7046404e8cd07aedd86e5ab82d4d0065cadcfe0fb947913e379454f1eb99` |
| AArch64 soak rollup | `target/benchmarks/qualification/pq2-m4-parse-ba70a52-aarch64-soak-rollup` | `1f751233008bd4db19bd33cb812c5d6046d4f3f19d5ea7b40ebae921fbbadaab` |

The full and soak rollup preflights are `626436f79ba0607860f64ea04200df03696617329e6ae4e90ac39ab000d0995e` and `2aac2deb2fb494ec75013de06d35f943ffb5447c7785a74c6d50581ea87a0192`. Both rollups passed offline replay and bind every required scale, one architecture and tier, the exact correctness and inventory digests, one runtime contract, and one six-digest worker identity.

## Review Closure

The parser retains its generic fallback, exact canonical output obligation, one-allocation qualification contract, typed negative-zero measurement-record representation, and controlled rejection paths. The current refresh changed no parser semantics or threshold; it reran the group because the shared worker source and performance inventory changed when canonical printing graduated.

The source-current milestone audit and GPT-5.6/max review found no remaining parser implementation or evidence defect. The legacy parser row remains explicitly reworked by this exact source-owned replacement group rather than silently serving as duplicate evidence.

## Remaining Work

1. Produce the same clean full and soak scale families and rollups on a controlled native Linux x86-64 host. No x86-64 timing conclusion is claimed.
2. Qualify the remaining PQ2 runtime groups. Parser and canonical printing do not close the broader circuit-model feature.
3. Capture stack profiles on an authorized host if future parser work needs line-level attribution. `perf_event_paranoid=4` blocks local stack sampling, but profiling is not an acceptance blocker for this passing group.

## Verification

The evidence revision passed workspace format, Clippy, tests, correctness and performance inventory checks, deterministic regeneration checks, benchmark smoke, worker reproducibility, exact CQ report regeneration and preflight, immediate offline replay for all twelve parser and printer reports, all twelve regression checks, and replay of all four architecture rollups. No required process remains running.
