# PQ2 Circuit Parse Qualification Progress Report

> Evidence-status note, 2026-07-15: the measurements below remain accepted historical evidence for revision `d2df9766c5e3543c8df016db31f48f552354d79f`, but they do not bind current performance inventory `b6bf408c54461d65670200fe701f7fff3e5e0470509d8fc89aeab729a242781b` or the shared worker extended with canonical printing. Regenerate all six parser reports and both rollups from the clean current worker before citing them as source-current.

## Status

The first PQ2 product group, `PERFQ-M4-CIRCUIT-PARSE`, now passes the unchanged `1.25x` timing gate at every full and soak scale on the controlled Linux AArch64 host as of 2026-07-15.

All six promotable measurements pass with upper bootstrap confidence bounds below `0.927x`. Stab uses between 8.2 percent and 14.0 percent less measured parse time than pinned Stim across these rows. The former parser optimization blocker is closed without changing the comparator, workload, scale, semantic output obligation, or threshold.

This report closes only the first proving group. It does not complete `PERF-CIRCUIT-MODEL`, the remaining PQ2 runtime groups, PQ2 on AArch64, or the separately required native x86-64 evidence.

## Frozen Inputs

- Stab evidence revision: `d2df9766c5e3543c8df016db31f48f552354d79f`, clean and unchanged before and after every promotable report and both rollups.
- Stim baseline: v1.16.0 at `e2fc1eca7fd21684d433aa5f10f4504ea4860d07`.
- Correctness inventory: `4d9faa21e318eeebc4614c7bf62491bb2db73b5db57ae3dab7d0f19f3fda7cad`.
- Performance inventory: `9687d45fa57b97388e7a8c7b1676f2545619ff3185c19aa714583b25d1680924`.
- Runtime group contract: `11f905ce465c884aa07841a8d05ba9050e5dcb8c4c6032c9ef6936fa8890f720`.
- Profiler note: `benchmarks/profiler-notes/qualification/perfq-m4-circuit-parse.md` at `b37614a9843164d782c035d20f659fbf11be310c310caa4715a42ef792654a6b`.

## Correctness Preflight

The clean report at `target/qualification/pq2-circuit-parse-reviewed-full` selected and passed exactly these two parser owners:

- `cq-evidence-qualification-633fa529edf5f549`
- `cq-evidence-qualification-e660819ae9a223c6`

The report passed canonical offline regeneration and exact dependent preflight with these bindings:

| Artifact | SHA-256 |
| --- | --- |
| Request | `50f7d370b5e7a28ff9334ea7c093a0c182d555001bd75e83b4ec49aeeb0d58e4` |
| Report | `8fb7c2808615a3e91a6d6b9f001755904e50eac9626518e26da226e32202b0cd` |
| Completion | `05f81d460661e26bc19aabcb08d041a4a93cc39e5f483e6e037859b7413a9047` |
| Preflight | `3c9b0c327fa9114f88370f1ad70159a9aec883e1a1b4f7a8af75653ac352105e` |

Every performance report reopens these artifacts and reconstructs their canonical receipts before timing.

## Reproducible Workers

`just bench::qualification-worker-reproducibility` rebuilt both private workers twice from the clean unchanged evidence revision, verified each live protocol identity, and proved exact pre-barrier rejection of the first unsupported circuit size.

| Worker identity | SHA-256 |
| --- | --- |
| Stim source | `1a22bfd87554e0c184f130de45ae89c59786e5d2592ded4ebddc701cde5a0abe` |
| Stim build fingerprint | `b17167a0fd156f37c27bb03ee96f0ceca3a6103ae2c9f6b427bca860d43875fe` |
| Stim binary | `d6d4a654bfb810c73bc1d4b13de744e3cf4c8b4cec59af828c4bc57d50bfb2e1` |
| Stab source | `7568b5a1cd0d53959f5abaea776bfb79b4a26346745a907fc00b6fff71b10e87` |
| Stab build fingerprint | `e4d8cab353a1e2549994b60667dd47ea71cacd1410ec08ed8d4c0b05a7415aaf` |
| Stab binary | `20f532cb6693bc7cd37218a05b69eb0c6786cb278ab6ac6e433770604faa43f9` |

## AArch64 Timing Results

All reports used the verified `linux-aarch64-controlled` host, matched exact input bytes and digests, matched the semantic output digest, retained raw paired samples, and completed without a noise rerun.

| Scale | Tier | Pairs | Median ratio | Bootstrap 95% interval | Ratio rMAD | Outcome |
| --- | --- | ---: | ---: | --- | ---: | --- |
| 64 instructions | Full | 9 | 0.883627 | [0.880187, 0.890209] | 0.001659 | Passed |
| 64 instructions | Soak | 15 | 0.881622 | [0.876938, 0.889910] | 0.008228 | Passed |
| 4,096 instructions | Full | 9 | 0.860613 | [0.856153, 0.868392] | 0.004007 | Passed |
| 4,096 instructions | Soak | 15 | 0.859679 | [0.859250, 0.862876] | 0.003425 | Passed |
| 65,536 instructions | Full | 9 | 0.917797 | [0.908443, 0.922543] | 0.005171 | Passed |
| 65,536 instructions | Soak | 15 | 0.917843 | [0.914851, 0.926499] | 0.004860 | Passed |

