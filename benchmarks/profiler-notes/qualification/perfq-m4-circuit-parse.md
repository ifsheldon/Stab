# PERFQ-M4-CIRCUIT-PARSE Profiler Note

Owner: `stab-core` circuit parser.

Status: the comparable `parse` measurement failed the primary `1.25x` target at every historical AArch64 scale. This is a valid measured slowdown and is not waived, but the reports predate the current schema and must not be promoted as current evidence.

Dominant cost: the paired workers isolate repeated parsing and replacement of the previous parsed circuit. The fixture generation and final canonical digest are outside the timer, so the measured excess is in Stab's circuit parse, instruction construction, and prior-circuit destruction path. A stack-level attribution is not yet available because this controlled host has `perf_event_paranoid=4`; both `perf stat` and sampling access fail without elevated privileges. Source inspection alone is insufficient to assign the excess to line scanning, gate resolution, target construction, allocation, or destruction.

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

Next owner action: capture permitted release stack profiles for both workers at the medium and large scales on the same architecture, then add allocation-count evidence or phase instrumentation only if the stack profile cannot distinguish parsing, construction, and destruction. Optimize the measured owner without changing the fixture, semantic work, output digest, confidence rule, or `1.25x` target, and rerun one full and one soak report at all three scales after the change. Until then, keep all six outcomes failed and do not promote a threshold pass.
