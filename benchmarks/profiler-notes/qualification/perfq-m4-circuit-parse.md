# PERFQ-M4-CIRCUIT-PARSE Profiler Note

Owner: `stab-core` circuit parser.

Status: the comparable `parse` measurement failed the primary `1.25x` target at every pre-optimization AArch64 full and soak scale. The threshold, fixture, semantic work, scales, and comparator remain unchanged and unwaived. Current acceptance must come from clean full and soak reports produced against this profiler-note digest.

Dominant cost: the paired workers isolate repeated parsing and replacement of the previous parsed circuit. The fixture generation and final canonical digest are outside the timer, so the measured excess is in Stab's circuit parse, instruction construction, and prior-circuit destruction path. A stack-level profile remains unavailable on this host because `perf_event_paranoid=4` rejects both counting and sampling without elevated privileges.

Allocation and phase instrumentation narrowed the qualification cycle without changing its work. Parsing one six-instruction cycle performs exactly one allocation for the pre-sized top-level circuit item vector. The inline target storage does not allocate for the cycle's one-target and two-target instructions, so per-target heap traffic is not the cause. Release phase probes identified the exact `S`, `TICK`, and single-record `DETECTOR` lines as meaningful generic-parser overhead. Reducing inline target capacity produced only marginal, noisy changes and would penalize common three-target and four-target instructions; boxing instruction arguments or tags reduced object size but regressed parse throughput. Both alternatives were rejected.

## Implemented Optimization

Commit `f99861fe9ea0da05c3bf437c2ab5e3179793396d` adds allocation-free exact fast paths for the qualification cycle's common plain instructions while preserving the generic parser for aliases, decorations, unusual whitespace, multiple targets, arguments, and errors. The single-record detector path still delegates record-target validation to `Target::from_str`, so record-offset semantics remain centralized. Focused regressions prove exact and fallback equivalence, Unicode-whitespace fallback, invalid detector-target rejection, and the one-allocation qualification-cycle contract. They also preserve Stim v1.16's text-parser-only `rec[-0]` behavior while proving that detection conversion, DEM analysis, and sampler compilation reject zero lookback through controlled domain errors.

A clean PR-tier diagnostic produced before this note changed measured a small-scale median ratio of `0.961658x` with an upper bootstrap bound of `0.968127x`. That report is optimization guidance only because it binds the preceding profiler-note digest. It is not promotable evidence for this revised source contract.

## Historical Clean Evidence

All rows below were produced from clean commit `969f399c93c6540022f8ca5aeb9f0c26ed13a49f` on the verified `linux-aarch64-controlled` host. Each report bound the same exact CQ2 preflight and matching Stim/Stab semantic output digest, but these reports predate the current exact-input, failure-owner, memory-reporting, and rollup schemas. The values remain diagnostic context for the owner and are not current promotable evidence.

| Scale | Tier | Pairs | Median ratio | Bootstrap 95% interval | Ratio rMAD | Outcome |
| --- | --- | ---: | ---: | --- | ---: | --- |
| 64 instructions | Full | 9 | 1.309554 | [1.295565, 1.327019] | 0.007683 | Failed |
| 64 instructions | Soak | 15 | 1.304847 | [1.299859, 1.311239] | 0.004125 | Failed |
| 4,096 instructions | Full | 9 | 1.291017 | [1.272868, 1.298798] | 0.006027 | Failed |
| 4,096 instructions | Soak | 15 | 1.291177 | [1.281882, 1.301668] | 0.007812 | Failed |
| 65,536 instructions | Full | 9 | 1.387929 | [1.379217, 1.428395] | 0.006277 | Failed |
| 65,536 instructions | Soak | 15 | 1.367472 | [1.357951, 1.387558] | 0.007348 | Failed |

The large-scale regression is materially worse than the small and medium results. That scaling shape is evidence to preserve, not a reason to shrink the workload.

## Memory Context

| Scale | Tier | Stim peak RSS | Stab peak RSS | Stab versus Stim |
| --- | --- | ---: | ---: | ---: |
| 64 instructions | Soak | 3,395,584 bytes | 4,435,968 bytes | +30.64% |
| 4,096 instructions | Soak | 4,222,976 bytes | 5,316,608 bytes | +25.90% |
| 65,536 instructions | Soak | 19,472,384 bytes | 18,501,632 bytes | -4.99% |

The large fixture uses slightly less peak RSS in Stab despite the larger timing deficit. Peak RSS therefore does not support blaming the large-scale slowdown on retained circuit size alone.

Next owner action: regenerate exact correctness preflight, private-worker reproducibility, one full and one soak report at all three scales, regression checks, and architecture-scoped rollups from one clean commit that binds this note. Preserve every failed or noisy outcome instead of promoting the diagnostic PR result. Native x86-64 evidence remains independent of AArch64 evidence.