Both full and soak family outcomes are `passed` with three passing measurements and zero noisy measurements. `qualification-regression` accepts every source report against the unchanged `1.25x` median and confidence-bound rules.

The large scale remains slower relative to the other Stab rows, but it is still a measured speedup over Stim and has more than 32 percentage points of confidence-bound headroom below the gate.

## AArch64 Memory Results

Memory remains separate evidence. Stab has higher peak RSS at small and medium scales but lower peak RSS at the large scale.

| Scale | Tier | Stim peak RSS | Stab peak RSS | Direction |
| --- | --- | ---: | ---: | --- |
| 64 instructions | Soak | 3,420,160 bytes | 4,374,528 bytes | Stab higher |
| 4,096 instructions | Soak | 4,177,920 bytes | 5,263,360 bytes | Stab higher |
| 65,536 instructions | Soak | 19,456,000 bytes | 18,456,576 bytes | Stab lower |

## Authoritative Artifacts

| Evidence | Path | Report SHA-256 |
| --- | --- | --- |
| Small full | `target/benchmarks/qualification/pq2-circuit-parse-reviewed-small-full` | `ae9d0ce097a72131765fd3a6a0cd5700537a21763a08fbb066aa4fd93864f4d1` |
| Medium full | `target/benchmarks/qualification/pq2-circuit-parse-reviewed-medium-full` | `b6a1ed26475b114c3d20d80b4dedf1bffee55065640c22bd1b2916f30c04fafe` |
| Large full | `target/benchmarks/qualification/pq2-circuit-parse-reviewed-large-full` | `479b41aa1a9821eecd241a170aad5d02deea0678c6ee9bc8ac61e83b96f2683c` |
| Small soak | `target/benchmarks/qualification/pq2-circuit-parse-reviewed-small-soak` | `ce2652a8f908cf3963883f52cd32a3b0a6d3359c97b45cf8691db8c7bdcbb6f2` |
| Medium soak | `target/benchmarks/qualification/pq2-circuit-parse-reviewed-medium-soak` | `3fd990c3103084c60188591a0f438ff40d59acbe556f6855bc1f8d5c38774b7f` |
| Large soak | `target/benchmarks/qualification/pq2-circuit-parse-reviewed-large-soak` | `0415f60a09ce2f1f3ee5094269ce96e9194ca48efc24f84a16dfd9c38b49a466` |
| AArch64 full rollup | `target/benchmarks/qualification/pq2-circuit-parse-reviewed-aarch64-full-rollup` | `78aa09bbcf916b9a068d68caffd9f9ef3d6ea9bd7d11459c322dfca964fdae61` |
| AArch64 soak rollup | `target/benchmarks/qualification/pq2-circuit-parse-reviewed-aarch64-soak-rollup` | `87c2524bc9a6201fd3f3838ee94167ab5a9d83abf4656c502ee212b9fa34f279` |

Both rollups passed offline replay and bind all three required scales, one architecture, one tier, one correctness preflight, one runtime contract, and one exact six-digest worker identity.

## Review Closure

The parser optimization retained the generic parser fallback and one-allocation qualification contract. Review then found Stim's unusual context-specific `rec[-0]` behavior. Commit `efd1c1299f1d407b574ec49cfb34a0b5305805d7` added an explicit typed representation, exact analyzer fixtures, controlled detection and sampling rejection, and direct transform, flow, missing-detector, detecting-region, and selected inverse-QEC regressions.

The final GPT-5.6/max full-code-review found no remaining code, compatibility, architecture, or modernization issue. Its only P1 was the intentionally stale correctness and performance inventory binding after the new evidence owners landed. Commit `3527b660` regenerated and froze both inventories without changing any threshold or comparator.

The milestone audit found no remaining implementation defect or new under-specification. The former evidence is retained as historical context in the profiler note; none of its failed or pre-review passing rows is promoted as current evidence.

## Remaining Work

1. Produce the same clean full and soak scale families and rollups on a controlled native Linux x86-64 host. No x86-64 timing conclusion is currently claimed.
2. Implement and qualify the remaining PQ2 runtime groups. This first group does not close the broader deterministic performance inventory.
3. Capture stack profiles on an authorized host if future parser work needs line-level attribution. `perf_event_paranoid=4` blocks local stack sampling, but profiling is no longer an acceptance blocker for this passing group.

## Verification

The evidence revision passed workspace format, Clippy, tests, correctness and performance inventory checks, deterministic regeneration checks, benchmark smoke, worker reproducibility, exact CQ report regeneration and preflight, immediate offline replay for all six performance reports, all six regression checks, and full and soak rollup replay.

Milestone audit and GPT-5.6/max full-code-review were completed before final evidence generation. No required process remains running.
